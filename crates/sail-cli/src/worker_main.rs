//! One isolated native job. The parent owns process lifetime and byte limits.
#![forbid(unsafe_code)]
use std::io::{Read as _, Write as _};

/// mimalloc, as in the D.I.V.E. CLI: state-space exploration allocates
/// heavily, and the system allocator made checks 3–16% slower (#765).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
    let mut args = std::env::args_os().skip(1);
    if let Some(mode) = args.next() {
        if args.next().is_some() {
            return Err("unknown worker arguments".into());
        }
        return match mode.to_str() {
            Some("--edit-session") => edit_session(&input),
            Some("--project-invariants") => {
                let query: sail_dive::edits::projection::Query =
                    sail_api::temporal::decode(&input)?;
                write_relation(&query.compare()?)
            }
            Some("--compare-loaded-modules") => {
                let query: sail_dive::edits::loaded::Query = sail_api::temporal::decode(&input)?;
                write_relation(&query.compare()?)
            }
            Some("--project-first-temporal-claim") => {
                let query: sail_dive::edits::first_claim::Query =
                    sail_api::temporal::decode(&input)?;
                write_relation(&query.compare()?)
            }
            _ => Err("unknown worker mode".into()),
        };
    }
    let request = sail_dive::decode_request(&input)?;
    let response = sail_dive::execute(&request);
    // Serializing into the bounded frame stops at the bound instead of first
    // materializing an oversized response (#767).
    let mut frame = Frame {
        bytes: Vec::new(),
        limit: sail_dive::MAX_DOCUMENT_BYTES,
    };
    serde_json::to_writer(&mut frame, &response)?;
    std::io::stdout().write_all(&frame.bytes)?;
    Ok(())
}

fn write_relation(relation: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = Frame {
        bytes: Vec::new(),
        limit: sail_dive::MAX_DOCUMENT_BYTES,
    };
    serde_json::to_writer(&mut frame, relation)?;
    std::io::stdout().write_all(&frame.bytes)?;
    Ok(())
}

/// Hold the complete frame until every entry has been checked. Serializing each
/// entry into the bounded frame avoids retaining all traces alongside the frame.
struct Frame {
    bytes: Vec<u8>,
    limit: usize,
}

impl std::io::Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "worker evidence exceeds its byte bound",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn edit_session(input: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use sail_dive::edits::{BACKEND_SOURCE, Batch, Entry, Evidence, Session};
    let batch: Batch = sail_api::temporal::decode(input)?;
    batch.admit()?;
    let mut session = Session::default();
    let mut frame = Frame {
        bytes: Vec::new(),
        limit: sail_dive::MAX_DOCUMENT_BYTES,
    };
    frame.write_all(br#"{"backend_source":"#)?;
    serde_json::to_writer(&mut frame, BACKEND_SOURCE)?;
    frame.write_all(br#","result":{"version":1,"entries":["#)?;
    // The batch owns the original payloads and the bounded frame owns the original
    // evidence. Indexes and digests suffice even when a trace is large.
    let mut completed: Vec<(usize, String)> = Vec::new();
    for (index, request) in batch.checks.iter().enumerate() {
        if index != 0 {
            frame.write_all(b",")?;
        }
        let started = std::time::Instant::now();
        let entry = if let Some((basis, digest)) = completed
            .iter()
            .find(|(prior, _)| &batch.checks[*prior] == request)
        {
            Entry {
                request_sha256: digest.clone(),
                elapsed_us: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
                evidence: Evidence::ExactRequest {
                    basis_entry: *basis,
                },
            }
        } else {
            let entry = session.check(request)?;
            if entry.evidence.completed_ordinary() {
                completed.push((index, entry.request_sha256.clone()));
            }
            entry
        };
        serde_json::to_writer(&mut frame, &entry)?;
    }
    frame.write_all(b"]}}")?;
    std::io::stdout().write_all(&frame.bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_bound_precedes_byte_growth() -> std::io::Result<()> {
        let mut frame = Frame {
            bytes: Vec::new(),
            limit: 5,
        };
        frame.write_all(b"123")?;
        assert!(frame.write_all(b"456").is_err());
        assert_eq!(frame.bytes, b"123");
        frame.write_all(b"45")?;
        assert!(frame.write_all(b"6").is_err());
        assert_eq!(frame.bytes, b"12345");
        Ok(())
    }
}
