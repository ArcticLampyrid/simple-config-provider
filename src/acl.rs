use regress::Regex;
use serde::Deserialize;

use crate::{
    error::{Error, Result},
    formats::Value,
};

pub struct Acl {
    token: String,
    filters: Vec<Filter>,
}

struct Filter {
    pattern: Regex,
    invert: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    token: String,
    #[serde(default)]
    filter: Option<Filters>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Filters {
    One(FilterSpec),
    Many(Vec<FilterSpec>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FilterSpec {
    Pattern(String),
    Rule(Rule),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    pattern: String,
    #[serde(default)]
    invert: bool,
}

fn compile_filter(spec: FilterSpec) -> Result<Filter> {
    let Rule { pattern, invert } = match spec {
        FilterSpec::Pattern(pattern) => Rule {
            pattern,
            invert: false,
        },
        FilterSpec::Rule(rule) => rule,
    };
    Ok(Filter {
        pattern: Regex::new(&pattern).map_err(|e| Error::ConfigSource(e.into()))?,
        invert,
    })
}

fn compile_entry(entry: Entry) -> Result<Acl> {
    let filters = match entry.filter {
        None => Vec::new(),
        Some(Filters::One(filter)) => vec![filter],
        Some(Filters::Many(filters)) => filters,
    };
    Ok(Acl {
        token: entry.token,
        filters: filters
            .into_iter()
            .map(compile_filter)
            .collect::<Result<_>>()?,
    })
}

/// Accepts a list of entries, a single entry, or a `{ token: filter }` map.
pub fn compile_acls(value: &Value) -> Result<Vec<Acl>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Sequence(values) => values.iter().map(compile_value).collect(),
        Value::Mapping(map) if map.contains_key("token") || map.contains_key("filter") => {
            Ok(vec![compile_value(value)?])
        }
        Value::Mapping(map) => map
            .iter()
            .map(|(token, filter)| {
                compile_entry(Entry {
                    token: String::deserialize(token).map_err(|e| Error::ConfigSource(e.into()))?,
                    filter: Option::<Filters>::deserialize(filter)
                        .map_err(|e| Error::ConfigSource(e.into()))?,
                })
            })
            .collect(),
        _ => Err(Error::Config("acls must be an object or an array".into())),
    }
}

fn compile_value(value: &Value) -> Result<Acl> {
    compile_entry(Entry::deserialize(value).map_err(|e| Error::ConfigSource(e.into()))?)
}

pub fn is_allowed(acls: &[Acl], token: &str, flavor: &str) -> bool {
    acls.iter().filter(|acl| acl.token == token).any(|acl| {
        acl.filters.is_empty()
            || acl
                .filters
                .iter()
                .any(|filter| filter.invert != filter.pattern.find(flavor).is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Format, parse};

    #[test]
    fn acl_forms_and_or_semantics() {
        let value = parse(r#"[{token:'secret',filter:['^base$',{pattern:'^_',invert:true}]},{token:'',filter:'public'},{token:'admin'},{token:'123',filter:'^num$'}]"#, Format::Json5).unwrap();
        let acls = compile_acls(&value).unwrap();
        assert!(is_allowed(&acls, "secret", "base"));
        assert!(is_allowed(&acls, "secret", "anything"));
        assert!(!is_allowed(&acls, "secret", "_hidden"));
        assert!(is_allowed(&acls, "", "nested/public-copy"));
        assert!(!is_allowed(&acls, "", "base"));
        assert!(is_allowed(&acls, "admin", "_hidden"));
        assert!(is_allowed(&acls, "123", "num"));
        assert!(!is_allowed(&acls, "wrong", "public"));
        for source in ["{token:'x',filter:'a'}", "{x:['a']}"] {
            let value = parse(source, Format::Json5).unwrap();
            assert!(is_allowed(&compile_acls(&value).unwrap(), "x", "a"));
        }
        assert!(!is_allowed(&compile_acls(&Value::Null).unwrap(), "", "a"));
    }

    #[test]
    fn regex_uses_native_unicode_and_pattern_semantics() {
        for (pattern, flavor) in [(r"(?<=x)(a)\1", "xaa"), ("^.$", "😀")] {
            let acl = Acl {
                token: String::new(),
                filters: vec![compile_filter(FilterSpec::Pattern(pattern.into())).unwrap()],
            };
            assert!(is_allowed(&[acl], "", flavor));
        }
        assert!(compile_filter(FilterSpec::Pattern("[".into())).is_err());
    }

    #[test]
    fn invalid_acl_types_are_rejected_not_coerced() {
        for source in [
            "{token:123}",
            "{token:null}",
            "{token:true}",
            "{token:[]}",
            "{token:{}}",
            "{filter:'a'}",
            "[42]",
            "[null]",
            "{token:'x',filter:42}",
            "{token:'x',filter:{pattern:'a',invert:'true'}}",
            "{token:'x',filters:'a'}",
            "'secret'",
        ] {
            let value = parse(source, Format::Json5).unwrap();
            assert!(compile_acls(&value).is_err(), "{source}");
        }
    }
}
