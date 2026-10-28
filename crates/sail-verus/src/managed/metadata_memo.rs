//! Serves Cargo-Verus's own `cargo metadata` call from the runner's read of
//! the same metadata, made moments earlier in the same managed run.
//!
//! Plain Cargo-Verus reads `cargo metadata` once more after the runner did.
//! When that call would run exactly the same Cargo, with exactly the same
//! arguments, directory and environment, on inputs that kept their stamps
//! since before the runner's read, it prints exactly the same bytes: the
//! Cargo alias then writes the runner's recorded stdout instead of starting
//! Cargo. Every other call, and every doubt, runs the real Cargo.
//!
//! The runner records a memo only for a read that succeeded with an empty
//! stderr, when every metadata input (the [`MetadataGuard`] of the previous
//! run's selection: manifests, lockfile, target-discovery directories, Cargo
//! configuration and toolchain files) and the toolchain's identity (the
//! Cargo program, rustup's settings and toolchains) kept the stamps they had
//! before that read. The alias checks, before serving:
//!
//! - the program, the complete argument list (including the managed metadata
//!   target) and the resolved working directory are identical;
//! - the effective environment is identical, apart from Sail's and Verus's
//!   own variables, which Cargo does not read, and `RUSTC_BOOTSTRAP`, which
//!   the runner sets for Cargo-Verus (below);
//! - every stamp still holds, and the recorded stdout has its digest.
//!
//! `RUSTC_BOOTSTRAP=1` lets a stable Cargo accept unstable features. It can
//! change the metadata only through an unstable input: a `-Z` argument, a
//! `CARGO_UNSTABLE_*` or channel-override variable, an `[unstable]` table or
//! `include` in a Cargo configuration file, or `cargo-features` in a
//! manifest. A memo whose read ran without the alias's `RUSTC_BOOTSTRAP`
//! value serves only when none of these is present in its stamped inputs.
//!
//! `SAIL_MANAGED_METADATA_MEMO=0` turns the memo off.
//! `SAIL_MANAGED_METADATA_MEMO_TRACE=FILE` logs each decision to `FILE` and,
//! for diagnosis, makes every eligible call also run the real Cargo, print
//! its output and log whether the memo matched it byte for byte.

use super::input::{MetadataGuard, MetadataRead, stamp_paths};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The private transport setting naming this run's memo file.
pub(crate) const MEMO_ENV: &str = "SAIL_MANAGED_METADATA_MEMO_FILE";
/// The memo file in a run directory.
pub(super) const MEMO_FILE: &str = "metadata-memo.json";
const STDOUT_FILE: &str = "metadata-memo.stdout";
const VERSION: u32 = 1;
const BOOTSTRAP: &str = "RUSTC_BOOTSTRAP";

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct Memo {
    version: u32,
    program: PathBuf,
    args: Vec<String>,
    directory: PathBuf,
    /// [`environment_digest`] of the read's effective environment.
    environment: String,
    /// The read's effective `RUSTC_BOOTSTRAP`.
    bootstrap: Option<Vec<u8>>,
    /// No unstable input is present, so `RUSTC_BOOTSTRAP` cannot matter.
    bootstrap_neutral: bool,
    /// The metadata inputs and their stamp from before the read.
    inputs: BTreeSet<PathBuf>,
    inputs_stamp: String,
    /// The toolchain identity paths and their stamp from before the read.
    identity: BTreeSet<PathBuf>,
    identity_stamp: String,
    stdout: PathBuf,
    stdout_sha256: String,
}

/// Whether the memo is turned off.
fn disabled() -> bool {
    std::env::var_os("SAIL_MANAGED_METADATA_MEMO").is_some_and(|value| value == "0")
}

/// The stamp of the toolchain's identity, taken before the runner's read.
pub(super) struct Identity {
    paths: BTreeSet<PathBuf>,
    stamp: String,
}

impl Identity {
    /// The identity of `cargo` (as the runner and the alias both start it).
    pub(super) fn begin(cargo: &Path) -> Option<Self> {
        if disabled() {
            return None;
        }
        let paths = identity_paths(cargo)?;
        let stamp = stamp_paths(&paths)?;
        Some(Self { paths, stamp })
    }
}

