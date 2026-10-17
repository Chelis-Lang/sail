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
    let destination = root.join(name);
    if destination
        .try_exists()
        .map_err(|failure| error(failure.to_string()))?
    {
        return Err(error("fixture record already exists"));
    }
    let temporary = root.join(format!(".{name}.temporary"));
    let mut file = options
        .open(&temporary)
        .map_err(|failure| error(failure.to_string()))?;
    file.write_all(&bytes)
        .map_err(|failure| error(failure.to_string()))?;
    drop(file);
    std::fs::rename(temporary, destination).map_err(|failure| error(failure.to_string()))
}

/// One selected admitted native producer's isolated coordination handle.
pub(super) struct Native {
    root: PathBuf,
    adapter_pid: u32,
}

/// Pause after native source key/graph capture, before the actual child exists.
pub(super) fn native_before_spawn(
    command: &Command,
    original: &[OsString],
) -> Result<Option<Native>> {
    let Some(root) = directory()? else {
        return Ok(None);
    };
    let selected = std::env::var("SAIL_NATIVE_BOUNDARY_PACKAGE")
        .map_err(|_| error("native fixture requires an explicit package"))?;
    if std::env::var("CARGO_PKG_NAME").as_deref() != Ok(selected.as_str()) {
        return Ok(None);
    }
    let pid = std::process::id();
    let proceed = root.join(format!("continue-{pid}"));
    if proceed
        .try_exists()
        .map_err(|failure| error(failure.to_string()))?
    {
        return Err(error("native fixture continuation already exists"));
    }
    let overrides: BTreeMap<_, _> = command
        .get_envs()
        .filter(|(name, _)| {
            *name == "SAIL_NATIVE_COMPILER_INPUTS" || *name == "SAIL_NATIVE_COMPILER_INPUTS_BINDING"
        })
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.map(std::ffi::OsStr::to_owned),
            )
        })
        .collect();
    write(
        &root,
        &format!("ready-{pid}.json"),
        &serde_json::json!({
            "kind":"sail-native-producer-spawn-fixture", "schema":1,
            "pid":pid, "package":selected,
            "cwd":std::env::current_dir().map_err(|failure| error(failure.to_string()))?,
            "program":command.get_program(),
            "original_args":original,
            "args":command.get_args().collect::<Vec<_>>(),
            "environment_overrides":overrides,
        }),
    )?;
    await_continuation(&proceed)?;
    Ok(Some(Native {
        root,
        adapter_pid: pid,
    }))
}

impl Native {
    /// Pause after actual wait, before producer completion checks/publication.
    pub(super) fn after_completion(&self, compiler_pid: u32, exit: Option<i32>) -> Result<()> {
        let proceed = self.root.join(format!("publish-{}", self.adapter_pid));
        if proceed
            .try_exists()
            .map_err(|failure| error(failure.to_string()))?
        {
            return Err(error(
                "native fixture publication continuation already exists",
            ));
        }
        write(
            &self.root,
            &format!("completed-{}.json", self.adapter_pid),
            &serde_json::json!({"kind":"sail-native-producer-completed-fixture",
                "schema":1, "pid":self.adapter_pid,"compiler_pid":compiler_pid,"exit":exit}),
        )?;
        await_continuation(&proceed)
    }
}

fn await_continuation(proceed: &Path) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(180) {
        match std::fs::read(proceed) {
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
