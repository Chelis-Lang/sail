//! Read-only descriptions of original solver checks for diagnostic replay.
//!
//! Descriptions authenticate no file, solver, session, function, source or proof.
//! The caller must bind the original invocation and retained input separately.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;
use serde_json::Value;

use super::{LogFile, ObligationKind, identity, spinoff_index, verus_functions};

const MAX_DOCUMENTS: usize = 128;
const MAX_LOGS: usize = 256;
const MAX_NODES: usize = 500_000;
const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_LABEL_BYTES: usize = 16 * 1024;
const MAX_CHECKS: usize = 100_000;
const MAX_OUTPUT_TEXT: usize = 8 * 1024 * 1024;
const MAX_ROW_TEXT: usize = 8 * 1024 * 1024;

/// A diagnostic description refusal, never a proof outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(&'static str);

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "error[SAIL5104]: diagnostic check binding: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// One original check position, without execution or proof authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckBinding {
    /// Exact public function label corroborated by the existing timing join.
    pub function: String,
    /// Original AIR obligation kind, never rewritten.
    pub kind: String,
    /// Original per-function/kind ordinal within this log; zero for a spinoff.
    pub ordinal: u32,
    /// Original spinoff index when the compiler supplied one.
    pub spinoff: Option<u32>,
    /// Zero-based check position within the obligation.
    pub check_in_obligation: u32,
    /// Exact original full-process input digest reported by the transcript.
    pub input_sha256: String,
    /// Zero-based check position in that complete process, not its reply index.
    pub session_check: u32,
}

fn bounded_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_LABEL_BYTES && !text.chars().any(char::is_control)
}

fn digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

// The existing row helper clones the containing module for every function.
// Bound its expanded representation before either identity or row collection.
fn preflight_rows(docs: &[Value]) -> Result<(), Error> {
    let mut rows = 0usize;
    let mut bytes = 0usize;
    for doc in docs {
        let modules = doc
            .pointer("/times-ms/smt/smt-run-module-times")
            .and_then(Value::as_array);
        for module in modules.into_iter().flatten() {
            let name = module.get("module").and_then(Value::as_str).unwrap_or("");
            let functions = module.get("function-breakdown").and_then(Value::as_array);
            for row in functions.into_iter().flatten() {
                if row.get("function").and_then(Value::as_str).is_none() {
                    continue;
                }
                rows += 1;
                if rows > MAX_CHECKS {
                    return Err(Error("timing row count exceeds its bound"));
                }
                let cost = ["function", "mode:", "function-path"]
                    .into_iter()
                    .filter_map(|field| row.get(field).and_then(Value::as_str))
                    .map(str::len)
                    .try_fold(name.len(), usize::checked_add)
                    .ok_or(Error("timing row text overflow"))?;
                bytes = bytes
                    .checked_add(cost)
                    .filter(|size| *size <= MAX_ROW_TEXT)
                    .ok_or(Error("expanded timing row text budget exceeded"))?;
            }
        }
    }
    Ok(())
}

fn preflight(docs: &[Value]) -> Result<(), Error> {
    if docs.is_empty() || docs.len() > MAX_DOCUMENTS {
        return Err(Error("unsupported document count"));
    }
    let mut stack: Vec<_> = docs.iter().filter_map(|doc| doc.get("times-ms")).collect();
    let mut nodes = 0usize;
    let mut text_bytes = 0usize;
    let mut text = |value: &str| -> Result<(), Error> {
        if value.len() > MAX_LABEL_BYTES {
            return Err(Error("timing label exceeds its bound"));
        }
        text_bytes = text_bytes
            .checked_add(value.len())
            .filter(|size| *size <= MAX_TEXT_BYTES)
            .ok_or(Error("timing text budget exceeded"))?;
        Ok(())
    };
    while let Some(value) = stack.pop() {
        nodes += 1;
        let remaining = MAX_NODES
            .checked_sub(nodes.saturating_add(stack.len()))
            .ok_or(Error("timing node budget exceeded"))?;
        match value {
            Value::Array(values) => {
                if values.len() > remaining {
                    return Err(Error("timing node budget exceeded"));
                }
                stack.extend(values);
            }
            Value::Object(values) => {
                if values.len() > remaining {
                    return Err(Error("timing node budget exceeded"));
                }
                for (key, value) in values {
                    text(key)?;
                    stack.push(value);
                }
            }
            Value::String(value) => text(value)?,
            _ => {}
        }
    }
    preflight_rows(docs)
}

