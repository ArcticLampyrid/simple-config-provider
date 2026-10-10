use std::path::Path;

use byte_unit::Byte;
use serde::Deserialize;

use crate::{
    acl::{Acl, compile_acls},
    error::{Error, Result},
    formats::Value,
};

const CONFIG_FILE: &str = "config.json5";
const DEFAULT_MEMORY_LIMIT: Byte = Byte::from_u64(64 << 20);
// Scripts run on tokio's blocking threads, which have a 2 MiB native stack.
// QuickJS must raise its stack-overflow error well before that is exhausted;
// larger values (or 0, which disables the check) abort the whole process.
const MAX_STACK_SIZE_CEILING: Byte = Byte::from_u64(1 << 20);

pub struct Config {
    pub acls: Vec<Acl>,
    pub vm_options: VmOptions,
}

/// Limits applied to each per-request script VM.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VmOptions {
    /// JavaScript engine memory limit; `0` means unlimited.
    pub memory_limit: Byte,
    /// JavaScript stack limit, at most `MAX_STACK_SIZE_CEILING`; unset keeps
    /// LLRT's default.
    pub max_stack_size: Option<Byte>,
}

impl Default for VmOptions {
    fn default() -> Self {
        Self {
            memory_limit: DEFAULT_MEMORY_LIMIT,
            max_stack_size: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    acls: Value,
    #[serde(default)]
    vm_options: VmOptions,
}

/// Reads `config.json5`. Requests call this every time, so edits take effect
/// without a restart.
pub async fn load(config_dir: &Path) -> Result<Config> {
    parse(&tokio::fs::read_to_string(config_dir.join(CONFIG_FILE)).await?)
}

fn parse(source: &str) -> Result<Config> {
    let raw: RawConfig = json5::from_str(source).map_err(|e| Error::ConfigSource(e.into()))?;
    if let Some(size) = raw.vm_options.max_stack_size
        && (size.as_u64() == 0 || size > MAX_STACK_SIZE_CEILING)
    {
        return Err(Error::Config(format!(
            "vm_options.max_stack_size must be greater than 0 and at most {MAX_STACK_SIZE_CEILING:#}"
        )));
    }
    Ok(Config {
        acls: compile_acls(&raw.acls)?,
        vm_options: raw.vm_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vm_options(source: &str) -> Result<(u64, Option<u64>)> {
        let options = parse(source)?.vm_options;
        Ok((
            options.memory_limit.as_u64(),
            options.max_stack_size.map(Byte::as_u64),
        ))
    }

    #[test]
    fn vm_options_parse_byte_units() {
        assert_eq!(vm_options("{}").unwrap(), (64 << 20, None));
        assert_eq!(vm_options("{vm_options: {}}").unwrap(), (64 << 20, None));
        assert_eq!(
            vm_options("{vm_options: {memory_limit: '8 MiB', max_stack_size: '256KiB'}}").unwrap(),
            (8 << 20, Some(256 << 10))
        );
        assert_eq!(
            vm_options("{vm_options: {memory_limit: '1MB', max_stack_size: '1MiB'}}").unwrap(),
            (1_000_000, Some(1 << 20))
        );
        assert_eq!(
            vm_options("{vm_options: {memory_limit: '0'}}").unwrap().0,
            0
        );
    }

    #[test]
    fn invalid_vm_options_are_rejected() {
        for source in [
            "{vm_options: {memory_limit: 'lots'}}",
            "{vm_options: {memory_limit: 67108864}}",
            "{vm_options: {memory_limit: '-1MiB'}}",
            "{vm_options: {max_stack_size: '0'}}",
            "{vm_options: {max_stack_size: '2MiB'}}",
            "{vm_options: {memoryLimit: '8MiB'}}",
            "{vmOptions: {}}",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}
