//! Fresh experimental bounded Kani checks, separate from Verus jobs and receipts.
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, PartialEq)]
struct Options {
    project: PathBuf,
    cargo_home: PathBuf,
    harness: String,
    unwind: u32,
    timeout_ms: u64,
    json: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut values = std::collections::BTreeMap::new();
    let mut tokens = args.iter();
    while let Some(flag) = tokens.next() {
        if !matches!(
            flag.as_str(),
            "--backend"
                | "--project"
                | "--cargo-home"
                | "--harness"
                | "--unwind"
                | "--timeout-ms"
                | "--output"
        ) {
            return Err(format!("unsupported Kani option `{flag}`"));
        }
        let value = tokens
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("missing value for `{flag}`"))?;
        if values.insert(flag.as_str(), value.as_str()).is_some() {
            return Err(format!("duplicate Kani option `{flag}`"));
        }
    }
    if values.get("--backend") != Some(&"kani") {
        return Err("expected --backend kani".into());
    }
    let required = |name| {
        values
            .get(name)
            .copied()
            .ok_or_else(|| format!("missing `{name}`"))
    };
    let harness = required("--harness")?.to_owned();
    if harness.is_empty()
        || harness.starts_with('-')
        || harness.chars().any(char::is_control)
        || harness.len() > 4096
    {
        return Err("use the exact hand-written harness name reported by Kani".into());
    }
    let unwind = required("--unwind")?
        .parse::<u32>()
        .map_err(|_| "unwind must be a positive integer")?;
    let timeout_ms = values
        .get("--timeout-ms")
        .unwrap_or(&"600000")
        .parse::<u64>()
        .map_err(|_| "timeout-ms must be an integer")?;
    if unwind == 0 || !(1..=600_000).contains(&timeout_ms) {
        return Err("unwind must be positive and timeout-ms must be 1..=600000".into());
    }
    let json = match values.get("--output") {
        None => false,
        Some(&"json") => true,
        _ => return Err("expected --output json".into()),
    };
    let cargo_home = values
        .get("--cargo-home")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("CARGO_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or("specify a prepared --cargo-home directory")?;
    Ok(Options {
        project: PathBuf::from(values.get("--project").unwrap_or(&".")),
        cargo_home,
        harness,
        unwind,
        timeout_ms,
        json,
    })
}

pub(crate) fn requested(args: &[String]) -> bool {
    args.iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--backend" || arg.starts_with("--backend="))
}

pub(crate) fn verify(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let result = parse(args).and_then(|options| execute(&options, out));
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let _ = writeln!(err, "error[SAIL6306]: experimental Kani check: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn execute(options: &Options, out: &mut dyn Write) -> Result<u8, String> {
    use std::sync::{Arc, atomic::AtomicBool};
    use std::time::Duration;
    let project = options
        .project
        .canonicalize()
        .map_err(|error| format!("project: {error}"))?;
    let cargo_home = options
        .cargo_home
        .canonicalize()
        .map_err(|error| format!("Cargo home: {error}"))?;
    // Project and prepared dependencies are ordinary local inputs, not an
    // authenticated source closure. No receipt/currentness/cache path is called.
    let root = crate::kani_setup::root()?;
    let runtime = root.join("original-0.68.0-linux-x86_64");
    let tools =
        sail_kani::tools::Tools::authenticate(&runtime.join("bundle"), &runtime.join("rust"))
            .map_err(|error| format!("{error}; run `sail engine install --backend kani`"))?;
    let stage_path = root.join("stage-0.68.0-linux-x86_64");
    let stage = if std::fs::symlink_metadata(&stage_path).is_ok() {
        tools.inspect_stage(&stage_path)
    } else {
        tools.stage(&stage_path)
    }
    .map_err(|error| error.to_string())?;
    let attempt = crate::kani_setup::private_attempt(&root)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancel);
    ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::SeqCst))
        .map_err(|error| format!("cannot install cancellation handler: {error}"))?;
    let capture = sail_kani::execution::run(
        &stage,
        &sail_kani::execution::Request {
            project: &project,
            cargo_home: &cargo_home,
            harness: &options.harness,
            unwind: options.unwind,
            process_limit: Duration::from_millis(options.timeout_ms),
        },
        &attempt.join("capture"),
        &cancel,
    )
    .map_err(|error| error.to_string())?;
    let success = capture.observation.outcome() == sail_kani::Outcome::BoundedSuccess;
    if options.json {
        let result = serde_json::json!({
            "kind": "sail-kani-bounded-result", "version": 1, "backend": "kani",
            "experimental": true, "harness": options.harness, "unwind": options.unwind,
            "observation": capture.observation, "capture": capture.directory,
            "input_closure_authenticated": false, "proof_authority": false,
            "currentness_authority": false, "cache_reused": false,
        });
        serde_json::to_writer(&mut *out, &result).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        let outcome = match capture.observation.outcome() {
            sail_kani::Outcome::BoundedSuccess => "bounded_success",
            sail_kani::Outcome::Counterexample => "counterexample",
            sail_kani::Outcome::BoundInsufficient => "bound_insufficient",
        };
        writeln!(out, "experimental kani: {outcome} for {} (unwind {})\nOriginal captures: {}\nNo reusable proof or source-currentness authority.", options.harness, options.unwind, capture.directory.display()).map_err(|error| error.to_string())?;
    }
    Ok(u8::from(!success))
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn execute(_options: &Options, _out: &mut dyn Write) -> Result<u8, String> {
    Err("the pinned experimental Kani backend supports native Linux x86-64".into())
}

#[cfg(test)]
mod tests {
    use super::{parse, requested};
    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }
    #[test]
    fn bounds_identity_and_mode_boundaries_are_explicit() {
        let valid = "--backend kani --harness pilot::proof --unwind 6 --cargo-home prepared";
        let result = parse(&args(valid));
        assert!(result.is_ok(), "{result:?}");
        for suffix in [
            " --unwind 8",
            " --output xml",
            " --background",
            " -- --engine interactive",
            " --timeout-ms 0",
            " --timeout-ms 600001",
        ] {
            assert!(
                parse(&args(&format!("{valid}{suffix}"))).is_err(),
                "{suffix}"
            );
        }
        for text in [
            "--backend kani",
            "--backend other --harness pilot::proof --unwind 6",
            "--backend kani --harness pilot::proof --unwind 0",
        ] {
            assert!(parse(&args(text)).is_err(), "{text}");
        }
        assert!(
            parse(&args(
                "--backend kani --harness proof --unwind 6 --cargo-home prepared"
            ))
            .is_ok()
        );
        assert!(requested(&args(valid)));
        assert!(!requested(&args("-- --backend kani")));
        assert!(requested(&args("--backend=kani")));
    }
}
