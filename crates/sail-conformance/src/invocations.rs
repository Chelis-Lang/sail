//! Spike prototype (#632, VI-0039): complete per-compiler-invocation output.
//!
//! Concurrent `cargo verus` workers each write a multi-line JSON document,
//! and Cargo forwards compiler stdout line by line, so the combined stdout
//! is not a JSON document stream. `sail-conformance-tee` records each
//! invocation's exact streams. This module:
//! - loads those records ([`load`]);
//! - proves the records account for the combined stdout: the combined
//!   stream must be an order-preserving interleaving of the records' lines,
//!   with no line left over on either side ([`explain`]);
//! - pairs the two arms' invocations by identity and applies the existing
//!   comparison to each pair ([`compare_arms`]).
//!
//! Nothing here normalizes or drops output. An unexplained stream, an
//! incomplete record or an unpaired invocation is a difference.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use sail_verus::run::RunRecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// What the tee records about one driver invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// `CARGO_PKG_NAME`.
    pub package_name: Option<String>,
    /// `CARGO_PKG_VERSION`.
    pub package_version: Option<String>,
    /// `CARGO_MANIFEST_DIR`.
    pub manifest_dir: Option<String>,
    /// `--crate-name`.
    pub crate_name: Option<String>,
    /// Every `--crate-type`, sorted.
    pub crate_types: Vec<String>,
    /// Whether `--test` was passed.
    pub test: bool,
    /// `CARGO_BIN_NAME`.
    pub bin_name: Option<String>,
    /// `--target`, if any.
    pub compile_target: Option<String>,
    /// SHA-256 over the arguments and the `__VERUS_DRIVER_*` environment.
    pub arg_sha256: String,
    /// The arguments (lossy UTF-8).
    pub argv: Vec<String>,
    /// The `__VERUS_DRIVER_*` environment.
    pub driver_env: BTreeMap<String, String>,
    /// Start time, nanoseconds since the epoch (record naming only).
    pub started_ns: u128,
    /// The tee's process id (record naming only).
    pub pid: u32,
    /// The driver's exit code (`None` if killed by a signal).
    pub exit_code: Option<i32>,
    /// Bytes the driver wrote to stdout.
    pub stdout_len: usize,
    /// Bytes the driver wrote to stderr.
    pub stderr_len: usize,
    /// SHA-256 of the stdout bytes.
    pub stdout_sha256: String,
    /// SHA-256 of the stderr bytes.
    pub stderr_sha256: String,
    /// The first error forwarding a stream, if any.
    pub forward_error: Option<String>,
}

/// Lower-case hex SHA-256.
#[must_use]
pub fn hex_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for byte in Sha256::digest(bytes) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The argument digest: every argument and every `__VERUS_DRIVER_*`
/// variable, length-prefixed so no two lists collide.
#[must_use]
pub fn argument_digest(argv: &[String], driver_env: &BTreeMap<String, String>) -> String {
    let mut bytes = Vec::new();
    let mut push = |s: &str| {
        bytes.extend_from_slice(&(s.len() as u64).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
    };
    push("sail-conformance-tee-v0");
    for arg in argv {
        push(arg);
    }
    push("env");
    for (key, value) in driver_env {
        push(key);
        push(value);
    }
    hex_sha256(&bytes)
}

/// One recorded invocation with its exact streams.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// Identity and status.
    pub meta: Meta,
    /// Exact stdout bytes.
    pub stdout: Vec<u8>,
    /// Exact stderr bytes.
    pub stderr: Vec<u8>,
}

/// The identity two arms are paired on: package, then target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Identity {
    /// Package name.
    pub package_name: String,
    /// Package version.
    pub package_version: String,
    /// The package's manifest directory.
    pub manifest_dir: String,
    /// Crate name.
    pub crate_name: String,
    /// Crate types, joined by `,`.
    pub crate_types: String,
    /// Test harness build.
    pub test: bool,
    /// Compile target (empty for the host).
    pub compile_target: String,
}

impl Invocation {
    /// Whether Cargo forwards this invocation's stdout: a compiler job for
    /// a package, as opposed to Cargo's own probes (`rustc -vV`, `--print`),
    /// which run without `CARGO_PKG_NAME`.
    #[must_use]
    pub fn forwarded(&self) -> bool {
        self.meta.package_name.is_some()
    }

