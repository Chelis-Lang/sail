//! Function scores from exact session hashes and ordered AIR/transcript checks.

use std::collections::BTreeMap;
use std::path::Path;

use sail_api::output::{Fragility, Report};
use sail_portfolio::fragility::{self, CheckFragility};
use sail_portfolio::sessions::SessionFiles;
use sail_verus::attribution::LogFile;
use sha2::{Digest, Sha256};

/// Sail measurement flags, parsed only before the Verus separator.
pub(crate) fn extract(args: &[String]) -> Result<(Vec<String>, Option<u64>), String> {
    let mut rest = Vec::new();
    let mut enabled = false;
    let mut seeds = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--" => {
                rest.push(arg.clone());
                rest.extend(it.cloned());
                break;
            }
            "--measure-fragility" => enabled = true,
            "--fragility-seeds" => {
                let n = it
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .filter(|n| (1..=32).contains(n))
                    .ok_or_else(|| "--fragility-seeds needs an integer from 1 to 32".to_owned())?;
                seeds = Some(n);
            }
            _ => rest.push(arg.clone()),
        }
    }
    if seeds.is_some() && !enabled {
        return Err("--fragility-seeds requires --measure-fragility".to_owned());
    }
    if enabled && std::env::var("CI").as_deref() == Ok("true") {
        return Err("fragility measurement is local-only".to_owned());
    }
    Ok((rest, enabled.then_some(seeds.unwrap_or(8))))
}

/// Maps every check only if exact input and the entire sequence match. The
/// returned keys identify primary, non-advisory checks used in function scores.
fn bind(
    checks: &mut [CheckFragility],
    sessions: &[SessionFiles],
    logs: &[LogFile],
) -> BTreeMap<String, Vec<(String, usize)>> {
    let mut primary = BTreeMap::<String, Vec<(String, usize)>>::new();
    for session in sessions {
        let Ok(input) = std::fs::read(&session.input) else {
            continue;
        };
        let hash = format!("{:x}", Sha256::digest(input));
        let matching: Vec<_> = logs.iter().filter(|l| l.input_sha256 == hash).collect();
        let [log] = matching.as_slice() else { continue };
        let selected: Vec<_> = checks
            .iter_mut()
            .filter(|c| c.session == session.id)
            .collect();
        let expected = log.ops.iter().map(|op| op.checks.len()).sum::<usize>();
        if log.air.len() != log.ops.len()
            || selected.len() != expected
            || selected.iter().enumerate().any(|(seq, c)| c.seq != seq)
        {
            continue;
        }
        let mut seq = 0;
        for (air, op) in log.air.iter().zip(&log.ops) {
            for (index, _) in op.checks.iter().enumerate() {
                if let Some(c) = checks
                    .iter_mut()
                    .find(|c| c.session == session.id && c.seq == seq)
                {
                    c.function = Some(air.function.clone());
                    if index == 0 && !air.kind.contains("Recommends") {
                        primary
                            .entry(air.function.clone())
                            .or_default()
                            .push((session.id.clone(), seq));
                    }
                }
                seq += 1;
            }
        }
    }
    primary
}

fn score(checks: &[&CheckFragility], runs: usize) -> Option<Fragility> {
    if checks.is_empty()
        || runs == 0
        || checks
            .iter()
            .any(|c| c.answers.len() != runs || c.answers.iter().any(Option::is_none))
    {
        return None;
    }
    let proved = (0..runs)
        .filter(|&i| {
            checks
                .iter()
                .all(|c| c.answers[i].as_deref() == Some("unsat"))
        })
        .count();
    let runs = u32::try_from(runs).ok()?;
    let proved = u32::try_from(proved).ok()?;
    Some(Fragility {
        score: 1.0 - f64::from(proved) / f64::from(runs),
        runs,
        proved,
    })
}

/// Attaches conservative function scores without changing any verdict. Missing
/// mappings and incomplete replays remain null and carry an explanatory note.
pub(crate) fn attach(
    report: &mut Report,
    measured: &mut fragility::Report,
    sessions: &[SessionFiles],
    logs: &[LogFile],
) {
    let primary = bind(&mut measured.checks, sessions, logs);
    measured.functions = fragility::group(&measured.checks, |c| c.function.clone());
    for f in report.crates.iter_mut().flat_map(|c| &mut c.functions) {
        let keys = primary.get(&f.function).cloned().unwrap_or_default();
        let expected = f
            .obligations
            .as_ref()
            .map(|os| os.iter().filter(|o| !o.advisory).count());
        let checks: Vec<_> = keys
            .iter()
            .filter_map(|(s, seq)| {
                measured
                    .checks
                    .iter()
                    .find(|c| &c.session == s && c.seq == *seq)
            })
            .collect();
        let complete = expected == Some(checks.len())
            && keys.iter().all(|(s, _)| {
                !measured
                    .infra
                    .iter()
                    .chain(&measured.unpaired_runs)
                    .any(|e| e.starts_with(&format!("{s}@")))
            });
        f.fragility = if complete {
            score(&checks, measured.configs.len())
        } else {
            None
        };
        if let Some(value) = &f.fragility {
            f.unstable |= value.proved > 0 && value.proved < value.runs;
        } else {
            report.attribution.notes.push(format!(
                "{}: fragility unavailable: missing exact obligation mapping or incomplete replay",
                f.function
            ));
        }
    }
    report.summary = sail_api::output::Summary::of(&report.crates);
    report.attribution.notes.push("Fragility replays captured solver prefixes; it does not establish a complete seeded Verus verification".to_owned());
}

