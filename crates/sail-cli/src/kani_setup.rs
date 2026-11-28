//! Explicit original-tool setup from package-local pinned resources.
use std::io::Write;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let result = command(args).and_then(|value| {
        serde_json::to_writer(&mut *out, &value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(err, "error[SAIL6306]: Kani runtime setup: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) fn root() -> Result<PathBuf, String> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    let cache = sail_verus::cache_dir().map_err(|error| error.to_string())?;
    let root = cache.join("kani");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&root)
        .map_err(|error| error.to_string())?;
    let metadata = std::fs::symlink_metadata(&root).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("Kani cache must be a private regular directory".into());
    }
    root.canonicalize().map_err(|error| error.to_string())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) fn private_attempt(parent: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::DirBuilderExt as _;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = parent.join(format!("attempt-{}-{stamp}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn command(args: &[String]) -> Result<serde_json::Value, String> {
    let mut args = args.to_vec();
    if args.ends_with(&["--output".into(), "json".into()]) {
        args.truncate(args.len() - 2);
    }
    let [action, flag, backend] = args.as_slice() else {
        return Err("usage: sail engine install|status --backend kani [--output json]".into());
    };
    if flag != "--backend" || backend != "kani" || !matches!(action.as_str(), "install" | "status")
    {
        return Err("usage: sail engine install|status --backend kani [--output json]".into());
    }
    let root = root()?;
    let destination = root.join(format!(
        "{}-linux-x86_64",
        sail_kani::tools::RUNTIME_RELEASE
    ));
    if action == "install" && std::fs::symlink_metadata(&destination).is_err() {
        install(&root, &destination)?;
    }
    let tools = sail_kani::tools::Tools::authenticate(
        &destination.join("bundle"),
        &destination.join("rust"),
    )
    .map_err(|error| error.to_string())?;
    Ok(
        serde_json::json!({"kind":"sail-kani-runtime-status","version":1,"backend":"kani","experimental":true,"runtime_release":sail_kani::tools::RUNTIME_RELEASE,"bundle":tools.bundle(),"rust":tools.rust(),"runtime_authenticated":true,"backend_executed":false,"proof_authority":false}),
    )
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn install(root: &Path, destination: &Path) -> Result<(), String> {
    let preparation = private_attempt(root)?;
    let result = (|| {
        let resources: [(&str, &[u8]); 6] = [
            (
                "ci/kani-fetch.py",
                include_bytes!("../resources/ci/kani-fetch.py"),
            ),
            (
                "ci/kani-install.py",
                include_bytes!("../resources/ci/kani-install.py"),
            ),
            (
                "ci/kani-bundle.py",
                include_bytes!("../resources/ci/kani-bundle.py"),
            ),
            (
                "ci/kani-toolchain.py",
                include_bytes!("../resources/ci/kani-toolchain.py"),
            ),
            (
                "engine/kani/fork-bundle-linux.json",
                include_bytes!("../resources/engine/kani/fork-bundle-linux.json"),
            ),
            (
                "engine/kani/toolchain-linux.json",
                include_bytes!("../resources/engine/kani/toolchain-linux.json"),
            ),
        ];
        for (name, bytes) in resources {
            let path = preparation.join(name);
            std::fs::create_dir_all(path.parent().ok_or("missing preparation parent")?)
                .map_err(|error| error.to_string())?;
            std::fs::write(path, bytes).map_err(|error| error.to_string())?;
        }
        let output = std::process::Command::new("python3")
            .arg("-I")
            .arg(preparation.join("ci/kani-fetch.py"))
            .arg("--destination")
            .arg(destination)
            .output()
            .map_err(|error| {
                format!("original runtime installation requires Python 3.11+: {error}")
            })?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(preparation);
    result
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn command(_args: &[String]) -> Result<serde_json::Value, String> {
    Err("the pinned experimental Kani runtime supports native Linux x86-64".into())
}
