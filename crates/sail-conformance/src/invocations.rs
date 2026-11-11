//! Per-compiler-invocation capture (#632, VI-0039): complete per-compiler-invocation output.
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

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// What the tee records about one driver invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// Capture format version.
    pub version: u32,
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
    /// Original arguments (non-UTF-8 metadata makes the capture invalid).
    pub argv: Vec<String>,
    /// The `__VERUS_DRIVER_*` environment.
    pub driver_env: BTreeMap<String, String>,
    /// Source checkout root (identity only; never changes recorded output).
    pub source_root: String,
    /// Fresh target directory (identity only).
    pub target_root: String,
    /// Original driver directory (identity only).
    pub runtime_root: String,
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
    push("sail-conformance-tee-v1");
    push(&argv.len().to_string());
    for arg in argv {
        push(arg);
    }
    push("env");
    push(&driver_env.len().to_string());
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
    /// Binary target name, if Cargo supplied it.
    pub bin_name: Option<String>,
    /// Every argument, with declared path roots made relative. Build-script
    /// probes can have a crate name too; arguments preserve their association.
    pub arguments: Vec<String>,
}

impl Invocation {
    /// Whether this is a pinned Cargo compiler job. Cargo requests JSON
    /// diagnostic/artifact messages for these jobs, including build-script
    /// compilation. A build script's own probes inherit package variables,
    /// so package or crate names alone cannot establish this distinction.
    #[must_use]
    pub fn forwarded(&self) -> bool {
        self.meta.package_name.is_some()
            && self.meta.argv.iter().any(|a| a == "--error-format=json")
            && self.meta.argv.iter().any(|a| {
                a.strip_prefix("--json=")
                    .is_some_and(|v| v.split(',').any(|v| v == "artifacts"))
            })
    }

    fn is_probe(&self) -> bool {
        !self.forwarded()
    }

    fn known_probe_output(&self) -> bool {
        self.meta.argv.iter().any(|a| {
            a == "--print"
                || a.starts_with("--print=")
                || a == "-vV"
                || a == "--version"
                || a.starts_with("-Zunpretty=")
        })
    }

