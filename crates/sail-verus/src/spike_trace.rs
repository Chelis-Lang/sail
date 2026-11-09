//! SPIKE (v0.3.0 Track D, #696/#683): per-predicate reason tracing.
//!
//! Prototype only. When the directory `/tmp/sail-spike` exists, every site
//! that can lower a result to `Unknown` appends one JSON line to
//! `/tmp/sail-spike/trace.jsonl`. A fixed path instead of an environment
//! variable keeps the proof-context key and the forwarded environment
//! untouched. Nothing is written otherwise.

use std::io::Write as _;

/// Directory whose existence enables tracing.
pub const DIR: &str = "/tmp/sail-spike";

/// Whether tracing is enabled.
#[must_use]
pub fn enabled() -> bool {
    std::path::Path::new(DIR).is_dir()
}

/// Appends one line naming `site` with `fields`.
pub fn trace(site: &str, fields: &serde_json::Value) {
    if !enabled() {
        return;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let line = format!(
        "{}\n",
        serde_json::json!({
            "t_ns": nanos.to_string(),
            "pid": std::process::id(),
            "site": site,
            "fields": fields,
        })
    );
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{DIR}/trace.jsonl"))
    {
        let _ = file.write_all(line.as_bytes());
    }
}