/// Measures captured sessions with the original solver and preserves the
/// original verification report if measurement fails.
pub(crate) fn measure(report: &mut Report, z3: &Path, traces: &Path, logs: &[LogFile], seeds: u64) {
    let result = (|| {
        let found = sail_portfolio::sessions::discover(traces).map_err(|e| e.to_string())?;
        if found.is_empty() {
            return Err("no complete solver sessions were captured".to_owned());
        }
        let solver = sail_portfolio::Solver {
            path: z3.to_path_buf(),
            timeout: std::time::Duration::from_secs(3600),
        };
        let options = fragility::Options {
            seeds,
            jobs: 2,
            ..fragility::Options::default()
        };
        let mut measured =
            fragility::measure(&solver, &found, &options, |_, _| {}).map_err(|e| e.to_string())?;
        attach(report, &mut measured, &found, logs);
        Ok::<(), String>(())
    })();
    if let Err(e) = result {
        report
            .attribution
            .notes
            .push(format!("fragility unavailable: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(answers: &[Option<&str>]) -> CheckFragility {
        serde_json::from_value(serde_json::json!({"session":"s", "seq":0,"function":null,"recorded":"unsat","limit":100,"answers":answers,"reasons":[],"rlimit":[],"answer_counts":{},"proved":0,"proved_fraction":null,"effort":null,"max_used_fraction":null,"margin":null,"stability":"incomplete","score":0})).unwrap_or_else(|e| unreachable!("fixture: {e}"))
    }

    #[test]
    fn function_score_requires_all_obligations_in_the_same_configuration() {
        let a = check(&[Some("unsat"), Some("unknown"), Some("unsat")]);
        let b = check(&[Some("unknown"), Some("unsat"), Some("unsat")]);
        let value = score(&[&a, &b], 3).unwrap_or_else(|| unreachable!("complete"));
        assert_eq!((value.proved, value.runs), (1, 3));
        assert!((value.score - 2.0 / 3.0).abs() < 1e-12);
        assert!(score(&[&check(&[Some("unsat"), None])], 2).is_none());
        assert!(score(&[], 2).is_none());
    }
    #[test]
    fn exact_input_hash_and_complete_sequence_are_required() {
        use sail_verus::attribution::Transcript;
        let transcript = include_bytes!("../tests/fixtures/attr/logs/root.smt_transcript");
        let log = LogFile::new(
            "root",
            include_str!("../tests/fixtures/attr/logs/root.air"),
            transcript,
        );
        let dir = std::env::temp_dir().join(format!("sail-fj-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&dir).is_ok());
        let input = dir.join("s.in.smt2");
        assert!(std::fs::write(&input, Transcript::parse(transcript).pipe_input()).is_ok());
        let session = SessionFiles::from_input("s", &input);
        let count = log.ops.iter().map(|o| o.checks.len()).sum::<usize>();
        let fresh = || {
            (0..count)
                .map(|seq| {
                    let mut c = check(&[Some("unsat")]);
                    c.seq = seq;
                    c
                })
                .collect::<Vec<_>>()
        };
        let mut good = fresh();
        assert!(
            !bind(
                &mut good,
                std::slice::from_ref(&session),
                std::slice::from_ref(&log)
            )
            .is_empty()
        );
        assert!(good.iter().all(|c| c.function.is_some()));
        let mut short = fresh();
        short.pop();
        assert!(
            bind(
                &mut short,
                std::slice::from_ref(&session),
                std::slice::from_ref(&log)
            )
            .is_empty()
        );
        assert!(short.iter().all(|c| c.function.is_none()));
        assert!(std::fs::write(&input, b"different bytes").is_ok());
        let mut bad = fresh();
        assert!(bind(&mut bad, &[session], &[log]).is_empty());
        assert!(bad.iter().all(|c| c.function.is_none()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