    /// The pairing identity, or `None` for a probe with no crate (for
    /// example Cargo's `rustc -vV`).
    #[must_use]
    pub fn identity(&self) -> Option<Identity> {
        let m = &self.meta;
        Some(Identity {
            package_name: m.package_name.clone()?,
            package_version: m.package_version.clone().unwrap_or_default(),
            manifest_dir: m.manifest_dir.clone().unwrap_or_default(),
            crate_name: m.crate_name.clone()?,
            crate_types: m.crate_types.join(","),
            test: m.test,
            compile_target: m.compile_target.clone().unwrap_or_default(),
        })
    }
}

/// Loads every record under `dir`, in start order.
///
/// # Errors
///
/// A record the tee did not complete (`*.tmp`), an unreadable file, or
/// streams whose length or digest disagree with the record.
pub fn load(dir: &Path) -> Result<Vec<Invocation>, String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let path = dir.join(&name);
        if Path::new(&name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tmp"))
        {
            return Err(format!("incomplete capture {}", path.display()));
        }
        let read = |file: &str| {
            std::fs::read(path.join(file)).map_err(|e| format!("{}/{file}: {e}", path.display()))
        };
        let meta: Meta = serde_json::from_slice(&read("meta.json")?)
            .map_err(|e| format!("{}/meta.json: {e}", path.display()))?;
        let stdout = read("stdout")?;
        let stderr = read("stderr")?;
        if hex_sha256(&stdout) != meta.stdout_sha256 || hex_sha256(&stderr) != meta.stderr_sha256 {
            return Err(format!(
                "capture {} does not match its digests",
                path.display()
            ));
        }
        if let Some(error) = &meta.forward_error {
            return Err(format!(
                "capture {}: forwarding failed: {error}",
                path.display()
            ));
        }
        out.push(Invocation {
            meta,
            stdout,
            stderr,
        });
    }
    Ok(out)
}

/// The result of [`explain`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct Explanation {
    /// The combined stream is exactly an interleaving of the parts' lines.
    pub explained: bool,
    /// The search was abandoned (never a pass).
    pub undecided: bool,
    /// Lines in the combined stream.
    pub lines: usize,
    /// Non-empty parts.
    pub parts: usize,
    /// Parts that appear contiguously (byte for byte) in the combined stream.
    pub contiguous_parts: usize,
    /// Parts whose last line has no terminating newline.
    pub unterminated_parts: usize,
    /// Whether the combined stream's last line has no terminating newline.
    pub combined_unterminated: bool,
    /// The largest number of simultaneous candidate assignments.
    pub max_frontier: usize,
    /// Why the stream is not explained.
    pub failure: Option<String>,
}

