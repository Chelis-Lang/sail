//! v0.3.0 Track B spike driver (proof health, retention, trace profile).
//! Rough measurement tooling, not a product surface.
//!
//! ```text
//! health_spike census DIR                      per-check TSV of captured sessions
//! health_spike slice Z3 LIST JOBS              sliced vs full-session replay
//! health_spike perturb Z3 LIST JOBS            sliced replay, untraced vs traced
//! health_spike perturb-full Z3 LIST JOBS       whole sessions, untraced vs traced
//! health_spike profile Z3 INPUT SEQ TOP [KEEP] traced sliced replay through a FIFO
//! health_spike count TRACE TOP                 count a trace file
//! health_spike health REPORT.json HISTORY N    per-function health cost
//! ```
//! LIST lines are `INPUT_PATH<TAB>CHECK_SEQ` (`perturb-full`: `INPUT_PATH`).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    missing_docs
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sail_portfolio::fragility::limits_at_checks;
use sail_portfolio::profile::{Counter, Profile, origin};
use sail_portfolio::slice::slice_check;
use sail_portfolio::transcript::{
    CheckObservation, Class, Cmd, Transcript, split_commands, split_replies,
};
use sail_portfolio::{Solver, parallel, sessions};

fn text(b: Option<&[u8]>) -> String {
    b.map_or_else(
        || "-".to_owned(),
        |b| String::from_utf8_lossy(b).trim().replace(['\t', '\n'], " "),
    )
}

fn num(n: Option<u64>) -> String {
    n.map_or_else(|| "-".to_owned(), |n| n.to_string())
}

fn check_indices(commands: &[Cmd]) -> Vec<usize> {
    commands
        .iter()
        .enumerate()
        .filter(|(_, c)| c.class == Class::Check)
        .map(|(i, _)| i)
        .collect()
}

struct Ran {
    checks: Vec<CheckObservation>,
    errors: usize,
    unpaired: usize,
    infra: Option<String>,
    ms: u128,
}

fn run(solver: &Solver, input: &[u8], extra: &[OsString]) -> Ran {
    match sail_solver::run_with_args(solver, extra, input) {
        Ok(r) => {
            let t = Transcript::from_parts(split_commands(input), &split_replies(&r.stdout));
            Ran {
                checks: t.checks(),
                errors: t.errors.len(),
                unpaired: t.unpaired_batches,
                infra: (r.outcome == sail_solver::Outcome::InfraTimeout)
                    .then(|| "infra-timeout".to_owned()),
                ms: r.elapsed.as_millis(),
            }
        }
        Err(e) => Ran {
            checks: Vec::new(),
            errors: 0,
            unpaired: 0,
            infra: Some(e.to_string()),
            ms: 0,
        },
    }
}

fn trace_args(file: &Path) -> Vec<OsString> {
    let mut name = OsString::from("trace_file_name=");
    name.push(file);
    vec![OsString::from("trace=true"), name]
}

fn census(dir: &Path) {
    println!(
        "session\tin_bytes\tout_bytes\tseq\tanswer\tlimit\tused\tfraction\treason\tunpaired\tinput"
    );
    for s in sessions::discover(dir).unwrap_or_default() {
        let (Ok(input), Ok(output)) = (std::fs::read(&s.input), std::fs::read(&s.output)) else {
            eprintln!("unreadable session {}", s.id);
            continue;
        };
        let t = Transcript::new(&input, &output);
        let limits = limits_at_checks(&t.commands);
        let checks = t.checks();
        if checks.is_empty() {
            println!(
                "{}\t{}\t{}\t-\t-\t-\t-\t-\t-\t{}\t{}",
                s.id,
                input.len(),
                output.len(),
                t.unpaired_batches,
                s.input.display()
            );
        }
        for (seq, c) in checks.iter().enumerate() {
            let limit = limits.get(seq).copied().flatten();
            let fraction = match (c.rlimit_delta, limit) {
                (Some(u), Some(l)) if l > 0 => format!("{:.4}", u as f64 / l as f64),
                _ => "-".to_owned(),
            };
            println!(
                "{}\t{}\t{}\t{seq}\t{}\t{}\t{}\t{fraction}\t{}\t{}\t{}",
                s.id,
                input.len(),
                output.len(),
                text(c.answer.as_deref()),
                num(limit),
                num(c.rlimit_delta),
                text(c.reason_unknown()),
                t.unpaired_batches,
                s.input.display()
            );
        }
    }
}

fn read_list(path: &Path) -> Vec<(PathBuf, usize)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (p, seq) = l.split_once('\t').unwrap_or((l, "0"));
            (PathBuf::from(p), seq.trim().parse().unwrap_or(0))
        })
        .collect()
}

