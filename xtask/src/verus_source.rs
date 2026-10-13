//! `cargo xtask build-verus-source`: a source build of the pinned Verus
//! commit, for the deep tier's `rust_verify_test` conformance (ADR 0019).
//!
//! It follows Verus's own build instructions (`BUILD.md` at the pinned
//! commit), with two differences that keep it reproducible:
//! - **z3 is the pinned one.** `source/tools/get-z3.sh` is never run. The
//!   build uses the z3 of the installed release (`cargo xtask
//!   fetch-verus`), checked by SHA-256 before the build and again after
//!   `vargo` copies it into `target-verus/release`.
//! - **The toolchain is the pinned one.** `rustup toolchain install` in the
//!   checkout installs exactly what Verus's `rust-toolchain.toml` lists
//!   (including `rustc-dev`), and the channel must equal
//!   `corpus/toolchain.toml`'s `rust_toolchain`.
//!
//! Steps: fetch the commit into `<cache>/verus-src-build/<commit>/verus`;
//! build `vargo` (`tools/vargo`); `vargo build --release`, which builds
//! Verus and builds and verifies vstd; `vargo test -p rust_verify_test
//! --release --no-run`, which builds the test binaries. Then, unless
//! `--keep-intermediates`, every build intermediate that the test binaries
//! do not need at run time is deleted, and `sail-build.json` records the
//! build. A complete build is left alone unless `--force` is given.
//!
//! `--interactive-engine` (`cargo xtask build-verus-engine`) builds the
//! optional engine instead: the exact commit of Sail's private Verus fork
//! pinned in `engine/verus/fork.toml`, fetched with a GitHub token, whose
//! upstream base must be the pinned commit. Local only; CI installs the
//! prebuilt runtime.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use sail_verus::source_build::{self, Manifest, TestBinary};
use sail_verus::toolchain::{AssetPin, Installed, Pins};

use crate::Result;
use crate::verus::{check_z3, file_sha256, host};

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent source-build command switches mirror explicit CLI options."
)]
struct Options {
    force: bool,
    keep_intermediates: bool,
    features: Vec<String>,
    print_path: bool,
    engine: bool,
}

fn parse(args: &[String]) -> Result<Options> {
    let mut o = Options::default();
    for arg in args {
        match arg.as_str() {
            "--force" => o.force = true,
            "--interactive-engine" => o.engine = true,
            "--features=singular" => o.features = vec!["singular".to_owned()],
            "--print-path" => o.print_path = true,
            "--keep-intermediates" => o.keep_intermediates = true,
            _ => {
                return Err(format!(
                    "unknown argument `{arg}`\nusage: cargo xtask build-verus-source [--force] [--keep-intermediates] [--features=singular]"
                ));
            }
        }
    }
    Ok(o)
}

/// Variables from the enclosing `cargo xtask` (or a user's shell) that must
/// not leak into Verus's build: Verus picks its own toolchain and flags.
const CARGO_ENV: &[&str] = &[
    "CARGO",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_MANIFEST_DIR",
    "CARGO_MANIFEST_PATH",
    "CARGO_PKG_NAME",
    "CARGO_PRIMARY_PACKAGE",
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
    "SAIL_VERUS_SOURCE_BUILD",
];

fn clean<'a>(cmd: &'a mut Command, jobs: &str) -> &'a mut Command {
    for var in CARGO_ENV {
        cmd.env_remove(var);
    }
    cmd.env("CARGO_BUILD_JOBS", jobs)
        .env("CARGO_TERM_PROGRESS_WHEN", "never")
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

/// A GitHub token for the private Verus fork: `SAIL_ENGINE_ARTIFACT_TOKEN`
/// (as for the engine artifact) or else `gh auth token`.
fn github_token() -> Result<String> {
    if let Ok(token) = std::env::var("SAIL_ENGINE_ARTIFACT_TOKEN")
        && !token.trim().is_empty()
    {
        return Ok(token.trim().to_owned());
    }
    let output = Command::new("gh")
        .args(["auth", "token"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("the Verus fork is private and `gh auth token` failed: {e}"))?;
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || token.is_empty() {
        return Err(
            "the Verus fork is private: set SAIL_ENGINE_ARTIFACT_TOKEN or run `gh auth login`"
                .to_owned(),
        );
    }
    Ok(token)
}

/// Runs `git ARGS` in `dir`, authenticating to GitHub with `token` through a
/// one-shot credential helper. The token reaches git only through the
/// environment of this one process and is never printed or stored.
fn git_with_token(dir: &Path, token: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .env("SAIL_FORK_TOKEN", token)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["-c", "credential.helper="])
        .args([
            "-c",
            "credential.helper=!f() { echo username=x-access-token; echo \"password=$SAIL_FORK_TOKEN\"; }; f",
        ])
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} exited with {}",
            args.join(" "),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Fetches exactly the pinned commit of Sail's Verus fork into the build's
