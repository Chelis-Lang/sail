//! Private Cargo and Verus-driver roles for the same installed Sail executable.
//!
//! Metadata, rustc probes, build scripts, and non-verification packages are
//! forwarded with their original argument bytes. Verified targets alone get
//! isolated observation sidecars; these flags leave solver input unchanged.

use crate::managed::command_environment::{Command as CompilerCommand, Spawned};
use crate::managed::transport;
use crate::managed::{Invocation, Runtime, TargetIdentity, digest, event, file_digest, json};
use crate::{Error, Result};
use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

#[path = "adapter_attempts.rs"]
pub(crate) mod attempts;
#[path = "adapter_metadata_stdout.rs"]
mod metadata_stdout;
#[path = "adapter_verus_channel.rs"]
pub(crate) mod verus_channel;

/// Only Cargo-Verus's own metadata probe receives this private setting
/// (through [`transport`], never the environment).
pub(crate) const CARGO_METADATA_ENV: &str = "SAIL_MANAGED_CARGO_METADATA";

/// The `reason` of the record that opens one Cargo build's section of the
/// managed artifact journal.
pub(crate) const JOURNAL_BEGIN: &str = "sail-journal-begin";
/// The `reason` of the record that closes a section: Cargo's output reached
/// EOF, every unit record was written, and Cargo exited normally (with any
/// status). A section without it is incomplete evidence.
pub(crate) const JOURNAL_END: &str = "sail-journal-end";

fn journal_marker(reason: &str) -> Vec<u8> {
    let mut line = serde_json::json!({ "reason": reason })
        .to_string()
        .into_bytes();
    line.push(b'\n');
    line
}

/// An owned metadata-only namespace; it contains no compilation artifacts.
pub(crate) fn metadata_target(cache: &Path) -> Result<PathBuf> {
    std::path::absolute(cache)
        .map(|cache| cache.join("cargo-metadata"))
        .map_err(|error| Error::new("SAIL3112", error.to_string()))
}

/// Prevent environment or Cargo configuration from excluding source trees in
/// managed metadata. The CLI configuration has the highest Cargo precedence.
pub(crate) fn configure_metadata_target(command: &mut Command, target: &Path) -> Result<()> {
    let target_text = target
        .to_str()
        .ok_or_else(|| Error::new("SAIL3112", "managed metadata target is not UTF-8"))?;
    let quoted = serde_json::to_string(target_text)
        .map_err(|error| Error::new("SAIL3112", error.to_string()))?;
    command
        .env("CARGO_TARGET_DIR", target)
        .args(["--config", &format!("build.target-dir={quoted}")]);
    Ok(())
}

fn required(name: &str) -> Result<OsString> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::new("SAIL3111", format!("private adapter missing {name}")))
}

pub(crate) struct InvocationLog<'a> {
    pub(crate) directory: &'a Path,
    pub(crate) run_id: &'a str,
    pub(crate) target: &'a str,
}

impl InvocationLog<'_> {
    fn record(&self, child: &mut Spawned) -> Result<()> {
        let recorded = json(
            &self.directory.join(format!("{}.started", child.id())),
            &serde_json::json!({"target":self.target,"run_id":self.run_id}),
        );
        if recorded.is_err() {
            // A child without a durable invocation record cannot contribute
            // evidence or remain running after adapter setup fails.
            let _ = child.kill();
            let _ = child.wait();
        }
        recorded
    }
}

/// Own the compiler and its pipe reader across every streamed-output failure.
struct CompilerOutput<C: OutputChild = std::process::Child, Stream = Vec<u8>> {
    child: C,
    stdout: Option<std::thread::JoinHandle<std::io::Result<Stream>>>,
}

trait OutputChild {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>>;
    fn kill(&mut self) -> std::io::Result<()>;
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus>;
}

impl OutputChild for std::process::Child {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.try_wait()
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.kill()
    }
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.wait()
    }
}

impl OutputChild for Spawned {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.try_wait()
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.kill()
    }
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.wait()
    }
}

impl<C: OutputChild, Stream> CompilerOutput<C, Stream> {
    fn stdout(&mut self) -> Result<Stream> {
        self.stdout
            .take()
            .ok_or_else(|| Error::new("SAIL3111", "missing solver output pipe"))?
            .join()
            .map_err(|_| Error::new("SAIL3111", "solver output reader failed"))?
            .map_err(|error| {
                Error::new("SAIL3111", format!("cannot capture solver output: {error}"))
            })
    }

    fn stop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(reader) = self.stdout.take() {
            let _ = reader.join();
        }
    }
}

impl<C: OutputChild, Stream> Drop for CompilerOutput<C, Stream> {
    fn drop(&mut self) {
        self.stop();
    }
}

fn status(status: std::process::ExitStatus) -> ExitCode {
    ExitCode::from(
        status
            .code()
            .and_then(|n| u8::try_from(n & 255).ok())
            .unwrap_or(1),
    )
}

/// Dispatches installed private executable aliases before public CLI parsing.
/// Returns `None` for an ordinary Sail invocation.
#[must_use]
pub fn dispatch() -> Option<ExitCode> {
    let name = std::env::args_os()
        .next()
        .and_then(|p| PathBuf::from(p).file_name().map(std::ffi::OsStr::to_owned));
    let role = name.as_deref()?.to_str()?;
    let outcome = match role {
        "sail-cargo" => cargo(),
        "sail-verus-driver" => driver(),
        _ => return None,
    };
    Some(outcome.unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::from(2)
    }))
}

fn cargo() -> Result<ExitCode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut cmd = Command::new(transport::required("SAIL_MANAGED_CARGO")?);
    cmd.args(&args);
    // The real Cargo, and so every build script and compiler, gets the
    // caller's own `CARGO`, not the runner's alias (#385).
    transport::restore_cargo(&mut cmd);
    if args.first().is_some_and(|argument| argument == "metadata")
        && let Some(target) = transport::var(CARGO_METADATA_ENV)
    {
        // Cargo metadata probes rustc and can write .rustc_info.json in the
        // caller's target, even though it does not compile. Managed builds use
        // their explicit owned target, so that override cannot select artifacts.
        configure_metadata_target(&mut cmd, Path::new(&target))?;
    }
    transport::clear(&mut cmd, CARGO_METADATA_ENV);
    let build = args.iter().any(|s| s == "build" || s == "check");
    if !build {
        return cmd
            .status()
            .map(status)
            .map_err(|e| Error::new("SAIL3111", e.to_string()));
    }
    transport::forward(
        &mut cmd,
        "SAIL_MANAGED_WRAPPER",
        &required("RUSTC_WRAPPER")?,
    )?;
    cmd.env("RUSTC_WRAPPER", transport::required("SAIL_MANAGED_PROXY")?);
    if let Some(directory) = transport::var(crate::managed::build_environment::REQUEST) {
        let directory = PathBuf::from(directory);
        let version = Command::new(cmd.get_program())
            .args(["--version", "--verbose"])
            .output()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if !version.status.success() {
            return Ok(status(version.status));
        }
        std::fs::write(directory.join("cargo-version.txt"), version.stdout)
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        cmd.args(["--unit-graph", "-Z", "unstable-options"])
            .env("CARGO_CACHE_RUSTC_INFO", "0")
            .stdin(Stdio::null());
        let output = cmd
            .output()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if output.status.success() {
            std::fs::write(directory.join("graph.json"), output.stdout)
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        }
        return Ok(status(output.status));
    }
    // Cargo messages remain on stdout; target Verus documents live in sidecars.
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let path = PathBuf::from(transport::required("SAIL_MANAGED_ARTIFACTS")?);
    let mut records = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    cargo_messages(&mut cmd, &mut records, &mut std::io::stdout())
}

/// Record the actual child lifecycle without changing forwarded Cargo bytes.
fn cargo_messages(
    cmd: &mut Command,
    records: &mut impl Write,
    forwarded: &mut impl Write,
) -> Result<ExitCode> {
    use crate::managed::source_inputs::write_provider;
    use sail_acceptance::inputs::ProviderEvent;
    records
        .write_all(&journal_marker(JOURNAL_BEGIN))
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let producer = std::process::id();
    // Declare the required child before spawn. Any subsequent early return
    // leaves an unsealed intent, including spawn and pipe-read failures.
    write_provider(
        records,
        &ProviderEvent {
            kind: 0,
            producer,
            child: 0,
            frames: 0,
            exited: false,
        },
    )
    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let mut owned: CompilerOutput = CompilerOutput {
        child: cmd
            .spawn()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?,
        stdout: None,
    };
    let child = &mut owned.child;
    let pid = child.id();
    write_provider(
        records,
        &ProviderEvent {
            kind: 1,
            producer,
            child: pid,
            frames: 0,
            exited: false,
        },
    )
    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    #[cfg(unix)]
    let markers = std::env::var_os(crate::managed::visible_output::ENV).map_or(Ok(None), |path| {
        crate::managed::visible_output::MarkerObserver::open(Path::new(&path)).map(Some)
    });
    #[cfg(unix)]
    let mut observed = markers.as_ref().ok().map(|_| Vec::new());
    #[cfg(not(unix))]
    let mut observed = Some(Vec::new());
    if let Some(stdout) = child.stdout.take() {
        let mut stdout = std::io::BufReader::new(stdout);
        loop {
            let mut frame = Vec::new();
            if stdout
                .read_until(b'\n', &mut frame)
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?
                == 0
            {
                break;
            }
            let line = frame.strip_suffix(b"\n").unwrap_or(&frame);
            #[cfg(unix)]
            capture_cargo_observation(
                &mut observed,
                &frame,
                64 * 1024 * 1024,
                markers.as_ref().ok().and_then(Option::as_ref),
            );
            #[cfg(not(unix))]
            capture_cargo_line(&mut observed, line, 64 * 1024 * 1024);
            forwarded
                .write_all(line)
                .and_then(|()| forwarded.write_all(b"\n"))
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        }
    } else {
        return Err(Error::new("SAIL3111", "missing Cargo output pipe"));
    }
    let actual = child
        .wait()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    // A normal wait is independent of whether the mixed output could be
    // classified. Poisoning withholds the terminal, never changes that status.
    if let Some(observed) = observed
        && let Some(frames) = cargo_artifacts(&observed, records, producer, pid)?
    {
        write_provider(
            records,
            &ProviderEvent {
                kind: 3,
                producer,
                child: pid,
                frames,
                exited: actual.code().is_some(),
            },
        )
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    }
    if actual.code().is_some() {
        records
            .write_all(&journal_marker(JOURNAL_END))
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    }
    Ok(status(actual))
}

/// Bound only optional accounting retention. A budget refusal is sticky and
/// cannot suppress output forwarding or change the actual child status.
fn capture_cargo_line(observed: &mut Option<Vec<u8>>, line: &[u8], limit: usize) {
    if let Some(buffer) = observed {
        if line.len() < limit.saturating_sub(buffer.len()) {
            buffer.extend_from_slice(line);
            buffer.push(b'\n');
        } else {
            *observed = None;
        }
    }
}

/// Strip only a validated current output-control marker from the accounting
/// copy, before forwarding can acknowledge and retire its peer socket.
#[cfg(unix)]
fn capture_cargo_observation(
    observed: &mut Option<Vec<u8>>,
    frame: &[u8],
    limit: usize,
    markers: Option<&crate::managed::visible_output::MarkerObserver>,
) {
    if observed.is_none() {
        return;
    }
    if let Some(markers) = markers {
        match markers.decode(frame) {
            Ok(Some(bytes)) => {
                if !bytes.is_empty() {
                    capture_cargo_line(
                        observed,
                        bytes.strip_suffix(b"\n").unwrap_or(&bytes),
                        limit,
                    );
                }
                return;
            }
            Err(_) => {
                *observed = None;
                return;
            }
            Ok(None) => {}
        }
    }
    capture_cargo_line(observed, frame.strip_suffix(b"\n").unwrap_or(frame), limit);
}

/// Parse complete JSON documents so genuine multiline Verus output is not
/// mistaken for malformed line-delimited Cargo metadata. This is accounting,
/// not a proof of the surrounding mixed-output parser or physical transport.
fn cargo_artifacts(
    stdout: &[u8],
    records: &mut impl Write,
    producer: u32,
    child: u32,
) -> Result<Option<usize>> {
    use crate::managed::source_inputs::write_artifact;
    let (documents, remainder) = crate::attribution::json_documents(stdout);
    // The remainder normally contains Verus summaries, timing tables and text.
    // Any JSON-looking remainder failed complete-document framing: it cannot
    // silently disappear into an apparently completed empty provider.
    let mut classified = !remainder.split(|byte| *byte == b'\n').any(|line| {
        line.iter()
            .find(|byte| !byte.is_ascii_whitespace())
            .is_some_and(|byte| matches!(byte, b'{' | b'[' | b'"' | b'}' | b']'))
    });
    let mut frames = 0usize;
    for value in documents {
        match value["reason"].as_str() {
            Some("compiler-artifact" | "build-script-executed") => {
                write_artifact(records, value, producer, child)
                    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
                frames = frames
                    .checked_add(1)
                    .ok_or_else(|| Error::new("SAIL3111", "artifact count overflow"))?;
            }
            Some("compiler-message") => {}
            Some("build-finished") if value["success"].is_boolean() => {}
            None if value.get("reason").is_none() && value["verification-results"].is_object() => {}
            // Future/unsupported Cargo reasons or other JSON have no implicit
            // exemption. Forward their bytes but decline provider completion.
            _ => classified = false,
        }
    }
    Ok(classified.then_some(frames))
}

fn argument(args: &[OsString], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].to_string_lossy().into_owned())
}

fn verifying(args: &[OsString]) -> bool {
    if argument(args, "--crate-name").as_deref() == Some("___")
        || std::env::var("CARGO_CRATE_NAME").is_ok_and(|v| v.starts_with("build_script_"))
    {
        return false;
    }
    cargo_package_id().is_some_and(|id| {
        std::env::var(format!("__VERUS_DRIVER_VERIFY_{id}")).as_deref() == Ok("1")
    })
}