    /// Package/target identity and complete argument association.
    #[must_use]
    pub fn identity(&self) -> Identity {
        let m = &self.meta;
        Identity {
            package_name: m.package_name.clone().unwrap_or_default(),
            package_version: m.package_version.clone().unwrap_or_default(),
            manifest_dir: m
                .manifest_dir
                .as_ref()
                .map(|p| {
                    Path::new(p)
                        .strip_prefix(&m.source_root)
                        .unwrap_or(Path::new(p))
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default(),
            crate_name: m.crate_name.clone().unwrap_or_default(),
            crate_types: m.crate_types.join(","),
            test: m.test,
            compile_target: m.compile_target.clone().unwrap_or_default(),
            bin_name: m.bin_name.clone(),
            arguments: m.argv.iter().map(|arg| relative_argument(arg, m)).collect(),
        }
    }
}

fn relative_argument(arg: &str, meta: &Meta) -> String {
    for (label, root) in [
        ("source", &meta.source_root),
        ("target", &meta.target_root),
        ("runtime", &meta.runtime_root),
    ] {
        if !root.is_empty()
            && let Ok(relative) = Path::new(arg).strip_prefix(root)
        {
            return format!("<{label}>/{}", relative.display());
        }
    }
    if let Some((key, value)) = arg.split_once('=') {
        let relative = relative_argument(value, meta);
        return format!("{key}={relative}");
    }
    arg.to_owned()
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
        if meta.version != 1 || meta.source_root.is_empty() {
            return Err(format!("unsupported or unbound capture {}", path.display()));
        }
        let stdout = read("stdout")?;
        std::str::from_utf8(&stdout).map_err(|e| format!("capture stdout is not UTF-8: {e}"))?;
        let stderr = read("stderr")?;
        if stdout.len() != meta.stdout_len
            || stderr.len() != meta.stderr_len
            || hex_sha256(&stdout) != meta.stdout_sha256
            || hex_sha256(&stderr) != meta.stderr_sha256
            || argument_digest(&meta.argv, &meta.driver_env) != meta.arg_sha256
        {
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
    (
        bytes.split_inclusive(|b| *b == b'\n').collect(),
        unterminated,
    )
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

/// A run and the complete compiler captures that justify its Cargo output.
#[derive(Debug)]
pub struct CapturedRun {
    /// Original aggregate result, retained without rewriting its streams.
    pub record: sail_verus::run::RunRecord,
    /// Every invocation, including probes; absent for direct Verus units.
    pub invocations: Option<Vec<Invocation>>,
}

impl CapturedRun {
    /// Structured results for totals, engine seal checks and focus selection.
    /// The exact original aggregate streams remain in `record`; the comparison
    /// uses paired invocations, never this completion-order-independent view.
    #[must_use]
    pub fn summary(&self) -> sail_verus::run::RunRecord {
        let mut record = self.record.clone();
        if let Some(invocations) = &self.invocations {
            record.outputs = grouped(invocations)
                .into_values()
                .flatten()
                .flat_map(|i| invocation_record(i).outputs)
                .collect();
        }
        record
    }
}

/// Runs a unit, requiring complete captures for Cargo units.
///
/// # Errors
///
/// A run failure, unreadable/incomplete capture, or unexplained aggregate stdout.
pub fn run(
    installed: &sail_verus::toolchain::Installed,
    unit: &sail_verus::corpus::Unit,
    solver: &sail_verus::run::Solver,
    work: &Path,
    tee: &Path,
) -> Result<CapturedRun, String> {
    let record = sail_verus::run::run_unit_captured(installed, unit, solver, work, tee)
        .map_err(|e| e.to_string())?;
    let invocations = if unit.build == sail_verus::corpus::Build::CargoVerus {
        let invocations = load(&work.join("invocations"))?;
        if invocations.is_empty() {
            return Err("error[SAIL3018]: no compiler invocations captured".to_owned());
        }
        if invocations
            .iter()
            .any(|i| !i.forwarded() && !i.stdout.is_empty() && !i.known_probe_output())
        {
            return Err("error[SAIL3018]: stdout from an unrecognized compiler invocation; capture cannot establish Cargo forwarding".to_owned());
        }
        let stdout = std::fs::read(work.join("stdout.txt")).map_err(|e| e.to_string())?;
        let parts: Vec<&[u8]> = invocations
            .iter()
            .filter(|i| i.forwarded())
            .map(|i| i.stdout.as_slice())
            .collect();
        let explanation = explain(&stdout, &parts, 50_000);
        let json = serde_json::to_vec_pretty(&explanation).map_err(|e| e.to_string())?;
        std::fs::write(work.join("capture-check.json"), json).map_err(|e| e.to_string())?;
        if !explanation.explained {
            return Err(format!(
                "error[SAIL3018]: aggregate stdout is not explained: {:?}",
                explanation.failure
            ));
        }
        Some(invocations)
    } else {
        None
    };
    Ok(CapturedRun {
        record,
        invocations,
    })
}

fn invocation_record(invocation: &Invocation) -> sail_verus::run::RunRecord {
    let mut stream =
        serde_json::Deserializer::from_slice(&invocation.stdout).into_iter::<serde_json::Value>();
    let mut outputs = Vec::new();
    let mut end = 0;
    while let Some(Ok(doc)) = stream.next() {
        outputs.push(doc);
        end = stream.byte_offset();
    }
    sail_verus::run::RunRecord {
        exit_code: invocation.meta.exit_code,
        stderr: String::from_utf8_lossy(&invocation.stderr).into_owned(),
        outputs,
        stdout_rest: String::from_utf8_lossy(invocation.stdout.get(end..).unwrap_or_default())
            .into_owned(),
        wall_ms: 0.0,
        user_s: None,
        sys_s: None,
        max_rss_bytes: None,
    }
}

fn compare_invocations(a: &Invocation, b: &Invocation) -> crate::Comparison {
    if a.is_probe() || b.is_probe() {
        let mut c = crate::Comparison::default();
        if a.meta.exit_code != b.meta.exit_code {
            c.differences.push("probe exit code differs".to_owned());
        }
        if a.stdout != b.stdout {
            c.differences.push("probe stdout bytes differ".to_owned());
        }
        if a.stderr != b.stderr {
            c.differences.push("probe stderr bytes differ".to_owned());
        }
        return c;
    }
    let mut c = crate::compare(&invocation_record(a), &invocation_record(b));
    // Pairing isolates each compiler's diagnostics, so their exact bytes can
    // be checked even when Cargo interleaves different compiler processes.
    if a.stderr != b.stderr {
        c.differences
            .push("compiler stderr bytes differ".to_owned());
    }
    c
}

fn grouped(invocations: &[Invocation]) -> BTreeMap<Identity, Vec<&Invocation>> {
    let mut map = BTreeMap::new();
    for invocation in invocations {
        map.entry(invocation.identity())
            .or_insert_with(Vec::new)
            .push(invocation);
    }
    map
}

/// Compares every paired invocation without using document completion order.
/// Repeated identical identities are compared as a multiset, consuming every
/// occurrence exactly once. Missing or extra probes are failures too.
#[must_use]
pub fn compare_arms(plain: &[Invocation], sail: &[Invocation]) -> crate::Comparison {
    let mut c = crate::Comparison::default();
    let (p, mut s) = (grouped(plain), grouped(sail));
    for (identity, plain_list) in p {
        let mut sail_list = s.remove(&identity).unwrap_or_default();
        if plain_list.len() != sail_list.len() {
            c.differences.push(format!(
                "invocation count for {identity:?}: plain {} / sail {}",
                plain_list.len(),
                sail_list.len()
            ));
        }
        for a in plain_list {
            if sail_list.is_empty() {
                continue;
            }
            let index = sail_list
                .iter()
                .position(|b| compare_invocations(a, b).differences.is_empty())
                .unwrap_or(0);
            let b = sail_list.remove(index);
            let pair = compare_invocations(a, b);
            c.functions += pair.functions;
            c.rlimit_drift.extend(pair.rlimit_drift);
            c.accepted.extend(pair.accepted);
            c.differences.extend(
                pair.differences
                    .into_iter()
                    .map(|d| format!("{identity:?}: {d}")),
            );
        }
    }
    for (identity, invocations) in s {
        c.differences.push(format!(
            "{} extra Sail invocations for {identity:?}",
            invocations.len()
        ));
    }
    c
}

/// Compares complete captures, retaining the aggregate exit and diagnostics
/// oracle as well as the stricter invocation checks.
#[must_use]
pub fn compare(plain: &CapturedRun, sail: &CapturedRun) -> crate::Comparison {
    match (&plain.invocations, &sail.invocations) {
        (None, None) => crate::compare(&plain.record, &sail.record),
        (Some(p), Some(s)) => {
            // The aggregate stdout has already been proved an exact byte
            // interleaving. Compare its complete documents by compiler identity.
            let aggregate = |run: &sail_verus::run::RunRecord| {
                let mut run = run.clone();
                run.outputs.clear();
                run.stdout_rest.clear();
                run
            };
            let mut c = crate::compare(&aggregate(&plain.record), &aggregate(&sail.record));
            let pairs = compare_arms(p, s);
            c.functions = pairs.functions;
            c.differences.extend(pairs.differences);
            c.rlimit_drift.extend(pairs.rlimit_drift);
            c.accepted.extend(pairs.accepted);
            c
        }
        _ => crate::Comparison {
            differences: vec!["only one arm has compiler captures".to_owned()],
            ..crate::Comparison::default()
        },
    }
}

/// Applies ADR 0009 using the same invocation-aware oracle for all plain
/// repetitions and Sail. Capture failures never reach this classifier.
#[must_use]
pub fn classify(plains: &[CapturedRun], sail: &CapturedRun) -> crate::Classification {
    if sail.invocations.is_none() && plains.iter().all(|p| p.invocations.is_none()) {
        return crate::classify(
            &plains.iter().map(|p| p.record.clone()).collect::<Vec<_>>(),
            &sail.record,
        );
    }

    let Some(first) = plains.first() else {
        return crate::Classification {
            verdict: crate::Verdict::InfraError,
            comparison: crate::Comparison::default(),
            plain_runs: 0,
            unstable: Vec::new(),
        };
    };
    let comparison = compare(first, sail);
    let unstable: Vec<String> = plains
        .iter()
        .skip(1)
        .flat_map(|p| compare(first, p).differences)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let verdict = if comparison.differences.is_empty() {
        crate::Verdict::Same
    } else if !unstable.is_empty()
        && plains
            .iter()
            .any(|p| compare(p, sail).differences.is_empty())
    {
        crate::Verdict::Unstable
    } else {
        crate::Verdict::Different
    };
    crate::Classification {
        verdict,
        comparison,
        plain_runs: plains.len(),
        unstable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(root: &str, name: Option<&str>, stdout: &[u8]) -> Invocation {
        let argv = name.map_or_else(
            || vec!["-vV".to_owned()],
            |name| {
                vec![
                    "--crate-name".to_owned(),
                    name.to_owned(),
                    "--error-format=json".to_owned(),
                    "--json=artifacts".to_owned(),
                ]
            },
        );
        let driver_env = BTreeMap::new();
        Invocation {
            meta: Meta {
                version: 1,
                package_name: name.map(str::to_owned),
                package_version: name.map(|_| "1.0".to_owned()),
                manifest_dir: name.map(|_| format!("{root}/member")),
                crate_name: name.map(str::to_owned),
                crate_types: vec!["lib".to_owned()],
                test: false,
                bin_name: None,
                compile_target: None,
                arg_sha256: argument_digest(&argv, &driver_env),
                argv,
                driver_env,
                source_root: root.to_owned(),
                target_root: format!("{root}/target"),
                runtime_root: "/runtime".to_owned(),
                started_ns: 1,
                pid: 1,
                exit_code: Some(0),
                stdout_len: stdout.len(),
                stderr_len: 0,
                stdout_sha256: hex_sha256(stdout),
                stderr_sha256: hex_sha256(b""),
                forward_error: None,
            },
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn interleaving_preserves_every_byte_and_within_process_order() {
        let parts = [b"{\n  a\n}\n".as_slice(), b"[\n  b\n]\n".as_slice()];
        assert!(explain(b"{\n[\n  a\n  b\n}\n]\n", &parts, 100).explained);
        assert!(!explain(b"{\n[\n  a\n  b\n}\n]", &parts, 100).explained);
        assert!(!explain(b"{[\n\n  a\n  b\n}\n]\n", &parts, 100).explained);
        assert!(!explain(b"{\n[\n  b\n}\n  a\n]\n", &parts, 100).explained);
        assert!(!explain(b"{\n[\n  a\n  b\n}\n]\nextra\n", &parts, 100).explained);
    }

    #[test]
    fn ambiguous_shared_lines_backtrack_and_budget_exhaustion_never_passes() {
        let parts = [b"a\nb\n".as_slice(), b"a\nc\n".as_slice()];
        assert!(explain(b"a\na\nc\nb\n", &parts, 10).explained);
        let limited = explain(b"a\na\nc\nb\n", &parts, 1);
        assert!(limited.undecided);
        assert!(!limited.explained);
    }

    #[test]
    fn pairing_uses_package_identity_across_checkout_roots_and_completion_order() {
        let plain = [
            invocation("/first", Some("a"), b"{}\n"),
            invocation("/first", Some("b"), b"{}\n"),
        ];
        let sail = [
            invocation("/second", Some("b"), b"{}\n"),
            invocation("/second", Some("a"), b"{}\n"),
        ];
        assert_eq!(compare_arms(&plain, &sail).differences, [] as [String; 0]);
        assert_ne!(
            compare_arms(&plain, &sail[..1]).differences,
            [] as [String; 0]
        );
        let mut changed = sail.clone();
        changed[1].stderr = b"a diagnostic\n".to_vec();
        assert_ne!(
            compare_arms(&plain, &changed).differences,
            [] as [String; 0]
        );
    }

    #[test]
    fn all_probes_are_paired_with_multiplicity_and_exact_output() {
        let a = invocation("/first", None, b"version one\n");
        let b = invocation("/second", None, b"version two\n");
        assert_ne!(
            compare_arms(std::slice::from_ref(&a), std::slice::from_ref(&b)).differences,
            [] as [String; 0]
        );
        assert_eq!(
            compare_arms(&[a.clone(), b.clone()], &[b, a.clone()]).differences,
            [] as [String; 0]
        );
        assert_ne!(
            compare_arms(&[a.clone(), a.clone()], &[a]).differences,
            [] as [String; 0]
        );
    }

    #[test]
    fn named_build_script_probes_cannot_swap_source_status_associations() {
        let mut a = invocation("/root", Some("proc_macro2"), b"");
        a.meta.argv = vec![
            "--crate-name=proc_macro2".to_owned(),
            "src/probe/one.rs".to_owned(),
        ];
        let mut b = a.clone();
        b.meta.argv[1] = "src/probe/two.rs".to_owned();
        b.meta.exit_code = Some(101);
        assert_ne!(a.identity(), b.identity());
        assert!(!a.forwarded());
        assert!(!b.forwarded());
        let mut changed_a = a.clone();
        let mut changed_b = b.clone();
        changed_a.meta.exit_code = Some(101);
        changed_b.meta.exit_code = Some(0);
        assert_ne!(
            compare_arms(&[a, b], &[changed_a, changed_b]).differences,
            [] as [String; 0]
        );
    }

    #[test]
    fn probe_path_identity_rewrites_declared_roots_only() {
        let mut a = invocation("/first", None, b"");
        let mut b = invocation("/second", None, b"");
        a.meta.argv = vec![
            "--print=file-names".to_owned(),
            "dependency=/first/target/deps".to_owned(),
        ];
        b.meta.argv = vec![
            "--print=file-names".to_owned(),
            "dependency=/second/target/deps".to_owned(),
        ];
        assert_eq!(a.identity(), b.identity());
        b.meta.argv[1] = "dependency=/second/target/deps-other".to_owned();
        assert_ne!(a.identity(), b.identity());
    }
}