#[derive(Default)]
struct Collection {
    bindings: Vec<CheckBinding>,
    checks: BTreeSet<(String, u32)>,
    obligations: BTreeSet<(String, String, u32, Option<u32>)>,
    text_bytes: usize,
}

impl Collection {
    fn log(
        &mut self,
        log: &LogFile,
        names: &identity::Names,
        occurrences: &BTreeMap<String, usize>,
    ) -> Result<(), Error> {
        if !digest(&log.input_sha256)
            || !bounded_text(&log.stem)
            || log.air.is_empty()
            || log.air.len() != log.ops.len()
        {
            return Err(Error("incomplete or unsupported process description"));
        }
        let spinoff = spinoff_index(&log.stem);
        let mut ordinals = BTreeMap::new();
        let mut session_check = 0u32;
        for (air, op) in log.air.iter().zip(&log.ops) {
            let query = names
                .query(air)
                .ok_or(Error("compiler query identity is unavailable"))?;
            if !bounded_text(query.function)
                || !bounded_text(query.kind)
                || !ObligationKind::is_query_kind(query.kind)
                || occurrences.get(query.function) != Some(&1)
                || op.checks.is_empty()
            {
                return Err(Error("missing or ambiguous function/check owner"));
            }
            let next = ordinals.entry((query.function, query.kind)).or_insert(0u32);
            let ordinal = if spinoff.is_some() { 0 } else { *next };
            *next = next.checked_add(1).ok_or(Error("ordinal overflow"))?;
            if !self.obligations.insert((
                query.function.to_owned(),
                query.kind.to_owned(),
                ordinal,
                spinoff,
            )) {
                return Err(Error("duplicate obligation description"));
            }
            for check in 0..op.checks.len() {
                let cost = query.function.len() + query.kind.len() + log.input_sha256.len();
                self.text_bytes = self
                    .text_bytes
                    .checked_add(cost)
                    .filter(|size| *size <= MAX_OUTPUT_TEXT)
                    .ok_or(Error("description text budget exceeded"))?;
                if self.bindings.len() >= MAX_CHECKS
                    || !self
                        .checks
                        .insert((log.input_sha256.clone(), session_check))
                {
                    return Err(Error("duplicate or over-budget process check"));
                }
                self.bindings.push(CheckBinding {
                    function: query.function.to_owned(),
                    kind: query.kind.to_owned(),
                    ordinal,
                    spinoff,
                    check_in_obligation: u32::try_from(check)
                        .map_err(|_| Error("check index overflow"))?,
                    input_sha256: log.input_sha256.clone(),
                    session_check,
                });
                session_check = session_check
                    .checked_add(1)
                    .ok_or(Error("process check index overflow"))?;
            }
        }
        Ok(())
    }
}

/// Describe complete-process check positions using existing identity rules.
///
/// Unknown legacy producers retain exact friendly-label matching, requiring one
/// timing row per function. Named producers require the original corroborated
/// AIR/timing paths. Mixed, missing, duplicated or conflicting evidence refuses
/// the entire collection. Sorting never rewrites input or invents ordinals.
/// No file is opened, solver run, report changed or proof/currentness granted.
///
/// # Errors
/// `SAIL5104` for unavailable joins or fixed document/log/node/text/check bounds.
pub fn collect(docs: &[Value], logs: &[LogFile]) -> Result<Vec<CheckBinding>, Error> {
    preflight(docs)?;
    if logs.is_empty() || logs.len() > MAX_LOGS {
        return Err(Error("unsupported log count"));
    }
    let names = identity::Names::new(docs);
    if names.error.is_some() {
        return Err(Error("compiler timing identities are unavailable"));
    }
    let mut occurrences = BTreeMap::new();
    for row in docs.iter().flat_map(verus_functions) {
        let name = names.row_name(&row);
        if !bounded_text(&name) {
            return Err(Error("unsupported timing function label"));
        }
        *occurrences.entry(name).or_insert(0usize) += 1;
    }
    if occurrences.is_empty() {
        return Err(Error("timing functions are unavailable"));
    }
    let mut collection = Collection::default();
    for log in logs {
        collection.log(log, &names, &occurrences)?;
    }
    collection.bindings.sort_by(|left, right| {
        (&left.input_sha256, left.session_check).cmp(&(&right.input_sha256, right.session_check))
    });
    Ok(collection.bindings)
}

#[cfg(test)]
mod tests;
