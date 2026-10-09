//! Owned Cargo targets and fresh verification for explicit measurements.

use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};

/// Preserve dependencies between measurements while cleaning only the verified
/// workspace packages. Never clean or overwrite the caller's Cargo target.
pub(crate) fn prepare(args: &[String], focus: bool) -> Result<(Vec<String>, File), String> {
    let split = args.iter().position(|a| a == "--").unwrap_or(args.len());
    if args[..split]
        .iter()
        .any(|a| a == "--target-dir" || a.starts_with("--target-dir="))
    {
        return Err(
            "fragility measurement owns its Cargo target; --target-dir is not supported".to_owned(),
        );
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    hash.update(cwd.as_os_str().as_encoded_bytes());
    // Include manifest selection, not changing Verus flags or source contents:
    // those must be able to reuse compilation, while verification always reruns.
    let mut it = args[..split].iter();
    while let Some(a) = it.next() {
        if a == "--manifest-path" {
            hash.update(a.as_bytes());
            if let Some(value) = it.next() {
                hash.update(value.as_bytes());
            }
        } else if a.starts_with("--manifest-path=") {
            hash.update(a.as_bytes());
        }
    }
    let dir = sail_verus::cache_dir()
        .map_err(|e| e.to_string())?
        .join("measurement")
        .join(format!("{:x}", hash.finalize()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ownership = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("cargo.lock"))
        .map_err(|e| e.to_string())?;
    ownership
        .try_lock()
        .map_err(|e| format!("measurement target is already in use or cannot be locked: {e}"))?;
    let target = dir.join("target");
    let mut prepared = args[..split].to_vec();
    prepared.extend([
        "--target-dir".to_owned(),
        target.to_string_lossy().into_owned(),
    ]);
    prepared.extend(args[split..].iter().cloned());
    crate::portfolio::force_verification(&prepared, &target, focus)?;
    Ok((prepared, ownership))
}
