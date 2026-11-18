//! One isolated native job. The parent owns process lifetime and byte limits.
#![forbid(unsafe_code)]
use std::io::{Read as _, Write as _};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("SAIL4210: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    std::io::stdin()
        .take(u64::try_from(sail_dive::MAX_DOCUMENT_BYTES)? + 1)
        .read_to_end(&mut input)?;
    if input.len() > sail_dive::MAX_DOCUMENT_BYTES {
        return Err("worker request exceeds its byte bound".into());
    }
    let request = sail_dive::decode_request(&input)?;
    let response = sail_dive::execute(&request);
    let bytes = serde_json::to_vec(&response)?;
    if bytes.len() > sail_dive::MAX_DOCUMENT_BYTES {
        return Err("worker evidence exceeds its byte bound".into());
    }
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
