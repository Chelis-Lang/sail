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
    /// Digest of the exact requested argv/packed channels before owned logging.
    pub requested_arg_sha256: String,
    /// A `-` argument requires complete raw compiler stdin capture.
    pub stdin_captured: bool,
    /// Retained stdin byte count, zero for a non-stdin call.
    pub stdin_len: usize,
    /// Exact stdin SHA-256 (including the empty input).
    pub stdin_sha256: String,
    /// Original arguments (non-UTF-8 metadata makes the capture invalid).
    pub argv: Vec<String>,
    /// The `__VERUS_DRIVER_*` environment.
    pub driver_env: BTreeMap<String, String>,
    /// Exact packed channels requested by Cargo, before owned harness logging.
    pub requested_driver_env: BTreeMap<String, String>,
    /// This arm's output root (identity only).
    pub work_root: String,
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
    /// Exact input bytes, when the compiler receives source through stdin.
    pub stdin: Vec<u8>,
}

/// A lossless argument identity: path-root labels are typed, never raw text
/// that a literal user argument could impersonate. Original bytes stay in Meta.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Argument {
    /// An argument with no declared root substitution.
    Literal {
        /// Complete original spelling.
        value: String,
    },
    /// Exactly one declared root at the start of a path-valued suffix.
    Rooted {
        /// Every byte before the root, including any assignment prefixes.
        prefix: String,
        /// One of the explicit source, target, runtime or work namespaces.
        root: String,
        /// Every byte after the root, including separators and dot components.
        suffix: String,
    },
    /// The sole admitted autocfg instance label; the raw label remains recorded.
    GeneratedProbe {
        /// Exact monotonic probe ordinal; stdin and all other arguments still bind.
        ordinal: u64,
    },
}