/// checkout (`engine/verus/fork.toml`). The engine variant is built from it
/// instead of the upstream commit plus a patch. Local only: CI installs the
/// prebuilt runtime and never builds Verus.
fn fetch_fork(build: &Path, fork: &sail_verus::engine::Fork) -> Result<PathBuf> {
    let dir = source_build::checkout_dir(build);
    let marker = dir.join(".sail-fetched");
    if std::fs::read_to_string(&marker).is_ok_and(|c| c.trim() == fork.commit) {
        return Ok(dir);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let token = github_token()?;
    git_with_token(&dir, &token, &["init", "-q"])?;
    git_with_token(&dir, &token, &["remote", "add", "origin", &fork.repo])?;
    git_with_token(
        &dir,
        &token,
        &["fetch", "-q", "--depth", "1", "origin", &fork.commit],
    )?;
    git_with_token(&dir, &token, &["checkout", "-q", "--detach", "FETCH_HEAD"])?;
    let head = git_with_token(&dir, &token, &["rev-parse", "HEAD"])?;
    if head != fork.commit {
        return Err(format!(
            "fetched {head}, expected the pinned fork commit {}",
            fork.commit
        ));
    }
    std::fs::write(&marker, &fork.commit)
        .map_err(|e| format!("cannot write {}: {e}", marker.display()))?;
    Ok(dir)
}

/// The channel in a `rust-toolchain.toml`.
fn toolchain_channel(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    value
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} has no toolchain.channel", path.display()))
}

/// The test executables in `cargo test --no-run --message-format=json`
/// output, by test target name.
fn test_executables(stdout: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let is_test_target = msg["target"]["kind"]
            .as_array()
            .is_some_and(|k| k.iter().any(|v| v == "test"));
        if msg["reason"] == "compiler-artifact"
            && msg["profile"]["test"] == true
            && is_test_target
            && let (Some(name), Some(exe)) =
                (msg["target"]["name"].as_str(), msg["executable"].as_str())
        {
            out.push((name.to_owned(), PathBuf::from(exe)));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path).map_or(0, |entries| {
        entries.flatten().map(|e| dir_size(&e.path())).sum::<u64>()
    })
}

/// Deletes what the test binaries do not need at run time (ADR 0019).
///
/// Verus's Cargo configuration puts everything in `source/target-verus`.
/// The run-time artifacts are the files directly in
/// `target-verus/release` (`rust_verify`, `cargo-verus`, vstd, the macro
/// libraries, z3) and the test executables in its `deps` directory, which
/// the test harness locates relative to itself. Everything else there
/// (intermediate libraries, build scripts, incremental state), the
/// `source/target` directory vargo also writes, and vargo's own build go.
fn prune(checkout: &Path, keep: &BTreeSet<String>) -> Result<()> {
    remove(&checkout.join("tools").join("vargo").join("target"));
    let source = checkout.join("source");
    remove(&source.join("target"));
    let release = source.join("target-verus").join("release");
    for dir in ["build", "incremental", ".fingerprint", "examples"] {
        remove(&release.join(dir));
    }
    let deps = release.join("deps");
    for entry in std::fs::read_dir(&deps)
        .map_err(|e| format!("cannot read {}: {e}", deps.display()))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !keep.contains(&name) {
            remove(&entry.path());
        }
    }
    Ok(())
}

fn remove(path: &Path) {
    let _ = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
}

