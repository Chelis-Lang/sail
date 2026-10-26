//! Sail's own Verus options for a verified target, carried the way
//! Cargo-Verus carries the user's (#393).
//!
//! Cargo-Verus never puts verifier options on the compiler's command line: it
//! packs them into ` __VERUS_DRIVER_ARGS__` (and a per-package variable), and
//! the Verus driver appends them to its arguments. Procedural macros run in
//! the compiler process and can read its command line, so the driver adds
//! Sail's observation options (`--output-json`, `--time`, attribution logs and
//! the retained-proof checksum request) to that variable for its child
//! instead. The command line Cargo builds stays exactly plain Cargo-Verus's.
//!
//! Sail's options come first, before any of the user's (and so before any
//! `--` the user passed), as they did when they were arguments: the Verus
//! driver decodes the common variable, then the package's.
//!
//! The Verus driver records the variable's value in the target's dep-info,
//! which Cargo checks against its own environment on the next build. The
//! driver writes back the value Cargo has, so a fresh target stays fresh.
//! Compiler-input observations name the same variable; [`normalize`] maps
//! Sail's value back to the one Cargo gave.

use crate::{Error, Result};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Cargo-Verus's variable for the Verus options of every verified package.
pub(crate) const COMMON: &str = " __VERUS_DRIVER_ARGS__";
/// Cargo-Verus's separator before each packed option.
const SEPARATOR: &str = "__VERUS_DRIVER_ARGS_SEP__";

/// The variable's value without and with Sail's options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Channel {
    /// What the compiler would see without Sail's options.
    base: Option<String>,
    /// What it sees; `None` when Sail adds nothing.
    augmented: Option<String>,
}

impl Channel {
    /// The channel for `options`, on top of `base`: the value the compiler
    /// would otherwise get (Cargo's, or the launcher's relocated value).
    pub(crate) fn new(base: Option<String>, options: &[OsString]) -> Result<Self> {
        if options.is_empty() {
            return Ok(Self {
                base,
                augmented: None,
            });
        }
        let mut augmented = String::new();
        for option in options {
            let option = option.to_str().ok_or_else(|| {
                Error::new("SAIL3111", "a managed Verus option is not valid UTF-8")
            })?;
            if option.contains(SEPARATOR) {
                return Err(Error::new(
                    "SAIL3111",
                    "a managed Verus option contains Cargo-Verus's separator",
                ));
            }
            augmented.push_str(SEPARATOR);
            augmented.push_str(option);
        }
        // The driver ignores anything before the first separator, so only the
        // packed options of the base follow Sail's.
        if let Some(start) = base.as_deref().and_then(|base| base.find(SEPARATOR))
            && let Some(rest) = base.as_deref().and_then(|base| base.get(start..))
        {
            augmented.push_str(rest);
        }
        Ok(Self {
            base,
            augmented: Some(augmented),
        })
    }

    /// The value Cargo gave this process, which a launcher relocation may
    /// replace (`relocated`).
    pub(crate) fn cargo_value(relocated: &BTreeMap<String, String>) -> Result<Option<String>> {
        if let Some(value) = relocated.get(COMMON) {
            return Ok(Some(value.clone()));
        }
        match std::env::var(COMMON) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(Error::new(
                "SAIL3123",
                format!("invalid effective Cargo driver arguments: {error}"),
            )),
        }
    }

    /// Gives `command` the augmented value.
    pub(crate) fn apply(&self, command: &mut std::process::Command) {
        if let Some(value) = &self.augmented {
            command.env(COMMON, value);
        }
    }

    /// Makes this the channel [`normalize`] maps back, for this process's one
    /// verified compiler.
    pub(crate) fn publish(&self) {
        let _ = ACTIVE.set(self.clone());
    }

    /// Writes the base value back into the dep-info at `path` (see
    /// [`dep_info_path`]), where the driver recorded the augmented one.
    /// Best effort: a dep-info left unchanged only makes Cargo build the
    /// target again next time.
    pub(crate) fn restore_dep_info(&self, path: Option<&Path>) {
        let (Some(augmented), Some(path)) = (&self.augmented, path) else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        if let Some(restored) = restore_line(&text, augmented, self.base.as_deref())
            && restored != text
        {
            let temporary = path.with_extension("d.sail-tmp");
            if std::fs::write(&temporary, restored)
                .and_then(|()| std::fs::rename(&temporary, path))
                .is_err()
            {
                let _ = std::fs::remove_file(&temporary);
            }
        }
    }
}