/// The identity two arms are paired on: package, then target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Identity {
    /// Package name.
    pub package_name: String,
    /// Package version.
    pub package_version: String,
    /// The package's manifest directory.
    pub manifest_dir: Option<Argument>,
    /// Crate name.
    pub crate_name: String,
    /// Crate types, joined by `,`.
    pub crate_types: String,
    /// Test harness build.
    pub test: bool,
    /// Compile target (absent for the host).
    pub compile_target: Option<Argument>,
    /// Binary target name, if Cargo supplied it.
    pub bin_name: Option<String>,
    /// Every argument, with declared path roots made relative. Build-script
    /// probes can have a crate name too; arguments preserve their association.
    pub arguments: Vec<Argument>,
    /// Complete requested Verus driver channels, including physical leading
    /// spaces and every packed option; harness-only logging is recorded in
    /// `Meta::driver_env` separately from this original input identity.
    pub driver_env: BTreeMap<String, Vec<Argument>>,
    /// Exact stdin source identity, distinct from a non-stdin compiler call.
    pub stdin_sha256: Option<String>,
    /// Known native autocfg probe's monotonic ordinal. Its random instance
    /// label is recorded verbatim, while this ordinal and exact stdin bind
    /// the corresponding request. Never used for verified Cargo jobs.
    pub generated_probe_ordinal: Option<u64>,
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
        }) || (self.meta.argv.iter().any(|a| a == "--emit=metadata")
            && self.meta.argv.windows(2).any(|args| args == ["-o", "-"]))
    }

    fn generated_probe(&self) -> Option<(String, u64)> {
        let m = &self.meta;
        if self.forwarded()
            || !m.stdin_captured
            || m.argv.last().map(String::as_str) != Some("-")
            || !m.argv.iter().any(|a| a == "--emit=llvm-ir")
            || m.crate_types != ["lib"]
        {
            return None;
        }
        let package = m.package_name.as_ref()?;
        let version = m.package_version.as_ref()?;
        let selected = format!("__VERUS_DRIVER_VERIFY_{package}-{version}-");
        if m.requested_driver_env
            .iter()
            .any(|(key, value)| key.starts_with(&selected) && value == "1")
        {
            return None;
        }
        let name = m.crate_name.as_ref()?;
        let rest = name.strip_prefix("autocfg_")?;
        let (instance, ordinal) = rest.split_once('_')?;
        if instance.len() != 16
            || !instance
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let number = ordinal.parse::<u64>().ok()?;
        if number.to_string() != ordinal
            || m.argv
                .iter()
                .filter(|arg| arg.as_str() == "--crate-name" || arg.starts_with("--crate-name="))
                .count()
                != 1
            || !m
                .argv
                .windows(2)
                .any(|a| a[0] == "--crate-name" && &a[1] == name)
        {
            return None;
        }
        Some((format!("autocfg_<instance>_{ordinal}"), number))
    }

    /// Package/target identity and complete argument association.
    #[must_use]
    pub fn identity(&self) -> Identity {
        let m = &self.meta;
        let generated = self.generated_probe();
        Identity {
            package_name: m.package_name.clone().unwrap_or_default(),
            package_version: m.package_version.clone().unwrap_or_default(),
            manifest_dir: m.manifest_dir.as_ref().map(|p| relative_argument(p, m)),
            crate_name: generated.as_ref().map_or_else(
                || m.crate_name.clone().unwrap_or_default(),
                |(name, _)| name.clone(),
            ),
            crate_types: m.crate_types.join(","),
            test: m.test,
            compile_target: m.compile_target.as_ref().map(|a| relative_argument(a, m)),
            bin_name: m.bin_name.clone(),
            stdin_sha256: m.stdin_captured.then(|| m.stdin_sha256.clone()),
            generated_probe_ordinal: generated.as_ref().map(|(_, ordinal)| *ordinal),
            arguments: m
                .argv
                .iter()
                .enumerate()
                .map(|(i, arg)| {
                    if let Some((_, ordinal)) = &generated
                        && i > 0
                        && m.argv.get(i - 1).is_some_and(|a| a == "--crate-name")
                        && m.crate_name.as_ref() == Some(arg)
                    {
                        Argument::GeneratedProbe { ordinal: *ordinal }
                    } else {
                        relative_argument(arg, m)
                    }
                })
                .collect(),
            driver_env: m
                .requested_driver_env
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        value
                            .split("__VERUS_DRIVER_ARGS_SEP__")
                            .map(|a| relative_argument(a, m))
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}

fn relative_argument(arg: &str, meta: &Meta) -> Argument {
    let mut value = arg;
    let mut offset = 0;
    loop {
        for (label, root) in [
            ("source", &meta.source_root),
            ("target", &meta.target_root),
            ("runtime", &meta.runtime_root),
            ("work", &meta.work_root),
        ] {
            if !root.is_empty()
                && let Some(suffix) = value.strip_prefix(root)
                && (suffix.is_empty() || suffix.starts_with('/') || root.ends_with('/'))
            {
                return Argument::Rooted {
                    prefix: arg.get(..offset).unwrap_or_default().to_owned(),
                    root: label.to_owned(),
                    suffix: suffix.to_owned(),
                };
            }
        }
        let Some((prefix, rest)) = value.split_once('=') else {
            return Argument::Literal {
                value: arg.to_owned(),
            };
        };
        offset += prefix.len() + 1;
        value = rest;
    }
}

/// Loads every record under `dir`, in start order.
///
/// # Errors
///
/// A record the tee did not complete (`*.tmp`), an unreadable file, or
/// streams whose length or digest disagree with the record.
pub fn load(dir: &Path) -> Result<Vec<Invocation>, String> {
    load_records(dir).map_err(capture_error)
}
fn capture_error(error: impl std::fmt::Display) -> String {
    format!("error[SAIL3018]: {error}")
}
fn load_records(dir: &Path) -> Result<Vec<Invocation>, String> {
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
        if meta.version != 3 || meta.source_root.is_empty() {
            return Err(format!("unsupported or unbound capture {}", path.display()));
        }
        let stdout = read("stdout")?;
        let stderr = read("stderr")?;
        let stdin = read("stdin")?;
        if stdout.len() != meta.stdout_len
            || stderr.len() != meta.stderr_len
            || hex_sha256(&stdout) != meta.stdout_sha256
            || hex_sha256(&stderr) != meta.stderr_sha256
            || argument_digest(&meta.argv, &meta.driver_env) != meta.arg_sha256
            || argument_digest(&meta.argv, &meta.requested_driver_env) != meta.requested_arg_sha256
            || stdin.len() != meta.stdin_len
            || hex_sha256(&stdin) != meta.stdin_sha256
            || meta.stdin_captured != meta.argv.iter().any(|arg| arg == "-")
            || (!meta.stdin_captured && !stdin.is_empty())
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
        let invocation = Invocation {
            meta,
            stdout,
            stderr,
            stdin,
        };
        if invocation.forwarded() {
            std::str::from_utf8(&invocation.stdout)
                .map_err(|e| format!("forwarded compiler stdout is not UTF-8: {e}"))?;
        }
        out.push(invocation);
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
                .flat_map(|i| document_record(i).outputs)
                .collect();
        }
        record
    }

    /// Only actual Cargo compiler jobs require target engine seals. Discovery
    /// probes retain their JSON in the complete summary and comparison, but
    /// do not constitute requested target verification invocations.
    #[must_use]
    pub fn verification_summary(&self) -> sail_verus::run::RunRecord {
        let mut record = self.record.clone();
        if let Some(invocations) = &self.invocations {
            record.outputs = grouped(invocations)
                .into_values()
                .flatten()
                .filter(|i| i.forwarded())
                .flat_map(|i| document_record(i).outputs)
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
        let failures = std::fs::read(work.join("capture-errors"))
            .map_err(|e| format!("error[SAIL3018]: missing capture health record: {e}"))?;
        if !failures.is_empty()
            || record
                .stderr
                .lines()
                .any(|line| line.starts_with("error[SAIL3018]:"))
        {
            return Err(
                "error[SAIL3018]: a compiler delegate reported a capture failure".to_owned(),
            );
        }
        let invocations = load(&work.join("invocations"))?;
        if !invocations.iter().any(Invocation::forwarded) {
            return Err("error[SAIL3018]: no Cargo compiler jobs captured".to_owned());
        }
        if invocations
            .iter()
            .any(|i| !i.forwarded() && !i.stdout.is_empty() && !i.known_probe_output())
        {
            return Err("error[SAIL3018]: stdout from an unrecognized compiler invocation; capture cannot establish Cargo forwarding".to_owned());
        }
        let stdout = std::fs::read(work.join("stdout.txt")).map_err(capture_error)?;
        let parts: Vec<&[u8]> = invocations
            .iter()
            .filter(|i| i.forwarded())
            .map(|i| i.stdout.as_slice())
            .collect();
        let explanation = explain(&stdout, &parts, 50_000);
        let json = serde_json::to_vec_pretty(&explanation).map_err(capture_error)?;
        std::fs::write(work.join("capture-check.json"), json).map_err(capture_error)?;
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

// A selected package's `-vV` probe can print Rust's verbose text followed by
// Verus's ordinary JSON document. Keep both text boundaries byte-exact and
// apply the SAME existing JSON oracle as for ordinary compiler invocations.
// In particular, a fork's recorded version metadata is not a proof verdict.
fn version_probe(invocation: &Invocation) -> Option<(sail_verus::run::RunRecord, &[u8], &[u8])> {
    if !invocation.is_probe() || !invocation.meta.argv.iter().any(|a| a == "-vV") {
        return None;
    }
    let bytes = &invocation.stdout;
    let start = bytes.iter().enumerate().find_map(|(i, byte)| {
        (*byte == b'{' && (i == 0 || bytes.get(i - 1) == Some(&b'\n'))).then_some(i)
    })?;
    let mut stream =
        serde_json::Deserializer::from_slice(bytes.get(start..)?).into_iter::<serde_json::Value>();
    let mut outputs = Vec::new();
    let mut end = 0;
    while let Some(Ok(doc)) = stream.next() {
        if !doc.is_object()
            || doc.get("verification-results").is_none()
            || doc.get("func-details").is_none()
        {
            return None;
        }
        outputs.push(doc);
        end = stream.byte_offset();
    }
    if outputs.is_empty() {
        return None;
    }
    let mut record = invocation_record(invocation);
    record.outputs = outputs;
    record.stdout_rest.clear();
    Some((record, bytes.get(..start)?, bytes.get(start + end..)?))
}

fn document_record(invocation: &Invocation) -> sail_verus::run::RunRecord {
    version_probe(invocation).map_or_else(|| invocation_record(invocation), |(record, _, _)| record)
}

fn compare_invocations(a: &Invocation, b: &Invocation) -> crate::Comparison {
    if a.is_probe() || b.is_probe() {
        if let (Some((plain, before_plain, after_plain)), Some((sail, before_sail, after_sail))) =
            (version_probe(a), version_probe(b))
        {
            let mut c = crate::compare(&plain, &sail);
            if before_plain != before_sail || after_plain != after_sail {
                c.differences
                    .push("probe non-JSON stdout bytes differ".to_owned());
            }
            if a.stderr != b.stderr {
                c.differences.push("probe stderr bytes differ".to_owned());
            }
            return c;
        }
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
        if identity.generated_probe_ordinal.is_some()
            && [&plain_list, &sail_list].into_iter().any(|list| {
                list.iter()
                    .map(|i| &i.meta.crate_name)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    > 1
            })
        {
            c.differences
                .push(format!("ambiguous generated probe labels for {identity:?}"));
        }
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
                version: 3,
                package_name: name.map(str::to_owned),
                package_version: name.map(|_| "1.0".to_owned()),
                manifest_dir: name.map(|_| format!("{root}/member")),
                crate_name: name.map(str::to_owned),
                crate_types: vec!["lib".to_owned()],
                test: false,
                bin_name: None,
                compile_target: None,
                arg_sha256: argument_digest(&argv, &driver_env),
                requested_arg_sha256: argument_digest(&argv, &driver_env),
                stdin_captured: false,
                stdin_len: 0,
                stdin_sha256: hex_sha256(b""),
                argv,
                requested_driver_env: driver_env.clone(),
                driver_env,
                work_root: "/work".to_owned(),
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
            stdin: Vec::new(),
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
    fn complete_summaries_include_probe_json_but_target_seals_require_cargo_jobs() {
        let doc = br#"{"func-details":{},"verification-results":{"verified":1,"errors":0},"times-ms":{"smt":{"smt-run-module-times":[{"module":"m","function-breakdown":[{"function":"f","success":true,"rlimit":1}]}]}}}"#;
        let mut bytes = b"rustc version\n".to_vec();
        bytes.extend_from_slice(doc);
        let probe = invocation("/root", None, &bytes);
        let job = invocation("/root", Some("p"), doc);
        let record = invocation_record(&job);
        let run = CapturedRun {
            record,
            invocations: Some(vec![probe, job]),
        };
        assert_eq!(run.summary().outputs.len(), 2);
        assert_eq!(run.summary().total("verified"), Some(2));
        assert_eq!(run.summary().functions().len(), 2);
        assert_eq!(run.verification_summary().outputs.len(), 1);
        let event = r#"{"event":"compiler-sealed","phase":"full","payload":{"normal_termination":true,"success":true}}"#;
        assert_eq!(
            crate::engine::check(&run.verification_summary(), event, false).0,
            [] as [String; 0]
        );
    }

    #[test]
    fn generated_native_probe_labels_keep_ordinal_source_and_outcome_association() {
        fn probe(instance: &str, ordinal: u64, source: &[u8], status: i32) -> Invocation {
            let mut i = invocation("/root", Some("num_traits"), b"");
            let name = format!("autocfg_{instance}_{ordinal}");
            i.meta.crate_name = Some(name.clone());
            i.meta.argv = vec![
                "--crate-name".to_owned(),
                name,
                "--emit=llvm-ir".to_owned(),
                "-".to_owned(),
            ];
            i.meta.stdin_captured = true;
            i.stdin = source.to_vec();
            i.meta.stdin_sha256 = hex_sha256(source);
            i.meta.exit_code = Some(status);
            i
        }
        let a = probe("0123456789abcdef", 0, b"one", 0);
        let b = probe("fedcba9876543210", 0, b"one", 0);
        assert_eq!(
            compare_arms(std::slice::from_ref(&a), std::slice::from_ref(&b)).differences,
            [] as [String; 0]
        );
        for changed in [
            probe("fedcba9876543210", 1, b"one", 0),
            probe("fedcba9876543210", 0, b"two", 0),
            probe("fedcba9876543210", 0, b"one", 7),
        ] {
            assert_ne!(
                compare_arms(std::slice::from_ref(&a), &[changed]).differences,
                [] as [String; 0]
            );
        }
        // Only the one recognized generator label may be paired across runs.
        // Duplicate options are rejected by rustc with identical generic stderr;
        // that does not permit arbitrary extra input values to disappear.
        for inline in [false, true] {
            let mut left = a.clone();
            let mut right = a.clone();
            for (invocation, name) in [(&mut left, "other_one"), (&mut right, "other_two")] {
                let end = invocation.meta.argv.len() - 1;
                let extra = if inline {
                    vec![format!("--crate-name={name}")]
                } else {
                    vec!["--crate-name".to_owned(), name.to_owned()]
                };
                invocation.meta.argv.splice(end..end, extra);
            }
            assert_eq!(left.identity().generated_probe_ordinal, None);
            assert_ne!(left.identity(), right.identity());
            assert_ne!(
                compare_arms(&[left], &[right]).differences,
                [] as [String; 0]
            );
        }
        assert_ne!(
            compare_arms(&[a.clone(), b.clone()], &[a, b]).differences,
            [] as [String; 0]
        );
    }

    #[test]
    fn typed_roots_cannot_collide_with_literal_markers_or_erase_path_spelling() {
        let a = invocation("/root", None, b"");
        for (root, marker) in [
            ("/root", "source"),
            ("/target", "target"),
            ("/runtime", "runtime"),
            ("/work", "work"),
        ] {
            let mut m = a.meta.clone();
            m.target_root = "/target".to_owned();
            m.runtime_root = "/runtime".to_owned();
            m.work_root = "/work".to_owned();
            for prefix in ["", "dependency=", "--option=foo="] {
                assert_ne!(
                    relative_argument(&format!("{prefix}{root}/empty"), &m),
                    relative_argument(&format!("{prefix}<{marker}>/empty"), &m)
                );
            }
        }
        assert_ne!(
            relative_argument("/root/a", &a.meta),
            relative_argument("/root/./a", &a.meta)
        );
        assert_ne!(
            relative_argument("/root/a", &a.meta),
            relative_argument("/root//a", &a.meta)
        );
        assert_ne!(
            relative_argument("/root/a", &a.meta),
            relative_argument("/rooted/a", &a.meta)
        );
        for packed in [false, true] {
            let mut left = a.clone();
            let mut right = a.clone();
            if packed {
                left.meta.requested_driver_env.insert(
                    " __VERUS_DRIVER_ARGS__".to_owned(),
                    "dependency=/root/empty".to_owned(),
                );
                right.meta.requested_driver_env.insert(
                    " __VERUS_DRIVER_ARGS__".to_owned(),
                    "dependency=<source>/empty".to_owned(),
                );
            } else {
                left.meta.argv.push("dependency=/root/empty".to_owned());
                right.meta.argv.push("dependency=<source>/empty".to_owned());
            }
            assert_ne!(
                compare_arms(&[left], &[right]).differences,
                [] as [String; 0]
            );
        }
    }

    #[test]
    fn stdin_sources_cannot_exchange_outcomes_under_identical_arguments() {
        let mut a = invocation("/root", None, b"");
        a.meta.argv = vec![
            "--emit=metadata".to_owned(),
            "-o".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        ];
        a.stdin = b"one\n".to_vec();
        a.meta.stdin_captured = true;
        a.meta.stdin_sha256 = hex_sha256(&a.stdin);
        let mut b = a.clone();
        b.stdin = b"two\n".to_vec();
        b.meta.stdin_sha256 = hex_sha256(&b.stdin);
        b.meta.exit_code = Some(7);
        assert_ne!(a.identity(), b.identity());
        let mut changed_a = a.clone();
        let mut changed_b = b.clone();
        changed_a.meta.exit_code = Some(7);
        changed_b.meta.exit_code = Some(0);
        assert_ne!(
            compare_arms(&[a, b], &[changed_a, changed_b]).differences,
            [] as [String; 0]
        );
    }

    #[test]
    fn known_binary_metadata_probes_keep_exact_bytes_and_input_identity() {
        let mut a = invocation("/root", None, b"rust\0\xff");
        a.meta.argv = vec![
            "--emit=metadata".to_owned(),
            "-o".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        ];
        a.meta.stdin_captured = true;
        assert!(!a.forwarded());
        assert!(a.known_probe_output());
        assert_eq!(
            compare_arms(std::slice::from_ref(&a), std::slice::from_ref(&a)).differences,
            [] as [String; 0]
        );
        let mut b = a.clone();
        b.stdout = b"rust\0\xfe".to_vec();
        assert_ne!(compare_arms(&[a], &[b]).differences, [] as [String; 0]);
    }

    #[test]
    fn version_probe_uses_existing_json_oracle_and_keeps_all_adjacent_text() {
        let left = b"rustc version\n{\"func-details\":{},\"verification-results\":{\"verified\":0,\"errors\":0},\"verus\":{\"version\":\"stock\"}}\n";
        let right = b"rustc version\n{\"func-details\":{},\"verification-results\":{\"verified\":0,\"errors\":0},\"verus\":{\"version\":\"fork\"}}\n";
        let a = invocation("/first", None, left);
        let b = invocation("/second", None, right);
        assert_eq!(
            compare_arms(std::slice::from_ref(&a), std::slice::from_ref(&b)).differences,
            [] as [String; 0]
        );
        for changed in [
            String::from_utf8_lossy(right).replace("rustc version", "changed rustc"),
            String::from_utf8_lossy(right).replace("\"errors\":0", "\"errors\":1"),
            format!("{}extra", String::from_utf8_lossy(right)),
            String::from_utf8_lossy(right).trim_end().to_owned(),
        ] {
            let changed = invocation("/second", None, changed.as_bytes());
            assert_ne!(
                compare_arms(std::slice::from_ref(&a), &[changed]).differences,
                [] as [String; 0]
            );
        }
    }

    #[test]
    fn physical_packed_verus_channels_keep_selector_status_associations() {
        let mut a = invocation("/root", Some("probe"), b"");
        a.meta.requested_driver_env.insert(
            " __VERUS_DRIVER_ARGS__".to_owned(),
            "__VERUS_DRIVER_ARGS_SEP__--verify-function__VERUS_DRIVER_ARGS_SEP__p::one".to_owned(),
        );
        let mut b = a.clone();
        b.meta.requested_driver_env.insert(
            " __VERUS_DRIVER_ARGS__".to_owned(),
            "__VERUS_DRIVER_ARGS_SEP__--verify-function__VERUS_DRIVER_ARGS_SEP__p::two".to_owned(),
        );
        b.meta.exit_code = Some(7);
        assert_ne!(a.identity(), b.identity());
        let mut changed_a = a.clone();
        let mut changed_b = b.clone();
        changed_a.meta.exit_code = Some(7);
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