/// Builds vargo, Verus (with vstd) and the `rust_verify_test` binaries in
/// `checkout`, and returns the test executables by target name.
fn compile(
    checkout: &Path,
    z3: &Path,
    asset: &AssetPin,
    jobs: &str,
    singular: bool,
    engine: bool,
) -> Result<Vec<(String, PathBuf)>> {
    // The toolchain and components Verus's rust-toolchain.toml lists
    // (rustc-dev, llvm-tools, rustfmt).
    run(clean(Command::new("rustup").current_dir(checkout), jobs).args(["toolchain", "install"]))?;

    println!("building vargo");
    let vargo_dir = checkout.join("tools").join("vargo");
    run(clean(Command::new("cargo").current_dir(&vargo_dir), jobs).args(["build", "--release"]))?;
    let vargo_bin = vargo_dir.join("target").join("release");
    let path = std::env::join_paths(
        std::iter::once(vargo_bin.clone()).chain(
            std::env::var_os("PATH")
                .iter()
                .flat_map(std::env::split_paths),
        ),
    )
    .map_err(|e| format!("cannot extend PATH: {e}"))?;
    let source = checkout.join("source");
    let vargo = |args: &[&str]| -> Command {
        let mut cmd = Command::new(vargo_bin.join("vargo"));
        clean(&mut cmd, jobs)
            .current_dir(&source)
            .env("PATH", &path)
            .env("VERUS_Z3_PATH", z3)
            .args(args);
        if singular {
            cmd.args(["--features", "singular"]);
        }
        cmd
    };

    println!("building Verus (vargo build --release; verifies vstd)");
    run(&mut vargo(&["build", "--release"]))?;
    let copied = source.join("target-verus").join("release").join("z3");
    if copied.is_file() {
        check_z3(&copied, asset)?;
    }

    if engine {
        // The Verus release workflow (`.github/workflows/build.yml` at the
        // pinned commit) builds vstd separately, from `source/vstd`, with
        // `cargo-verus build --release --manifest-path vstd/Cargo.toml`.
        // vargo's `vstd_build` compiles `vstd/vstd.rs` from `source`, so its
        // vstd records `vstd/seq.rs` where the release records `seq.rs`, and
        // engine diagnostics pointing into vstd differed from stock. Do what
        // the release does; cargo-verus hoists vstd.vir, libvstd.rlib and
        // the builtin libraries into `target-verus/release`.
        println!(
            "rebuilding vstd as the Verus release does (cargo-verus build --manifest-path vstd/Cargo.toml)"
        );
        let release = source.join("target-verus").join("release");
        run(clean(
            Command::new(release.join("cargo-verus")).current_dir(&source),
            jobs,
        )
        .env("PATH", &path)
        .env("VERUS_Z3_PATH", z3)
        .args(["build", "--release", "--manifest-path", "vstd/Cargo.toml"]))?;
        // That Cargo build leaves its build-dir intermediates (per-package
        // directories, dep-info and lock files) beside the runtime; the
        // runtime needs none of them (`deps` is pruned separately).
        for entry in std::fs::read_dir(&release)
            .map_err(|e| format!("cannot read {}: {e}", release.display()))?
            .flatten()
        {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if (path.is_dir() && name != "deps")
                || name == ".cargo-lock"
                || name == ".cargo-artifact-lock"
                || path.extension().is_some_and(|e| e == "d")
            {
                remove(&path);
            }
        }
        return Ok(Vec::new());
    }

    println!("building the rust_verify_test binaries");
    let output = vargo(&["test", "-p", "rust_verify_test", "--release"])
        .args(["--no-run", "--message-format=json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to run vargo test: {e}"))?;
    if !output.status.success() {
        return Err(format!("vargo test --no-run exited with {}", output.status));
    }
    let executables = test_executables(&String::from_utf8_lossy(&output.stdout));
    if executables.is_empty() {
        return Err("vargo test --no-run reported no rust_verify_test binaries".to_owned());
    }
    Ok(executables)
}

fn relative_binaries(build: &Path, executables: &[(String, PathBuf)]) -> Result<Vec<TestBinary>> {
    let root = build
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", build.display()))?;
    let mut test_binaries = Vec::new();
    for (name, exe) in executables {
        let rel = exe
            .strip_prefix(&root)
            .map_err(|_| format!("{} is outside {}", exe.display(), build.display()))?;
        test_binaries.push(TestBinary {
            name: name.clone(),
            path: rel.to_path_buf(),
        });
    }

    Ok(test_binaries)
}

fn active_pins() -> Result<(Pins, AssetPin)> {
    let pins = Pins::load(
        &crate::workspace_root()
            .join("corpus")
            .join("toolchain.toml"),
    )
    .map_err(|e| e.to_string())?;
    let asset = pins
        .assets
        .get(host())
        .ok_or_else(|| format!("no pinned Verus asset for host {}", host()))?
        .clone();
    Ok((pins, asset))
}

