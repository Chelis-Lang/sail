//! Versioned raw-query runtime, separate from Cargo verification commands.
use sail_api::runtime::{Answer, Outcome, Output, Request};
use sail_runtime::{Cancellation, Config, Runtime};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Bound the JSON envelope as well as the decoded script lengths.
const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    match command(args, out) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            let _ = writeln!(err, "{message}");
            ExitCode::from(2)
        }
    }
}

fn command(args: &[String], out: &mut dyn Write) -> Result<u8, String> {
    let (action, file, cache) = options(args)?;
    // Resolve Sail's existing runtime, never search PATH or install another solver.
    // Absence stays a structured backend capability/result, not a skipped query.
    let cache_root = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let pins = sail_verus::toolchain::Pins::embedded().map_err(|e| e.to_string())?;
    let z3 = sail_verus::engine::Installed::locate(&cache_root, &pins)
        .ok()
        .and_then(|installed| installed.verus.parent().map(|dir| dir.join("z3")));
    let runtime = Runtime::new(Config {
        z3,
        cache_directory: Some(cache.unwrap_or_else(|| cache_root.join("runtime"))),
        workers: 2,
    })
    .map_err(|e| e.to_string())?;
    if action == "capabilities" {
        write_json(out, &runtime.capabilities())?;
        return Ok(0);
    }
    let request = read_request(file.as_deref().ok_or_else(usage)?)?;
    let cancellation = Cancellation::default();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())
        .map_err(|e| format!("error[SAIL6102]: cannot install cancellation handler: {e}"))?;
    let response = runtime
        .execute(&request, &cancellation)
        .map_err(|e| e.to_string())?;
    let Output::Smt { results } = &response.output;
    let complete = results.iter().all(|result| {
        result.outcome == Outcome::Completed
            && result
                .checks
                .iter()
                .all(|check| check.answer != Answer::Unknown)
    });
    write_json(out, &response)?;
    Ok(u8::from(!complete))
}

fn usage() -> String {
    "error[SAIL6101]: usage: sail runtime capabilities [--cache DIR] | sail runtime execute --file FILE|- [--cache DIR]".into()
}

fn options(args: &[String]) -> Result<(&str, Option<PathBuf>, Option<PathBuf>), String> {
    let action = args.first().map(String::as_str).ok_or_else(usage)?;
    if !matches!(action, "capabilities" | "execute") {
        return Err(usage());
    }
    let mut file = None;
    let mut cache = None;
    let mut options = args[1..].iter();
    while let Some(option) = options.next() {
        let value = options.next().ok_or_else(usage)?;
        match option.as_str() {
            "--file" if action == "execute" && file.is_none() => file = Some(value.into()),
            "--cache" if cache.is_none() => cache = Some(value.into()),
            _ => return Err(usage()),
        }
    }
    if action == "execute" && file.is_none() {
        return Err(usage());
    }
    Ok((action, file, cache))
}

fn read_request(path: &Path) -> Result<Request, String> {
    let input: Box<dyn Read> = if path == Path::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).map_err(|e| {
            format!(
                "error[SAIL6101]: cannot read runtime request {}: {e}",
                path.display()
            )
        })?)
    };
    decode_request(input)
}

fn decode_request(input: impl Read) -> Result<Request, String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("error[SAIL6101]: cannot read runtime request: {e}"))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("error[SAIL6101]: runtime JSON exceeds 128 MiB".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("error[SAIL6101]: invalid runtime JSON: {e}"))
}

fn write_json(out: &mut dyn Write, value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *out, value)
        .and_then(|()| writeln!(out).map_err(serde_json::Error::io))
        .map_err(|e| format!("error[SAIL6102]: cannot write runtime response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_rejects_unknown_options_and_malformed_documents() {
        for args in [
            vec![],
            vec!["execute"],
            vec!["capabilities", "--file", "x"],
            vec!["execute", "--file", "x", "--file", "y"],
            vec!["execute", "--file", "x", "--cache"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(decode_request(&b"{}"[..]).is_err());
        assert!(
            decode_request(
                &b"{\"version\":1,\"input\":{\"kind\":\"smt\",\"queries\":[]},\"extra\":true}"[..]
            )
            .is_err()
        );
    }
}
