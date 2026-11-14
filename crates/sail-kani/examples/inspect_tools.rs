//! Contributor-only actual tool-payload inspection; no backend execution.
#[cfg(unix)]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [bundle, rust] = args.as_slice() else {
        return Err("usage: inspect_tools KANI_BUNDLE RUST_PREFIX".into());
    };
    let tools = sail_kani::tools::Tools::authenticate(
        std::path::Path::new(bundle),
        std::path::Path::new(rust),
    )?;
    tools.recheck()?;
    println!(
        "{}",
        serde_json::json!({
            "kind":"sail-kani-tool-payload-observation", "version":1,
            "bundle":tools.bundle(), "rust":tools.rust(),
            "rust_tree_sha256":tools.rust_tree_sha256(), "backend_executed":false,
            "proof_authority":false,
        })
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
