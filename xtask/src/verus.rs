//! `cargo xtask fetch-verus`: installs the pinned Verus release.
//!
//! Reads `corpus/toolchain.toml`, downloads the release archive for the host
//! platform with `curl`, checks its SHA-256 before unpacking it with `unzip`,
//! then checks the bundled z3's SHA-256. Installs are idempotent: a verified
//! install is left alone. It then installs the Rust toolchain the release
//! names (with rustup, if it is missing) and prints the `PATH` line that puts
//! `verus` and `cargo-verus` on `PATH`.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::{Result, workspace_root};

pub(crate) fn host() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}

pub(crate) fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(buf.get(..n).unwrap_or_default());
    }
    Ok(hasher.finalize().iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    }))
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run {cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} exited with {status}"))
    }
}

pub(crate) fn check_z3(z3: &Path, asset: &sail_verus::toolchain::AssetPin) -> Result<()> {
    let actual = file_sha256(z3)?;
    if asset.z3_sha256.is_empty() {
        println!(
            "note: no z3_sha256 pinned for {}; record this value in corpus/toolchain.toml: {actual}",
            host()
        );
        return Ok(());
    }
    if actual == asset.z3_sha256 {
        Ok(())
    } else {
        Err(format!(
            "{} has SHA-256 {actual}, expected {}",
            z3.display(),
            asset.z3_sha256
        ))
    }
}

/// Installs the pinned Verus release for the host and prints where it is.
pub fn fetch() -> Result<()> {
    let path = workspace_root().join("corpus").join("toolchain.toml");
    // Honours SAIL_VERUS_RELEASE (a secondary pin), like the harness does.
    let pins = sail_verus::toolchain::Pins::load(&path).map_err(|e| e.to_string())?;
    let asset = pins
        .assets
        .get(host())
        .ok_or_else(|| format!("no pinned Verus asset for host {}", host()))?;
    // SAIL_VERUS_HOME names the primary release's install only.
    let root = sail_verus::toolchain::install_root(&pins.verus.release, !pins.is_secondary)
        .map_err(|error| error.to_string())?;
    let install = root.join(&asset.dir);
    // A release that bundles no z3 uses the primary's (`z3 = "primary"`).
    let bundled = asset.z3.as_deref() != Some("primary");
    let local_z3 = install.join("z3");
    let installed = install.join("verus").is_file()
        && (!bundled || (local_z3.is_file() && check_z3(&local_z3, asset).is_ok()));

    if !installed {
        require_tools()?;
        let downloads = root.join("downloads");
        std::fs::create_dir_all(&downloads)
            .map_err(|e| format!("cannot create {}: {e}", downloads.display()))?;
        let name = asset.url.rsplit('/').next().unwrap_or("verus.zip");
        let archive = downloads.join(name);
        if !(archive.is_file() && file_sha256(&archive)? == asset.sha256) {
            let partial = downloads.join(format!("{name}.part"));
            println!("downloading {}", asset.url);
            run(Command::new("curl")
                .args(["-sSfL", "-o"])
                .arg(&partial)
                .arg(&asset.url))?;
            let actual = file_sha256(&partial)?;
            if actual != asset.sha256 {
                let _ = std::fs::remove_file(&partial);
                return Err(format!(
                    "{name} has SHA-256 {actual}, expected {}",
                    asset.sha256
                ));
            }
            std::fs::rename(&partial, &archive)
                .map_err(|e| format!("cannot move {}: {e}", partial.display()))?;
        }
        let staging = root.join("staging");
        let _ = std::fs::remove_dir_all(&staging);
        run(Command::new("unzip")
            .arg("-q")
            .arg(&archive)
            .arg("-d")
            .arg(&staging))?;
        let _ = std::fs::remove_dir_all(&install);
        std::fs::rename(staging.join(&asset.dir), &install)
            .map_err(|e| format!("cannot install into {}: {e}", install.display()))?;
        let _ = std::fs::remove_dir_all(&staging);
    }
    let z3 = sail_verus::toolchain::Installed::locate(pins.clone())
        .map_err(|e| {
            format!("{e} (for a release using the primary's z3, fetch the primary first)")
        })?
        .z3;
    check_z3(&z3, asset)?;

    let toolchain = &pins.verus.rust_toolchain;
    let toolchain_note = install_toolchain(toolchain);

    println!(
        "Verus {} (z3 {}) installed:",
        pins.verus.release, pins.z3.version
    );
    println!("  verus:          {}", install.join("verus").display());
    println!("  z3:             {}", z3.display());
    println!("  rust toolchain: {toolchain} ({toolchain_note})");
    println!();
    println!("Put Verus on PATH (add this line to your shell profile to keep it):");
    println!("  export PATH=\"{}:$PATH\"", install.display());
    Ok(())
}

/// Whether `program` can be run (it is on `PATH`).
fn available(program: &str, probe: &str) -> bool {
    Command::new(program)
        .arg(probe)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Installs the Rust toolchain Verus needs with rustup, unless it is already
/// installed. Returns a short note for the summary. Never fails the fetch:
/// without rustup it names the exact command to run.
fn install_toolchain(toolchain: &str) -> String {
    let installed = Command::new("rustup")
        .args(["run", toolchain, "rustc", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match installed {
        Ok(status) if status.success() => "installed".to_owned(),
        Ok(_) => {
            println!("installing Rust {toolchain}, which Verus needs (rustup toolchain install)");
            match run(Command::new("rustup").args([
                "toolchain",
                "install",
                toolchain,
                "--profile",
                "minimal",
            ])) {
                Ok(()) => "installed now".to_owned(),
                Err(e) => format!(
                    "NOT installed: {e}; run `rustup toolchain install {toolchain} --profile minimal`"
                ),
            }
        }
        Err(_) => format!(
            "NOT installed: rustup is not on PATH; install rustup (https://rustup.rs), then run \
             `rustup toolchain install {toolchain} --profile minimal`"
        ),
    }
}

/// Fails early, with the package to install, when a download tool is missing.
fn require_tools() -> Result<()> {
    for (program, probe) in [("curl", "--version"), ("unzip", "-v")] {
        if !available(program, probe) {
            return Err(format!(
                "`cargo xtask fetch-verus` needs `{program}` on PATH; install it first \
                 (Debian/Ubuntu: `sudo apt-get install {program}`)"
            ));
        }
    }
    Ok(())
}