fn slice_compare(solver: &Solver, list: &Path, jobs: usize) {
    let mut by_session: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (p, seq) in read_list(list) {
        by_session.entry(p).or_default().push(seq);
    }
    let items: Vec<(PathBuf, Vec<usize>)> = by_session.into_iter().collect();
    println!(
        "input\tseq\tkept\tdropped\tfull_answer\tfull_rlimit\tslice_answer\tslice_rlimit\tratio\tsame_answer\tfull_session_ms\tslice_ms\tslice_bytes\tnote"
    );
    let rows = parallel::map(
        &items,
        jobs,
        |(path, seqs)| {
            let Ok(input) = std::fs::read(path) else {
                return vec![format!("{}\t-\tunreadable", path.display())];
            };
            let commands = split_commands(&input);
            let indices = check_indices(&commands);
            let full = run(solver, &input, &[]);
            seqs.iter()
                .map(|&seq| {
                    let Some(slice) = indices.get(seq).and_then(|&i| slice_check(&commands, i))
                    else {
                        return format!("{}\t{seq}\tno-slice", path.display());
                    };
                    let sliced = run(solver, &slice.input, &[]);
                    let f = full.checks.get(seq);
                    let s = sliced.checks.first();
                    let (fa, sa) = (
                        text(f.and_then(|c| c.answer.as_deref())),
                        text(s.and_then(|c| c.answer.as_deref())),
                    );
                    let (fr, sr) = (
                        f.and_then(|c| c.rlimit_delta),
                        s.and_then(|c| c.rlimit_delta),
                    );
                    let ratio = match (fr, sr) {
                        (Some(a), Some(b)) if a > 0 => format!("{:.4}", b as f64 / a as f64),
                        _ => "-".to_owned(),
                    };
                    format!(
                        "{}\t{seq}\t{}\t{}\t{fa}\t{}\t{sa}\t{}\t{ratio}\t{}\t{}\t{}\t{}\t{}",
                        path.display(),
                        slice.kept,
                        slice.dropped,
                        num(fr),
                        num(sr),
                        fa == sa && fa != "-",
                        full.ms,
                        sliced.ms,
                        slice.input.len(),
                        format_args!(
                            "full_err={} slice_err={} slice_unpaired={} infra={:?}/{:?}",
                            full.errors, sliced.errors, sliced.unpaired, full.infra, sliced.infra
                        ),
                    )
                })
                .collect()
        },
        |n| eprintln!("sessions done: {n}"),
    );
    for r in rows.into_iter().flatten() {
        println!("{r}");
    }
}

fn perturb(solver: &Solver, list: &Path, jobs: usize, whole: bool) {
    let items = read_list(list);
    println!(
        "input\tseq\tplain_answer\tplain_rlimit\ttraced_answer\ttraced_rlimit\tsame_answer\tsame_rlimit\tplain_ms\ttraced_ms\tnote"
    );
    let rows = parallel::map(
        &items,
        jobs,
        |(path, seq)| {
            let Ok(session) = std::fs::read(path) else {
                return vec![format!("{}\t{seq}\tunreadable", path.display())];
            };
            let input = if whole {
                session
            } else {
                let commands = split_commands(&session);
                let Some(slice) = check_indices(&commands)
                    .get(*seq)
                    .and_then(|&i| slice_check(&commands, i))
                else {
                    return vec![format!("{}\t{seq}\tno-slice", path.display())];
                };
                slice.input
            };
            let plain = run(solver, &input, &[]);
            let traced = run(solver, &input, &trace_args(Path::new("/dev/null")));
            let n = plain.checks.len().max(traced.checks.len());
            (0..n)
                .map(|i| {
                    let (p, t) = (plain.checks.get(i), traced.checks.get(i));
                    let (pa, ta) = (
                        text(p.and_then(|c| c.answer.as_deref())),
                        text(t.and_then(|c| c.answer.as_deref())),
                    );
                    let (pr, tr) = (
                        p.and_then(|c| c.rlimit_delta),
                        t.and_then(|c| c.rlimit_delta),
                    );
                    format!(
                        "{}\t{}\t{pa}\t{}\t{ta}\t{}\t{}\t{}\t{}\t{}\tinfra={:?}/{:?} err={}/{}",
                        path.display(),
                        if whole { i } else { *seq },
                        num(pr),
                        num(tr),
                        pa == ta && pa != "-",
                        pr == tr && pr.is_some(),
                        plain.ms,
                        traced.ms,
                        plain.infra,
                        traced.infra,
                        plain.errors,
                        traced.errors,
                    )
                })
                .collect()
        },
        |n| eprintln!("items done: {n}"),
    );
    for r in rows.into_iter().flatten() {
        println!("{r}");
    }
}

