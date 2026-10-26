//! Sail's private settings for its managed Cargo and Verus-driver aliases.
//!
//! Plain Cargo-Verus gives Cargo, every build script and every compiler the
//! caller's environment plus Cargo-Verus's and Cargo's own variables. Any
//! further variable is visible to the program being verified: `env!` and
//! `option_env!`, procedural macros and build scripts can read it and change
//! what is compiled (#385). So the runner never puts its private settings in
//! the environment. It writes them to one file beside the two aliases in the
//! compiler family directory, which its exclusive family lock owns for the
//! whole run, and each alias reads that file through the path it was started
//! as. A caller's own `SAIL_MANAGED_*` variables pass through unchanged.
//!
//! Without the file, for example when a test runs a role directly, the
//! settings are read from the environment as before.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The file beside the aliases.
pub(crate) const FILE: &str = "managed-transport.json";
const VERSION: u32 = 1;

/// The aliases whose settings live in [`FILE`].
const ROLES: &[&str] = &["sail-cargo", "sail-verus-driver"];

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Transport {
    version: u32,
    /// Each private setting's value; an absent name is unset.
    values: BTreeMap<String, Vec<u8>>,
    /// The caller's own `CARGO`. The runner names its Cargo alias there for
    /// Cargo-Verus; the real Cargo and everything below it get this back.
    caller_cargo: Option<Vec<u8>>,
}

impl Transport {
    /// Settings for one Cargo-Verus invocation; `caller_cargo` is the
    /// caller's own `CARGO`, if set.
    pub(crate) fn new(caller_cargo: Option<&OsStr>) -> Self {
        Self {
            version: VERSION,
            values: BTreeMap::new(),
            caller_cargo: caller_cargo.map(|value| value.as_encoded_bytes().to_vec()),
        }
    }

    /// Sets one private setting.
    pub(crate) fn set(&mut self, name: &str, value: impl AsRef<OsStr>) -> &mut Self {
        self.values
            .insert(name.to_owned(), value.as_ref().as_encoded_bytes().to_vec());
        self
    }

    /// One private setting, if set.
    pub(crate) fn get(&self, name: &str) -> Option<OsString> {
        self.values.get(name).and_then(|bytes| decode(bytes))
    }

    /// Writes the settings to [`FILE`] in `directory`, replacing it whole.
    pub(crate) fn write(&self, directory: &Path) -> Result<()> {
        let path = directory.join(FILE);
        let temporary = directory.join(format!("{FILE}.{}.tmp", std::process::id()));
        let bytes =
            serde_json::to_vec(self).map_err(|error| Error::new("SAIL3104", error.to_string()))?;
        std::fs::write(&temporary, bytes)
            .and_then(|()| std::fs::rename(&temporary, &path))
            .map_err(|error| {
                let _ = std::fs::remove_file(&temporary);
                Error::new("SAIL3104", error.to_string())
            })
    }

    fn read(path: &Path) -> Option<Self> {
        let transport: Self = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        (transport.version == VERSION).then_some(transport)
    }
}

#[cfg(unix)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "The same signature as the non-Unix decoder, which can fail."
)]
fn decode(bytes: &[u8]) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt as _;
    Some(OsString::from_vec(bytes.to_vec()))
}

#[cfg(not(unix))]
fn decode(bytes: &[u8]) -> Option<OsString> {
    String::from_utf8(bytes.to_vec()).ok().map(OsString::from)
}

/// [`FILE`] for this process, when it runs as one of the aliases.
fn location() -> Option<PathBuf> {
    let program = PathBuf::from(std::env::args_os().next()?);
    let role = program.file_name()?.to_str()?;
    ROLES.contains(&role).then(|| program.with_file_name(FILE))
}

/// This process's settings: the file beside the alias it runs as, or `None`
/// when there is none.
fn loaded() -> Option<&'static Transport> {
    static LOADED: OnceLock<Option<Transport>> = OnceLock::new();
    LOADED
        .get_or_init(|| location().and_then(|path| Transport::read(&path)))
        .as_ref()
}

