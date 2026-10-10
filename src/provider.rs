use relative_path::RelativePath;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{
    config::VmOptions,
    error::{Error, Result},
    formats::{self, FORMATS, Format, Value},
    runtime,
};

// Scripts take priority over data files; data files follow the FORMATS order.
const SCRIPT_EXTENSIONS: &[&str] = &["mjs", "js"];

pub(crate) struct Flavor {
    pub name: String,
    pub source: Source,
}

pub(crate) enum Source {
    Script(PathBuf),
    Data(Value),
}

/// Resolves `.`, `..` and empty segments, and rejects names that would leave
/// the flavor directory.
pub fn normalize_flavor_name(name: &str) -> Result<String> {
    let normalized = RelativePath::new(name).normalize().to_string();
    if matches!(normalized.as_str(), "" | "." | "..")
        || normalized.starts_with("../")
        || name.starts_with('/')
        || normalized.contains(['\\', '\0'])
    {
        return Err(Error::InvalidFlavorName(name.into()));
    }
    Ok(normalized)
}

pub(crate) fn load_flavor(config_dir: &Path, name: &str) -> Result<Flavor> {
    let name = normalize_flavor_name(name)?;
    let path_for = |extension: &str| {
        config_dir
            .join("flavor")
            .join(format!("{name}.{extension}"))
    };
    for extension in SCRIPT_EXTENSIONS {
        let path = path_for(extension);
        if fs::exists(&path)? {
            return Ok(Flavor {
                name,
                source: Source::Script(path),
            });
        }
    }
    for (extension, format) in FORMATS {
        if let Some(text) = read_if_exists(&path_for(extension))? {
            let value = formats::parse(&text, *format)?;
            return Ok(Flavor {
                name,
                source: Source::Data(value),
            });
        }
    }
    Err(Error::FlavorNotFound(name))
}

fn read_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Renders a flavor into `format`. Work runs on a blocking thread because file
/// access is synchronous and the script VM is `!Send`.
pub async fn render(
    config_dir: PathBuf,
    name: String,
    format: Format,
    vm_options: VmOptions,
) -> Result<String> {
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let flavor = load_flavor(&config_dir, &name)?;
        let value = match flavor.source {
            Source::Data(value) => value,
            Source::Script(_) => {
                handle.block_on(runtime::evaluate(config_dir, flavor, vm_options))?
            }
        };
        formats::serialize(&value, format)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_paths_not_output_formats() {
        assert_eq!(normalize_flavor_name("base.JSON").unwrap(), "base.JSON");
        assert_eq!(normalize_flavor_name("a/../b").unwrap(), "b");
        assert_eq!(normalize_flavor_name("a//b").unwrap(), "a/b");
        for name in ["", ".", "..", "../base", "/base", "a\\b", "a\0b"] {
            assert!(normalize_flavor_name(name).is_err(), "{name}");
        }
    }
}
