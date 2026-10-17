//! Private Cargo and Verus-driver roles for the same installed Sail executable.
//!
//! Metadata, rustc probes, build scripts, and non-verification packages are
//! forwarded with their original argument bytes. Verified targets alone get
//! isolated observation sidecars; these flags leave solver input unchanged.

use crate::managed::{Invocation, Runtime, TargetIdentity, digest, event, file_digest, json};
use crate::{Error, Result};
use std::ffi::OsString;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// Only Cargo-Verus's own metadata probe receives this private marker. The
/// build subprocess removes it so nested user commands retain their environment.
pub(crate) const CARGO_METADATA_ENV: &str = "SAIL_MANAGED_CARGO_METADATA";

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
    fn record(&self, child: &mut std::process::Child) -> Result<()> {
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
    let mut cmd = Command::new(required("SAIL_MANAGED_CARGO")?);
    cmd.args(&args);
    if args.first().is_some_and(|argument| argument == "metadata")
        && let Some(target) = std::env::var_os(CARGO_METADATA_ENV)
    {
        // Cargo metadata probes rustc and can write .rustc_info.json in the
        // caller's target, even though it does not compile. Managed builds use
        // their explicit owned target, so that override cannot select artifacts.
        configure_metadata_target(&mut cmd, Path::new(&target))?;
    }
    cmd.env_remove(CARGO_METADATA_ENV);
    let build = args.iter().any(|s| s == "build" || s == "check");
    if !build {
        return cmd
            .status()
            .map(status)
            .map_err(|e| Error::new("SAIL3111", e.to_string()));
    }
    cmd.env("SAIL_MANAGED_WRAPPER", required("RUSTC_WRAPPER")?)
        .env("RUSTC_WRAPPER", required("SAIL_MANAGED_PROXY")?);
    // Cargo messages remain on stdout; target Verus documents live in sidecars.
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let path = PathBuf::from(required("SAIL_MANAGED_ARTIFACTS")?);
    let mut records = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout).split(b'\n') {
            let line = line.map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line)
                && matches!(
                    value["reason"].as_str(),
                    Some("compiler-artifact" | "build-script-executed")
                )
            {
                records
                    .write_all(&line)
                    .and_then(|()| records.write_all(b"\n"))
                    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            }
            std::io::stdout()
                .write_all(&line)
                .and_then(|()| std::io::stdout().write_all(b"\n"))
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        }
    }
    child
        .wait()
        .map(status)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))
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
    for key in [
        " __VERUS_DRIVER_ARGS__".to_owned(),
        format!(" __VERUS_DRIVER_ARGS_FOR_{id}"),
    ] {
        if let Some(value) = lookup(&key)? {
            // Match the pinned rust_verify decoder exactly: only delimited
            // tokens after the leading separator are effective driver options.
            arguments.extend(
                value
                    .split("__VERUS_DRIVER_ARGS_SEP__")
                    .skip(1)
                    .map(str::to_owned),
            );
            environment.insert(key, value);
        }
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

#[allow(
    clippy::too_many_lines,
    reason = "Engine fallback is kept alongside the owned subprocess drain and seal check."
)]
fn driver() -> Result<ExitCode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let wrapper = PathBuf::from(required("SAIL_MANAGED_WRAPPER")?);
    // Native prerequisites always execute through Cargo's real compiler. The
    // experimental cache is consulted only once Cargo reaches verified vstd.
    if !verifying(&args) {
        let plan = crate::managed::shipped::active();
        let observation = plan
            .as_ref()
            .and_then(|plan| crate::managed::shipped::native::Observation::begin(plan, &args));
        let actual = Command::new(wrapper)
            .args(&args)
            .status()
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        if actual.success()
            && let Some(observation) = observation
        {
            observation.complete();
        }
        return Ok(status(actual));
    }
    let plan = crate::managed::shipped::active();
    let mut call = plan.as_ref().and_then(|plan| {
        crate::managed::shipped::Call::vstd_boundary(plan, &args, std::env::vars_os())
    });
    if custom_compiler_context(&args) {
        return Err(Error::new(
            "SAIL3111",
            "managed verification does not support a custom sysroot or JSON target specification",
        ));
    }
    let runtime_path = PathBuf::from(required("SAIL_MANAGED_RUNTIME")?);
    let runtime: Runtime = serde_json::from_slice(
        &std::fs::read(runtime_path).map_err(|e| Error::new("SAIL3111", e.to_string()))?,
    )
    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    let identity = TargetIdentity {
        manifest: PathBuf::from(required("CARGO_MANIFEST_DIR")?).join("Cargo.toml"),
        name: argument(&args, "--crate-name").unwrap_or_default(),
        crate_type: argument(&args, "--crate-type").unwrap_or_default(),
        rustc_args_sha256: digest(
            &args
                .iter()
                .flat_map(|s| s.as_encoded_bytes().iter().copied().chain([0]))
                .collect::<Vec<_>>(),
        ),
    };
    crate::managed::retire_receipts(&runtime, &identity)?;
    let dir = runtime.sidecars.join(identity.key());
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
                "target":identity.name,"kind":identity.crate_type,
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
        && let Some(replay) = call
            .as_ref()
            .and_then(|call| call.serve(&mut std::io::stderr()))
    {
        event(
            &serde_json::json!({"event":"target-started","run_id":runtime.run_id,"target":identity.name}),
        );
        std::io::stderr()
            .write_all(&replay.stderr)
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        finish(
            &runtime,
            identity,
            &dir,
            &Captured {
                stdout: &replay.stdout,
                stderr: &replay.stderr,
                exit: Some(0),
            },
            None,
            Some(&replay.origin),
        )?;
        let shown_stdout = crate::verus_stdout::rebuild(&replay.stdout, flags);
        std::fs::write(dir.join("verus.shown-stdout"), &shown_stdout)
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        std::io::stdout()
            .write_all(&shown_stdout)
            .and_then(|()| std::io::stdout().flush())
            .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut cmd = Command::new(&wrapper);
    cmd.args(&launcher_args)
        .envs(&launcher_environment)
        .args(observation_args(flags, logs.as_deref()))
        .args(
            call.as_ref()
                .map(crate::managed::shipped::Call::record_args)
                .unwrap_or_default(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.env_remove("SAIL_COMPILER_INPUTS")
        .env_remove("SAIL_COMPILER_INPUTS_BINDING");
    if let Some(call) = &mut call {
        call.observe_compiler(&mut cmd);
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
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    if let Some(call) = &mut call {
        call.compiler_started(child.id());
    }
    let invocation_log = InvocationLog {
        directory: &runtime.invocations,
        run_id: &runtime.run_id,
        target: &identity.name,
    };
    invocation_log.record(&mut child)?;
    event(
        &serde_json::json!({"event":"target-started","run_id":runtime.run_id,"target":identity.name}),
    );
    let stdout = child.stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    });
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        let mut buf = vec![0; 65536];
        loop {
            let n = pipe
                .read(&mut buf)
                .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            if n == 0 {
                break;
            }
            stderr.extend_from_slice(&buf[..n]);
            if runtime.engine.is_none() {
                std::io::stderr()
                    .write_all(&buf[..n])
                    .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
            }
        }
    }
    let exit = child
        .wait()
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    if let Some(call) = &mut call {
        call.compiler_finished();
    }
    let stdout = stdout
        .ok_or_else(|| Error::new("SAIL3111", "missing solver output pipe"))?
        .join()
        .map_err(|_| Error::new("SAIL3111", "solver output reader failed"))?
        .map_err(|e| Error::new("SAIL3111", format!("cannot capture solver output: {e}")))?;
    if let Some(relay) = relay {
        relay.finish();
    }
    let mut established = None;
    let (stdout, stderr, exit) = if let Some(engine) = &runtime.engine {
        let mut stock = Command::new(&engine.stock_wrapper);
        stock
            .args(&launcher_args)
            .envs(&launcher_environment)
            .args(observation_args(flags, logs.as_deref()));
        let captured = std::process::Output {
            stdout,
            stderr,
            status: exit,
        };
        let (output, trusted) = engine_output(&dir, stock, captured, &invocation_log)?;
        if trusted {
            save_focus(&runtime, &identity, &dir)?;
        }
        established = Some(engine.provenance(if trusted {
            sail_api::output::EngineEstablishment::Engine
        } else {
            sail_api::output::EngineEstablishment::StockFallback
        }));
        (output.stdout, output.stderr, output.status)
    } else {
        (stdout, stderr, exit)
    };
    if runtime.engine.is_some() {
        std::io::stderr()
            .write_all(&stderr)
            .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    }
    let invocation_key = identity.key();
    finish(
        &runtime,
        identity,
        &dir,
        &Captured {
            stdout: &stdout,
            stderr: &stderr,
            exit: exit.code(),
        },
        established.as_ref(),
        None,
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
            call.record(&stdout, &stderr, exit.code());
        }
    }
    let shown = crate::verus_stdout::rebuild(&stdout, flags);
    // The dependency store must replay exactly what this target displayed,
    // including effective per-package output flags, when it seeds a new target.
    std::fs::write(dir.join("verus.shown-stdout"), &shown)
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    // Plain Verus's stdout, rebuilt from the document Sail asked for; Cargo
    // forwards it, and the managed runner shows it to the user.
    std::io::stdout()
        .write_all(&shown)
        .and_then(|()| std::io::stdout().flush())
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
    Ok(status(exit))
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
pub(crate) fn engine_output(
    dir: &Path,
    mut stock: Command,
    captured: std::process::Output,
    invocation_log: &InvocationLog<'_>,
) -> Result<(std::process::Output, bool)> {
    // A target that needs a Verus feature the engine lacks gets the same
    // stock fallback as an engine fault (SAIL3124): the engine's rejection is
    // not an answer from the configured Verus.
    let available = crate::engine::runtime_cargo_features();
    let missing = crate::engine::required_feature(&captured.stderr, &available)
        .or_else(|| crate::engine::required_feature(&captured.stdout, &available));
    if missing.is_none()
        && crate::engine_events::completed(
            &dir.join("engine.events.jsonl"),
            &captured.stdout,
            captured.status.code(),
        )
    {
        return Ok((captured, true));
    }
    if let Some(feature) = &missing {
        std::fs::write(dir.join(crate::engine::MISSING_FEATURE), feature)
            .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    }
    std::fs::write(dir.join("engine.failed.stdout"), &captured.stdout)
        .and_then(|()| std::fs::write(dir.join("engine.failed.stderr"), &captured.stderr))
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    let _ = std::fs::remove_file(dir.join("focus.source.json"));
    for directory in [dir.join("logs"), dir.join("provenance")] {
        if directory.exists() {
            std::fs::remove_dir_all(&directory)
                .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
        }
    }
    std::fs::create_dir_all(dir.join("provenance"))
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    for key in [
        "SAIL_ENGINE",
        "SAIL_ENGINE_FUNCTION",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE_CONTEXT",
    ] {
        stock.env_remove(key);
    }
    let mut child = stock
        .env("SAIL_PROVENANCE_DIR", dir.join("provenance"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    invocation_log.record(&mut child)?;
    let output = child
        .wait_with_output()
        .map_err(|e| Error::new("SAIL3123", e.to_string()))?;
    Ok((output, false))
}

struct Captured<'a> {
    stdout: &'a [u8],
    stderr: &'a [u8],
    exit: Option<i32>,
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

fn finish(
    runtime: &Runtime,
    identity: TargetIdentity,
    dir: &Path,
    captured: &Captured<'_>,
    engine: Option<&sail_api::output::EngineProvenance>,
    dependency_build: Option<&sail_api::project::DependencyBuildOrigin>,
) -> Result<()> {
    let Captured {
        stdout,
        stderr,
        exit,
    } = *captured;
    std::fs::write(dir.join("verus.stdout"), stdout)
        .and_then(|()| std::fs::write(dir.join("verus.stderr"), stderr))
        .map_err(|e| Error::new("SAIL3111", e.to_string()))?;
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
    let mut report = crate::report::build(&crate::report::Inputs {
        args: &report_args,
        exit_code: exit,
        solver_path: Some(runtime.z3.to_string_lossy().into_owned()),
        solver_sha256: Some(runtime.z3_sha256.clone()),
        daemon: runtime.daemon,
        attribution: &attr,
        diagnostics: &crate::diagnostics::parse(&rendered_diagnostics(stderr)),
    });
    if let Some(engine) = engine {
        mark_engine(&mut report, engine);
    }
    let success = receipt_success(&report, &report_args);
    // The complete read of Verus below (about 26 MB) overlaps the reads of
    // the dependencies; both happen after verification, as before.
    let (dependencies, verifier_readable) = std::thread::scope(|scope| {
        let verifier = scope.spawn(|| file_digest(&runtime.original_wrapper).is_ok());
        (driver_dependencies(), verifier.join().unwrap_or(false))
    });
    let dependencies = dependencies?;
    let invocation = Invocation {
        identity,
        run_id: runtime.run_id.clone(),
        report,
        success,
        dependencies,
        dependency_build: dependency_build.cloned(),
    };
    json(
        &runtime
            .invocations
            .join(format!("{}.json", invocation.identity.key())),
        &invocation,
    )?;
    event(
        &serde_json::json!({"event":"target-complete","run_id":runtime.run_id,"target":invocation.identity.name,"success":success}),
    );
    // Solver bytes are evidence, never a transformed input.
    if !verifier_readable {
        return Err(Error::new(
            "SAIL3111",
            "Verus disappeared during verification",
        ));
    }
    Ok(())
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
    // Complete reads of every dependency overlap; the first failure in
    // argument order is still the reported error.
    let digests = crate::parallel::in_order(&present, |path| file_digest(path));
    let mut files = std::collections::BTreeMap::new();
    for (path, digest) in present.into_iter().zip(digests) {
        files.insert(path, digest?);
    }
    Ok(files)
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
            target: identity.name.clone(),
            kind: if identity.crate_type == "rlib" {
                "lib".into()
            } else {
                identity.crate_type.clone()
            },
            report,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn capture_engine(
        log: &InvocationLog<'_>,
        document: &[u8],
        exit: &str,
    ) -> std::result::Result<std::process::Output, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(log.directory)?;
        let input = log.directory.join("engine.output");
        std::fs::write(&input, document)?;
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "cat \"$1\"; printf 'original engine diagnostic\\n' >&2; exit \"$2\"",
                "engine",
            ])
            .arg(input)
            .arg(exit)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        log.record(&mut child)?;
        Ok(child.wait_with_output()?)
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_engine_restarts_exact_stock_args_and_preserves_failure()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-engine-fallback-{}", std::process::id()));
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
            let captured = capture_engine(&log, document, if index == 3 { "101" } else { "1" })?;
            let stock_document = if index >= 4 {
                document
            } else {
                b"stock-reply\n"
            };
            let reply = dir.join("stock.output");
            std::fs::write(&reply, stock_document)?;
            let mut stock = Command::new("/bin/sh");
            stock.current_dir(&dir).env("SAIL_ENGINE", "interactive").args([
                "-c",
                "reply=$1; shift; printf '%s\\0' \"$@\" > stock.argv; printf '%s' \"${SAIL_ENGINE-unset}\" > stock.engine; cat \"$reply\"; printf 'original engine diagnostic\\n' >&2; exit 1",
                "stock",
            ]).arg(reply).args(["crate path.rs", "--", "literal;$(untouched)"]);
            let (output, trusted) = engine_output(&dir, stock, captured, &log)?;
            assert!(!trusted);
            assert_eq!(output.stdout, stock_document);
            assert_eq!(output.stderr, b"original engine diagnostic\n");
            assert_eq!(output.status.code(), Some(1));
            assert_eq!(
                std::fs::read(dir.join("stock.argv"))?,
                b"crate path.rs\0--\0literal;$(untouched)\0"
            );
            assert_eq!(std::fs::read(dir.join("stock.engine"))?, b"unset");
            assert_eq!(std::fs::read(dir.join("engine.failed.stdout"))?, document);
            assert_eq!(
                std::fs::read(dir.join("engine.failed.stderr"))?,
                b"original engine diagnostic\n"
            );
            assert!(!dir.join("focus.source.json").exists());
            assert!(!dir.join("logs/unsealed").exists());
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
        let success = include_bytes!("../tests/fixtures/engine-terminal/success.json");
        // Control: a completed engine run is trusted as is.
        std::fs::write(
            dir.join("engine.events.jsonl"),
            include_bytes!("../tests/fixtures/engine-terminal/terminal-success.jsonl"),
        )?;
        let captured = capture_engine(&log, success, "0")?;
        let mut stock = Command::new("/bin/sh");
        stock.args(["-c", "exit 2"]);
        let (_, trusted) = engine_output(&dir, stock, captured, &log)?;
        assert!(trusted);
        // An injected engine crash (exit 101, no seal) falls back to stock.
        std::fs::remove_file(dir.join("engine.events.jsonl"))?;
        let captured = capture_engine(&log, success, "101")?;
        let reply = dir.join("stock.output");
        std::fs::write(&reply, success)?;
        let mut stock = Command::new("/bin/sh");
        stock.args(["-c", "cat \"$1\"", "stock"]).arg(&reply);
        let (output, trusted) = engine_output(&dir, stock, captured, &log)?;
        assert!(!trusted);
        assert_eq!(output.stdout, success);
        assert!(dir.join("engine.failed.stdout").is_file());
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
            let captured = capture_engine(&log, document, code)?;
            let mut stock = Command::new("/bin/sh");
            stock
                .current_dir(&dir)
                .args(["-c", "touch stock.started; exit 2"]);
            let (output, trusted) = engine_output(&dir, stock, captured, &log)?;
            assert!(trusted);
            assert_eq!(output.stdout, document);
            assert_eq!(output.stderr, b"original engine diagnostic\n");
            assert_eq!(
                output.status.code().map(|code| code.to_string()).as_deref(),
                Some(code)
            );
            assert!(!dir.join("stock.started").exists());
            assert!(!dir.join("engine.failed.stdout").exists());
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