fn lines(bytes: &[u8]) -> (Vec<&[u8]>, bool) {
    if bytes.is_empty() {
        return (Vec::new(), false);
    }
    let unterminated = bytes.last() != Some(&b'\n');
    let body = if unterminated {
        bytes
    } else {
        bytes.get(..bytes.len() - 1).unwrap_or_default()
    };
    (body.split(|b| *b == b'\n').collect(), unterminated)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    // Parts are whole documents; anchoring on the first line keeps this cheap.
    if needle.is_empty() || needle.len() > haystack.len() {
        return needle.is_empty();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .any(|(i, w)| (i == 0 || haystack.get(i - 1) == Some(&b'\n')) && w == needle)
}

/// Checks that `combined` is exactly an order-preserving interleaving of the
/// lines of `parts`: every combined line is the next unread line of some
/// part, and every part is read to its end. A torn line (a tear that is not
/// on a line boundary) matches no part's next line, so it fails here.
///
/// The search keeps every assignment still possible, so identical lines in
/// different parts cannot cause a false failure. Beyond `limit` simultaneous
/// candidates it gives up and reports `undecided`.
#[must_use]
pub fn explain(combined: &[u8], parts: &[&[u8]], limit: usize) -> Explanation {
    let mut e = Explanation::default();
    let (combined_lines, combined_unterminated) = lines(combined);
    e.lines = combined_lines.len();
    e.combined_unterminated = combined_unterminated;
    let mut ids: HashMap<&[u8], u32> = HashMap::new();
    let mut sequences: Vec<Vec<u32>> = Vec::new();
    let mut total = 0;
    for part in parts.iter().filter(|p| !p.is_empty()) {
        let (part_lines, unterminated) = lines(part);
        e.unterminated_parts += usize::from(unterminated);
        e.contiguous_parts += usize::from(contains(combined, part));
        total += part_lines.len();
        let mut sequence = Vec::with_capacity(part_lines.len());
        for line in part_lines {
            let next = u32::try_from(ids.len()).unwrap_or(u32::MAX);
            sequence.push(*ids.entry(line).or_insert(next));
        }
        sequences.push(sequence);
    }
    e.parts = sequences.len();
    if total != combined_lines.len() {
        e.failure = Some(format!(
            "combined stream has {} lines, the captures have {total}",
            combined_lines.len()
        ));
        return e;
    }
    let mut frontier: HashSet<Vec<u32>> = HashSet::new();
    frontier.insert(vec![0; sequences.len()]);
    e.max_frontier = 1;
    for (number, line) in combined_lines.iter().enumerate() {
        let Some(id) = ids.get(line) else {
            e.failure = Some(format!(
                "combined line {} is no line of any capture: {:?}",
                number + 1,
                String::from_utf8_lossy(line.get(..line.len().min(120)).unwrap_or_default())
            ));
            return e;
        };
        let mut next: HashSet<Vec<u32>> = HashSet::new();
        for state in &frontier {
            for (part, sequence) in sequences.iter().enumerate() {
                let position = state.get(part).copied().unwrap_or(u32::MAX) as usize;
                if sequence.get(position) == Some(id) {
                    let mut advanced = state.clone();
                    if let Some(slot) = advanced.get_mut(part) {
                        *slot += 1;
                    }
                    next.insert(advanced);
                }
            }
        }
        if next.is_empty() {
            e.failure = Some(format!(
                "combined line {} is not the next line of any capture: {:?}",
                number + 1,
                String::from_utf8_lossy(line.get(..line.len().min(120)).unwrap_or_default())
            ));
            return e;
        }
        e.max_frontier = e.max_frontier.max(next.len());
        if next.len() > limit {
            e.undecided = true;
            e.failure = Some(format!(
                "more than {limit} candidate assignments at combined line {}",
                number + 1
            ));
            return e;
        }
        frontier = next;
    }
    // Line counts are equal, so any surviving assignment reads every part
    // to its end.
    e.explained = true;
    e
}

/// Splits exact stdout bytes into JSON documents and the unparsed rest.
#[must_use]
pub fn documents(stdout: &[u8]) -> (Vec<Value>, String) {
    let mut stream = serde_json::Deserializer::from_slice(stdout).into_iter::<Value>();
    let mut docs = Vec::new();
    let mut end = 0;
    while let Some(Ok(doc)) = stream.next() {
        docs.push(doc);
        end = stream.byte_offset();
    }
    let rest = stdout.get(end..).unwrap_or_default();
    (docs, String::from_utf8_lossy(rest).trim().to_owned())
}

/// What one arm's records say about its own combined stream.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArmReport {
    /// Recorded invocations.
    pub invocations: usize,
    /// Cargo's own compiler probes (no package): Cargo reads their output
    /// itself and forwards none of it, so they are not parts of the stream.
    pub probes: usize,
    /// Invocations that wrote to stdout.
    pub with_stdout: usize,
    /// Invocations that wrote to stderr.
    pub with_stderr: usize,
    /// JSON documents recovered from the records.
    pub documents: usize,
    /// Records whose stdout is not a complete JSON document stream.
    pub incomplete_documents: usize,
    /// JSON documents the existing whole-stream parse finds.
    pub combined_documents: usize,
    /// Whether the existing whole-stream parse consumed the whole stream.
    pub combined_parses: bool,
    /// The interleaving check on stdout.
    pub stdout: Explanation,
}

/// Checks one arm: its records against its combined stdout.
#[must_use]
pub fn check_arm(invocations: &[Invocation], combined_stdout: &[u8], limit: usize) -> ArmReport {
    let mut report = ArmReport {
        invocations: invocations.len(),
        ..ArmReport::default()
    };
    let mut parts: Vec<&[u8]> = Vec::new();
    for invocation in invocations {
        if !invocation.forwarded() {
            report.probes += 1;
            continue;
        }
        report.with_stdout += usize::from(!invocation.stdout.is_empty());
        report.with_stderr += usize::from(!invocation.stderr.is_empty());
        let (docs, rest) = documents(&invocation.stdout);
        report.documents += docs.len();
        report.incomplete_documents += usize::from(!rest.is_empty());
        parts.push(invocation.stdout.as_slice());
    }
    let (docs, rest) = documents(combined_stdout);
    report.combined_documents = docs.len();
    report.combined_parses = rest.is_empty();
    report.stdout = explain(combined_stdout, &parts, limit);
    report
}