static ACTIVE: OnceLock<Channel> = OnceLock::new();

/// Maps an observed ` __VERUS_DRIVER_ARGS__` that carries exactly Sail's
/// options back to the value without them. Any other value is unchanged.
pub(crate) fn normalize(environment: &mut BTreeMap<String, Option<String>>) {
    if let Some(channel) = ACTIVE.get() {
        normalize_with(channel, environment);
    }
}

fn normalize_with(channel: &Channel, environment: &mut BTreeMap<String, Option<String>>) {
    if let (Some(augmented), Some(value)) = (&channel.augmented, environment.get_mut(COMMON))
        && value.as_deref() == Some(augmented.as_str())
    {
        value.clone_from(&channel.base);
    }
}

/// rustc's escaping of an `env-dep` value.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn env_dep_line(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("# env-dep:{COMMON}={}", escape(value)),
        None => format!("# env-dep:{COMMON}"),
    }
}

/// `text` with its one `env-dep` line for exactly `augmented` replaced by
/// `base`'s; `None` when there is no such line.
fn restore_line(text: &str, augmented: &str, base: Option<&str>) -> Option<String> {
    let expected = env_dep_line(Some(augmented));
    let mut found = false;
    let mut restored = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (content, ending) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        if content == expected {
            found = true;
            restored.push_str(&env_dep_line(base));
        } else {
            restored.push_str(content);
        }
        restored.push_str(ending);
    }
    found.then_some(restored)
}

