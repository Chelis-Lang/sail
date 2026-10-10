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
fn command(args: &[String]) -> sail_verus::Result<sail_verus::engine::Status> {
    let mut args = args.to_vec();
    if args.ends_with(&["--output".into(), "json".into()]) {
        args.truncate(args.len() - 2);
    }
    let cache = sail_verus::cache_dir()?;
    let pins = sail_verus::toolchain::Pins::embedded()?;
    match args.as_slice() {
        [command] if command == "status" => sail_verus::engine::status(&cache, &pins),
        [command, from, path] if command == "install" && from == "--from" => {
            sail_verus::engine::import(&cache, &pins, Path::new(path))
        }
        _ => Err(sail_verus::Error::new(
            "SAIL3120",
            "usage: sail engine status [--output json] | sail engine install --from PATH [--output json]; build with cargo xtask build-verus-engine",
        )),
    }
}