fn cargo_package_id() -> Option<String> {
    let package = ["CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR"].map(std::env::var);
    let [Ok(name), Ok(version), Ok(manifest)] = package else {
        return None;
    };
    let hash = digest(
        Path::new(&manifest)
            .join("Cargo.toml")
            .to_string_lossy()
            .as_bytes(),
    );
    Some(format!(
        "{name}-{version}-{}",
        hash.get(..12).unwrap_or_default()
    ))
}

fn cargo_driver_options(id: &str) -> Result<DriverOptions> {
    cargo_driver_options_with(id, |key| match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(Error::new(
            "SAIL3123",
            format!("invalid effective Cargo driver arguments: {error}"),
        )),
    })
}

/// Exact effective per-package Verus tokens, using the shipped decoder.
/// Experimental reuse declines an invalid decoder result instead of guessing.
pub(crate) fn effective_verus_arguments() -> Option<Vec<String>> {
    cargo_driver_options(&cargo_package_id()?)
        .ok()
        .map(|options| options.arguments)
}

struct DriverOptions {
    arguments: Vec<String>,
    environment: std::collections::BTreeMap<String, String>,
}

fn cargo_driver_options_with(
    id: &str,
    mut lookup: impl FnMut(&str) -> Result<Option<String>>,
) -> Result<DriverOptions> {
    let mut arguments = Vec::new();
    let mut environment = std::collections::BTreeMap::new();
    let common_key = " __VERUS_DRIVER_ARGS__".to_owned();
    let package_key = format!(" __VERUS_DRIVER_ARGS_FOR_{id}");
    let common = lookup(&common_key)?;
    let package = lookup(&package_key)?;
    let mut common_count = 0;
    for (index, (key, source)) in [(common_key, &common), (package_key, &package)]
        .into_iter()
        .enumerate()
    {
        if let Some(value) = source {
            // Match the pinned rust_verify decoder exactly: only delimited
            // tokens after the leading separator are effective driver options.
            let decoded: Vec<String> = value
                .split("__VERUS_DRIVER_ARGS_SEP__")
                .skip(1)
                .map(str::to_owned)
                .collect();
            arguments.extend(decoded);
            environment.insert(key, value.clone());
        }
        if index == 0 {
            common_count = arguments.len();
        }
    }
    if !sail_api::acceptance::effective_driver_arguments_match(
        &common,
        &package,
        common_count,
        &arguments,
    ) {
        return Err(Error::new(
            "SAIL3123",
            "effective Cargo driver arguments do not match the original packed values",
        ));
    }
    let effective: Vec<_> = arguments
        .iter()
        .take_while(|argument| *argument != "--")
        .collect();
    if effective.iter().any(|argument| *argument == "--no-vstd")
        && effective.iter().any(|argument| {
            matches!(
                argument.as_str(),
                "--is-vstd" | "--is-core" | "--is-stdlib-outside-of-core"
            )
        })
    {
        return Err(Error::new(
            "SAIL3123",
            "contradictory effective Cargo launcher modes",
        ));
    }
    if effective
        .iter()
        .filter(|argument| argument.as_str() == "--no-vstd")
        .count()
        > 1
    {
        return Err(Error::new(
            "SAIL3123",
            "duplicate effective Cargo no-vstd mode",
        ));
    }
    Ok(DriverOptions {
        arguments,
        environment,
    })
}

fn relocate_no_vstd(
    options: &DriverOptions,
    arguments: &[OsString],
) -> Result<std::collections::BTreeMap<String, String>> {
    if !options
        .arguments
        .iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--no-vstd")
    {
        return Ok(std::collections::BTreeMap::new());
    }
    if arguments
        .iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--no-vstd")
    {
        return Err(Error::new(
            "SAIL3123",
            "duplicate literal and effective Cargo no-vstd mode",
        ));
    }
    let separator = "__VERUS_DRIVER_ARGS_SEP__";
    let mut relocated = options.environment.clone();
    let mut removed = false;
    // Decoder order matters when a later `--` hides positional tokens. Remove
    // only the one effective token; preserve even identical positional bytes.
    let keys = std::iter::once(" __VERUS_DRIVER_ARGS__").chain(
        options
            .environment
            .keys()
            .map(String::as_str)
            .filter(|key| *key != " __VERUS_DRIVER_ARGS__"),
    );
    for key in keys {
        let Some(value) = options.environment.get(key) else {
            continue;
        };
        let mut parts = value.split(separator);
        let mut replacement = parts.next().unwrap_or_default().to_owned();
        for part in parts {
            if !removed && part == "--no-vstd" {
                removed = true;
            } else {
                replacement.push_str(separator);
                replacement.push_str(part);
            }
        }
        relocated.insert(key.to_owned(), replacement);
    }
    Ok(relocated)
}

fn launcher_arguments(args: &[OsString], engine: bool, effective: &[String]) -> Vec<OsString> {
    let mut arguments = args.to_vec();
    let boundary = args
        .iter()
        .position(|argument| argument == "--")
        .unwrap_or(args.len());
    if engine
        && effective
            .iter()
            .take_while(|argument| *argument != "--")
            .any(|argument| argument == "--no-vstd")
        && !args[..boundary]
            .iter()
            .any(|argument| argument == "--no-vstd")
    {
        arguments.insert(boundary, OsString::from("--no-vstd"));
    }
    arguments
}

/// The actual compiler `wrapper`, in the environment plain Cargo-Verus gives
/// it (#385): Cargo's, which is the caller's plus Cargo-Verus's and Cargo's
/// own, and no private alias setting. Only Sail's requests to its Verus fork
/// for a shipped dependency build are added, as before only when this run
/// has one (ADR 0048, VI-0031); Cargo and build scripts no longer see them.
fn compiler_command(wrapper: &Path) -> CompilerCommand {
    let mut command = CompilerCommand::new(wrapper);
    transport::clear(&mut command, crate::managed::visible_output::ENV);
    if transport::file_backed() {
        for name in [
            crate::managed::shipped::SOURCE_BASE_ENV,
            crate::managed::shipped::SPAN_REMAP_ENV,
        ] {
            if let Some(value) = transport::var(name) {
                command.env(name, value);
            }
        }
    }
    command
}

fn custom_compiler_context(args: &[OsString]) -> bool {
    if args
        .iter()
        .any(|a| a == "--sysroot" || a.to_string_lossy().starts_with("--sysroot="))
    {
        return true;
    }
    let target = argument(args, "--target").or_else(|| {
        args.iter()
            .find_map(|a| a.to_str()?.strip_prefix("--target=").map(str::to_owned))
    });
    target.is_some_and(|v| {
        Path::new(&v)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|v| v.eq_ignore_ascii_case("json"))
    })
}

// The runner serializes its admitted plan before applying managed child
// environment overrides. Do not synthesize a public research switch here:
// the outer caller's absence is part of the archive restore contract.
fn retained_runtime(mode: crate::managed::research::Mode) -> Option<Runtime> {
    if mode == crate::managed::research::Mode::VstdBoundary {
        return None;
    }
    let path = transport::var("SAIL_MANAGED_RUNTIME")?;
    let runtime: Runtime = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    runtime.retained.as_ref()?;
    Some(runtime)
}

