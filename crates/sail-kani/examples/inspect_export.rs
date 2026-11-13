//! Contributor-only native export inspection; no execution or proof receipt.
use std::path::Path;

fn main() -> std::process::ExitCode {
    match inspect() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}

fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [harness, exit, stdout, export] = args.as_slice() else {
        return Err("usage: inspect_export HARNESS EXIT STDOUT EXPORT".into());
    };
    let observation = sail_kani::decode(
        harness,
        exit.parse()?,
        &std::fs::read(Path::new(stdout))?,
        &std::fs::read(Path::new(export))?,
    )?;
    println!("{}", serde_json::to_string(&observation)?);
    Ok(())
}