/// The comparison of two arms, invocation by invocation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArmsReport {
    /// Rule-one differences (any entry fails the unit).
    pub differences: Vec<String>,
    /// Invocation pairs compared.
    pub pairs: usize,
    /// Pairs with JSON documents.
    pub pairs_with_documents: usize,
    /// Functions compared, over all pairs.
    pub functions: usize,
    /// Pairs whose argument digests are equal.
    pub equal_arg_digests: usize,
    /// Pairs whose stdout bytes are equal.
    pub stdout_byte_equal: usize,
    /// Pairs whose stderr bytes are equal.
    pub stderr_byte_equal: usize,
    /// For pairs whose stdout bytes differ: how often each top-level
    /// document key differs.
    pub differing_keys: BTreeMap<String, usize>,
    /// Functions whose rlimit differs, over all pairs.
    pub rlimit_drift: usize,
    /// Package invocations in the plain arm that carry no crate identity
    /// and are therefore not paired (compiler probes run by build scripts).
    /// Their streams reach the build script, not Cargo's output.
    pub unidentified_plain: usize,
    /// The same count for the Sail arm; a different count is a difference.
    pub unidentified_sail: usize,
}

fn record(invocation: &Invocation) -> RunRecord {
    let (outputs, stdout_rest) = documents(&invocation.stdout);
    RunRecord {
        exit_code: invocation.meta.exit_code,
        stderr: String::from_utf8_lossy(&invocation.stderr).into_owned(),
        outputs,
        stdout_rest,
        wall_ms: 0.0,
        user_s: None,
        sys_s: None,
        max_rss_bytes: None,
    }
}

fn by_identity(invocations: &[Invocation]) -> BTreeMap<Identity, Vec<&Invocation>> {
    let mut map = BTreeMap::<_, Vec<_>>::new();
    for invocation in invocations {
        if let Some(identity) = invocation.identity() {
            map.entry(identity).or_default().push(invocation);
        }
    }
    map
}

