//! Explicit installation through the same pinned installer used by CI.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn staging() -> Result<Staging, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("error[SAIL3120]: cannot create installer directory: {error}"))?
        .as_nanos();
    for attempt in 0..16 {
        let path = std::env::temp_dir().join(format!(
            "sail-engine-install-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(Staging(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "error[SAIL3120]: cannot create installer directory: {error}"
                ));
            }
        }
    }
    Err("error[SAIL3120]: cannot allocate a private installer directory".into())
}

pub(crate) fn install(
    cache: &Path,
    pins: &sail_verus::toolchain::Pins,
) -> Result<sail_verus::engine::Status, String> {
    if let Ok(status) = sail_verus::engine::status(cache, pins) {
        return Ok(status);
    }
    let (platform, pin): (&str, &[u8]) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => (
            "linux",
            include_bytes!("../resources/engine/verus/artifact-linux.json"),
        ),
        ("macos", "aarch64") => (
            "macos",
            include_bytes!("../resources/engine/verus/artifact-macos.json"),
        ),
        _ => {
            return Err(
                "error[SAIL3120]: prebuilt runtimes support Linux x86-64 and Apple-silicon macOS"
                    .into(),
            );
        }
    };
    let staging = staging()?;
    let pin_path = format!("engine/verus/artifact-{platform}.json");
    let inputs: [(&str, &[u8]); 4] = [
        (
            "ci/engine-artifact.py",
            include_bytes!("../resources/ci/engine-artifact.py"),
        ),
        (
            "engine/verus/fork.toml",
            sail_verus::engine::FORK.as_bytes(),
        ),
        (
            "engine/verus/Cargo.lock",
            sail_verus::engine::SOURCE_LOCK.as_bytes(),
        ),
        (&pin_path, pin),
    ];
    for (relative, bytes) in inputs {
        let path = staging.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("error[SAIL3120]: cannot stage the pinned installer: {error}")
            })?;
        }
        std::fs::write(path, bytes).map_err(|error| {
            format!("error[SAIL3120]: cannot stage the pinned installer: {error}")
        })?;
    }
    let output = Command::new("python3")
        .arg("-I")
        .arg(staging.0.join("ci/engine-artifact.py"))
        .arg("fetch")
        .arg("--pin")
        .arg(staging.0.join(pin_path))
        .arg("--cache")
        .arg(cache)
        .output()
        .map_err(|error| {
            format!("error[SAIL3120]: engine installation requires Python 3: {error}")
        })?;
    if !output.status.success() {
        return Err(format!(
            "error[SAIL3120]: pinned runtime installation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    sail_verus::engine::status(cache, pins).map_err(|error| error.to_string())
}
