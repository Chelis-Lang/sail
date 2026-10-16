//! `sail-bench snapshot-exactness`: is a resumed solver snapshot exact?
//!
//! Prototype evidence for ADR 0049 (draft). For each solver input transcript
//! (`<id>.in.smt2` from a shim trace, `SAIL_TRACE_DIR`), it runs the pinned
//! Z3:
//! 1. **fresh**: unmodified, on the whole input (the reference);
//! 2. **trace**: compares the traced process's own output (`<id>.out`, if
//!    present) with fresh: the stock run-to-run baseline;
//! 3. **marked**: with the `sail-snap` library loaded and a snapshot marker
//!    before every top-level `(push)`: its output must equal fresh's, and it
//!    leaves one parked snapshot per marker;
//! 4. **resumed**: for up to `--points` of those snapshots, resumes it, sends
//!    the rest of the input and compares every reply with fresh's replies to
//!    the same commands.
//!
//! Replies are compared exactly, and again with the statistics that differ
//! between any two plain runs masked: `:time`, `:memory`, `:max-memory` and
//! `:num-allocs` (the trace comparison measures how often). `:rlimit-count`
//! and every other byte are compared exactly. Differing statistics keys are
//! counted per key, `:num-allocs` included.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

use sail_session::Session;
use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::split::{CommandSplitter, ItemKind, ResponseSplitter};

/// Options for one run.
#[derive(Debug, Clone)]
pub struct Options {
    /// The pinned Z3.
    pub z3: PathBuf,
    /// The `sail-snap` library.
    pub lib: PathBuf,
    /// Socket directory (short path).
    pub dir: PathBuf,
    /// Most snapshots resumed per transcript (evenly spaced, last included).
    pub points: usize,
    /// Transcripts processed in parallel.
    pub jobs: usize,
    /// Transcripts, or directories searched recursively for `*.in.smt2`.
    pub inputs: Vec<PathBuf>,
}

/// A comparison of two reply sequences.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Cmp {
    /// Replies compared.
    pub compared: usize,
    /// Byte-identical.
    pub identical: usize,
    /// Identical once `:time`, `:memory`, `:max-memory` and `:num-allocs`
    /// are masked.
    pub masked_identical: usize,
    /// Different even when masked.
    pub differ: usize,
    /// Whether the sequences had the same number of replies.
    pub same_count: bool,
    /// Different replies that are not statistics (answers, models, echoes,
    /// reasons): any of these is a verdict-level difference.
    pub non_stats_differ: usize,
    /// For differing statistics replies: how often each key's value differed.
    pub stat_keys: std::collections::BTreeMap<String, usize>,
    /// The first masked difference, for the record.
    pub first_diff: Option<String>,
}

impl Cmp {
    /// True when every reply matched once masked, and the counts agree.
    #[must_use]
    pub fn exact(&self) -> bool {
        self.same_count && self.differ == 0
    }
}

/// One resumed snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct Resume {
    /// Index of the snapshot point among the transcript's points.
    pub point: usize,
    /// Input offset of the point.
    pub offset: usize,
    /// Replies before the point (not re-solved).
    pub replies_before: usize,
    /// The comparison with fresh's replies after the point.
    pub cmp: Cmp,
    /// Exit status reported for the resumed solver.
    pub status: Option<i32>,
    /// Seconds from connecting to the end of output.
    pub secs: f64,
}

/// One transcript's result.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileReport {
    /// The transcript.
    pub file: String,
    /// Input bytes.
    pub bytes: usize,
    /// Replies in fresh's output.
    pub replies: usize,
    /// `check-sat` commands.
    pub checks: usize,
    /// Snapshot points (top-level pushes).
    pub points: usize,
    /// Fresh run seconds.
    pub fresh_secs: f64,
    /// Marked run seconds.
    pub marked_secs: f64,
    /// Snapshots found parked after the marked run.
    pub parked: usize,
    /// Traced output against fresh (stock baseline), when `.out` exists.
    pub trace: Option<Cmp>,
    /// Marked run against fresh.
    pub marked: Option<Cmp>,
    /// Resumed snapshots.
    pub resumes: Vec<Resume>,
    /// Memory of the parked snapshots (Linux), kB: (rss, pss, private).
    pub parked_kb: Option<(u64, u64, u64)>,
    /// Why the transcript was skipped or failed.
    pub error: Option<String>,
}

/// Statistics that differ between any two runs of the same input: wall
/// clock, memory, and the allocation count (which differs between two plain
/// runs too). Verus reads only `:rlimit-count` (ADR 0008, ADR 0011).
const MASKED: [&str; 4] = [":time", ":memory", ":max-memory", ":num-allocs"];