/// Pairs the plain and Sail arms' invocations by identity and applies the
/// existing comparison ([`crate::compare`]) to each pair. Invocations
/// present on one side only, and identities that are not unique within an
/// arm and cannot be told apart by argument digest, are differences.
#[must_use]
pub fn compare_arms(plain: &[Invocation], sail: &[Invocation]) -> ArmsReport {
    let mut report = ArmsReport::default();
    let (p, s) = (by_identity(plain), by_identity(sail));
    let unidentified = |arm: &[Invocation]| {
        arm.iter()
            .filter(|i| i.forwarded() && i.identity().is_none())
            .count()
    };
    report.unidentified_plain = unidentified(plain);
    report.unidentified_sail = unidentified(sail);
    if report.unidentified_plain != report.unidentified_sail {
        report.differences.push(format!(
            "package invocations without a crate identity: plain {} / sail {}",
            report.unidentified_plain, report.unidentified_sail
        ));
    }
    for identity in s.keys().filter(|k| !p.contains_key(*k)) {
        report
            .differences
            .push(format!("invocation only in sail arm: {identity:?}"));
    }
    for (identity, plain_list) in &p {
        let Some(sail_list) = s.get(identity) else {
            report
                .differences
                .push(format!("invocation only in plain arm: {identity:?}"));
            continue;
        };
        let pairs: Vec<(&Invocation, &Invocation)> = if plain_list.len() == 1
            && sail_list.len() == 1
        {
            plain_list
                .iter()
                .copied()
                .zip(sail_list.iter().copied())
                .collect()
        } else {
            // Repeated identity: only an exact argument digest tells them apart.
            let mut pairs = Vec::new();
            let mut unmatched = plain_list.len() != sail_list.len();
            for a in plain_list {
                let matches: Vec<_> = sail_list
                    .iter()
                    .filter(|b| b.meta.arg_sha256 == a.meta.arg_sha256)
                    .collect();
                match matches.as_slice() {
                    [b] => pairs.push((*a, **b)),
                    _ => unmatched = true,
                }
            }
            if unmatched {
                report.differences.push(format!(
                    "repeated invocation identity cannot be paired by argument digest: {identity:?} (plain {} / sail {})",
                    plain_list.len(),
                    sail_list.len()
                ));
            }
            pairs
        };
        for (a, b) in pairs {
            report.pairs += 1;
            report.equal_arg_digests += usize::from(a.meta.arg_sha256 == b.meta.arg_sha256);
            report.stdout_byte_equal += usize::from(a.stdout == b.stdout);
            report.stderr_byte_equal += usize::from(a.stderr == b.stderr);
            let (ra, rb) = (record(a), record(b));
            report.pairs_with_documents += usize::from(!ra.outputs.is_empty());
            if a.stdout != b.stdout && ra.outputs.len() == rb.outputs.len() {
                for (x, y) in ra.outputs.iter().zip(&rb.outputs) {
                    let keys: HashSet<&String> = x
                        .as_object()
                        .into_iter()
                        .chain(y.as_object())
                        .flat_map(|o| o.keys())
                        .collect();
                    for key in keys {
                        if x.get(key) != y.get(key) {
                            *report.differing_keys.entry(key.clone()).or_default() += 1;
                        }
                    }
                }
            }
            let comparison = crate::compare(&ra, &rb);
            report.functions += comparison.functions;
            report.rlimit_drift += comparison.rlimit_drift.len();
            for difference in comparison.differences {
                report.differences.push(format!(
                    "{} {} [{}{}]: {difference}",
                    identity.package_name,
                    identity.crate_name,
                    identity.crate_types,
                    if identity.test { ", test" } else { "" }
                ));
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::explain;

    #[test]
    fn whole_documents_in_any_order_are_explained() {
        let (a, b) = (
            b"{\n \"a\": 1\n}\n".as_slice(),
            b"{\n \"b\": 2\n}\n".as_slice(),
        );
        let e = explain(b"{\n \"b\": 2\n}\n{\n \"a\": 1\n}\n", &[a, b], 1000);
        assert!(e.explained, "{e:?}");
        assert_eq!(e.contiguous_parts, 2);
    }

    #[test]
    fn a_line_interleaving_is_explained_despite_identical_lines() {
        let (a, b) = (
            b"{\n \"a\": 1\n}\n".as_slice(),
            b"{\n \"b\": 2\n}\n".as_slice(),
        );
        let e = explain(b"{\n{\n \"b\": 2\n \"a\": 1\n}\n}\n", &[a, b], 1000);
        assert!(e.explained, "{e:?}");
        assert_eq!(e.contiguous_parts, 0);
    }

    #[test]
    fn a_tear_inside_a_line_is_not_explained() {
        let (a, b) = (
            b"{\n \"a\": 1\n}\n".as_slice(),
            b"{\n \"b\": 2\n}\n".as_slice(),
        );
        let e = explain(b"{\n \"a\"{\n \"b\": 2\n}\n: 1\n}\n", &[a, b], 1000);
        assert!(!e.explained && !e.undecided, "{e:?}");
    }

    #[test]
    fn a_reordered_line_within_one_capture_is_not_explained() {
        let a = b"{\n \"a\": 1,\n \"b\": 2\n}\n".as_slice();
        let e = explain(b"{\n \"b\": 2\n \"a\": 1,\n}\n", &[a], 1000);
        assert!(!e.explained, "{e:?}");
    }

    #[test]
    fn missing_and_extra_lines_are_not_explained() {
        let a = b"{\n \"a\": 1\n}\n".as_slice();
        assert!(!explain(b"{\n \"a\": 1\n", &[a], 1000).explained);
        assert!(!explain(b"{\n \"a\": 1\n}\n}\n", &[a], 1000).explained);
        assert!(!explain(b"{\n \"a\": 1\n}\n", &[a, a], 1000).explained);
    }

    #[test]
    fn exceeding_the_candidate_limit_is_undecided_never_explained() {
        let part = b"x\nx\nx\nx\n".as_slice();
        let e = explain(
            b"x\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\n",
            &[part, part, part],
            3,
        );
        assert!(e.undecided && !e.explained, "{e:?}");
    }
}