/// The Cargo program as given and resolved (a rustup proxy, a link, or the
/// toolchain's own Cargo), and rustup's settings (its default toolchain and
/// directory overrides) and installed toolchains. Cargo configuration and
/// `rust-toolchain(.toml)` files are already among the metadata inputs.
fn identity_paths(cargo: &Path) -> Option<BTreeSet<PathBuf>> {
    let program = std::path::absolute(cargo).ok()?;
    let mut paths = BTreeSet::from([program.clone()]);
    if let Ok(resolved) = program.canonicalize() {
        paths.insert(resolved);
    }
    let rustup = std::env::var_os("RUSTUP_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")));
    if let Some(rustup) = rustup.and_then(|home| std::path::absolute(home).ok()) {
        paths.insert(rustup.join("settings.toml"));
        paths.insert(rustup.join("toolchains"));
    }
    Some(paths)
}

/// Whether an environment variable can affect Cargo: everything except
/// Sail's and Verus's own variables and `RUSTC_BOOTSTRAP` (compared apart).
fn compared(name: &OsStr) -> bool {
    let name = name.as_encoded_bytes();
    !(name.starts_with(b"SAIL_") || name.starts_with(b"VERUS_") || name == BOOTSTRAP.as_bytes())
}

/// This process's environment with a command's changes applied.
fn effective<'a>(
    changes: impl IntoIterator<Item = (&'a OsStr, Option<&'a OsStr>)>,
) -> BTreeMap<OsString, OsString> {
    let mut environment: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    for (name, value) in changes {
        match value {
            Some(value) => environment.insert(name.to_owned(), value.to_owned()),
            None => environment.remove(name),
        };
    }
    environment
}

/// A digest of every compared variable's name and value. The memo records
/// only this digest, never the values, which may be secrets.
fn environment_digest(environment: &BTreeMap<OsString, OsString>) -> String {
    let mut digest = Sha256::new();
    for (name, value) in environment.iter().filter(|(name, _)| compared(name)) {
        for part in [name.as_encoded_bytes(), value.as_encoded_bytes()] {
            digest.update((part.len() as u64).to_le_bytes());
            digest.update(part);
        }
    }
    format!("{:x}", digest.finalize())
}

fn bootstrap(environment: &BTreeMap<OsString, OsString>) -> Option<Vec<u8>> {
    environment
        .get(OsStr::new(BOOTSTRAP))
        .map(|value| value.as_encoded_bytes().to_vec())
}

/// Whether no unstable input is present (see the module documentation):
/// arguments, environment and the stamped configuration and manifest files.
/// A file that cannot be read counts as unstable.
fn bootstrap_neutral(
    args: &[String],
    environment: &BTreeMap<OsString, OsString>,
    inputs: &BTreeSet<PathBuf>,
) -> bool {
    let unstable_variable = environment.keys().any(|name| {
        let name = name.as_encoded_bytes();
        name.starts_with(b"CARGO_UNSTABLE_")
            || name == b"__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS"
    });
    if unstable_variable || args.iter().any(|arg| arg.starts_with("-Z")) {
        return false;
    }
    inputs.iter().all(|path| {
        let name = path.file_name().and_then(OsStr::to_str);
        let configuration = matches!(name, Some("config" | "config.toml"))
            && path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|parent| parent == ".cargo");
        let manifest = name == Some("Cargo.toml");
        if !(configuration || manifest) {
            return true;
        }
        let text = match std::fs::read(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => return false,
        };
        let has = |needle: &[u8]| text.windows(needle.len()).any(|window| window == needle);
        if configuration {
            !has(b"unstable") && !has(b"include")
        } else {
            !has(b"cargo-features")
        }
    })
}

fn strings(values: &[OsString]) -> Option<Vec<String>> {
    values
        .iter()
        .map(|value| value.to_str().map(str::to_owned))
        .collect()
}

/// Records the memo for this run's metadata `read` in `run`, when it may
/// serve: the guard and identity still hold their stamps from before the
/// read, and `data` (the read, as the runner completed it) selects exactly
/// the guarded inputs. Returns whether a memo was written. Failing to write
/// one only leaves the real Cargo to run.
pub(super) fn record(
    run: &Path,
    read: Option<MetadataRead>,
    guard: Option<&MetadataGuard>,
    identity: Option<Identity>,
    data: &serde_json::Value,
) -> bool {
    let (Some(read), Some(guard), Some(identity)) = (read, guard, identity) else {
        return false;
    };
    if disabled()
        || !guard.unchanged_for(data)
        || stamp_paths(&identity.paths).as_deref() != Some(identity.stamp.as_str())
    {
        return false;
    }
    let (inputs, inputs_stamp) = guard.parts();
    let Some(memo) = memo(run, &read, inputs, inputs_stamp, identity) else {
        return false;
    };
    let path = run.join(MEMO_FILE);
    std::fs::write(run.join(STDOUT_FILE), &read.stdout).is_ok()
        && serde_json::to_vec(&memo).is_ok_and(|bytes| std::fs::write(&path, bytes).is_ok())
}

