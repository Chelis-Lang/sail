//! Isolated integration-fixture coordination; never enabled in product builds.

use crate::{Error, Result};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const DIRECTORY: &str = "SAIL_BOUNDARY_INPUT_FIXTURE";

fn error(message: impl Into<String>) -> Error {
    Error::new("SAIL3134", message.into())
}

fn directory() -> Result<Option<PathBuf>> {
    let Some(path) = std::env::var_os(DIRECTORY) else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    if !path.is_absolute() || !path.is_dir() {
        return Err(error(
            "fixture requires an existing absolute owned directory",
        ));
    }
    Ok(Some(path))
}

fn write(root: &Path, name: &str, value: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|failure| error(failure.to_string()))?;
    if bytes.len() > 1024 * 1024 {
        return Err(error("fixture record limit exceeded"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(root.join(name))
        .map_err(|failure| error(failure.to_string()))?;
    file.write_all(&bytes)
        .map_err(|failure| error(failure.to_string()))
}

/// Retain the genuine native compiler invocation for an isolated fixture replay.
pub(super) fn native(wrapper: &Path, args: &[OsString]) -> Result<()> {
    let Some(root) = directory()? else {
        return Ok(());
    };
    if std::env::var_os("CARGO_PKG_NAME").is_none() {
        return Ok(());
    }
    let environment: BTreeMap<_, _> = std::env::vars_os()
        .map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect::<Option<_>>()
        .ok_or_else(|| error("non-UTF-8 fixture environment"))?;
    let args: Vec<_> = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<_>>()
        .ok_or_else(|| error("non-UTF-8 fixture arguments"))?;
    write(
        &root,
        &format!("native-{}.json", std::process::id()),
        &serde_json::json!({
            "kind":"sail-boundary-native-fixture", "schema":1,
            "wrapper":wrapper, "args":args, "environment":environment,
            "cwd":std::env::current_dir().map_err(|failure| error(failure.to_string()))?,
        }),
    )
}

/// Pause only an admitted cache miss, after input keying and before real spawn.
pub(super) fn before_spawn(command: &Command) -> Result<()> {
    let Some(root) = directory()? else {
        return Ok(());
    };
    let pid = std::process::id();
    let proceed = root.join(format!("continue-{pid}"));
    if proceed
        .try_exists()
        .map_err(|failure| error(failure.to_string()))?
    {
        return Err(error("fixture continuation already exists"));
    }
    write(
        &root,
        &format!("ready-{pid}.json"),
        &serde_json::json!({
            "kind":"sail-boundary-spawn-fixture", "schema":1, "pid":pid,
            "program":command.get_program(),
            "args":command.get_args().collect::<Vec<_>>(),
            "environment_overrides":command.get_envs().collect::<Vec<_>>(),
        }),
    )?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(180) {
        match std::fs::read(&proceed) {
            Ok(bytes) if bytes == b"continue\n" => return Ok(()),
            Ok(_) => return Err(error("fixture continuation bytes differ")),
            Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(failure) => return Err(error(failure.to_string())),
        }
    }
    Err(error("fixture continuation deadline exceeded"))
}
