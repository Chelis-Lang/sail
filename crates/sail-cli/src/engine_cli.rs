//! Explicit management of the optional source-integrated engine.
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    match command(args) {
        Ok(status) => match serde_json::to_writer(&mut *out, &status)
            .and_then(|()| writeln!(out).map_err(serde_json::Error::io))
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(err, "error[SAIL3120]: cannot write engine status: {error}");
                ExitCode::from(2)
            }
        },
        Err(error) => {
            let _ = writeln!(err, "{error}");
            ExitCode::from(2)
        }
    }
}
fn command(args: &[String]) -> Result<sail_verus::engine::Status, String> {
    let mut args = args.to_vec();
    if args.ends_with(&["--output".into(), "json".into()]) {
        args.truncate(args.len() - 2);
    }
    let cache = sail_verus::cache_dir().map_err(|error| error.to_string())?;
    let pins = sail_verus::toolchain::Pins::embedded().map_err(|error| error.to_string())?;
    match args.as_slice() {
        [command] if command == "status" => sail_verus::engine::status(&cache, &pins).map_err(|error| error.to_string()),
        [command, from, path] if command == "install" && from == "--from" => {
            sail_verus::engine::import(&cache, &pins, Path::new(path)).map_err(|error| error.to_string())
        }
        _ => Err("error[SAIL3120]: usage: sail engine status [--output json] | sail engine install --from PATH [--output json]; build with cargo xtask build-verus-engine".into()),
    }
}