fn memo(
    run: &Path,
    read: &MetadataRead,
    inputs: &BTreeSet<PathBuf>,
    inputs_stamp: &str,
    identity: Identity,
) -> Option<Memo> {
    let args = strings(&read.args)?;
    let environment = effective(
        read.environment
            .iter()
            .map(|(name, value)| (name.as_os_str(), value.as_deref())),
    );
    let directory = match &read.directory {
        Some(directory) => directory.clone(),
        None => std::env::current_dir().ok()?,
    };
    Some(Memo {
        version: VERSION,
        program: PathBuf::from(&read.program),
        bootstrap_neutral: bootstrap_neutral(&args, &environment, inputs),
        args,
        directory: directory.canonicalize().ok()?,
        environment: environment_digest(&environment),
        bootstrap: bootstrap(&environment),
        inputs: inputs.clone(),
        inputs_stamp: inputs_stamp.to_owned(),
        identity: identity.paths,
        identity_stamp: identity.stamp,
        stdout: run.join(STDOUT_FILE),
        stdout_sha256: format!("{:x}", Sha256::digest(&read.stdout)),
    })
}

/// Why the alias runs the real Cargo, or that it may serve.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Serve(Vec<u8>),
    Refuse(&'static str),
}

fn decide(memo: &Memo, command: &Command) -> Decision {
    use Decision::Refuse;
    if memo.version != VERSION {
        return Refuse("version");
    }
    if command.get_program() != memo.program.as_os_str() {
        return Refuse("program");
    }
    let same_args = command.get_args().len() == memo.args.len()
        && command
            .get_args()
            .zip(&memo.args)
            .all(|(arg, recorded)| arg.to_str() == Some(recorded.as_str()));
    if !same_args {
        return Refuse("arguments");
    }
    let directory = match command.get_current_dir() {
        Some(directory) => directory.canonicalize().ok(),
        None => std::env::current_dir()
            .ok()
            .and_then(|directory| directory.canonicalize().ok()),
    };
    if directory.as_ref() != Some(&memo.directory) {
        return Refuse("directory");
    }
    let environment = effective(command.get_envs());
    if environment_digest(&environment) != memo.environment {
        return Refuse("environment");
    }
    if bootstrap(&environment) != memo.bootstrap && !memo.bootstrap_neutral {
        return Refuse("bootstrap");
    }
    if stamp_paths(&memo.inputs).as_deref() != Some(memo.inputs_stamp.as_str()) {
        return Refuse("inputs");
    }
    if stamp_paths(&memo.identity).as_deref() != Some(memo.identity_stamp.as_str()) {
        return Refuse("toolchain");
    }
    match std::fs::read(&memo.stdout) {
        Ok(stdout) if format!("{:x}", Sha256::digest(&stdout)) == memo.stdout_sha256 => {
            Decision::Serve(stdout)
        }
        _ => Refuse("stdout"),
    }
}