/// Wall-clock and memory figures, never reported as differences.
const WALL: [&str; 3] = [":time", ":memory", ":max-memory"];

/// A reply with the wall-clock and memory figures of statistics masked.
fn mask(reply: &[u8]) -> Vec<u8> {
    // Statistics compare as key/value maps without the masked keys: Z3
    // prints `:time` only when it rounds to non-zero, so even its presence
    // depends on the wall clock. Other replies compare byte for byte.
    let Some(map) = stats(reply) else {
        return reply.to_vec();
    };
    let mut out = String::from("(stats");
    for (k, v) in map.iter().filter(|(k, _)| !MASKED.contains(&k.as_str())) {
        let _ = write!(out, " {k} {v}");
    }
    out.push(')');
    out.into_bytes()
}

/// A statistics reply as key/value pairs; `None` for any other reply.
fn stats(reply: &[u8]) -> Option<std::collections::BTreeMap<String, String>> {
    let text = String::from_utf8_lossy(reply);
    let body = text.trim().strip_prefix("(:")?.strip_suffix(')')?;
    let mut out = std::collections::BTreeMap::new();
    for entry in format!(":{body}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .chunks(2)
    {
        if let [k, v] = entry {
            out.insert((*k).to_owned(), (*v).to_owned());
        }
    }
    Some(out)
}

/// Splits a solver's output into replies, as the shim does.
fn replies(out: &[u8]) -> Vec<Vec<u8>> {
    let mut s = ResponseSplitter::new();
    let mut items = Vec::new();
    s.push(out, &mut items);
    if let Some(last) = s.finish() {
        items.push(last);
    }
    let starts: Vec<usize> = items
        .iter()
        .map(|i| usize::try_from(i.offset).unwrap_or(usize::MAX))
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, &a)| {
            let b = starts.get(k + 1).copied().unwrap_or(out.len());
            out.get(a..b).unwrap_or_default().to_vec()
        })
        .collect()
}

fn compare(want: &[Vec<u8>], got: &[Vec<u8>]) -> Cmp {
    let mut c = Cmp {
        same_count: want.len() == got.len(),
        ..Cmp::default()
    };
    for (k, (w, g)) in want.iter().zip(got).enumerate() {
        c.compared += 1;
        if w == g {
            c.identical += 1;
            continue;
        }
        let (a, b) = (stats(w), stats(g));
        if let (Some(a), Some(b)) = (&a, &b) {
            let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                if !WALL.contains(&key.as_str()) && a.get(key) != b.get(key) {
                    *c.stat_keys.entry(key.clone()).or_default() += 1;
                }
            }
        }
        if mask(w) == mask(g) {
            c.masked_identical += 1;
            continue;
        }
        c.differ += 1;
        if a.is_none() || b.is_none() {
            c.non_stats_differ += 1;
        }
        if c.first_diff.is_none() {
            // Around the first differing byte of the masked replies.
            let (mw, mg) = (mask(w), mask(g));
            let at = mw
                .iter()
                .zip(&mg)
                .position(|(x, y)| x != y)
                .unwrap_or(mw.len().min(mg.len()));
            let cut = |b: &[u8]| {
                let from = at.saturating_sub(200).min(b.len());
                let to = (at + 200).min(b.len());
                String::from_utf8_lossy(b.get(from..to).unwrap_or_default()).into_owned()
            };
            c.first_diff = Some(format!(
                "reply {k} at byte {at}: want {:?} got {:?}",
                cut(&mw),
                cut(&mg)
            ));
        }
    }
    if !c.same_count && c.first_diff.is_none() {
        c.first_diff = Some(format!("{} replies, want {}", got.len(), want.len()));
    }
    c
}

/// Snapshot points with the transcript's reply and check counts.
type Points = (Vec<(usize, usize)>, usize, usize);

/// Snapshot points: (input offset, replies before) of each top-level push;
/// plus the number of replies and checks. `None` if any command's output
/// cannot be predicted.
fn points(input: &[u8]) -> Option<Points> {
    let mut sp = CommandSplitter::new();
    let mut items = Vec::new();
    sp.push(input, &mut items);
    if let Some(last) = sp.finish() {
        items.push(last);
    }
    let mut session = Session::new();
    let (mut pts, mut replies, mut checks) = (Vec::new(), 0usize, 0usize);
    for item in items {
        if item.kind != ItemKind::List {
            return None;
        }
        let kind = classify(&item.bytes).ok().map(|c| c.kind);
        if matches!(kind, Some(CommandKind::Push(_))) && session.depth() == 0 {
            pts.push((usize::try_from(item.offset).ok()?, replies));
        }
        if kind.as_ref().is_some_and(CommandKind::is_check) {
            checks += 1;
        }
        match session.expects_output(&item.bytes) {
            Output::One => replies += 1,
            Output::None => {}
            Output::Unknown => return None,
        }
        session.apply(&item.bytes);
    }
    Some((pts, replies, checks))
}