/// Where rustc writes this compilation's dep-info: an explicit
/// `--emit dep-info=PATH`, else `<out-dir>/<crate-name><extra-filename>.d`.
pub(crate) fn dep_info_path(args: &[OsString]) -> Option<PathBuf> {
    let args: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
    let value = |flag: &str| {
        let prefix = format!("{flag}=");
        args.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1])
            .or_else(|| {
                args.iter()
                    .find_map(|arg| arg.strip_prefix(prefix.as_str()))
            })
    };
    let emits = args
        .windows(2)
        .filter(|pair| pair[0] == "--emit")
        .map(|pair| pair[1])
        .chain(args.iter().filter_map(|arg| arg.strip_prefix("--emit=")));
    for emit in emits {
        for kind in emit.split(',') {
            if let Some(path) = kind.strip_prefix("dep-info=") {
                return Some(PathBuf::from(path));
            }
        }
    }
    let codegen = args
        .windows(2)
        .filter(|pair| pair[0] == "-C" || pair[0] == "--codegen")
        .map(|pair| pair[1])
        .chain(args.iter().filter_map(|arg| arg.strip_prefix("-C")))
        .filter_map(|option| option.strip_prefix("extra-filename="))
        .next_back()
        .unwrap_or_default();
    let directory = value("--out-dir")?;
    let name = value("--crate-name")?;
    Some(Path::new(directory).join(format!("{name}{codegen}.d")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn decode(value: &str) -> Vec<&str> {
        // The pinned driver's decoder (rust_verify `cargo_verus.rs`).
        value.split(SEPARATOR).skip(1).collect()
    }

    #[test]
    fn sail_options_come_first_and_the_base_options_follow_unchanged() -> Result<()> {
        let options = os(&["--output-json", "--time"]);
        let absent = Channel::new(None, &options)?;
        assert_eq!(
            decode(absent.augmented.as_deref().unwrap_or_default()),
            ["--output-json", "--time"]
        );
        let user = format!("{SEPARATOR}--rlimit{SEPARATOR}20{SEPARATOR}--{SEPARATOR}x");
        let present = Channel::new(Some(user.clone()), &options)?;
        assert_eq!(
            decode(present.augmented.as_deref().unwrap_or_default()),
            ["--output-json", "--time", "--rlimit", "20", "--", "x"]
        );
        assert_eq!(present.base.as_deref(), Some(user.as_str()));
        // Text before the first separator is ignored by the driver, so it is
        // dropped rather than glued to Sail's last option.
        let odd = Channel::new(Some(format!("junk{SEPARATOR}--rlimit")), &options)?;
        assert_eq!(
            decode(odd.augmented.as_deref().unwrap_or_default()),
            ["--output-json", "--time", "--rlimit"]
        );
        // Nothing to add leaves the variable alone.
        assert_eq!(Channel::new(Some(user), &[])?.augmented, None);
        Ok(())
    }

    #[test]
    fn options_that_cannot_be_packed_are_refused() {
        assert!(Channel::new(None, &os(&[&format!("--log-dir=a{SEPARATOR}b")])).is_err());
    }

    #[test]
    fn dep_info_gets_cargos_value_back_and_nothing_else_changes() -> Result<()> {
        let channel = Channel::new(None, &os(&["--output-json", "--time"]))?;
        let augmented = channel.augmented.clone().unwrap_or_default();
        let text = format!(
            "/t/libdemo.rmeta: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:VERUS_Z3_PATH=/z3\n{}\n# env-dep: __VERUS_DRIVER_ARGS_FOR_demo\n",
            env_dep_line(Some(&augmented))
        );
        let restored = restore_line(&text, &augmented, None).unwrap_or_default();
        assert_eq!(
            restored,
            "/t/libdemo.rmeta: src/lib.rs\n\nsrc/lib.rs:\n\n# env-dep:VERUS_Z3_PATH=/z3\n# env-dep: __VERUS_DRIVER_ARGS__\n# env-dep: __VERUS_DRIVER_ARGS_FOR_demo\n"
        );
        let base = format!("{SEPARATOR}a\\b\nc");
        let with_user = Channel::new(Some(base.clone()), &os(&["--time"]))?;
        let augmented = with_user.augmented.clone().unwrap_or_default();
        let line = env_dep_line(Some(&augmented));
        let restored = restore_line(&format!("{line}\n"), &augmented, Some(&base));
        assert_eq!(
            restored.as_deref(),
            Some(format!("# env-dep: __VERUS_DRIVER_ARGS__={SEPARATOR}a\\\\b\\nc\n").as_str())
        );
        // A value Sail did not write is left alone.
        assert_eq!(
            restore_line("# env-dep: __VERUS_DRIVER_ARGS__=other\n", &augmented, None),
            None
        );
        Ok(())
    }

    #[test]
    fn observations_of_sails_value_name_cargos() -> Result<()> {
        let channel = Channel::new(None, &os(&["--time"]))?;
        let mut observed = BTreeMap::from([
            (COMMON.to_owned(), channel.augmented.clone()),
            ("OTHER".to_owned(), Some("x".to_owned())),
        ]);
        normalize_with(&channel, &mut observed);
        assert_eq!(observed.get(COMMON), Some(&None));
        assert_eq!(observed.get("OTHER"), Some(&Some("x".to_owned())));
        // Another value is not Sail's and keeps refusing admission.
        let mut other = BTreeMap::from([(COMMON.to_owned(), Some("y".to_owned()))]);
        normalize_with(&channel, &mut other);
        assert_eq!(other.get(COMMON), Some(&Some("y".to_owned())));
        Ok(())
    }

    #[test]
    fn dep_info_is_where_cargo_tells_rustc_to_write_it() {
        assert_eq!(
            dep_info_path(&os(&[
                "rustc",
                "--crate-name",
                "demo",
                "--emit=dep-info,metadata,link",
                "-C",
                "extra-filename=-0123abcd",
                "--out-dir",
                "/t/debug/deps",
            ])),
            Some(PathBuf::from("/t/debug/deps/demo-0123abcd.d"))
        );
        assert_eq!(
            dep_info_path(&os(&[
                "rustc",
                "--crate-name=demo",
                "-Cextra-filename=-1",
                "--out-dir=/o",
            ])),
            Some(PathBuf::from("/o/demo-1.d"))
        );
        assert_eq!(
            dep_info_path(&os(&["rustc", "--emit", "dep-info=/x/y.d,metadata"])),
            Some(PathBuf::from("/x/y.d"))
        );
        assert_eq!(dep_info_path(&os(&["rustc", "--crate-name", "demo"])), None);
    }
}
