use serde::Serialize;
pub use yaml_serde::Value;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Json,
    Json5,
    Yaml,
    Toml,
}

// Source lookup and HTTP format handling share this ordered registry.
pub const FORMATS: &[(&str, Format)] = &[
    ("json5", Format::Json5),
    ("json", Format::Json),
    ("yaml", Format::Yaml),
    ("yml", Format::Yaml),
    ("toml", Format::Toml),
];

impl Format {
    pub fn from_extension(extension: &str) -> Option<Self> {
        FORMATS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(extension))
            .map(|(_, format)| *format)
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json; charset=utf-8",
            Self::Json5 => "application/json5; charset=utf-8",
            Self::Yaml => "application/yaml; charset=utf-8",
            Self::Toml => "application/toml; charset=utf-8",
        }
    }
}

pub fn parse(text: &str, format: Format) -> Result<Value> {
    match format {
        Format::Json => serde_json::from_str(text).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Json5 => json5::from_str(text).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Yaml => yaml_serde::from_str(text).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Toml => toml::from_str(text).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
    }
}

pub fn serialize(value: &impl Serialize, format: Format) -> Result<String> {
    match format {
        Format::Json => serde_json::to_string_pretty(value).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Json5 => json5::to_string(value).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Yaml => yaml_serde::to_string(value).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
        Format::Toml => toml::to_string_pretty(value).map_err(|e| Error::Format {
            format,
            source: e.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_all_formats() {
        let value = parse(
            r#"{"name":"sample","list":[1,2,3],"flag":true}"#,
            Format::Json,
        )
        .unwrap();
        for (_, format) in FORMATS {
            assert_eq!(
                parse(&serialize(&value, *format).unwrap(), *format).unwrap(),
                value
            );
        }
    }

    #[test]
    fn parsers_determine_empty_input_semantics() {
        assert!(parse("", Format::Json).is_err());
        assert!(parse("", Format::Json5).is_err());
        assert_eq!(parse("", Format::Yaml).unwrap(), Value::Null);
        assert_eq!(
            parse("", Format::Toml).unwrap(),
            Value::Mapping(Default::default())
        );
    }

    #[test]
    fn special_numbers() {
        let value = parse("{inf: Infinity, nan: NaN}", Format::Json5).unwrap();
        assert!(value["inf"].as_f64().unwrap().is_infinite());
        assert!(value["nan"].as_f64().unwrap().is_nan());
        assert!(
            serialize(&value, Format::Json5)
                .unwrap()
                .contains("Infinity")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&serialize(&value, Format::Json).unwrap())
                .unwrap(),
            serde_json::json!({"inf":null,"nan":null})
        );
        assert!(serialize(&Value::from(42), Format::Toml).is_err());
    }
}