/// Runs `cmd` with `input` on stdin; returns stdout and seconds.
fn run(mut cmd: Command, input: Vec<u8>) -> Result<(Vec<u8>, f64), String> {
    let t = Instant::now();
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let mut out = Vec::new();
    child
        .stdout
        .take()
        .ok_or("no stdout")?
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    let _ = writer.join();
    child.wait().map_err(|e| e.to_string())?;
    Ok((out, t.elapsed().as_secs_f64()))
}

fn args_of(file: &Path) -> Vec<String> {
    let jsonl = file
        .to_string_lossy()
        .strip_suffix(".in.smt2")
        .map(|s| PathBuf::from(format!("{s}.jsonl")));
    jsonl
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| {
            let first = t.lines().next()?.to_owned();
            let v: serde_json::Value = serde_json::from_str(&first).ok()?;
            Some(
                v.get("args")?
                    .as_array()?
                    .iter()
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect(),
            )
        })
        .unwrap_or_else(|| vec!["-smt2".to_owned(), "-in".to_owned()])
}

#[cfg(target_os = "linux")]
fn memory_kb(pid: &str) -> Option<(u64, u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    let field = |name: &str| -> u64 {
        text.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    Some((
        field("Rss:"),
        field("Pss:"),
        field("Private_Clean:") + field("Private_Dirty:"),
    ))
}

#[cfg(not(target_os = "linux"))]
fn memory_kb(_pid: &str) -> Option<(u64, u64, u64)> {
    None
}

fn evict(dir: &Path, names: &[String]) {
    for n in names {
        let _ = std::fs::remove_file(dir.join(n));
        let _ = std::fs::remove_file(dir.join(format!("{n}.pid")));
    }
}

/// Indices of up to `n` points, evenly spaced, the last always included.
fn chosen(total: usize, n: usize) -> Vec<usize> {
    if total == 0 || n == 0 {
        return Vec::new();
    }
    if total <= n {
        return (0..total).collect();
    }
    let mut v: Vec<usize> = (0..n).map(|k| k * (total - 1) / (n - 1).max(1)).collect();
    v.dedup();
    v
}

fn one(o: &Options, file: &Path, tag: usize) -> FileReport {
    let mut r = FileReport {
        file: file.display().to_string(),
        ..FileReport::default()
    };
    let Ok(input) = std::fs::read(file) else {
        r.error = Some("unreadable".to_owned());
        return r;
    };
    r.bytes = input.len();
    let Some((pts, _, checks)) = points(&input) else {
        r.error = Some("unpredictable command output".to_owned());
        return r;
    };
    r.points = pts.len();
    r.checks = checks;
    let args = args_of(file);

    let mut fresh = Command::new(&o.z3);
    fresh.args(&args);
    let (fresh_out, secs) = match run(fresh, input.clone()) {
        Ok(v) => v,
        Err(e) => {
            r.error = Some(format!("fresh: {e}"));
            return r;
        }
    };
    r.fresh_secs = secs;
    let want = replies(&fresh_out);
    r.replies = want.len();
    let out_path = file
        .to_string_lossy()
        .strip_suffix(".in.smt2")
        .map(|s| PathBuf::from(format!("{s}.out")));
    if let Some(traced) = out_path.and_then(|p| std::fs::read(p).ok()) {
        r.trace = Some(compare(&want, &replies(&traced)));
    }
    if pts.is_empty() {
        return r;
    }

    // The marked run: a marker before every top-level push.
    let names: Vec<String> = (0..pts.len()).map(|k| format!("t{tag}p{k}")).collect();
    let marked_input = with_markers(&input, &pts, &names);
    let preload = if cfg!(target_os = "macos") {
        "DYLD_INSERT_LIBRARIES"
    } else {
        "LD_PRELOAD"
    };
    let mut marked = Command::new(&o.z3);
    marked
        .args(&args)
        .env(preload, &o.lib)
        .env("SAIL_SNAP_DIR", &o.dir)
        .env("SAIL_SNAP_TTL_S", "900");
    let (marked_out, secs) = match run(marked, marked_input) {
        Ok(v) => v,
        Err(e) => {
            r.error = Some(format!("marked: {e}"));
            return r;
        }
    };
    r.marked_secs = secs;
    r.marked = Some(compare(&want, &replies(&marked_out)));

    (r.parked, r.parked_kb) = parked(&o.dir, &names);

    for k in chosen(pts.len(), o.points) {
        let (Some(&(at, before)), Some(name)) = (pts.get(k), names.get(k)) else {
            continue;
        };
        match resume_rest(&o.dir, name, input.get(at..).unwrap_or_default()) {
            Ok((out, status, secs)) => r.resumes.push(Resume {
                point: k,
                offset: at,
                replies_before: before,
                cmp: compare(want.get(before..).unwrap_or_default(), &replies(&out)),
                status,
                secs,
            }),
            Err(e) => r.error = Some(format!("resume point {k}: {e}")),
        }
    }
    evict(&o.dir, &names);
    r
}

/// `input` with a snapshot marker before each point.
fn with_markers(input: &[u8], pts: &[(usize, usize)], names: &[String]) -> Vec<u8> {
    let mut marked = Vec::with_capacity(input.len() + 16 * pts.len());
    let mut from = 0;
    for ((at, _), name) in pts.iter().zip(names) {
        marked.extend_from_slice(input.get(from..*at).unwrap_or_default());
        marked.extend_from_slice(&sail_api::snapshot::marker(name));
        from = *at;
    }
    marked.extend_from_slice(input.get(from..).unwrap_or_default());
    marked
}

/// How many of `names` are parked (waiting briefly, since they appear
/// asynchronously) and their memory (Linux).
fn parked(dir: &Path, names: &[String]) -> (usize, Option<(u64, u64, u64)>) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    let mut n;
    loop {
        n = names.iter().filter(|k| dir.join(k).exists()).count();
        if n == names.len() || Instant::now() > deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut mem = (0, 0, 0);
    let mut have_mem = false;
    for k in names {
        if let Ok(pid) = std::fs::read_to_string(dir.join(format!("{k}.pid")))
            && let Some((a, b, c)) = memory_kb(pid.trim())
        {
            have_mem = true;
            mem = (mem.0 + a, mem.1 + b, mem.2 + c);
        }
    }
    (n, have_mem.then_some(mem))
}

/// Resumes snapshot `name`, sends `rest`, and returns the solver's whole
/// output, its exit status and the seconds it took.
fn resume_rest(dir: &Path, name: &str, rest: &[u8]) -> Result<(Vec<u8>, Option<i32>, f64), String> {
    let t = Instant::now();
    let resumed = sail_api::snapshot::resume(dir, name).map_err(|e| e.to_string())?;
    let rest = rest.to_vec();
    let mut w = resumed.io.try_clone().map_err(|e| e.to_string())?;
    let writer = std::thread::spawn(move || {
        let _ = w.write_all(&rest);
        let _ = w.shutdown(std::net::Shutdown::Write);
    });
    let mut err = resumed.err;
    let errs = std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = err.read_to_end(&mut sink);
    });
    let mut out = Vec::new();
    let mut io = resumed.io;
    let _ = io.read_to_end(&mut out);
    let _ = writer.join();
    let _ = errs.join();
    let status = sail_api::snapshot::read_status(resumed.status);
    Ok((out, status, t.elapsed().as_secs_f64()))
}