#[allow(
    clippy::too_many_lines,
    reason = "Engine fallback is kept alongside the owned subprocess drain and seal check."
)]
fn driver() -> Result<ExitCode> {
    let research = crate::managed::research::selected()?;
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let wrapper = PathBuf::from(transport::required("SAIL_MANAGED_WRAPPER")?);
    if let Some(result) = crate::managed::build_environment::probe(&wrapper, &args) {
        return result;
    }
    // Native prerequisites always execute through Cargo's real compiler. The
    // experimental cache is consulted only once Cargo reaches verified vstd.
    if !verifying(&args) {
        let metadata_stdout = if let Some(index) = metadata_stdout::metadata_output(&args) {
            let artifacts = PathBuf::from(transport::required("SAIL_MANAGED_ARTIFACTS")?);
            let run = artifacts
                .parent()
                .ok_or_else(|| Error::new("SAIL3111", "managed artifacts have no run directory"))?;
            Some(metadata_stdout::MetadataStdout::prepare(
                &mut args, index, run,
            )?)
        } else {
            None
        };
        let runtime = retained_runtime(research);
        let plan = (research != crate::managed::research::Mode::RetainedContent)
            .then(crate::managed::shipped::active)
            .flatten();
        if research == crate::managed::research::Mode::Off && runtime.is_none() && plan.is_none() {
            let actual = compiler_command(&wrapper)
                .args(&args)
                .status()
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            if let Some(metadata) = &metadata_stdout {
                metadata.replay(&mut std::io::stdout().lock())?;
            }
            return Ok(status(actual));
        }
        // Dependents Cargo starts on this unit's metadata wait while it runs.
        let _running = runtime
            .as_ref()
            .and_then(|runtime| crate::managed::retained::Running::begin(runtime, &args));
        let mut observation = plan
            .as_ref()
            .and_then(|plan| crate::managed::shipped::native::Observation::begin(plan, &args));
        let mut retained = runtime
            .as_ref()
            // The shipped observer owns the one actual compiler read endpoint.
            // It independently certifies the same completed child for retained
            // use after its fresh native boundary checks succeed.
            .filter(|_| observation.is_none())
            .and_then(|runtime| crate::managed::retained::Native::begin(runtime, &args, &wrapper));
        let mut command = compiler_command(&wrapper);
        let native_args = if let Some(call) = &observation {
            call.arguments()
        } else if let Some(attempt) = &retained {
            attempt.arguments()
        } else {
            args.as_slice()
        };
        command.args(native_args);
        if let Some(call) = &observation {
            call.configure(&mut command);
        }
        if let Some(attempt) = &retained {
            attempt.configure(&mut command);
        }
        let mut child = command
            .spawn()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if child.construction_observed() {
            if let Some(call) = &mut observation {
                call.started(child.id());
            }
            if let Some(attempt) = &mut retained {
                attempt.started(child.id());
            }
        }
        let actual = child
            .wait()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if let Some(metadata) = &metadata_stdout {
            metadata.replay(&mut std::io::stdout().lock())?;
        }
        if actual.success()
            && let Some(observation) = observation
        {
            observation.complete(runtime.as_ref());
        }
        if let Some(attempt) = &retained {
            let _ = attempt.finish(actual.success());
        }
        return Ok(status(actual));
    }
    let plan = (research != crate::managed::research::Mode::RetainedContent)
        .then(crate::managed::shipped::active)
        .flatten();
    let mut call = plan.as_ref().and_then(|plan| {
        crate::managed::shipped::Call::vstd_boundary(plan, &args, std::env::vars_os())
    });
    if custom_compiler_context(&args) {
        return Err(Error::new(
            "SAIL3111",
            "managed verification does not support a custom sysroot or JSON target specification",
        ));
    }
    let runtime_path = PathBuf::from(transport::required("SAIL_MANAGED_RUNTIME")?);
    let runtime: Runtime = serde_json::from_slice(
        &std::fs::read(runtime_path).map_err(|e| Error::new("SAIL3111", e.to_string()))?,
    )
    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let _running = crate::managed::retained::Running::begin(&runtime, &args);
    let identity = target_identity(
        &args,
        PathBuf::from(required("CARGO_MANIFEST_DIR")?).join("Cargo.toml"),
        std::env::var("CARGO_BIN_NAME").ok(),
    );
    crate::managed::retire_receipts(&runtime, &identity)?;
    let sidecar = runtime.sidecars.join(identity.key());
    let unit = identity.compiler_unit();
    let attempt_directory = sidecar.join("attempts");
    let mut attempts = attempts::Journal::new(runtime.run_id.as_bytes(), &unit, &attempt_directory);
    let allocation = attempts.allocate();
    let dir = allocation.as_ref().map_or_else(
        || sidecar.clone(),
        |allocation| allocation.directory().to_owned(),
    );
    std::fs::create_dir_all(dir.join("provenance"))
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let logs = runtime.attribution_logs.then(|| dir.join("logs"));
    // VI-0011: the pinned launcher chooses macro mode before decoding Cargo's
    // effective per-package options. Relocate only an already requested mode,
    // and only for the explicitly selected optional engine. Stock is unchanged.
    let options = if runtime.engine.is_some() {
        let id = cargo_package_id().ok_or_else(|| {
            Error::new(
                "SAIL3123",
                "engine invocation lacks its Cargo package identity",
            )
        })?;
        cargo_driver_options(&id)?
    } else {
        DriverOptions {
            arguments: Vec::new(),
            environment: std::collections::BTreeMap::new(),
        }
    };
    let launcher_environment = relocate_no_vstd(&options, &args)?;
    let launcher_args = launcher_arguments(&args, runtime.engine.is_some(), &options.arguments);
    if launcher_args != args {
        json(
            &dir.join("launcher-options.json"),
            &serde_json::json!({
                "version":1,"run_id":runtime.run_id,"context_sha256":runtime.context_sha256,
                "package_id":cargo_package_id(),"project_package_id":runtime.packages.get(&identity.manifest),
                "target":identity.target_name(),"kind":identity.crate_type,
                "forwarded":["--no-vstd"],"source":"effective-cargo-driver-options","engine_only":true,
                "original_arguments":args.iter().map(|arg|arg.as_encoded_bytes()).collect::<Vec<_>>(),
                "launcher_arguments":launcher_args.iter().map(|arg|arg.as_encoded_bytes()).collect::<Vec<_>>(),
                "effective_arguments":options.arguments,
                "deferred_environment":launcher_environment,
                "adaptation":"relocate-exact-effective-token",
                "cargo_identity": (["CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR", "CARGO_CRATE_NAME", "CARGO_PRIMARY_PACKAGE"]
                    .into_iter().filter_map(|key|std::env::var(key).ok().map(|value|(key,value)))
                    .collect::<std::collections::BTreeMap<_,_>>())
            }),
        )?;
    }
    // The Verus flags this crate actually receives: Cargo-Verus forwards
    // common and per-crate arguments (`--fwd-verus-args-to`), so the user's
    // command line alone can name flags a crate does not get, or miss ones it
    // does.
    let flags = cargo_package_id()
        .and_then(|id| cargo_driver_options(&id).ok())
        .map_or_else(
            || crate::verus_stdout::UserFlags::of(&runtime.args),
            |effective| crate::verus_stdout::UserFlags::of_verus(effective.arguments.iter()),
        );
    // The engine runs its own phases; a shipped dependency is never the
    // engine's selected crate, and the engine is not asked to replay one.
    if runtime.engine.is_none()
        && let Some(replay) = call.as_ref().and_then(crate::managed::shipped::Call::serve)
    {
        event(
            &serde_json::json!({"event":"target-started","run_id":runtime.run_id,"target":identity.target_name()}),
        );
        crate::managed::visible_output::emit(
            crate::managed::visible_output::Stream::Stderr,
            &replay.stderr,
        )
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        finish(
            &runtime,
            identity,
            &sidecar,
            &Captured {
                stdout: &replay.stdout,
                stderr: &replay.stderr,
                exit: Some(0),
            },
            None,
            Some(&replay.origin),
            CompletionOrigin::Bundled,
            None,
        )?;
        if let Some(proof) = &replay.retained {
            let _ = crate::managed::retained::publish_shipped(&runtime, proof);
        }
        let shown_stdout = crate::verus_stdout::rebuild(&replay.stdout, flags);
        std::fs::write(sidecar.join("verus.shown-stdout"), &shown_stdout)
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        crate::managed::visible_output::emit(
            crate::managed::visible_output::Stream::Stdout,
            &shown_stdout,
        )
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        return Ok(ExitCode::SUCCESS);
    }
    let retention_started = std::time::Instant::now();
    let mut retained = runtime
        .retained
        .as_ref()
        .filter(|retained| {
            research != crate::managed::research::Mode::VstdBoundary
                && (plan.is_none() || retained.requested(&identity))
        })
        .and_then(|plan| {
            (wrapper.canonicalize().ok()? == runtime.original_wrapper.canonicalize().ok()?)
                .then(|| plan.begin(&runtime, &identity, &launcher_args, &dir))
                .flatten()
        });
    if let Some(attempt) = &retained
        && let Some(restored) = attempt.restore(&runtime)?
    {
        finish(
            &runtime,
            identity.clone(),
            &sidecar,
            &Captured {
                stdout: &restored.stdout,
                stderr: &restored.stderr,
                exit: Some(0),
            },
            None,
            None,
            CompletionOrigin::Retained,
            // The establishing run's observation; the restore itself also
            // requires every keyed caller variable unchanged.
            restored.compiler_environment.clone(),
        )?;
        let path = runtime.invocations.join(format!("{}.json", identity.key()));
        let mut invocation: Invocation = serde_json::from_slice(
            &std::fs::read(&path).map_err(|e| Error::new("SAIL3111", e.to_string()))?,
        )
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        invocation.retained_from = Some(restored.establishing_run);
        invocation.retained_history = Some(restored.history);
        invocation.report = restored.report;
        invocation.selected_attempt = restored.selected_attempt;
        if invocation.selected_attempt.is_none() {
            invocation.success = false;
        }
        json(&path, &invocation)?;
        crate::managed::visible_output::emit(
            crate::managed::visible_output::Stream::Stderr,
            &restored.stderr,
        )
        .and_then(|()| {
            crate::managed::visible_output::emit(
                crate::managed::visible_output::Stream::Stdout,
                &crate::verus_stdout::rebuild(&restored.stdout, flags),
            )
        })
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut retention = retained.as_ref().map(|_| retention_started.elapsed());
    let mut cmd = compiler_command(&wrapper);
    let compiler_args = if let Some(call) = &call {
        call.compiler_arguments()
    } else if let Some(attempt) = &retained {
        attempt.arguments()
    } else {
        launcher_args.as_slice()
    };
    // #386: the tracked environment of both compiler phases, ghost code
    // included. The retention observer owns the one request endpoint when it
    // is active, and so does the research observer once it has begun below;
    // otherwise this target requests its own.
    let mut ghost = if retained.is_none() {
        crate::managed::ghost_environment::Observation::begin(
            &dir,
            &runtime.run_id,
            &identity.key(),
        )
    } else {
        None
    };
    // Sail's own Verus options travel as Cargo-Verus's do, outside the
    // command line procedural macros can read (#393).
    let base = verus_channel::Channel::cargo_value(&launcher_environment)?;
    let mut options = observation_args(flags, logs.as_deref());
    if let Some(attempt) = &retained {
        options.extend(attempt.compiler_options().iter().map(OsString::from));
    } else if ghost.is_some()
        && !crate::managed::ghost_environment::checksum_requested(compiler_args)
    {
        // The report needs a source checksum (see the observer).
        options.extend(
            crate::managed::ghost_environment::OPTIONS
                .iter()
                .map(OsString::from),
        );
    }
    let channel = verus_channel::Channel::new(base.clone(), &options)?;
    channel.publish();
    let dep_info = verus_channel::dep_info_path(compiler_args);
    cmd.args(compiler_args)
        .envs(&launcher_environment)
        .args(
            call.as_ref()
                .map(crate::managed::shipped::Call::record_args)
                .unwrap_or_default(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    channel.apply(&mut cmd);
    cmd.env_remove("SAIL_COMPILER_INPUTS")
        .env_remove("SAIL_COMPILER_INPUTS_BINDING");
    if let Some(call) = &mut call {
        call.observe_compiler(&mut cmd);
    }
    if let Some(attempt) = &retained {
        attempt.configure(&mut cmd);
    }
    if call
        .as_ref()
        .is_some_and(crate::managed::shipped::Call::observes_compiler)
    {
        ghost = None;
    }
    if let Some(observation) = &ghost {
        observation.configure(&mut cmd);
    }
    // Session records join cached answers to transcripts; without logs there
    // is nothing to join them to.
    if logs.is_some() {
        cmd.env("SAIL_PROVENANCE_DIR", dir.join("provenance"));
    }
    let source_events = dir.join("engine.events.jsonl");
    if let Some(engine) = &runtime.engine {
        cmd.env("SAIL_ENGINE", "interactive")
            .env("SAIL_ENGINE_EVENTS", &source_events)
            .env("SAIL_ENGINE_CONTEXT", &runtime.context_sha256)
            .env("SAIL_ENGINE_FOCUS_REPORT", dir.join("focus.source.json"));
        if let Some(function) = &engine.function {
            cmd.env("SAIL_ENGINE_FUNCTION", function);
        }
    }
    let relay = runtime
        .engine
        .as_ref()
        .map(|_| crate::engine_events::Relay::start(source_events.clone(), &runtime, &identity));
    let reserved = allocation.and_then(|allocation| attempts.configure(allocation, &cmd));
    let compile_started = std::time::Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let observing = attempts.started_owned(reserved, &child);
    if child.construction_observed() {
        if let Some(call) = &mut call {
            call.compiler_started(child.id());
        }
        if let Some(attempt) = &mut retained {
            attempt.started(child.id());
        }
    }
    if let Some(observation) = &mut ghost {
        observation.started(child.id());
    }
    let invocation_log = InvocationLog {
        directory: &runtime.invocations,
        run_id: &runtime.run_id,
        target: identity.target_name(),
    };
    invocation_log.record(&mut child)?;
    event(
        &serde_json::json!({"event":"target-started","run_id":runtime.run_id,"target":identity.target_name()}),
    );
    let stdout = child
        .take_stdout()
        .map(|pipe| std::thread::spawn(move || pipe.read_all()));
    let mut output = CompilerOutput { child, stdout };
    let stderr = if let Some(mut pipe) = output.child.take_stderr() {
        let mut buf = vec![0; 65536];
        loop {
            let n = pipe
                .read(&mut buf)
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            if n == 0 {
                break;
            }
            if runtime.engine.is_none() {
                crate::managed::visible_output::emit(
                    crate::managed::visible_output::Stream::Stderr,
                    &buf[..n],
                )
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            }
        }
        Some(pipe.finish())
    } else {
        attempts.refuse("missing compiler stderr pipe");
        None
    };
    output
        .child
        .wait()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let compiled = compile_started.elapsed();
    if let Some(call) = &mut call {
        call.compiler_finished();
    }
    let stdout = output.stdout()?;
    let completed = output
        .child
        .captured(stdout, stderr)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let captured = attempts.capture_owned(observing, completed);
    if let Some(relay) = relay {
        relay.finish();
    }
    let mut established = None;
    let mut stock_channel = None;
    let mut engine_restarted = false;
    let captured = if let Some(engine) = &runtime.engine {
        let stock = |directory: &Path| {
            let mut stock = compiler_command(&engine.stock_wrapper);
            let logs = runtime.attribution_logs.then(|| directory.join("logs"));
            stock.args(&launcher_args).envs(&launcher_environment);
            let channel = verus_channel::Channel::new(
                base.clone(),
                &observation_args(flags, logs.as_deref()),
            )?;
            channel.apply(&mut stock);
            stock_channel = Some(channel);
            Ok(stock)
        };
        let (output, trusted) =
            engine_output(&dir, stock, captured, &invocation_log, &mut attempts)?;
        engine_restarted = !trusted;
        if trusted {
            save_focus(&runtime, &identity, &dir)?;
        }
        established = Some(engine.provenance(if trusted {
            sail_api::output::EngineEstablishment::Engine
        } else {
            sail_api::output::EngineEstablishment::StockFallback
        }));
        output
    } else {
        captured
    };
    let exit = captured.status();
    let stdout = captured.stdout();
    let stderr = captured.stderr();
    if runtime.engine.is_some() {
        crate::managed::visible_output::emit(
            crate::managed::visible_output::Stream::Stderr,
            stderr,
        )
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    }
    let selected_attempt = captured.selected_attempt(&identity);
    let selected_directory = captured.directory().unwrap_or(&dir);
    // Cargo's next build compares the dep-info with its own environment.
    // Each restores only its own exact value, so at most one applies.
    channel.restore_dep_info(dep_info.as_deref());
    if let Some(channel) = stock_channel {
        channel.restore_dep_info(dep_info.as_deref());
    }
    let compiler_environment = (exit.success() && !engine_restarted)
        .then(|| {
            if let Some(observation) = &ghost {
                observation.observed()
            } else if let Some(attempt) = &retained {
                attempt.compiler_reads()
            } else {
                call.as_ref()?.observed_reads()
            }
        })
        .flatten()
        .and_then(|observed| declare_ghost_reads(&args, observed));
    let invocation_key = identity.key();
    let measured = (identity.manifest.clone(), identity.target_name().to_owned());
    let observation_available = finish(
        &runtime,
        identity,
        selected_directory,
        &Captured {
            stdout,
            stderr,
            exit: exit.code(),
        },
        established.as_ref(),
        None,
        CompletionOrigin::Live(
            captured.observation(),
            captured.unavailable_reason(),
            selected_attempt.as_ref(),
        ),
        compiler_environment,
    )?;
    // A successful process alone is insufficient: partial/no-verify output
    // must never become a reusable proof. Reuse only the complete successful
    // invocation that finish just established from Verus's full document.
    if let Some(call) = &call {
        let completed = std::fs::read(runtime.invocations.join(format!("{invocation_key}.json")))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Invocation>(&bytes).ok())
            .is_some_and(|invocation| {
                invocation.success && crate::managed::receipt_complete(&invocation.report)
            });
        if completed {
            call.record(stdout, stderr, exit.code());
        }
    }
    if let Some(attempt) = &retained {
        let finishing = std::time::Instant::now();
        let _ = attempt.finish(exit.success() && observation_available);
        retention = retention.map(|before| before + finishing.elapsed());
    }
    if exit.success() && runtime.engine.is_none() {
        crate::managed::retained::cost::record(
            &runtime.receipts,
            &measured.0,
            &measured.1,
            compiled,
            retention,
        );
    }
    let shown = crate::verus_stdout::rebuild(stdout, flags);
    // The dependency store must replay exactly what this target displayed,
    // including effective per-package output flags, when it seeds a new target.
    std::fs::write(selected_directory.join("verus.shown-stdout"), &shown)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    // Wait for the managed writer to deliver the rebuilt stdout before this
    // driver exits: Cargo's subsequent diagnostics must not overtake it.
    crate::managed::visible_output::emit(crate::managed::visible_output::Stream::Stdout, &shown)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    Ok(status(exit))
}

/// The environment rustc tracked in both phases of a successful verified
/// compiler child, once its ghost-only reads (variables and files) are
/// declared in the unit's dep-info ([`crate::managed::ghost_environment`]).
/// `None` when they cannot be: the result then authorizes no reuse.
fn declare_ghost_reads(
    args: &[OsString],
    observed: crate::managed::ghost_environment::Observed,
) -> Option<crate::managed::ghost_environment::Environment> {
    for path in crate::managed::ghost_environment::dep_info(args)? {
        crate::managed::ghost_environment::merge(&path, &observed)?;
    }
    Some(observed.environment)
}

/// The logging flags the managed driver adds to every verified target,
/// followed by the target's own log directory. They leave solver input
/// unchanged; the conformance harness's managed arm adds the same flags.
pub const OBSERVATION_LOG_FLAGS: &[&str] =
    &["--log", "air", "--log", "smt-transcript", "--log-dir"];

/// The flags the driver adds to every verified target: the structured
/// output the user did not already request, and, when the run's report is
/// read ([`Runtime::attribution_logs`]), isolated attribution logs.
fn observation_args(flags: crate::verus_stdout::UserFlags, logs: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = flags.sail_flags().into_iter().map(OsString::from).collect();
    if let Some(logs) = logs {
        args.extend(OBSERVATION_LOG_FLAGS.iter().map(OsString::from));
        args.push(logs.as_os_str().to_owned());
    }
    args
}

/// The attribution note of a report replayed from Sail's shipped dependency
/// build (ADR 0048). Its functions' verdicts, rlimit and time are Verus's own,
/// from the release build that verified the dependency.
pub const SHIPPED_NOT_RECORDED: &str = "obligation detail not recorded: this dependency's complete report comes from an earlier compiler-boundary verification";

/// The attribution note of a report whose run asked Verus for no logs
/// (ADR 0047). Its functions' verdicts, rlimit and time are Verus's own.
pub const NOT_REQUESTED: &str = "obligation detail not recorded: the run that established this report did not request it (plain `sail verify`); `sail verify --output json`, jobs and MCP record it";

/// Retains interrupted engine evidence and restarts the exact stock compiler
/// invocation. Engine observations and focused evidence cannot seal a report
/// without the independent complete full document and normal process exit.
/// A target that needs a Verus feature the engine lacks also restarts with
/// stock, and its sidecar records the feature.
pub(crate) fn engine_output<'a>(
    dir: &Path,
    stock: impl FnOnce(&Path) -> Result<CompilerCommand>,
    captured: attempts::CapturedChild<'a>,
    invocation_log: &InvocationLog<'_>,
    attempts: &mut attempts::Journal<'a>,
) -> Result<(attempts::CapturedChild<'a>, bool)> {
    // A target that needs a Verus feature the engine lacks gets the same
    // stock fallback as an engine fault (SAIL3124): the engine's rejection is
    // not an answer from the configured Verus.
    let available = crate::engine::runtime_cargo_features();
    let missing = crate::engine::required_feature(captured.stderr(), &available)
        .or_else(|| crate::engine::required_feature(captured.stdout(), &available));
    let available_bytes = available
        .iter()
        .map(|feature| feature.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let (protocol, documents) = crate::engine_events::completion_candidates(
        &dir.join("engine.events.jsonl"),
        captured.stdout(),
    );
    let trusted = captured.directory() == Some(dir)
        && captured.observation().is_some_and(|observed| {
            matches!(
                sail_acceptance::authority::engine_decision(
                    observed,
                    &protocol,
                    &documents,
                    missing.as_deref().map(str::as_bytes),
                    &available_bytes
                ),
                sail_acceptance::authority::EngineDecision::Complete
            )
        });
    if trusted {
        return Ok((captured, true));
    }
    if let Err(failure) = std::fs::write(dir.join("engine.failed.stdout"), captured.stdout())
        .and_then(|()| std::fs::write(dir.join("engine.failed.stderr"), captured.stderr()))
    {
        attempts.refuse(&format!(
            "cannot preserve predecessor presentation: {failure}"
        ));
    }
    let allocation = attempts.allocate();
    let fallback_directory = allocation.as_ref().map_or_else(
        || dir.to_owned(),
        |allocation| allocation.directory().to_owned(),
    );
    if let Err(failure) = std::fs::create_dir_all(fallback_directory.join("provenance")) {
        attempts.refuse(&format!("cannot create fallback provenance: {failure}"));
    }
    if let Some(feature) = &missing
        && let Err(failure) = std::fs::write(
            fallback_directory.join(crate::engine::MISSING_FEATURE),
            feature,
        )
    {
        attempts.refuse(&format!("cannot record fallback feature: {failure}"));
    }
    let mut stock = stock(&fallback_directory)?;
    for key in [
        "SAIL_ENGINE",
        "SAIL_ENGINE_FUNCTION",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE_CONTEXT",
    ] {
        stock.env_remove(key);
    }
    stock
        .env("SAIL_PROVENANCE_DIR", fallback_directory.join("provenance"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let reserved = allocation.and_then(|allocation| attempts.configure(allocation, &stock));
    let mut child = stock
        .spawn()
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    let observing = attempts.started_owned(reserved, &child);
    invocation_log.record(&mut child)?;
    let output = child
        .wait_with_output()
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    Ok((
        attempts
            .capture_owned(observing, output)
            .with_predecessor(captured),
        false,
    ))
}

struct Captured<'a> {
    stdout: &'a [u8],
    stderr: &'a [u8],
    exit: Option<i32>,
}

#[derive(Clone, Copy)]
enum CompletionOrigin<'a, 'b> {
    Live(
        Option<&'a sail_acceptance::authority::ObservedAttempt<'b>>,
        Option<&'a str>,
        Option<&'a attempts::SelectedAttempt>,
    ),
    Bundled,
    Retained,
}

/// Records engine provenance on a report and each of its functions.
pub(crate) fn mark_engine(
    report: &mut sail_api::output::Report,
    engine: &sail_api::output::EngineProvenance,
) {
    for function in report
        .crates
        .iter_mut()
        .flat_map(|krate| &mut krate.functions)
    {
        function.provenance.engine = Some(engine.clone());
    }
    report.engine = Some(engine.clone());
}

/// Whether a completed report may publish a whole-target proof receipt. A
/// stock fallback after an engine fault reports the stock verdicts but never
/// manufactures a successful engine receipt (ADR 0027).
fn receipt_success(report: &sail_api::output::Report, report_args: &[String]) -> bool {
    let fallback = report.engine.as_ref().is_some_and(|engine| {
        engine.established_by == sail_api::output::EngineEstablishment::StockFallback
    });
    crate::managed::receipt_complete(report) && !crate::managed::focused(report_args) && !fallback
}

/// The command describes this package's effective proof mode. Cargo can send
/// different options to dependencies than to the explicitly selected target.
/// Missing/invalid decoding cannot establish that verification was enabled.
fn report_verification_arguments(
    report_args: &[String],
    effective: Option<&[String]>,
) -> Option<Vec<String>> {
    let effective = effective?;
    let enabled = sail_api::acceptance::verification_enabled_options(effective);
    let mut forwarded = false;
    let mut arguments = Vec::with_capacity(report_args.len());
    for argument in report_args {
        if argument == "--" {
            forwarded = true;
        }
        if !(enabled && forwarded && argument == "--no-verify") {
            arguments.push(argument.clone());
        }
    }
    if !enabled && sail_api::acceptance::verification_enabled_command(&arguments) {
        if !forwarded {
            arguments.push("--".into());
        }
        arguments.push("--no-verify".into());
    }
    Some(arguments)
}

/// Records whether the target's origin evidence is bound and Verus remains
/// readable afterwards. Missing either records no reusable
/// success while preserving the target's own exit status, as with stock.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Keep the captured output, origin and compiler environment explicit in one unchanged completion-to-invocation handoff"
)]
fn finish(
    runtime: &Runtime,
    identity: TargetIdentity,
    dir: &Path,
    captured: &Captured<'_>,
    engine: Option<&sail_api::output::EngineProvenance>,
    dependency_build: Option<&sail_api::project::DependencyBuildOrigin>,
    origin: CompletionOrigin<'_, '_>,
    compiler_environment: Option<crate::managed::ghost_environment::Environment>,
) -> Result<bool> {
    let Captured {
        stdout,
        stderr,
        exit,
    } = *captured;
    if !matches!(origin, CompletionOrigin::Live(Some(_), ..)) {
        std::fs::write(dir.join("verus.stdout"), stdout)
            .and_then(|()| std::fs::write(dir.join("verus.stderr"), stderr))
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    }
    let (docs, _) = crate::attribution::json_documents(stdout);
    let attr = if dependency_build.is_some() {
        // Replayed from Sail's shipped dependency build, which records no
        // attribution logs.
        crate::attribution::attribute(
            &docs,
            None,
            &crate::attribution::Provenance::NoCache,
            Some(SHIPPED_NOT_RECORDED),
        )
    } else if runtime.attribution_logs {
        let logs = crate::attribution::read_logs(&full_log_dir(&dir.join("logs"), engine))
            .unwrap_or_default();
        let provenance = if runtime.daemon {
            crate::attribution::Provenance::Records(crate::attribution::read_session_records(
                &dir.join("provenance"),
            ))
        } else {
            crate::attribution::Provenance::NoCache
        };
        crate::attribution::attribute(&docs, Some(&logs), &provenance, None)
    } else {
        crate::attribution::attribute(
            &docs,
            None,
            &crate::attribution::Provenance::NoCache,
            Some(NOT_REQUESTED),
        )
    };
    // Forwarded selectors are per package. A dependency did not receive the
    // requesting package's focused flags, so its full proof is not scoped away.
    let mut report_args = runtime.args.clone();
    if crate::managed::focused(&report_args) && !actual_focus() {
        report_args.truncate(
            report_args
                .iter()
                .position(|a| a == "--")
                .unwrap_or(report_args.len()),
        );
    }
    let effective = effective_verus_arguments();
    let observed_arguments = report_verification_arguments(&report_args, effective.as_deref());
    if let Some(arguments) = &observed_arguments {
        report_args.clone_from(arguments);
    }
    let mut report = crate::report::build(&crate::report::Inputs {
        args: &report_args,
        exit_code: exit,
        solver_path: Some(runtime.z3.to_string_lossy().into_owned()),
        solver_sha256: Some(runtime.z3_sha256.clone()),
        daemon: runtime.daemon,
        attribution: &attr,
        diagnostics: &crate::diagnostics::parse(&rendered_diagnostics(stderr)),
    });
    if observed_arguments.is_none() {
        report.error = Some(sail_api::output::ErrorInfo {
            code: "SAIL3111".into(),
            message: "effective Cargo verification arguments are unavailable or invalid; proof authority withheld".into(),
        });
    }
    if let Some(engine) = engine {
        mark_engine(&mut report, engine);
    }
    let origin_available = bind_process_observation(&mut report, captured, origin);
    // The complete read of Verus below (about 26 MB) overlaps the reads of
    // the dependencies; both happen after verification, as before. Solver
    // bytes are evidence, never a transformed input: a Verus that disappeared
    // during verification never establishes a reusable success.
    let (dependencies, verifier_readable) = std::thread::scope(|scope| {
        let verifier = scope.spawn(|| file_digest(&runtime.original_wrapper).is_ok());
        (driver_dependencies(), verifier.join().unwrap_or(false))
    });
    let dependencies = dependencies?;
    let selected_attempt = bind_selected_producer(&mut report, origin);
    let producer_available =
        !matches!(origin, CompletionOrigin::Live(..)) || selected_attempt.is_some();
    let observation_available = origin_available && verifier_readable && producer_available;
    let invocation = Invocation {
        retained_from: None,
        retained_history: None,
        selected_attempt,
        identity,
        run_id: runtime.run_id.clone(),
        success: observation_available && receipt_success(&report, &report_args),
        report,
        dependencies,
        dependency_build: dependency_build.cloned(),
        compiler_environment,
    };
    json(
        &runtime
            .invocations
            .join(format!("{}.json", invocation.identity.key())),
        &invocation,
    )?;
    event(
        &serde_json::json!({"event":"target-complete","run_id":runtime.run_id,"target":invocation.identity.target_name(),"success":invocation.success}),
    );
    Ok(observation_available)
}

fn bind_selected_producer(
    report: &mut sail_api::output::Report,
    origin: CompletionOrigin<'_, '_>,
) -> Option<attempts::SelectedAttempt> {
    let selected = match origin {
        CompletionOrigin::Live(Some(observed), _, Some(selected))
            if selected.matches_observed(observed) =>
        {
            Some(selected.clone())
        }
        _ => None,
    };
    if matches!(origin, CompletionOrigin::Live(..)) && selected.is_none() && report.error.is_none()
    {
        report.error = Some(sail_api::output::ErrorInfo {
            code: "SAIL3111".into(),
            message:
                "selected compiler attempt evidence unavailable; retained proof authority withheld"
                    .into(),
        });
    }
    selected
}

fn bind_process_observation(
    report: &mut sail_api::output::Report,
    captured: &Captured<'_>,
    origin: CompletionOrigin<'_, '_>,
) -> bool {
    match origin {
        CompletionOrigin::Live(observed, reason, _) => sail_api::acceptance::require_process_observation(
            report,
            observed,
            captured.stdout,
            captured.stderr,
            captured.exit,
            sail_api::output::ErrorInfo {
                code: "SAIL3111".into(),
                message: reason.map_or_else(
                    || "compiler process observation unavailable; retained proof authority withheld".into(),
                    |reason| format!("compiler process observation unavailable; retained proof authority withheld: {reason}")),
            },
        ),
        CompletionOrigin::Bundled | CompletionOrigin::Retained => true,
    }
}

fn rendered_diagnostics(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .fold(String::new(), |mut text, line| {
            let value = serde_json::from_str::<serde_json::Value>(line).ok();
            if let Some(rendered) = value.as_ref().and_then(|v| v["rendered"].as_str()) {
                text.push_str(rendered);
            } else {
                text.push_str(line);
                text.push('\n');
            }
            text
        })
}

fn actual_focus() -> bool {
    let package = ["CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_MANIFEST_DIR"].map(std::env::var);
    let [Ok(name), Ok(version), Ok(manifest)] = package else {
        return true;
    };
    let hash = digest(
        Path::new(&manifest)
            .join("Cargo.toml")
            .to_string_lossy()
            .as_bytes(),
    );
    let id = format!("{name}-{version}-{}", hash.get(..12).unwrap_or_default());
    [
        " __VERUS_DRIVER_ARGS__".to_owned(),
        format!(" __VERUS_DRIVER_ARGS_FOR_{id}"),
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok())
    .any(|value| {
        [
            "--verify-function",
            "--verify-module",
            "--verify-only-module",
            "--verify-root",
        ]
        .iter()
        .any(|flag| value.contains(flag))
    })
}

fn driver_dependencies() -> Result<std::collections::BTreeMap<PathBuf, String>> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut present = Vec::new();
    for pair in args.windows(2).filter(|w| w[0] == "--extern") {
        if let Some((_, path)) = pair[1].to_str().and_then(|s| s.split_once('=')) {
            let path = PathBuf::from(path);
            let vir = path.with_extension("vir");
            if path.is_file() {
                present.push(path);
            }
            if vir.is_file() {
                present.push(vir);
            }
        }
    }
    dependency_hashes(&present)
}

