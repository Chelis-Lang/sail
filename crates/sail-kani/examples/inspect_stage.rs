//! Contributor-only original-tool staging; no harness or backend execution.
#[cfg(unix)]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [bundle, rust, destination, trailing @ ..] = args.as_slice() else {
        return Err(
            "usage: inspect_stage KANI_BUNDLE RUST_PREFIX PRIVATE_STAGE [--check-existing]".into(),
        );
    };
    let tools = sail_kani::tools::Tools::authenticate(
        std::path::Path::new(bundle),
        std::path::Path::new(rust),
    )?;
    let stage = match trailing {
        [] => tools.stage(std::path::Path::new(destination))?,
        [flag] if flag == "--check-existing" => {
            tools.inspect_stage(std::path::Path::new(destination))?
        }
        _ => return Err("unexpected stage inspection arguments".into()),
    };
    stage.recheck()?;
    println!(
        "{}",
        serde_json::json!({"kind":"sail-kani-tool-stage","version":1,"root":stage.root(),"driver":stage.driver(),"backend_executed":false,"proof_authority":false})
    );
    Ok(())
}
#[cfg(not(unix))]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    Err("error[SAIL6303]: unsupported native host".into())
}
fn main() -> std::process::ExitCode {
    match inspect() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}