fn transcripts(inputs: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut v: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
                v.sort();
                for c in v {
                    walk(&c, out);
                }
            }
        } else if p.to_string_lossy().ends_with(".in.smt2") {
            out.push(p.to_path_buf());
        }
    }
    let mut out = Vec::new();
    for p in inputs {
        walk(p, &mut out);
    }
    out
}

/// Runs the experiment, calling `each` with every transcript's report as it
/// completes.
pub fn execute(o: &Options, each: &(dyn Fn(&FileReport) + Sync)) -> Vec<FileReport> {
    let files = transcripts(&o.inputs);
    let next = Mutex::new(0usize);
    let done = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.max(1) {
            s.spawn(|| {
                loop {
                    let k = {
                        let Ok(mut g) = next.lock() else { return };
                        let k = *g;
                        *g += 1;
                        k
                    };
                    let Some(f) = files.get(k) else { return };
                    let rep = one(o, f, k);
                    each(&rep);
                    if let Ok(mut d) = done.lock() {
                        d.push(rep);
                    }
                }
            });
        }
    });
    done.into_inner().unwrap_or_default()
}

/// Totals over reports, as one summary line per comparison kind.
#[must_use]
pub fn summary(reports: &[FileReport]) -> String {
    let mut s = String::new();
    let add = |acc: &mut (usize, usize, usize, usize, usize, usize), c: &Cmp| {
        acc.0 += 1;
        acc.1 += usize::from(c.exact());
        acc.2 += c.compared;
        acc.3 += c.identical;
        acc.4 += c.masked_identical;
        acc.5 += c.differ;
    };
    let (mut tr, mut mk, mut rs) = ((0, 0, 0, 0, 0, 0), (0, 0, 0, 0, 0, 0), (0, 0, 0, 0, 0, 0));
    let mut errors = 0;
    let mut bad_status = 0;
    let mut keys: [std::collections::BTreeMap<String, usize>; 3] = Default::default();
    let mut non_stats = [0usize; 3];
    let mut tally = |i: usize, c: &Cmp| {
        non_stats[i] += c.non_stats_differ;
        for (k, n) in &c.stat_keys {
            *keys[i].entry(k.clone()).or_default() += n;
        }
    };
    let (mut fresh, mut marked) = (0.0, 0.0);
    let (mut parked, mut pss, mut private) = (0, 0, 0);
    for r in reports {
        errors += usize::from(r.error.is_some());
        fresh += r.fresh_secs;
        marked += r.marked_secs;
        parked += r.parked;
        if let Some((_, p, u)) = r.parked_kb {
            pss += p;
            private += u;
        }
        if let Some(c) = &r.trace {
            add(&mut tr, c);
            tally(0, c);
        }
        if let Some(c) = &r.marked {
            add(&mut mk, c);
            tally(1, c);
        }
        for x in &r.resumes {
            add(&mut rs, &x.cmp);
            tally(2, &x.cmp);
            bad_status += usize::from(x.status != Some(0));
        }
    }
    let line = |label: &str, a: (usize, usize, usize, usize, usize, usize)| {
        format!(
            "{label}: {} of {} runs exact; replies {} compared, {} identical, {} identical when masked, {} different\n",
            a.1, a.0, a.2, a.3, a.4, a.5
        )
    };
    let _ = writeln!(s, "transcripts: {} ({errors} with errors)", reports.len());
    s.push_str(&line("trace vs fresh (stock baseline)", tr));
    s.push_str(&line("marked vs fresh", mk));
    s.push_str(&line("resumed vs fresh", rs));
    for (i, label) in ["trace", "marked", "resumed"].iter().enumerate() {
        let _ = writeln!(
            s,
            "{label}: non-statistics replies different {}; statistics keys different: {:?}",
            non_stats[i], keys[i]
        );
    }
    let _ = writeln!(
        s,
        "resumed exit status not 0: {bad_status}\nfresh {fresh:.1} s, marked {marked:.1} s\nparked snapshots: {parked}; memory (Linux) pss {pss} kB, private {private} kB"
    );
    s
}

