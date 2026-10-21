//! Namespace-separated exact solver records using the existing disk cache.

use crate::{Config, admission, hash};
use sail_api::runtime::{Answer, Check, Limits, Query, SolverIdentity};
use sail_cache::{DiskCache, StoreContext};
use sail_key::QueryKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{io::Read as _, path::Path};

const NAMESPACE: &str = "runtime-smt-v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    script_sha256: String,
    solver: SolverIdentity,
    limits: Limits,
    stdout: Vec<u8>,
}

pub(crate) fn file_hash(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0_u8; 8192];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        digest.update(&bytes[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn key(query: &Query, solver: &SolverIdentity) -> Option<QueryKey> {
    let content = serde_json::to_vec(&(
        NAMESPACE,
        &query.script,
        solver,
        &query.limits,
        "smt.random_seed=0",
        "sat.random_seed=0",
        "sail-process-env-v1:clear:LC_ALL=C",
    ))
    .ok()?;
    Some(QueryKey(sail_key::sha256(&content)))
}

pub(crate) fn read(
    config: &Config,
    query: &Query,
    solver: &SolverIdentity,
    plan: &admission::Plan,
) -> Option<Vec<u8>> {
    let cache = DiskCache::new(config.cache_directory.as_ref()?);
    let query_key = key(query, solver)?;
    // The cache is optional input too: bound the encoded JSON byte-array overhead
    // before decoding, rather than reading an arbitrarily large corrupt record.
    let bound = query
        .limits
        .output_bytes
        .checked_mul(4)?
        .checked_add(65_536)?;
    let mut bytes = Vec::new();
    std::fs::File::open(cache.entry_path(&query_key))
        .ok()?
        .take(u64::try_from(bound).ok()?.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > bound {
        return None;
    }
    let (meta, bytes) = sail_cache::parse_entry(&bytes, &query_key.to_string()).ok()?;
    if meta.mode != NAMESPACE
        || meta.command != NAMESPACE
        || meta.solver != serde_json::to_string(solver).ok()?
        || meta.provenance != "pinned-solver"
    {
        return None;
    }
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    if record.version != 1
        || record.script_sha256 != hash(query.script.as_bytes())
        || &record.solver != solver
        || record.limits != query.limits
        || record.stdout.len() > query.limits.output_bytes
    {
        return None;
    }
    let checks = admission::responses(plan, &record.stdout).ok()?;
    if checks.iter().any(|check| check.answer == Answer::Unknown) {
        return None;
    }
    Some(record.stdout)
}

pub(crate) fn write(
    config: &Config,
    query: &Query,
    solver: &SolverIdentity,
    stdout: &[u8],
    checks: &[Check],
) {
    if checks.iter().any(|check| check.answer == Answer::Unknown) {
        return;
    }
    let Some(directory) = &config.cache_directory else {
        return;
    };
    let Some(key) = key(query, solver) else {
        return;
    };
    let record = Record {
        version: 1,
        script_sha256: hash(query.script.as_bytes()),
        solver: solver.clone(),
        limits: query.limits.clone(),
        stdout: stdout.to_vec(),
    };
    let Ok(bytes) = serde_json::to_vec(&record) else {
        return;
    };
    let Ok(identity) = serde_json::to_string(solver) else {
        return;
    };
    let context = StoreContext {
        solver: &identity,
        provenance: "pinned-solver",
        mode: NAMESPACE,
    };
    let _ = DiskCache::new(directory).put_scoped(&key, &bytes, NAMESPACE, &context, None);
}