fn trace(line: &str) {
    if let Some(path) = std::env::var_os("SAIL_MANAGED_METADATA_MEMO_TRACE")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// In the Cargo alias, for a `cargo metadata` call about to run as
/// `command`: answers it from the memo and returns its exit code, or returns
/// `None` for the real Cargo to run.
///
/// # Errors
/// `SAIL3111` when the answer, or the traced real Cargo's output, cannot be
/// written.
pub(crate) fn serve(command: &mut Command) -> Result<Option<ExitCode>> {
    let Some(path) = super::transport::var(MEMO_ENV).filter(|_| !disabled()) else {
        return Ok(None);
    };
    let decision = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Memo>(&bytes).ok())
        .map_or(Decision::Refuse("memo"), |memo| decide(&memo, command));
    let stdout = match decision {
        Decision::Refuse(reason) => {
            trace(&format!("refused {reason}"));
            return Ok(None);
        }
        Decision::Serve(stdout) => stdout,
    };
    let failed = |error: std::io::Error| Error::new("SAIL3111", error.to_string());
    if std::env::var_os("SAIL_MANAGED_METADATA_MEMO_TRACE").is_some() {
        // Diagnosis: Cargo answers, and the trace says whether the memo would
        // have answered the same.
        let output = command.output().map_err(failed)?;
        let same = output.status.success() && output.stderr.is_empty() && output.stdout == stdout;
        trace(&format!(
            "{} {} bytes",
            if same { "match" } else { "mismatch" },
            stdout.len()
        ));
        std::io::stdout()
            .write_all(&output.stdout)
            .and_then(|()| std::io::stdout().flush())
            .and_then(|()| std::io::stderr().write_all(&output.stderr))
            .map_err(failed)?;
        let code = output
            .status
            .code()
            .and_then(|code| u8::try_from(code & 255).ok())
            .unwrap_or(1);
        return Ok(Some(ExitCode::from(code)));
    }
    trace(&format!("served {} bytes", stdout.len()));
    let mut out = std::io::stdout().lock();
    out.write_all(&stdout)
        .and_then(|()| out.flush())
        .map_err(failed)?;
    Ok(Some(ExitCode::SUCCESS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-metadata-memo-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(&root)?;
        Ok(root.canonicalize()?)
    }

    /// A memo for `program args` run in `directory` with `changes`.
    fn fixture(
        root: &Path,
        program: &Path,
        args: &[&str],
        changes: &[(&str, Option<&str>)],
        stdout: &[u8],
    ) -> std::result::Result<Memo, Box<dyn std::error::Error>> {
        let manifest = root.join("Cargo.toml");
        std::fs::write(&manifest, "[package]\nname = \"p\"\n")?;
        let inputs = BTreeSet::from([manifest, root.join("Cargo.lock")]);
        let read = MetadataRead {
            program: program.as_os_str().to_owned(),
            args: args.iter().map(OsString::from).collect(),
            directory: Some(root.to_owned()),
            environment: changes
                .iter()
                .map(|(name, value)| (OsString::from(name), value.map(OsString::from)))
                .collect(),
            stdout: stdout.to_vec(),
        };
        std::fs::write(root.join(STDOUT_FILE), stdout)?;
        let stamp = stamp_paths(&inputs).ok_or("stamp")?;
        let identity = Identity {
            paths: identity_paths(program).ok_or("identity")?,
            stamp: String::new(),
        };
        let identity = Identity {
            stamp: stamp_paths(&identity.paths).ok_or("identity stamp")?,
            ..identity
        };
        memo(root, &read, &inputs, &stamp, identity).ok_or_else(|| "memo".into())
    }

    fn command(
        root: &Path,
        program: &Path,
        args: &[&str],
        changes: &[(&str, Option<&str>)],
    ) -> Command {
        let mut command = Command::new(program);
        command.current_dir(root).args(args);
        for (name, value) in changes {
            match value {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        command
    }

    #[cfg(unix)]
    #[test]
    fn identical_call_is_served_and_every_difference_runs_cargo()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = root("decide")?;
        let program = root.join("cargo");
        std::fs::write(&program, "")?;
        let args = ["metadata", "--format-version", "1"];
        let target = [("CARGO_TARGET_DIR", Some("/t")), ("RUSTUP_TOOLCHAIN", None)];
        let memo = fixture(&root, &program, &args, &target, b"{}\n")?;
        assert!(memo.bootstrap_neutral);
        let same = command(&root, &program, &args, &target);
        assert_eq!(decide(&memo, &same), Decision::Serve(b"{}\n".to_vec()));
        // Sail's and Verus's own variables, and the bootstrap Sail sets for
        // Cargo-Verus without unstable inputs, are not Cargo inputs.
        let mut sail = command(&root, &program, &args, &target);
        sail.env("SAIL_SOLVER_MODE", "x")
            .env("VERUS_Z3_PATH", "/z")
            .env(BOOTSTRAP, "1");
        assert_eq!(decide(&memo, &sail), Decision::Serve(b"{}\n".to_vec()));

        let refused = |command: &Command| decide(&memo, command);
        let other = root.join("other");
        std::fs::create_dir_all(&other)?;
        assert_eq!(
            refused(&command(&other, &program, &args, &target)),
            Decision::Refuse("directory")
        );
        assert_eq!(
            refused(&command(&root, &root.join("cargo2"), &args, &target)),
            Decision::Refuse("program")
        );
        assert_eq!(
            refused(&command(
                &root,
                &program,
                &["metadata", "--format-version", "1", "--offline"],
                &target
            )),
            Decision::Refuse("arguments")
        );
        for change in [
            ("CARGO_TARGET_DIR", Some("/u")),
            ("RUSTUP_TOOLCHAIN", Some("nightly")),
            ("CARGO_HOME", Some("/h")),
            ("RUSTC", Some("/r")),
            ("PATH", Some("/p")),
        ] {
            let mut changed = command(&root, &program, &args, &target);
            match change.1 {
                Some(value) => changed.env(change.0, value),
                None => changed.env_remove(change.0),
            };
            assert_eq!(
                refused(&changed),
                Decision::Refuse("environment"),
                "{change:?}"
            );
        }
        // A stamped input or the Cargo program changed after the read.
        std::fs::write(root.join("Cargo.lock"), "")?;
        assert_eq!(refused(&same), Decision::Refuse("inputs"));
        std::fs::remove_file(root.join("Cargo.lock"))?;
        assert_eq!(decide(&memo, &same), Decision::Serve(b"{}\n".to_vec()));
        std::fs::write(&program, "replaced")?;
        assert_eq!(refused(&same), Decision::Refuse("toolchain"));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn altered_stdout_is_never_served() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = root("stdout")?;
        let program = root.join("cargo");
        std::fs::write(&program, "")?;
        let args = ["metadata"];
        let memo = fixture(&root, &program, &args, &[], b"{\"a\":1}\n")?;
        std::fs::write(root.join(STDOUT_FILE), b"{\"a\":2}\n")?;
        assert_eq!(
            decide(&memo, &command(&root, &program, &args, &[])),
            Decision::Refuse("stdout")
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unstable_inputs_make_the_bootstrap_difference_refuse()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = root("bootstrap")?;
        let program = root.join("cargo");
        std::fs::write(&program, "")?;
        let args = ["metadata"];
        let configuration = root.join(".cargo/config.toml");
        std::fs::create_dir_all(root.join(".cargo"))?;
        let manifest = root.join("Cargo.toml");
        let inputs = BTreeSet::from([manifest.clone(), configuration.clone()]);
        let none = BTreeMap::new();
        std::fs::write(&manifest, "[package]\n")?;
        assert!(bootstrap_neutral(&[], &none, &inputs));
        // An `[unstable]` table or an `include` in a configuration file, and
        // `cargo-features` in a manifest, are unstable inputs.
        for text in ["[unstable]\nbindeps = true\n", "include = \"other.toml\"\n"] {
            std::fs::write(&configuration, text)?;
            assert!(!bootstrap_neutral(&[], &none, &inputs), "{text}");
        }
        std::fs::remove_file(&configuration)?;
        std::fs::write(&manifest, "cargo-features = [\"x\"]\n")?;
        assert!(!bootstrap_neutral(&[], &none, &inputs));
        std::fs::write(&manifest, "[package]\n")?;
        assert!(!bootstrap_neutral(&["-Zx".to_owned()], &none, &inputs));
        for name in [
            "CARGO_UNSTABLE_BINDEPS",
            "__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS",
        ] {
            let unstable = BTreeMap::from([(OsString::from(name), OsString::from("1"))]);
            assert!(!bootstrap_neutral(&[], &unstable, &inputs), "{name}");
        }
        // A memo read without the alias's bootstrap, and not neutral, refuses
        // the call Cargo-Verus makes with `RUSTC_BOOTSTRAP=1`.
        let mut memo = fixture(&root, &program, &args, &[(BOOTSTRAP, None)], b"{}")?;
        memo.bootstrap_neutral = false;
        let mut bootstrapped = command(&root, &program, &args, &[]);
        bootstrapped.env(BOOTSTRAP, "1");
        assert_eq!(decide(&memo, &bootstrapped), Decision::Refuse("bootstrap"));
        memo.bootstrap_neutral = true;
        assert_eq!(
            decide(&memo, &bootstrapped),
            Decision::Serve(b"{}".to_vec())
        );
        let mut same = command(&root, &program, &args, &[]);
        same.env_remove(BOOTSTRAP);
        memo.bootstrap_neutral = false;
        assert_eq!(decide(&memo, &same), Decision::Serve(b"{}".to_vec()));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn the_memo_never_records_environment_values() {
        let environment = BTreeMap::from([(
            OsString::from("CARGO_REGISTRY_TOKEN"),
            OsString::from("secret-value"),
        )]);
        let digest = environment_digest(&environment);
        assert!(!digest.contains("secret"));
        assert_eq!(digest.len(), 64);
        assert!(!compared(OsStr::new("SAIL_X")));
        assert!(!compared(OsStr::new("VERUS_Z3_PATH")));
        assert!(!compared(OsStr::new(BOOTSTRAP)));
        assert!(compared(OsStr::new("CARGO_HOME")));
        assert!(compared(OsStr::new("RUSTUP_TOOLCHAIN")));
    }
}
