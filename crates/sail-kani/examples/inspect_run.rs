//! Contributor-only original-driver attempt; no selected CLI backend or receipt.

#[cfg(target_os = "linux")]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    let args = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "arguments must be valid UTF-8")?;
    let [
        bundle,
        rust,
        stage,
        project,
        cargo_home,
        harness,
        unwind,
        seconds,
        output,
    ] = args.as_slice()
    else {
        return Err(
            "usage: inspect_run BUNDLE RUST STAGE PROJECT CARGO_HOME HARNESS UNWIND SECONDS NEW_OUTPUT"
                .into(),
        );
    };
    let tools = sail_kani::tools::Tools::authenticate(Path::new(bundle), Path::new(rust))?;
    let stage = tools.inspect_stage(Path::new(stage))?;
    let request = sail_kani::execution::Request {
        project: Path::new(project),
        cargo_home: Path::new(cargo_home),
        harness,
        unwind: unwind.parse()?,
        process_limit: Duration::from_secs(seconds.parse()?),
    };
    let capture =
        sail_kani::execution::run(&stage, &request, Path::new(output), &AtomicBool::new(false))?;
    println!(
        "{}",
        serde_json::json!({
            "kind": "sail-kani-original-driver-attempt",
            "capture": capture.directory.to_str().ok_or("non-UTF-8 output path")?,
            "observation": capture.observation,
            "input_closure_authenticated": false,
            "proof_authority": false,
        })
    );
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    Err("only pinned native Linux original-driver attempts are supported".into())
}

fn main() -> std::process::ExitCode {
    match inspect() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            #[cfg(target_os = "linux")]
            if error.is::<sail_kani::execution::Error>() || error.is::<sail_kani::tools::Error>() {
                eprintln!("{error}");
                return std::process::ExitCode::from(2);
            }
            eprintln!("error[SAIL6304]: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
