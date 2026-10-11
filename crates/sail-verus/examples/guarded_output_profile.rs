//! Output-seal component comparison on a complete retained report. This is not
//! a live RPC/reader benchmark and establishes no input Currentness or verdict.

use sail_verus::managed::GuardedOutput;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{self, Read as _};
use std::path::Path;
use std::sync::{Arc, Barrier, Mutex, atomic::AtomicBool};
use std::time::Instant;

const SAMPLES: usize = 16;

fn seal(path: &Path) -> io::Result<(String, usize)> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(sail_api::jobs::MAX_FRAME as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > sail_api::jobs::MAX_FRAME {
        return Err(io::Error::other(
            "SAIL0814: output exceeds complete frame limit",
        ));
    }
    Ok((format!("{:x}", Sha256::digest(&bytes)), bytes.len()))
}

fn measure(
    path: &Path,
    expected: &str,
    owner: &Mutex<GuardedOutput>,
    clients: usize,
    guarded: bool,
) -> io::Result<serde_json::Value> {
    let start = Arc::new(Barrier::new(clients));
    let rows: Vec<_> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..clients {
            let start = Arc::clone(&start);
            handles.push(scope.spawn(move || {
                start.wait();
                let mut rows = Vec::new();
                for _ in 0..SAMPLES {
                    let clock = Instant::now();
                    // The prospective complete output operation includes both
                    // namespace boundaries and all helper/kernel barriers.
                    // Busy/failed optional coverage takes the original full SHA.
                    let hit = guarded
                        && owner
                            .try_lock()
                            .is_ok_and(|mut owner| owner.namespace_barrier() && owner.clean());
                    if !hit && seal(path)?.0 != expected {
                        return Err(io::Error::other("SAIL0816: retained output differs"));
                    }
                    rows.push(json!({"wall_us":clock.elapsed().as_micros(), "guarded_hit":hit}));
                }
                Ok(rows)
            }));
        }
        let mut rows = Vec::new();
        for handle in handles {
            rows.extend(
                handle
                    .join()
                    .map_err(|_| io::Error::other("SAIL0806: profile worker failed"))??,
            );
        }
        Ok::<_, io::Error>(rows)
    })?;
    Ok(json!({"clients":clients,"guarded":guarded,"samples_per_client":SAMPLES,"samples":rows}))
}

fn main() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or_else(|| io::Error::other("SAIL0806: expected complete retained output path"))?;
    if args.next().is_some() {
        return Err(io::Error::other("SAIL0806: unexpected profile argument"));
    }
    let path = Path::new(&path);
    let before = seal(path)?;
    let clock = Instant::now();
    let owner = GuardedOutput::begin(path, &before.0, Arc::new(AtomicBool::new(false)))
        .ok_or_else(|| io::Error::other("SAIL3114: optional output coverage unavailable"))?;
    let construction_us = clock.elapsed().as_micros();
    let retained_bytes = owner.retained_bytes();
    let owner = Mutex::new(owner);
    let mut comparisons = Vec::new();
    for clients in [1, 4] {
        // Fixed order is fully retained; no best-of/retry selection.
        comparisons.push(measure(path, &before.0, &owner, clients, false)?);
        comparisons.push(measure(path, &before.0, &owner, clients, true)?);
    }
    let after = seal(path)?;
    if after != before {
        return Err(io::Error::other(
            "SAIL0816: output seal changed during profile",
        ));
    }
    println!(
        "{}",
        json!({"kind":"guarded-output-component-profile","construction_us":construction_us,
        "requested_retained_bytes":retained_bytes,"output_bytes":before.1,"output_sha256":before.0,
        "comparisons":comparisons,"output_seal_unchanged":true,
        "live_rpc_or_current_proof_acceptance":false,"daemon_path_enabled":false})
    );
    Ok(())
}