fn dependency_hashes(present: &[PathBuf]) -> Result<std::collections::BTreeMap<PathBuf, String>> {
    // A repeated exact extern path denotes the same key; coalesce its request
    // before reading while preserving the first argument's error priority.
    let mut names = std::collections::BTreeSet::new();
    let paths: Vec<_> = present
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| names.insert(path.as_os_str().as_encoded_bytes()))
        .collect();
    let reads = crate::parallel::in_order(&paths, |&path| (path, file_digest(path)));
    dependency_hashes_readback(&paths, reads)
}

fn dependency_hashes_readback(
    paths: &[&Path],
    reads: Vec<(&Path, Result<String>)>,
) -> Result<std::collections::BTreeMap<PathBuf, String>> {
    let expected: Vec<_> = paths
        .iter()
        .map(|path| (path.as_os_str().as_encoded_bytes(), &[][..]))
        .collect();
    let returned: Vec<_> = reads
        .iter()
        .map(|(path, _)| (path.as_os_str().as_encoded_bytes(), &[][..]))
        .collect();
    let incomplete = || {
        Error::new(
            "SAIL3101",
            "dependency read outcomes do not cover the requested inventory",
        )
    };
    if !crate::parallel::exact_descriptors(&expected, &returned) {
        return Err(incomplete());
    }
    let mut by_name: std::collections::BTreeMap<_, _> = reads
        .into_iter()
        .map(|(path, hash)| (path.as_os_str().as_encoded_bytes(), hash))
        .collect();
    paths
        .iter()
        .map(|path| {
            let hash = by_name
                .remove(path.as_os_str().as_encoded_bytes())
                .ok_or_else(incomplete)??;
            Ok((path.to_path_buf(), hash))
        })
        .collect()
}