/// Builds the pinned Verus commit from source; see the module docs.
#[allow(
    clippy::too_many_lines,
    reason = "Source build identity, patch application, and final manifest publication stay one lifecycle."
)]
pub fn build(args: &[String]) -> Result<()> {
    let o = parse(args)?;
    let (pins, asset) = active_pins()?;
    let installed = Installed::locate(pins.clone())
        .map_err(|e| format!("{e}\nthe source build uses the release's pinned z3"))?;
    check_z3(&installed.z3, &asset)?;
    let z3_sha256 = file_sha256(&installed.z3)?;
    let commit = &pins.verus.commit;
    let toolchain = &pins.verus.rust_toolchain;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let features = if o.engine {
        sail_verus::engine::features()
    } else {
        o.features.clone()
    };
    let build = source_build::variant_dir(&cache, commit, toolchain, &features);
    if o.print_path {
        println!("{}", build.display());
        return Ok(());
    }
    let _ownership = source_build::lock_build(&build).map_err(|e| e.to_string())?;

    if !o.force
        && let Ok(m) = Manifest::load(&build)
        && m.commit == *commit
        && m.rust_toolchain == *toolchain
        && m.features == features
        && m.z3_sha256 == z3_sha256
        && (if o.engine {
            sail_verus::engine::Installed::locate(&cache, &pins).is_ok()
        } else {
            m.is_complete(&build)
        })
    {
        println!(
            "Verus {commit} source build is complete in {} ({} test binaries); --force rebuilds it",
            build.display(),
            m.test_binaries.len()
        );
        return Ok(());
    }
    let started = Instant::now();
    if o.force {
        let _ = std::fs::remove_file(build.join(source_build::MANIFEST));
    }
    std::fs::create_dir_all(&build)
        .map_err(|e| format!("cannot create {}: {e}", build.display()))?;
    let checkout = if o.engine {
        let fork = sail_verus::engine::fork().map_err(|e| e.to_string())?;
        if fork.upstream != *commit {
            return Err(format!(
                "engine/verus/fork.toml is based on {}, but corpus/toolchain.toml pins {commit}",
                fork.upstream
            ));
        }
        println!(
            "fetching the Verus fork {} @ {} into {}",
            fork.repo,
            fork.commit,
            build.display()
        );
        fetch_fork(&build, &fork)?
    } else {
        println!("fetching verus @ {commit} into {}", build.display());
        source_build::fetch_source(&build, commit).map_err(|e| e.to_string())?
    };
    if o.engine {
        std::fs::write(
            checkout.join("source/Cargo.lock"),
            sail_verus::engine::SOURCE_LOCK,
        )
        .map_err(|e| e.to_string())?;
    }
    let channel = toolchain_channel(&checkout.join("rust-toolchain.toml"))?;
    if channel != *toolchain {
        return Err(format!(
            "verus@{commit} pins Rust {channel}, but corpus/toolchain.toml says {toolchain}"
        ));
    }
    let jobs = std::env::var("SAIL_BUILD_JOBS")
        .ok()
        .filter(|j| j.parse::<u32>().is_ok_and(|n| n > 0))
        .unwrap_or_else(|| "4".to_owned());
    let executables = compile(
        &checkout,
        &installed.z3,
        &asset,
        &jobs,
        features.iter().any(|feature| feature == "singular"),
        o.engine,
    )?;
    let test_binaries = relative_binaries(&build, &executables)?;

    if !o.keep_intermediates {
        let before = dir_size(&build);
        let keep: BTreeSet<String> = executables
            .iter()
            .filter_map(|(_, e)| e.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        prune(&checkout, &keep)?;
        println!(
            "pruned build intermediates: {} MiB -> {} MiB",
            before >> 20,
            dir_size(&build) >> 20
        );
    }
    let manifest = Manifest {
        schema_version: source_build::SCHEMA_VERSION,
        commit: commit.clone(),
        rust_toolchain: toolchain.clone(),
        z3_sha256,
        features,
        build_seconds: started.elapsed().as_secs(),
        test_binaries,
        artifacts: source_build::artifact_hashes(&build).map_err(|e| e.to_string())?,
    };
    manifest.save(&build).map_err(|e| e.to_string())?;
    println!(
        "Verus {commit} built from source in {}s: {} test binaries; {} ({} MiB)",
        manifest.build_seconds,
        manifest.test_binaries.len(),
        build.display(),
        dir_size(&build) >> 20
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::test_executables;
    use std::path::PathBuf;

    #[test]
    fn finds_test_executables_in_cargo_messages() {
        let stdout = r#"{"reason":"compiler-artifact","target":{"name":"air","kind":["lib"]},"profile":{"test":false},"executable":null}
{"reason":"compiler-artifact","target":{"name":"modes","kind":["test"]},"profile":{"test":true},"executable":"/b/deps/modes-1"}
{"reason":"compiler-artifact","target":{"name":"basic","kind":["test"]},"profile":{"test":true},"executable":"/b/deps/basic-2"}
not json
{"reason":"build-finished","success":true}"#;
        assert_eq!(
            test_executables(stdout),
            vec![
                ("basic".to_owned(), PathBuf::from("/b/deps/basic-2")),
                ("modes".to_owned(), PathBuf::from("/b/deps/modes-1")),
            ]
        );
    }
}