/// Whether this process reads its settings from a transport file.
pub(crate) fn file_backed() -> bool {
    loaded().is_some()
}

/// Removes one private setting from `command`'s environment when settings
/// travel there (no transport file). With a file, a variable of that name
/// is the caller's own and passes through.
pub(crate) fn clear(command: &mut impl super::command_environment::ConfigureCommand, name: &str) {
    if !file_backed() {
        command.env_remove(name);
    }
}

/// One private setting: from the transport file when this process has one,
/// otherwise from the environment.
pub(crate) fn var(name: &str) -> Option<OsString> {
    match loaded() {
        Some(transport) => transport.get(name),
        None => std::env::var_os(name),
    }
}

/// Like [`var`], but a missing or empty setting is an error.
pub(crate) fn required(name: &str) -> Result<OsString> {
    var(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::new("SAIL3111", format!("private adapter missing {name}")))
}

/// Passes one private setting to the processes Cargo starts below `command`.
/// With a transport file, the setting is added to it and `command`'s
/// environment is left alone; otherwise it goes in `command`'s environment.
pub(crate) fn forward(
    command: &mut std::process::Command,
    name: &str,
    value: &OsStr,
) -> Result<()> {
    if let (Some(transport), Some(path)) = (loaded(), location()) {
        let mut updated = transport.clone();
        updated.set(name, value);
        let directory = path
            .parent()
            .ok_or_else(|| Error::new("SAIL3111", "managed transport has no directory"))?;
        updated.write(directory)
    } else {
        command.env(name, value);
        Ok(())
    }
}

/// Gives the real Cargo the caller's own `CARGO` (or none, so that Cargo
/// names itself to build scripts and compilers, as under plain Cargo-Verus)
/// instead of the runner's Cargo alias.
pub(crate) fn restore_cargo(command: &mut std::process::Command) {
    if let Some(transport) = loaded() {
        match transport.caller_cargo.as_deref().and_then(decode) {
            Some(cargo) => command.env("CARGO", cargo),
            None => command.env_remove("CARGO"),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::io::Result<PathBuf> {
        let root = std::env::temp_dir().join(format!(
            "sail-managed-transport-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    #[test]
    fn settings_round_trip_through_the_file_and_absent_names_stay_unset()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = scratch("round-trip")?;
        let mut transport = Transport::new(Some(OsStr::new("/caller/cargo")));
        transport
            .set("SAIL_MANAGED_RUNTIME", "/context/runtime.json")
            .set("SAIL_MANAGED_EMPTY", "");
        transport.write(&directory)?;
        let read = Transport::read(&directory.join(FILE)).ok_or("unreadable")?;
        assert_eq!(read, transport);
        assert_eq!(
            read.get("SAIL_MANAGED_RUNTIME"),
            Some(OsString::from("/context/runtime.json"))
        );
        // Present-empty and absent remain distinct.
        assert_eq!(read.get("SAIL_MANAGED_EMPTY"), Some(OsString::new()));
        assert_eq!(read.get("SAIL_MANAGED_SHIPPED"), None);
        assert_eq!(
            read.caller_cargo.as_deref().and_then(decode),
            Some(OsString::from("/caller/cargo"))
        );
        // No temporary file is left behind.
        assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
        Ok(())
    }

    #[test]
    fn another_version_is_not_read() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = scratch("version")?;
        std::fs::write(
            directory.join(FILE),
            br#"{"version":0,"values":{},"caller_cargo":null}"#,
        )?;
        assert!(Transport::read(&directory.join(FILE)).is_none());
        Ok(())
    }

    #[test]
    fn only_the_two_aliases_read_the_file() {
        // The test binary is neither alias, so its settings come from the
        // environment.
        assert!(location().is_none());
        assert!(loaded().is_none());
    }
}