/// Where the full phase's Verus logs are.
///
/// Stock Verus, and therefore the stock fallback, writes them to the given
/// `--log-dir`. The engine's Verus fork (VI-0004, ADR 0039) runs via Cargo
/// here and gives each crate its own subdirectory: `<crate name>` for a
/// library and `<crate name>-bin` for a binary, from Cargo's
/// `CARGO_CRATE_NAME` and `CARGO_BIN_NAME`, which this driver shares with the
/// Verus it runs. The focused phase keeps its explicit `focus` directory.
fn full_log_dir(logs: &Path, engine: Option<&sail_api::output::EngineProvenance>) -> PathBuf {
    let by_engine =
        engine.is_some_and(|e| e.established_by == sail_api::output::EngineEstablishment::Engine);
    match std::env::var("CARGO_CRATE_NAME") {
        Ok(name) if !name.is_empty() => {
            let crate_dir = if std::env::var_os("CARGO_BIN_NAME").is_some() {
                logs.join(format!("{name}-bin"))
            } else {
                logs.join(name)
            };
            // Sail's Verus (ADR 0043) logs each Cargo crate to its own
            // subdirectory (VI-0004 fix); a development override with stock
            // Verus logs to the given directory.
            if by_engine || crate_dir.is_dir() {
                crate_dir
            } else {
                logs.to_owned()
            }
        }
        _ => logs.to_owned(),
    }
}

fn save_focus(runtime: &Runtime, identity: &TargetIdentity, dir: &Path) -> Result<()> {
    let path = dir.join("focus.source.json");
    if !path.is_file() {
        return Ok(());
    }
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path).map_err(|e| Error::new("SAIL3123", e.to_string()))?,
    )
    .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    let function = source["function"]
        .as_str()
        .ok_or_else(|| Error::new("SAIL3123", "focused source evidence has no function"))?;
    let logs = crate::attribution::read_logs(&dir.join("logs/focus")).unwrap_or_default();
    let docs = vec![source["verus"].clone()];
    let attr = crate::attribution::attribute(
        &docs,
        Some(&logs),
        &crate::attribution::Provenance::NoCache,
        None,
    );
    let mut report = crate::report::build(&crate::report::Inputs {
        args: &runtime.args,
        exit_code: Some(i32::from(
            source["verus"]["verification-results"]["errors"]
                .as_u64()
                .unwrap_or(1)
                > 0
                || source["verus"]["verification-results"]["encountered-error"] == true
                || source["verus"]["verification-results"]["encountered-vir-error"] == true,
        )),
        solver_path: Some(runtime.z3.to_string_lossy().into_owned()),
        solver_sha256: Some(runtime.z3_sha256.clone()),
        daemon: false,
        attribution: &attr,
        diagnostics: &[],
    });
    report.diagnostics = source["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(crate::engine_events::diagnostic)
        .collect();
    report.command.extend([
        "--".into(),
        "--sail-repair-function".into(),
        function.into(),
    ]);
    for krate in &mut report.crates {
        krate.functions.retain(|item| item.function == function);
        for item in &mut krate.functions {
            item.establishment = sail_api::output::EstablishmentName::Provisional;
            item.evidence_tier = None;
            item.verdict = sail_api::output::VerdictName::Provisional;
        }
    }
    report.summary = sail_api::output::Summary::of(&report.crates);
    json(
        &dir.join("focus.report.json"),
        &crate::engine::FocusTarget {
            function: function.to_owned(),
            package_id: runtime
                .packages
                .get(&identity.manifest)
                .cloned()
                .unwrap_or_default(),
            target: identity.target_name().to_owned(),
            kind: if identity.crate_type == "rlib" {
                "lib".into()
            } else {
                identity.crate_type.clone()
            },
            report,
        },
    )
}