fn print_profile(p: &Profile, top: usize) {
    println!(
        "lines={} checks={} complete={} quantifier_instances={} theory_instances={} unattributed_instances={} unattributed_matches={} qids={} peak_pending={}",
        p.lines,
        p.checks,
        p.complete,
        p.instances(),
        p.theory_instances,
        p.unattributed_instances,
        p.unattributed_matches,
        p.quantifiers.len(),
        p.peak_pending
    );
    println!("rank\tinstances\tmatches\tshare\tkind\titem\tqid");
    let total = p.instances().max(1);
    for (i, q) in p.quantifiers.iter().take(top).enumerate() {
        let o = origin(&q.qid);
        println!(
            "{}\t{}\t{}\t{:.1}%\t{}\t{}\t{}",
            i + 1,
            q.instances,
            q.matches,
            100.0 * q.instances as f64 / total as f64,
            o.kind,
            o.item.as_deref().unwrap_or("-"),
            q.qid
        );
    }
}

/// Runs the sliced check with tracing into a FIFO and counts the stream.
fn profile(solver: &Solver, input: &Path, seq: usize, top: usize, keep: Option<&Path>) {
    let Ok(session) = std::fs::read(input) else {
        eprintln!("cannot read {}", input.display());
        return;
    };
    let commands = split_commands(&session);
    let Some(slice) = check_indices(&commands)
        .get(seq)
        .and_then(|&i| slice_check(&commands, i))
    else {
        eprintln!("cannot slice check {seq}");
        return;
    };
    let plain = run(solver, &slice.input, &[]);
    let fifo = std::env::temp_dir().join(format!("sail-profile-{}.fifo", std::process::id()));
    let _ = std::fs::remove_file(&fifo);
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    if !made.is_ok_and(|s| s.success()) {
        eprintln!("cannot create {}", fifo.display());
        return;
    }
    let reader = {
        let fifo = fifo.clone();
        let keep = keep.map(Path::to_path_buf);
        std::thread::spawn(move || {
            let mut counter = Counter::new();
            let mut bytes = 0u64;
            let mut copy = keep.and_then(|k| std::fs::File::create(k).ok());
            if let Ok(mut f) = std::fs::File::open(&fifo) {
                let mut buf = vec![0u8; 1 << 20];
                while let Ok(n) = f.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let chunk = buf.get(..n).unwrap_or_default();
                    bytes += n as u64;
                    counter.push(chunk);
                    if let Some(c) = copy.as_mut() {
                        let _ = c.write_all(chunk);
                    }
                }
            }
            (counter.finish(), bytes)
        })
    };
    let started = Instant::now();
    let traced = run(solver, &slice.input, &trace_args(&fifo));
    let traced_wall = started.elapsed();
    std::thread::sleep(Duration::from_millis(200));
    if !reader.is_finished() {
        // The solver never opened the FIFO: unblock the reader.
        let _ = std::fs::OpenOptions::new().write(true).open(&fifo);
    }
    let (p, bytes) = reader.join().unwrap_or_default();
    let _ = std::fs::remove_file(&fifo);
    let show = |name: &str, r: &Ran| {
        let c = r.checks.first();
        println!(
            "{name}: answer={} rlimit={} reason={} ms={} errors={} infra={:?}",
            text(c.and_then(|c| c.answer.as_deref())),
            num(c.and_then(|c| c.rlimit_delta)),
            text(c.and_then(CheckObservation::reason_unknown)),
            r.ms,
            r.errors,
            r.infra
        );
    };
    println!(
        "slice: bytes={} kept={} dropped={}",
        slice.input.len(),
        slice.kept,
        slice.dropped
    );
    show("sliced untraced", &plain);
    show("sliced traced  ", &traced);
    println!(
        "trace: bytes={bytes} wall_ms={} (streamed through a FIFO, not stored)",
        traced_wall.as_millis()
    );
    print_profile(&p, top);
}

fn count_file(path: &Path, top: usize) {
    let started = Instant::now();
    let mut counter = Counter::new();
    let mut bytes = 0u64;
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = vec![0u8; 1 << 20];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 {
                break;
            }
            bytes += n as u64;
            counter.push(buf.get(..n).unwrap_or_default());
        }
    }
    let p = counter.finish();
    let secs = started.elapsed().as_secs_f64();
    println!(
        "bytes={bytes} seconds={secs:.3} MiB_per_s={:.0}",
        bytes as f64 / 1_048_576.0 / secs.max(1e-9)
    );
    print_profile(&p, top);
}

/// Health of one function: the largest used/limit over its checks.
fn collect(value: &serde_json::Value, worst: &mut Option<(u64, u64)>, checks: &mut u64) {
    match value {
        serde_json::Value::Object(map) => {
            if let (Some(limit), Some(used)) = (
                map.get("rlimit_limit").and_then(serde_json::Value::as_u64),
                map.get("rlimit_used").and_then(serde_json::Value::as_u64),
            ) {
                *checks += 1;
                // Compare used/limit without floats: a/b > c/d iff a*d > c*b.
                let better = worst.is_none_or(|(u, l)| {
                    u128::from(used) * u128::from(l) > u128::from(u) * u128::from(limit)
                });
                if limit > 0 && better {
                    *worst = Some((used, limit));
                }
            }
            for v in map.values() {
                collect(v, worst, checks);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                collect(v, worst, checks);
            }
        }
        _ => {}
    }
}

