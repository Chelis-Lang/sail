//! Contributor-only actual tool-payload inspection; no backend execution.
#[cfg(unix)]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "arguments must be valid UTF-8")?;
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
            "bundle":tools.bundle().to_str().ok_or("non-UTF-8 canonical bundle path")?, "rust":tools.rust().to_str().ok_or("non-UTF-8 canonical Rust path")?,
            "rust_tree_sha256":tools.rust_tree_sha256(), "backend_executed":false,
            "proof_authority":false,
        })
    );
    Ok(())
}

#[cfg(not(unix))]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    Err("unsupported native host".into())
}

fn main() -> std::process::ExitCode {
    match inspect() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            #[cfg(unix)]
            if error.is::<sail_kani::tools::Error>() {
                eprintln!("{error}");
                return std::process::ExitCode::from(2);
            }
            eprintln!("error[SAIL6303]: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