fn target_identity(
    args: &[OsString],
    manifest: PathBuf,
    cargo_bin_name: Option<String>,
) -> TargetIdentity {
    let crate_type = argument(args, "--crate-type").unwrap_or_default();
    TargetIdentity {
        manifest,
        name: argument(args, "--crate-name").unwrap_or_default(),
        cargo_name: (crate_type == "bin").then_some(cargo_bin_name).flatten(),
        crate_type,
        rustc_args_sha256: digest(
            &args
                .iter()
                .flat_map(|s| s.as_encoded_bytes().iter().copied().chain([0]))
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    struct MarkerWriter(std::sync::mpsc::Sender<Vec<u8>>);
    #[cfg(unix)]
    impl std::io::Write for MarkerWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .send(bytes.to_vec())
                .map_err(|_| std::io::Error::other("marker observation closed"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[cfg(unix)]
    fn marker_accounting(
        raw: &[u8],
        markers: &crate::managed::visible_output::MarkerObserver,
    ) -> Option<Vec<u8>> {
        let mut observed = Some(Vec::new());
        for frame in raw.split_inclusive(|byte| *byte == b'\n') {
            super::capture_cargo_observation(&mut observed, frame, 64 * 1024 * 1024, Some(markers));
        }
        observed
    }

    #[cfg(unix)]
    #[test]
    fn cargo_accounting_uses_exact_live_markers_without_acknowledging()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::managed::visible_output::{MarkerObserver, Receiver, send_test};
        let receiver = Receiver::bind()?;
        let endpoint = receiver.path().to_owned();
        let markers = MarkerObserver::open(&endpoint)?;
        let raw = b"{\"reason\":\"build-script-executed\",\"package_id\":\"demo\"}\n";
        let (tx, rx) = std::sync::mpsc::channel();
        let sender_path = endpoint.clone();
        let sender =
            std::thread::spawn(move || send_test(&sender_path, raw, &mut MarkerWriter(tx)));
        let payload = rx.recv_timeout(std::time::Duration::from_secs(5))?;
        let marker = rx.recv_timeout(std::time::Duration::from_secs(5))?;
        assert_eq!(payload, raw);
        let wire = [payload.as_slice(), marker.as_slice()].concat();
        let counted = marker_accounting(&wire, &markers).ok_or("valid accounting refused")?;
        assert_eq!(counted, raw);
        assert_eq!(
            super::cargo_artifacts(&counted, &mut Vec::new(), 1, 2)?,
            Some(1)
        );
        assert!(
            !sender.is_finished(),
            "accounting must not acknowledge the sender"
        );

        // Preserve a valid Cargo JSON record preceding a marker in the same frame.
        let embedded = [
            raw.strip_suffix(b"\n").ok_or("payload newline")?,
            marker.as_slice(),
        ]
        .concat();
        assert_eq!(marker_accounting(&embedded, &markers), Some(raw.to_vec()));
        // Every original non-marker byte is retained, including malformed JSON.
        let malformed_prefix = [b"{broken".as_slice(), marker.as_slice()].concat();
        let kept = marker_accounting(&malformed_prefix, &markers).ok_or("marker framing")?;
        assert_eq!(kept, b"{broken\n");
        assert_eq!(super::cargo_artifacts(&kept, &mut Vec::new(), 1, 2)?, None);

        let mut foreign = marker.clone();
        let capability_start = b"[[sail-stdout:".len();
        foreign[capability_start] = if foreign[capability_start] == b'0' {
            b'1'
        } else {
            b'0'
        };
        let unrecognized = marker_accounting(&foreign, &markers).ok_or("foreign text retained")?;
        assert_eq!(unrecognized, foreign);
        assert_eq!(
            super::cargo_artifacts(&unrecognized, &mut Vec::new(), 1, 2)?,
            None
        );
        let capability = std::fs::read_to_string(
            endpoint
                .parent()
                .ok_or("endpoint parent")?
                .join("capability"),
        )?;
        for malformed in [
            format!("[[sail-stdout:{capability}:0-0]]\n").into_bytes(),
            format!("[[sail-stdout:{capability}:bad-name]]\n").into_bytes(),
            marker.strip_suffix(b"\n").ok_or("marker newline")?.to_vec(),
            [
                marker.strip_suffix(b"\n").ok_or("marker newline")?,
                b"junk\n",
            ]
            .concat(),
        ] {
            assert!(marker_accounting(&malformed, &markers).is_none());
        }
        receiver
            .decode(&marker)?
            .completion
            .ok_or("completion missing")?
            .acknowledge()?;
        sender.join().map_err(|_| "sender thread")??;
        assert!(
            markers.decode(&marker).is_err(),
            "retired peers are never recognized later"
        );
        receiver.finish()?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cargo_provider_records_actual_child_and_nonzero_normal_exit()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let raw = "{\"reason\":\"build-script-executed\",\"package_id\":\"demo\"}";
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", "printf '%s\\n' \"$1\"; exit 1", "fixture", raw])
            .stdout(std::process::Stdio::piped());
        let mut journal = Vec::new();
        let mut forwarded = Vec::new();
        let status = super::cargo_messages(&mut command, &mut journal, &mut forwarded)?;
        assert_eq!(status, std::process::ExitCode::from(1));
        assert_eq!(forwarded, format!("{raw}\n").as_bytes());
        assert!(crate::managed::source_inputs::fixture_complete(&journal));
        let lines: Vec<serde_json::Value> = String::from_utf8(journal)?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<_, _>>()?;
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0]["reason"], super::JOURNAL_BEGIN);
        assert_eq!(lines[5]["reason"], super::JOURNAL_END);
        let actual = lines[2]["child"].as_u64().ok_or("actual child PID")?;
        assert_ne!(actual, 0);
        assert_eq!(lines[3]["sail_source_provider"]["child"], actual);
        assert_eq!(lines[4]["child"], actual);
        assert_eq!(lines[4]["frames"], 1);
        assert_eq!(lines[4]["exited"], true);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn file_backed_cargo_provider_uses_original_output_control()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::managed::visible_output::{Receiver, send_test};
        const CHILD: &str = "SAIL_FILE_BACKED_PROVIDER_TEST";
        if let Some(root) = std::env::var_os(CHILD) {
            assert!(crate::managed::transport::file_backed());
            let wire = std::fs::read(std::path::Path::new(&root).join("wire"))?;
            let mut command = std::process::Command::new("sh");
            command
                .args(["-c", "cat \"$1\"", "fixture"])
                .arg(std::path::Path::new(&root).join("wire"))
                .stdout(std::process::Stdio::piped());
            let mut journal = Vec::new();
            let mut forwarded = Vec::new();
            assert_eq!(
                super::cargo_messages(&mut command, &mut journal, &mut forwarded)?,
                std::process::ExitCode::SUCCESS
            );
            assert_eq!(forwarded, wire, "accounting preserves original forwarding");
            assert!(
                crate::managed::source_inputs::fixture_complete(&journal),
                "the live original output marker must not poison provider completion"
            );
            return Ok(());
        }
        let root = std::env::temp_dir().join(format!("sail-file-provider-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let alias = root.join("sail-cargo");
        std::os::unix::fs::symlink(std::env::current_exe()?, &alias)?;
        let receiver = Receiver::bind()?;
        crate::managed::transport::Transport::new(None)
            .set(crate::managed::visible_output::ENV, receiver.path())
            .write(&root)?;
        let endpoint = receiver.path().to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        let sender = std::thread::spawn(move || {
            send_test(
                &endpoint,
                b"{\"reason\":\"build-script-executed\",\"package_id\":\"demo\"}\n",
                &mut MarkerWriter(tx),
            )
        });
        let payload = rx.recv_timeout(std::time::Duration::from_secs(5))?;
        let marker = rx.recv_timeout(std::time::Duration::from_secs(5))?;
        std::fs::write(root.join("wire"), [payload, marker.clone()].concat())?;
        // Each child has its own transport OnceLock. A caller variable cannot
        // override the file, and no private endpoint need appear in its environment.
        let results = [None, Some("/sail-no-such-caller-output-control")].map(|caller| {
            let mut command = std::process::Command::new(&alias);
            command
                .args([
                    "--exact",
                    "adapter::tests::file_backed_cargo_provider_uses_original_output_control",
                    "--nocapture",
                ])
                .env(CHILD, &root)
                .env_remove(crate::managed::visible_output::ENV);
            if let Some(value) = caller {
                command.env(crate::managed::visible_output::ENV, value);
            }
            command.output()
        });
        receiver
            .decode(&marker)?
            .completion
            .ok_or("completion missing")?
            .acknowledge()?;
        sender.join().map_err(|_| "sender thread")??;
        receiver.finish()?;
        std::fs::remove_dir_all(root)?;
        for result in results {
            let result = result?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
        }
        Ok(())
    }

    #[test]
    fn cargo_provider_capture_budget_poison_is_sticky() {
        let mut observed = Some(Vec::new());
        super::capture_cargo_line(&mut observed, b"{}", 4);
        assert_eq!(observed, Some(b"{}\n".to_vec()));
        super::capture_cargo_line(&mut observed, b"x", 4);
        assert_eq!(observed, None);
        super::capture_cargo_line(&mut observed, b"{}", 4);
        assert_eq!(observed, None);
    }

    #[cfg(unix)]
    #[test]
    fn cargo_provider_malformed_stdout_cannot_become_an_empty_observation()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-provider-json-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let pretty = serde_json::to_string_pretty(&serde_json::json!({
            "verification-results":{"verified":1,"errors":0,"success":true},
            "verus":{"version":"fixture"}
        }))?;
        let summary = "verification results:: 1 verified, 0 errors\n";
        for (index, raw, exit, covered) in [
            (
                0,
                "{\"reason\":\"compiler-artifact\"\n".to_owned(),
                0,
                false,
            ),
            (
                1,
                "{\"reason\":\"compiler-artifact\",\"filenames\":[".to_owned(),
                0,
                false,
            ),
            (
                2,
                "{\"reason\":\"unsupported-cargo-event\"}\n".to_owned(),
                0,
                false,
            ),
            (
                7,
                "{\"reason\":null,\"verification-results\":{}}\n".to_owned(),
                0,
                false,
            ),
            (8, "{\"unsupported\":true}\n".to_owned(), 0, false),
            (3, String::new(), 1, true),
            (4, summary.to_owned(), 0, true),
            (5, format!("{pretty}\n"), 0, true),
            (
                6,
                format!(
                    "{summary}{pretty}\n{{\"reason\":\"compiler-message\",\"message\":{{}}}}\n{{\"reason\":\"build-finished\",\"success\":false}}\n"
                ),
                1,
                true,
            ),
        ] {
            let directory = root.join(index.to_string());
            std::fs::create_dir(&directory)?;
            let mut command = std::process::Command::new("sh");
            command
                .args([
                    "-c",
                    "printf '%s' \"$1\"; exit \"$2\"",
                    "fixture",
                    &raw,
                    &exit.to_string(),
                ])
                .stdout(std::process::Stdio::piped());
            let mut journal = Vec::new();
            let mut forwarded = Vec::new();
            let actual = super::cargo_messages(&mut command, &mut journal, &mut forwarded)?;
            assert_eq!(actual, std::process::ExitCode::from(u8::try_from(exit)?));
            let expected = if raw.is_empty() || raw.ends_with('\n') {
                raw.clone()
            } else {
                format!("{raw}\n")
            };
            assert_eq!(
                forwarded,
                expected.as_bytes(),
                "case {index} forwarded bytes"
            );
            assert_eq!(
                crate::managed::source_inputs::fixture_provider_coverage(&directory, &journal)?,
                covered,
                "case {index} declaration coverage"
            );
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn cargo_provider_spawn_failure_leaves_an_unsealed_intent() {
        let mut command = std::process::Command::new("/sail-no-such-provider-executable");
        command.stdout(std::process::Stdio::piped());
        let mut journal = Vec::new();
        let mut forwarded = Vec::new();
        assert!(super::cargo_messages(&mut command, &mut journal, &mut forwarded).is_err());
        assert_ne!(journal, [] as [u8; 0]);
        assert!(!crate::managed::source_inputs::fixture_complete(&journal));
        assert_eq!(forwarded, [] as [u8; 0]);
    }

    #[test]
    fn a_vanished_verus_records_no_success_and_keeps_the_exit_status()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-adapter-vanished-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("invocations"))?;
        let runtime = super::Runtime {
            run_id: "run".into(),
            args: Vec::new(),
            z3: root.join("z3"),
            z3_sha256: String::new(),
            daemon: false,
            sidecars: root.join("sidecars"),
            invocations: root.join("invocations"),
            original_wrapper: root.join("missing-verus"),
            receipts: root.join("receipts"),
            context_sha256: "context".into(),
            input_revision: "source".into(),
            packages: std::collections::BTreeMap::new(),
            target_kinds: std::collections::BTreeMap::new(),
            engine: None,
            retained: None,
            attribution_logs: false,
        };
        let identity = super::TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "args".into(),
            cargo_name: None,
        };
        let key = identity.key();
        let captured = super::Captured {
            stdout: b"",
            stderr: b"",
            exit: Some(0),
        };
        // Before, this was an error after `success` had been recorded.
        let readable = super::finish(
            &runtime,
            identity.clone(),
            &root,
            &captured,
            None,
            None,
            super::CompletionOrigin::Bundled,
            None,
        )?;
        assert!(!readable);
        let recorded: super::Invocation = serde_json::from_slice(&std::fs::read(
            root.join("invocations").join(format!("{key}.json")),
        )?)?;
        assert!(!recorded.success);
        // Readable verifier bytes alone cannot rehabilitate missing process
        // evidence or mark retained capture complete after the merge.
        std::fs::write(&runtime.original_wrapper, b"readable verifier fixture")?;
        let observed = super::finish(
            &runtime,
            identity,
            &root,
            &captured,
            None,
            None,
            super::CompletionOrigin::Live(None, None, None),
            None,
        )?;
        assert!(!observed);
        let recorded: super::Invocation = serde_json::from_slice(&std::fs::read(
            root.join("invocations").join(format!("{key}.json")),
        )?)?;
        assert!(!recorded.success);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn failed_stream_cleanup_reaps_compiler_and_joins_stdout()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use std::io::Read as _;
        let mut child = std::process::Command::new("python3")
            .args(["-c", "import os,time; os.write(2,b'R'); time.sleep(30)"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).map(|_| bytes)
            })
        });
        let mut output = super::CompilerOutput { child, stdout };
        let mut ready = [0];
        output
            .child
            .stderr
            .as_mut()
            .ok_or("missing stderr")?
            .read_exact(&mut ready)?;
        assert_eq!(ready, [b'R']);
        assert!(output.child.try_wait()?.is_none());
        // This is the same cleanup called by Drop after a refused ACK or write.
        output.stop();
        assert!(
            output
                .child
                .try_wait()?
                .is_some_and(|status| !status.success())
        );
        assert!(output.stdout.is_none(), "stdout reader must be joined");
        // A second Drop cleanup must remain harmless.
        Ok(())
    }

    #[test]
    fn automatic_retained_plan_reaches_owned_managed_child()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        const CHILD: &str = "SAIL_RETAINED_ADAPTER_CHILD";
        if std::env::var_os(CHILD).is_some() {
            assert_eq!(std::env::var("RUSTC_BOOTSTRAP")?, "1");
            assert!(std::env::var_os("SAIL_RETAINED_CONTENT_RESEARCH").is_none());
            let mode = crate::managed::research::selected()?;
            assert_eq!(mode, crate::managed::research::Mode::Off);
            let runtime = super::retained_runtime(mode).ok_or("automatic plan lost")?;
            let wire = serde_json::to_value(&runtime)?;
            assert_eq!(runtime.run_id, "owned-outer-run");
            assert!(
                wire["retained"]["caller_environment"]["values"]
                    .get("RUSTC_BOOTSTRAP")
                    .is_none()
            );
            assert!(
                super::retained_runtime(crate::managed::research::Mode::VstdBoundary).is_none()
            );
            let path = std::env::var_os("SAIL_MANAGED_RUNTIME").ok_or("runtime path")?;
            let mut malformed = wire;
            malformed["retained"]
                .as_object_mut()
                .ok_or("plan")?
                .remove("caller_environment");
            std::fs::write(&path, serde_json::to_vec(&malformed)?)?;
            assert!(super::retained_runtime(mode).is_none());
            malformed["retained"] = serde_json::Value::Null;
            std::fs::write(&path, serde_json::to_vec(&malformed)?)?;
            assert!(super::retained_runtime(mode).is_none());
            return Ok(());
        }
        let root = std::env::temp_dir().join(format!("sail-auto-adapter-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let path = root.join("runtime.json");
        // An actual Runtime/Plan wire file exercises the existing managed
        // child transport. This fixture is not compiler/source admission.
        let wire = serde_json::json!({
            "run_id":"owned-outer-run", "args":[], "z3":"/pinned/z3",
            "z3_sha256":"solver", "daemon":false, "sidecars":root,
            "invocations":root, "original_wrapper":"/pinned/verus", "receipts":root,
            "context_sha256":"context", "input_revision":"revision",
            "packages":{}, "target_kinds":{}, "engine":null,
            "retained":{"root":root,"target":root,"metadata":null,"roots":[],
                "closure":{"directories":{}},"launcher":"/pinned/verus",
                "launcher_sha256":"launcher","compiler_sha256":"compiler",
                "caller_environment":{"version":1,"run":"owned-outer-run","values":{}}}
        });
        let _: super::Runtime = serde_json::from_value(wire.clone())?;
        std::fs::write(&path, serde_json::to_vec(&wire)?)?;
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "adapter::tests::automatic_retained_plan_reaches_owned_managed_child",
            ])
            .env(CHILD, "1")
            .env("SAIL_MANAGED_RUNTIME", &path)
            .env("RUSTC_BOOTSTRAP", "1")
            .env_remove("SAIL_RETAINED_CONTENT_RESEARCH")
            .env_remove("SAIL_VSTD_BOUNDARY_RESEARCH")
            .output()?;
        std::fs::remove_dir_all(root)?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        Ok(())
    }

    #[test]
    fn conflicting_research_refuses_before_adapter_setup()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        if std::env::var("SAIL_RESEARCH_SELECTION_EXPECTED").as_deref() == Ok("conflict") {
            assert_eq!(
                super::driver().err().ok_or("expected refusal")?.code,
                "SAIL3135"
            );
        }
        Ok(())
    }

    #[test]
    fn tagged_dependency_reads_cannot_cancel_physical_swaps()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-tagged-dependencies-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let a = root.join("a.vir");
        let b = root.join("b.vir");
        std::fs::write(&a, "first")?;
        std::fs::write(&b, "second")?;
        let paths = [a.as_path(), b.as_path()];
        let read = || crate::parallel::in_order(&paths, |&path| (path, file_digest(path)));
        let expected = dependency_hashes(&[a.clone(), b.clone(), a.clone()])?;
        assert_eq!(expected.len(), 2);
        let mut reversed = read();
        reversed.reverse();
        assert_eq!(dependency_hashes_readback(&paths, reversed)?, expected);
        let mut missing = read();
        missing.pop();
        assert!(dependency_hashes_readback(&paths, missing).is_err());
        let duplicate = vec![
            (a.as_path(), file_digest(&a)),
            (a.as_path(), file_digest(&a)),
        ];
        assert!(dependency_hashes_readback(&paths, duplicate).is_err());
        let mut extra = read();
        extra.push((root.as_path(), Ok("foreign".into())));
        assert!(dependency_hashes_readback(&paths, extra).is_err());
        std::fs::write(&a, "second")?;
        std::fs::write(&b, "first")?;
        let mut swapped = read();
        swapped.reverse();
        assert_ne!(dependency_hashes_readback(&paths, swapped)?, expected);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn cargo_lint_bytes_order_and_repetitions_remain_in_the_actual_target_identity()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/retained-cargo-lints.json"))?;
        let mut args: Vec<std::ffi::OsString> = [
            "/rustc",
            "--crate-name",
            "sample",
            "--crate-type",
            "lib",
            "src/lib.rs",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let lint_start = args.len();
        for value in fixture["lint_arguments"]
            .as_array()
            .ok_or("lint arguments")?
        {
            args.push(value.as_str().ok_or("lint argument")?.into());
        }
        let manifest = std::path::PathBuf::from("/owned/Cargo.toml");
        let original = super::target_identity(&args, manifest.clone(), None);
        let mut repeated = args.clone();
        repeated.push("--warn=unexpected_cfgs".into());
        assert_ne!(
            original.key(),
            super::target_identity(&repeated, manifest.clone(), None).key()
        );
        let mut reversed = args.clone();
        reversed[lint_start..].reverse();
        assert_ne!(
            original.key(),
            super::target_identity(&reversed, manifest.clone(), None).key()
        );
        args.push("--deny=unexpected_cfgs".into());
        assert_ne!(
            original.key(),
            super::target_identity(&args, manifest, None).key()
        );
        Ok(())
    }

    #[test]
    fn retained_routing_fixture_uses_the_real_effective_driver_decoder()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/retained-cargo-routing.json"
        ))?;
        for row in fixture["rows"].as_array().ok_or("rows")? {
            let id = row["id"].as_str().ok_or("id")?;
            let decoded = super::cargo_driver_options_with(id, |name| {
                Ok(if name == " __VERUS_DRIVER_ARGS__" {
                    row["common"].as_str().map(str::to_owned)
                } else if name == format!(" __VERUS_DRIVER_ARGS_FOR_{id}") {
                    row["package"].as_str().map(str::to_owned)
                } else {
                    None
                })
            })?;
            assert_eq!(serde_json::to_value(decoded.arguments)?, row["tokens"]);
        }
        Ok(())
    }

    use super::*;

    #[test]
    fn report_proof_mode_binds_effective_package_arguments_without_implicit_flag_drift() {
        let ordinary = vec![
            "-p".into(),
            "demo".into(),
            "--".into(),
            "--rlimit=30".into(),
        ];
        let logged = vec![
            "--rlimit=30".into(),
            "--output-json".into(),
            "--time".into(),
        ];
        assert_eq!(
            report_verification_arguments(&ordinary, Some(&logged)),
            Some(ordinary.clone())
        );
        assert_eq!(report_verification_arguments(&ordinary, None), None);

        let disabled = vec!["--rlimit=30".into(), "--no-verify".into()];
        let Some(recorded) = report_verification_arguments(&ordinary, Some(&disabled)) else {
            unreachable!("decoded options are present");
        };
        assert_eq!(
            recorded,
            vec!["-p", "demo", "--", "--rlimit=30", "--no-verify"]
        );
        assert!(!sail_api::acceptance::verification_enabled_command(
            &recorded
        ));
        // A selected package's compile-only flag did not reach this dependency.
        assert_eq!(
            report_verification_arguments(&recorded, Some(&logged)),
            Some(ordinary)
        );
        let after_rust_separator = vec!["--".into(), "--no-verify".into()];
        assert_eq!(
            report_verification_arguments(&recorded, Some(&after_rust_separator)),
            Some(vec![
                "-p".into(),
                "demo".into(),
                "--".into(),
                "--rlimit=30".into()
            ])
        );
    }

    #[test]
    fn compile_only_success_json_cannot_publish_any_proof_authority() {
        // Exact terminal fields reproduced with the pinned stock verifier in
        // corpus/repros/verus-no-verify-success (VI-0035). These describe a
        // successful compile-only invocation of a proof containing assert(false).
        let document = serde_json::json!({
            "func-details": {},
            "verification-results": {
                "encountered-error": false,
                "encountered-vir-error": false,
                "success": true,
                "verified": 0,
                "errors": 0,
                "is-verifying-entire-crate": true
            }
        });
        let attribution = crate::attribution::attribute(
            &[document],
            None,
            &crate::attribution::Provenance::NoCache,
            None,
        );
        let command = vec!["--".into(), "--no-verify".into()];
        let report = crate::report::build(&crate::report::Inputs {
            args: &command,
            exit_code: Some(0),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        });
        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.crates.len(), 1);
        assert_eq!(report.crates[0].verus.success, Some(true));
        assert_eq!(report.crates[0].verus.errors, Some(0));
        assert_eq!(report.summary.functions, 0);
        for policy in [
            sail_acceptance::authority::CompletionPolicy::Receipt,
            sail_acceptance::authority::CompletionPolicy::Whole,
            sail_acceptance::authority::CompletionPolicy::Repair,
        ] {
            assert!(sail_api::acceptance::complete_report(&report, policy).is_none());
        }
        assert!(!receipt_success(&report, &command));
    }

    #[test]
    fn attribution_logs_are_requested_only_with_a_log_directory() {
        let flags = crate::verus_stdout::UserFlags::of(&[]);
        let plain = observation_args(flags, None);
        assert_eq!(plain, ["--output-json", "--time"].map(OsString::from));
        let logged = observation_args(flags, Some(Path::new("logs")));
        assert_eq!(
            logged,
            [
                "--output-json",
                "--time",
                "--log",
                "air",
                "--log",
                "smt-transcript",
                "--log-dir",
                "logs"
            ]
            .map(OsString::from)
        );
    }

    #[cfg(unix)]
    fn capture_engine<'a>(
        log: &InvocationLog<'_>,
        attempts: &mut attempts::Journal<'a>,
        document: &[u8],
        exit: &str,
    ) -> std::result::Result<attempts::CapturedChild<'a>, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(log.directory)?;
        let input = log.directory.join("engine.output");
        std::fs::write(&input, document)?;
        let mut command = CompilerCommand::new("/bin/sh");
        command
            .args([
                "-c",
                "cat \"$1\"; printf 'original engine diagnostic\\n' >&2; exit \"$2\"",
                "engine",
            ])
            .arg(&input)
            .arg(exit)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let allocation = attempts.allocate().ok_or("fixture allocation refused")?;
        let directory = allocation.directory().to_owned();
        // Fixtures seed a document outside the producer directory. The actual
        // child copies it into its own declared fresh event path before exit.
        let seed = log
            .directory
            .parent()
            .ok_or("fixture root")?
            .join("engine.events.jsonl");
        if seed.is_file() {
            command = CompilerCommand::new("/bin/sh");
            command.args(["-c", "cat \"$1\"; printf 'original engine diagnostic\\n' >&2; cat \"$3\" > \"$4\"; exit \"$2\"", "engine"])
                .arg(&input).arg(exit).arg(seed).arg(directory.join("engine.events.jsonl"))
                .stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        let reserved = attempts.configure(allocation, &command);
        let mut child = command.spawn()?;
        let observing = attempts.started_owned(reserved, &child);
        log.record(&mut child)?;
        let output = child.wait_with_output()?;
        Ok(attempts.capture_owned(observing, output))
    }

    #[cfg(unix)]
    fn test_attempt_unit() -> sail_acceptance::authority::CompilerUnit {
        sail_acceptance::authority::CompilerUnit {
            manifest: b"fixture/Cargo.toml".to_vec(),
            name: b"demo".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        }
    }

    #[cfg(unix)]
    fn assert_attempt_journal(
        directory: &Path,
        document: &[u8],
        fallback_document: &[u8],
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("0.observed"))?)?;
        let fallback: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("1.observed"))?)?;
        assert_eq!(original["slot"], 0);
        assert_eq!(fallback["slot"], 1);
        assert_eq!(original["run"], serde_json::json!(b"run".as_slice()));
        assert_eq!(original["stdout_sha256"], crate::managed::digest(document));
        assert_eq!(
            fallback["stdout_sha256"],
            crate::managed::digest(fallback_document)
        );
        let command: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("1.reserved"))?)?;
        assert_eq!(
            command["command"]["program"],
            serde_json::json!(b"/bin/sh".as_slice())
        );
        assert!(
            command["command"]["arguments"]
                .as_array()
                .ok_or("arguments")?
                .contains(&serde_json::json!(b"literal;$(untouched)".as_slice()))
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "Keep the original output/argv/spawn and new separate-history assertions in one fallback refusal matrix."
    )]
    fn interrupted_engine_restarts_exact_stock_args_and_preserves_failure()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-engine-restart-{}", std::process::id()));
        let proof = include_bytes!("../tests/fixtures/engine-terminal/failed-proof.json");
        let late = include_bytes!("../tests/fixtures/engine-terminal/late-exec.json");
        let documents = [
            b"".as_slice(),
            b"{broken",
            &proof[..proof.len() - 2],
            proof,
            proof,
            late,
        ];
        for (index, document) in documents.into_iter().enumerate() {
            let dir = root.join(index.to_string());
            std::fs::create_dir_all(dir.join("logs"))?;
            std::fs::create_dir_all(dir.join("provenance"))?;
            std::fs::write(dir.join("logs/unsealed"), b"old log")?;
            std::fs::write(dir.join("focus.source.json"), b"focused evidence")?;
            std::fs::write(
                dir.join("engine.events.jsonl"),
                "{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{\"success\":false}}\n",
            )?;
            if index == 3 {
                std::fs::write(
                    dir.join("engine.events.jsonl"),
                    include_bytes!("../tests/fixtures/engine-terminal/terminal-failure.jsonl"),
                )?;
            }
            let starts = dir.join("starts");
            let log = InvocationLog {
                directory: &starts,
                run_id: "run",
                target: "demo",
            };
            let unit = test_attempt_unit();
            let attempt_directory = dir.join("attempts");
            let mut attempts =
                attempts::Journal::new(log.run_id.as_bytes(), &unit, &attempt_directory);
            let captured = capture_engine(
                &log,
                &mut attempts,
                document,
                if index == 3 { "101" } else { "1" },
            )?;
            let stock_document = if index >= 4 {
                document
            } else {
                b"stock-reply\n"
            };
            let reply = dir.join("stock.output");
            std::fs::write(&reply, stock_document)?;
            let mut stock = CompilerCommand::new("/bin/sh");
            stock.current_dir(&dir).env("SAIL_ENGINE", "interactive").args([
                "-c",
                "reply=$1; shift; printf '%s\\0' \"$@\" > stock.argv; printf '%s' \"${SAIL_ENGINE-unset}\" > stock.engine; cat \"$reply\"; printf 'original engine diagnostic\\n' >&2; exit 1",
                "stock",
            ]).arg(reply).args(["crate path.rs", "--", "literal;$(untouched)"]);
            let engine_directory = captured
                .directory()
                .ok_or("missing engine namespace")?
                .to_owned();
            let (output, trusted) = engine_output(
                &engine_directory,
                |_| Ok(stock),
                captured,
                &log,
                &mut attempts,
            )?;
            let fallback_directory = output
                .directory()
                .ok_or("missing fallback namespace")?
                .to_owned();
            assert_ne!(engine_directory, fallback_directory);
            let output = output.into_output();
            assert!(!trusted);
            assert_eq!(output.stdout, stock_document);
            assert_eq!(output.stderr, b"original engine diagnostic\n");
            assert_eq!(output.status.code(), Some(1));
            assert_eq!(
                std::fs::read(dir.join("stock.argv"))?,
                b"crate path.rs\0--\0literal;$(untouched)\0"
            );
            assert_eq!(std::fs::read(dir.join("stock.engine"))?, b"unset");
            assert_eq!(
                std::fs::read(engine_directory.join("engine.failed.stdout"))?,
                document
            );
            assert_eq!(
                std::fs::read(engine_directory.join("engine.failed.stderr"))?,
                b"original engine diagnostic\n"
            );
            assert_eq!(
                std::fs::read(dir.join("focus.source.json"))?,
                b"focused evidence"
            );
            assert_eq!(std::fs::read(dir.join("logs/unsealed"))?, b"old log");
            assert!(!fallback_directory.join("focus.source.json").exists());
            assert!(!fallback_directory.join("logs/unsealed").exists());
            assert!(dir.join("engine.events.jsonl").is_file());
            let markers = std::fs::read_dir(&starts)?.collect::<std::io::Result<Vec<_>>>()?;
            assert_eq!(
                markers
                    .iter()
                    .filter(|entry| entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "started"))
                    .count(),
                2
            );
            assert_attempt_journal(&attempt_directory, document, stock_document)?;
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn completed_engine_is_trusted_and_a_crash_falls_back_to_stock()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-engine-crash-{}", std::process::id()));
        let dir = root.join("sidecar");
        std::fs::create_dir_all(&dir)?;
        let starts = dir.join("starts");
        let log = InvocationLog {
            directory: &starts,
            run_id: "run",
            target: "demo",
        };
        let unit = test_attempt_unit();
        let attempt_directory = dir.join("attempts");
        let mut attempts = attempts::Journal::new(log.run_id.as_bytes(), &unit, &attempt_directory);
        let success = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        // Control: a completed engine run is trusted as is.
        std::fs::write(
            dir.join("engine.events.jsonl"),
            include_bytes!("../tests/fixtures/engine-terminal/terminal-success.jsonl"),
        )?;
        let captured = capture_engine(&log, &mut attempts, success, "0")?;
        let mut stock = CompilerCommand::new("/bin/sh");
        stock.args(["-c", "exit 2"]);
        let engine_directory = captured
            .directory()
            .ok_or("missing engine namespace")?
            .to_owned();
        let (_, trusted) = engine_output(
            &engine_directory,
            |_| Ok(stock),
            captured,
            &log,
            &mut attempts,
        )?;
        assert!(trusted);
        // An injected engine crash (exit 101, no seal) falls back to stock.
        std::fs::remove_file(dir.join("engine.events.jsonl"))?;
        let captured = capture_engine(&log, &mut attempts, success, "101")?;
        let reply = dir.join("stock.output");
        std::fs::write(&reply, success)?;
        let mut stock = CompilerCommand::new("/bin/sh");
        stock.args(["-c", "cat \"$1\"", "stock"]).arg(&reply);
        let engine_directory = captured
            .directory()
            .ok_or("missing engine namespace")?
            .to_owned();
        let (output, trusted) = engine_output(
            &engine_directory,
            |_| Ok(stock),
            captured,
            &log,
            &mut attempts,
        )?;
        let output = output.into_output();
        assert!(!trusted);
        assert_eq!(output.stdout, success);
        assert!(engine_directory.join("engine.failed.stdout").is_file());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    fn checked_feature_refusal(
        captured: &attempts::CapturedChild<'_>,
    ) -> std::result::Result<usize, Box<dyn std::error::Error>> {
        use sail_acceptance::authority::{EngineCompletion, EngineDecision};
        let original_directory = captured.directory().ok_or("original directory")?;
        let observed = captured.observation().ok_or("actual observation")?;
        let (protocol, documents) = crate::engine_events::completion_candidates(
            &original_directory.join("engine.events.jsonl"),
            captured.stdout(),
        );
        assert_eq!(
            sail_acceptance::authority::engine_completion(
                &protocol,
                &documents,
                captured.stdout(),
                Some(0)
            ),
            EngineCompletion::Complete
        );
        let empty = Vec::new();
        let missing = crate::engine::required_feature(captured.stdout(), &[])
            .ok_or("native feature decoder")?;
        let decision = sail_acceptance::authority::engine_decision(
            observed,
            &protocol,
            &documents,
            Some(missing.as_bytes()),
            &empty,
        );
        let EngineDecision::Declined(declined) = decision else {
            return Err("missing checked decline".into());
        };
        assert_eq!(declined.observed_attempt().exit(), Some(0));
        assert_eq!(
            declined.completion_reason(),
            &EngineCompletion::MissingFeature
        );
        assert_eq!(
            declined
                .feature_refusal()
                .as_ref()
                .ok_or("feature token")?
                .feature_name(),
            b"singular"
        );
        for invalid in [b"bogus".as_slice(), b"singular`".as_slice(), b"".as_slice()] {
            assert!(
                sail_acceptance::authority::missing_engine_feature(observed, invalid, &empty)
                    .is_none()
            );
        }
        assert!(
            sail_acceptance::authority::missing_engine_feature(
                observed,
                b"singular",
                &[b"singular".to_vec()]
            )
            .is_none()
        );
        Ok(observed.reserved().slot())
    }

    #[cfg(unix)]
    #[test]
    fn valid_protocol_missing_feature_uses_checked_raw_refusal_and_independent_fallback()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-engine-feature-refusal-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let log = InvocationLog {
            directory: &root.join("starts"),
            run_id: "run",
            target: "demo",
        };
        let unit = test_attempt_unit();
        let directory = root.join("attempts");
        let mut attempts = attempts::Journal::new(log.run_id.as_bytes(), &unit, &directory);
        let mut document: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/engine-terminal/success.json"
        ))?;
        document["diagnostic"] =
            "Please cargo build with `--features singular` to use integer_ring attribute".into();
        let stdout = serde_json::to_vec(&document)?;
        std::fs::write(
            root.join("engine.events.jsonl"),
            include_bytes!("../tests/fixtures/engine-terminal/terminal-success.jsonl"),
        )?;
        let captured = capture_engine(&log, &mut attempts, &stdout, "0")?;
        let original_directory = captured.directory().ok_or("original directory")?.to_owned();
        let original_slot = checked_feature_refusal(&captured)?;
        let mut stock = CompilerCommand::new("/bin/sh");
        stock.args([
            "-c",
            "printf 'independent fallback reply'; printf 'original stock diagnostic' >&2; exit 1",
        ]);
        let (output, trusted) = engine_output(
            &original_directory,
            |_| Ok(stock),
            captured,
            &log,
            &mut attempts,
        )?;
        assert!(!trusted);
        assert_eq!(output.status().code(), Some(1));
        assert_eq!(output.stdout(), b"independent fallback reply");
        assert_eq!(output.stderr(), b"original stock diagnostic");
        assert!(
            output
                .observation()
                .ok_or("fallback observation")?
                .reserved()
                .slot()
                > original_slot
        );
        let fallback = output.directory().ok_or("fallback directory")?;
        assert_ne!(fallback, original_directory);
        assert_eq!(
            std::fs::read(fallback.join(crate::engine::MISSING_FEATURE))?,
            b"singular"
        );
        assert_eq!(
            std::fs::read(original_directory.join("verus.stdout"))?,
            stdout
        );
        assert!(original_directory.join("engine.events.jsonl").is_file());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn normal_zero_engine_refusal_retains_real_process_status_and_fresh_fallback()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-engine-decline-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let log = InvocationLog {
            directory: &root.join("starts"),
            run_id: "run",
            target: "demo",
        };
        let unit = test_attempt_unit();
        let directory = root.join("attempts");
        let mut attempts = attempts::Journal::new(log.run_id.as_bytes(), &unit, &directory);
        let success = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        let captured = capture_engine(&log, &mut attempts, success, "0")?;
        let (protocol, documents) = crate::engine_events::completion_candidates(
            &root.join("engine.events.jsonl"),
            captured.stdout(),
        );
        let original = captured
            .observation()
            .ok_or("missing real process observation")?;
        let declined = sail_acceptance::authority::decline_engine(original, &protocol, &documents)
            .ok_or("normal zero with missing protocol was accepted")?;
        assert_eq!(declined.observed_attempt().exit(), Some(0));
        assert_eq!(declined.observed_attempt().reserved().slot(), 0);
        assert!(matches!(
            declined.completion_reason(),
            sail_acceptance::authority::EngineCompletion::Terminal
        ));
        assert_eq!(declined.document_candidates().documents.len(), 1);
        assert!(declined.protocol_candidate().is_none());
        let reply = root.join("stock.output");
        std::fs::write(&reply, success)?;
        let mut stock = CompilerCommand::new("/bin/sh");
        stock
            .args([
                "-c",
                "cat \"$1\"; printf 'fresh stock diagnostic' >&2",
                "stock",
            ])
            .arg(&reply);
        let engine_directory = captured
            .directory()
            .ok_or("missing engine namespace")?
            .to_owned();
        let (output, trusted) = engine_output(
            &engine_directory,
            |_| Ok(stock),
            captured,
            &log,
            &mut attempts,
        )?;
        assert!(!trusted);
        assert_eq!(output.status().code(), Some(0));
        assert_eq!(output.stdout(), success);
        assert_eq!(output.stderr(), b"fresh stock diagnostic");
        assert_eq!(
            output
                .observation()
                .ok_or("fallback observation missing")?
                .reserved()
                .slot(),
            1
        );
        assert_eq!(
            std::fs::read(engine_directory.join("engine.failed.stdout"))?,
            success
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn stock_fallback_reports_engine_provenance_without_a_receipt() {
        let (documents, _) = crate::attribution::json_documents(include_bytes!(
            "../tests/fixtures/engine-terminal/success.json"
        ));
        let attribution = crate::attribution::attribute(
            &documents,
            None,
            &crate::attribution::Provenance::NoCache,
            None,
        );
        let build = || {
            crate::report::build(&crate::report::Inputs {
                args: &[],
                exit_code: Some(0),
                solver_path: None,
                solver_sha256: None,
                daemon: false,
                attribution: &attribution,
                diagnostics: &[],
            })
        };
        let stock = build();
        assert!(stock.engine.is_none());
        assert!(receipt_success(&stock, &[]), "stock control publishes");
        let request = crate::engine::Request {
            selection: None,
            function: None,
            cargo_verus: PathBuf::new(),
            manifest: PathBuf::new(),
            manifest_sha256: "ab".repeat(32),
            stock_wrapper: PathBuf::new(),
            artifacts: Vec::new(),
        };
        let mut engine = build();
        mark_engine(
            &mut engine,
            &request.provenance(sail_api::output::EngineEstablishment::Engine),
        );
        assert!(receipt_success(&engine, &[]));
        let mut fallback = build();
        mark_engine(
            &mut fallback,
            &request.provenance(sail_api::output::EngineEstablishment::StockFallback),
        );
        assert!(!receipt_success(&fallback, &[]));
        // Verdicts are the stock verdicts; only provenance differs.
        assert_eq!(fallback.crates.len(), stock.crates.len());
        for (marked, original) in fallback
            .crates
            .iter()
            .flat_map(|krate| &krate.functions)
            .zip(stock.crates.iter().flat_map(|krate| &krate.functions))
        {
            assert_eq!(marked.verdict, original.verdict);
            let engine = marked.provenance.engine.as_ref();
            assert_eq!(
                engine.map(|engine| engine.established_by),
                Some(sail_api::output::EngineEstablishment::StockFallback)
            );
        }
        // Stock serialization is unchanged: no engine fields appear.
        let text = serde_json::to_string(&stock).unwrap_or_default();
        assert!(!text.contains("\"engine\""), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn normal_completed_success_and_exact_failures_do_not_restart_stock()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir =
            std::env::temp_dir().join(format!("sail-engine-normal-failure-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("engine.events.jsonl"),
            "{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{\"success\":false}}\n",
        )?;
        let proof = include_bytes!("../tests/fixtures/engine-terminal/failed-proof.json");
        let late = include_bytes!("../tests/fixtures/engine-terminal/late-exec.json");
        let starts = dir.join("starts");
        let log = InvocationLog {
            directory: &starts,
            run_id: "run",
            target: "demo",
        };
        let unit = test_attempt_unit();
        let attempt_directory = dir.join("attempts");
        let mut attempts = attempts::Journal::new(log.run_id.as_bytes(), &unit, &attempt_directory);
        let success = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        for (index, (document, code)) in [
            (proof.as_slice(), "1"),
            (late.as_slice(), "1"),
            (success.as_slice(), "0"),
        ]
        .into_iter()
        .enumerate()
        {
            std::fs::write(
                dir.join("engine.events.jsonl"),
                if code == "0" {
                    include_bytes!("../tests/fixtures/engine-terminal/terminal-success.jsonl")
                        .as_slice()
                } else {
                    include_bytes!("../tests/fixtures/engine-terminal/terminal-failure.jsonl")
                        .as_slice()
                },
            )?;
            let captured = capture_engine(&log, &mut attempts, document, code)?;
            let mut stock = CompilerCommand::new("/bin/sh");
            stock
                .current_dir(&dir)
                .args(["-c", "touch stock.started; exit 2"]);
            let engine_directory = captured
                .directory()
                .ok_or("missing engine namespace")?
                .to_owned();
            let (output, trusted) = engine_output(
                &engine_directory,
                |_| Ok(stock),
                captured,
                &log,
                &mut attempts,
            )?;
            let output = output.into_output();
            assert!(trusted);
            assert_eq!(output.stdout, document);
            assert_eq!(output.stderr, b"original engine diagnostic\n");
            assert_eq!(
                output.status.code().map(|code| code.to_string()).as_deref(),
                Some(code)
            );
            assert!(!dir.join("stock.started").exists());
            assert!(!engine_directory.join("engine.failed.stdout").exists());
            let markers = std::fs::read_dir(&starts)?.collect::<std::io::Result<Vec<_>>>()?;
            assert_eq!(
                markers
                    .iter()
                    .filter(|entry| entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "started"))
                    .count(),
                index + 1
            );
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
    #[test]
    fn engine_reflects_only_exact_effective_no_vstd_once_before_separator() {
        let args = vec![OsString::from("crate.rs"), "--".into(), "trailing".into()];
        let effective = vec!["--no-vstd".into()];
        assert_eq!(
            launcher_arguments(&args, true, &effective),
            vec![
                OsString::from("crate.rs"),
                "--no-vstd".into(),
                "--".into(),
                "trailing".into()
            ]
        );
        assert_eq!(launcher_arguments(&args, false, &effective), args);
        assert_eq!(
            launcher_arguments(
                &args,
                true,
                &["prefix--no-vstd".into(), "--no-vstd=something".into()]
            ),
            args
        );
        let literal = vec![OsString::from("crate.rs"), "--no-vstd".into()];
        assert_eq!(launcher_arguments(&literal, true, &effective), literal);
    }

    #[test]
    fn effective_driver_options_use_only_common_and_exact_current_package() {
        let values = std::collections::BTreeMap::from([
            (" __VERUS_DRIVER_ARGS__", "__VERUS_DRIVER_ARGS_SEP__--time"),
            (
                " __VERUS_DRIVER_ARGS_FOR_this",
                "__VERUS_DRIVER_ARGS_SEP__--no-vstd",
            ),
            (
                " __VERUS_DRIVER_ARGS_FOR_other",
                "__VERUS_DRIVER_ARGS_SEP__--is-vstd",
            ),
        ]);
        let mut looked_up = Vec::new();
        let arguments = cargo_driver_options_with("this", |key| {
            looked_up.push(key.to_owned());
            Ok(values.get(key).map(|value| (*value).to_owned()))
        });
        assert_eq!(
            arguments.ok().map(|options| options.arguments),
            Some(vec!["--time".into(), "--no-vstd".into()])
        );
        assert_eq!(
            looked_up,
            [" __VERUS_DRIVER_ARGS__", " __VERUS_DRIVER_ARGS_FOR_this"]
        );
        assert_eq!(
            cargo_driver_options_with("other", |_| Ok(Some("--no-vstd".into())))
                .ok()
                .map(|options| options.arguments),
            Some(Vec::new())
        );
        let args = vec![OsString::from("crate.rs")];
        assert_eq!(
            launcher_arguments(&args, true, &["--".into(), "--no-vstd".into()]),
            args
        );
        assert!(
            cargo_driver_options_with("this", |_| Err(Error::new(
                "SAIL3123",
                "invalid argument encoding"
            )))
            .is_err()
        );
    }

    #[test]
    fn contradictory_effective_macro_modes_fail_closed() {
        for conflicting in ["--is-vstd", "--is-core", "--is-stdlib-outside-of-core"] {
            let result = cargo_driver_options_with("this", |key| {
                Ok(Some(if key.ends_with("this") {
                    "__VERUS_DRIVER_ARGS_SEP__--no-vstd".into()
                } else {
                    format!("__VERUS_DRIVER_ARGS_SEP__{conflicting}")
                }))
            });
            assert_eq!(result.err().map(|error| error.code), Some("SAIL3123"));
        }
    }

    #[test]
    fn relocation_preserves_other_packed_bytes_and_rejects_duplicate_modes() {
        let common = "prefix__VERUS_DRIVER_ARGS_SEP__--time__VERUS_DRIVER_ARGS_SEP____VERUS_DRIVER_ARGS_SEP__other-token";
        let options = cargo_driver_options_with("this", |key| {
            Ok(Some(if key.ends_with("this") {
                "prefix__VERUS_DRIVER_ARGS_SEP__--no-vstd__VERUS_DRIVER_ARGS_SEP__--other-option"
                    .into()
            } else {
                common.into()
            }))
        })
        .ok();
        assert!(options.is_some());
        let Some(options) = options else { return };
        let relocated = relocate_no_vstd(&options, &["crate.rs".into()]).ok();
        assert_eq!(
            relocated,
            Some(std::collections::BTreeMap::from([
                (" __VERUS_DRIVER_ARGS__".into(), common.into()),
                (
                    " __VERUS_DRIVER_ARGS_FOR_this".into(),
                    "prefix__VERUS_DRIVER_ARGS_SEP__--other-option".into()
                ),
            ]))
        );
        assert_eq!(
            relocate_no_vstd(&options, &["--no-vstd".into()])
                .err()
                .map(|error| error.code),
            Some("SAIL3123")
        );
        assert_eq!(
            cargo_driver_options_with("this", |_| Ok(Some(
                "__VERUS_DRIVER_ARGS_SEP__--no-vstd".into()
            )))
            .err()
            .map(|error| error.code),
            Some("SAIL3123")
        );
    }

    #[test]
    fn relocation_keeps_identical_positional_tokens_after_option_boundary() {
        let options = cargo_driver_options_with("this", |key| {
            Ok(Some(if key.ends_with("this") {
                "__VERUS_DRIVER_ARGS_SEP__--no-vstd".into()
            } else {
                "__VERUS_DRIVER_ARGS_SEP__--no-vstd__VERUS_DRIVER_ARGS_SEP__--".into()
            }))
        })
        .ok();
        assert!(options.is_some());
        let Some(options) = options else { return };
        let relocated = relocate_no_vstd(&options, &["crate.rs".into()]).ok();
        assert_eq!(
            relocated,
            Some(std::collections::BTreeMap::from([
                (
                    " __VERUS_DRIVER_ARGS__".into(),
                    "__VERUS_DRIVER_ARGS_SEP__--".into()
                ),
                (
                    " __VERUS_DRIVER_ARGS_FOR_this".into(),
                    "__VERUS_DRIVER_ARGS_SEP__--no-vstd".into()
                ),
            ]))
        );
    }

    #[test]
    fn explicit_custom_toolchain_inputs_fail_closed() {
        for args in [
            vec!["--sysroot", "/custom"],
            vec!["--sysroot=/custom"],
            vec!["--target", "custom.json"],
            vec!["--target=custom.json"],
        ] {
            assert!(custom_compiler_context(
                &args.into_iter().map(OsString::from).collect::<Vec<_>>()
            ));
        }
        assert!(!custom_compiler_context(&[
            "--target".into(),
            "x86_64-unknown-linux-gnu".into()
        ]));
    }
}