fn functions<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.contains_key("function") && map.contains_key("verdict") {
                out.push(value);
            } else {
                for v in map.values() {
                    functions(v, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                functions(v, out);
            }
        }
        _ => {}
    }
}

fn health(report: &Path, history: &Path, rounds: usize) {
    let Ok(bytes) = std::fs::read(report) else {
        eprintln!("cannot read {}", report.display());
        return;
    };
    let started = Instant::now();
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        eprintln!("not JSON");
        return;
    };
    let parse = started.elapsed();
    let mut compute = Vec::new();
    let mut append = Vec::new();
    let mut summary = String::new();
    for round in 0..rounds {
        let started = Instant::now();
        let mut fns = Vec::new();
        functions(&doc, &mut fns);
        let mut rows: Vec<(f64, &str, u64, u64, u64)> = Vec::new();
        let mut total_checks = 0u64;
        for f in &fns {
            let (mut worst, mut checks) = (None, 0u64);
            collect(f, &mut worst, &mut checks);
            total_checks += checks;
            if let Some((used, limit)) = worst {
                let name = f
                    .get("function")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                rows.push((used as f64 / limit as f64, name, used, limit, checks));
            }
        }
        rows.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        compute.push(started.elapsed());
        let started = Instant::now();
        let mut lines = String::new();
        for (fraction, name, used, limit, checks) in &rows {
            let class = match *fraction {
                f if f >= 1.0 => "exhausted",
                f if f >= 0.5 => "near-limit",
                f if f >= 0.25 => "watch",
                _ => "ok",
            };
            lines.push_str(&format!(
                "{{\"function\":{},\"used\":{used},\"limit\":{limit},\"margin\":{:.6},\"class\":\"{class}\",\"checks\":{checks}}}\n",
                serde_json::Value::from(*name),
                1.0 - fraction
            ));
        }
        let wrote = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(history)
            .and_then(|mut f| {
                f.write_all(lines.as_bytes())?;
                f.sync_all()
            });
        append.push(started.elapsed());
        if round == 0 {
            let over = |t: f64| rows.iter().filter(|r| r.0 >= t).count();
            summary = format!(
                "functions={} with_effort={} checks_with_effort={total_checks} watch(>=25%)={} near-limit(>=50%)={} exhausted(>=100%)={} history_bytes_per_run={} write_ok={}",
                fns.len(),
                rows.len(),
                over(0.25),
                over(0.5),
                over(1.0),
                lines.len(),
                wrote.is_ok()
            );
            for r in rows.iter().take(10) {
                println!("{:.1}%\t{}\tused={}\tlimit={}", 100.0 * r.0, r.1, r.2, r.3);
            }
        }
    }
    let ms = |d: &[Duration]| {
        let mut v: Vec<f64> = d.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        v.sort_by(f64::total_cmp);
        format!(
            "min={:.3} median={:.3} max={:.3}",
            v.first().copied().unwrap_or(0.0),
            v.get(v.len() / 2).copied().unwrap_or(0.0),
            v.last().copied().unwrap_or(0.0)
        )
    };
    println!("{summary}");
    println!(
        "report_bytes={} parse_ms={:.3} (the product holds the report in memory; shown as an upper bound)",
        bytes.len(),
        parse.as_secs_f64() * 1000.0
    );
    println!("compute_ms {} over {rounds} rounds", ms(&compute));
    println!(
        "history_append_fsync_ms {} over {rounds} rounds",
        ms(&append)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let n = |i: usize, d: usize| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(d);
    let solver = |i: usize| Solver {
        path: PathBuf::from(arg(i)),
        timeout: Duration::from_secs(3600),
    };
    match arg(0).as_str() {
        "census" => census(Path::new(&arg(1))),
        "slice" => slice_compare(&solver(1), Path::new(&arg(2)), n(3, 4)),
        "perturb" => perturb(&solver(1), Path::new(&arg(2)), n(3, 4), false),
        "perturb-full" => perturb(&solver(1), Path::new(&arg(2)), n(3, 4), true),
        "profile" => profile(
            &solver(1),
            Path::new(&arg(2)),
            n(3, 0),
            n(4, 10),
            args.get(5).map(Path::new),
        ),
        "count" => count_file(Path::new(&arg(1)), n(2, 10)),
        "health" => health(Path::new(&arg(1)), Path::new(&arg(2)), n(3, 20)),
        _ => eprintln!("see the module documentation for usage"),
    }
}