#[cfg(test)]
mod tests {
    use super::{chosen, mask, points, replies};

    #[test]
    fn masks_only_time_and_memory() {
        let a = b"(:memory 1.5\n :rlimit-count 7\n :time 0.01)\n";
        let b = b"(:memory 2.5\n :rlimit-count 7\n :time 0.02)\n";
        assert_eq!(mask(a), mask(b));
        // `:time` is absent when it rounds to zero.
        assert_eq!(mask(a), mask(b"(:memory 2.5\n :rlimit-count 7)\n"));
        let c = b"(:memory 2.5\n :rlimit-count 8\n :time 0.02)\n";
        assert_ne!(mask(a), mask(c));
    }

    #[test]
    fn points_are_top_level_pushes() {
        let input = b"(set-option :rlimit 5)\n(push)\n(push)\n(check-sat)\n(pop)\n(get-info :all-statistics)\n(pop)\n(push)\n(echo \"x\")\n(pop)\n";
        let (pts, replies, checks) = points(input).unwrap_or_default();
        assert_eq!(pts, vec![(23, 0), (88, 2)]);
        assert_eq!((replies, checks), (3, 1));
    }

    #[test]
    fn splits_replies() {
        assert_eq!(replies(b"unsat\n(:a 1)\nx\n").len(), 3);
    }

    #[test]
    fn spreads_points() {
        assert_eq!(chosen(3, 8), vec![0, 1, 2]);
        assert_eq!(chosen(10, 3), vec![0, 4, 9]);
        assert_eq!(chosen(0, 3), Vec::<usize>::new());
    }
}
