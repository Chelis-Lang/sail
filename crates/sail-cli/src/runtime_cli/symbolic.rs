//! A trusted external Dive rechecks a portable predicate in each process.
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::fs;
use std::io::Write;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

mod process;
use std::time::{Duration, Instant};

pub(super) const USAGE: &str = "sail runtime symbolic-check --dive FILE --session DIR --module MODULE --config CFG --time-limit SECONDS [--type-invariant NAME] [--property-proof NAME] [--strategy symbolic|occupancy|finite-induction] [--max-orbits N]";
const MAX_BYTES: u64 = 16 << 20;
struct Strategy {
    option: &'static str,
    report: &'static str,
    candidate: &'static str,
}

const SYMBOLIC: Strategy = Strategy {
    option: "symbolic",
    report: "dive-symbolic-report-v1",
    candidate: "sail-symbolic-predicate-v1.json",
};

const INDUCTION: Strategy = Strategy {
    option: "finite-induction",
    report: "dive-finite-induction-report-v1",
    candidate: "sail-finite-induction-certificate-v1.json",
};

const SUPPORT_FORMAT: &str = "dive-occupancy-support-v1";

const OCCUPANCY: Strategy = Strategy {
    option: "occupancy",
    report: "dive-occupancy-report-v1",
    candidate: "sail-occupancy-predicate-v1.json",
};

struct Job {
    dive: PathBuf,
    module: PathBuf,
    config: PathBuf,
    session: PathBuf,
    duration: Duration,
    type_invariant: Option<String>,
    strategy: Strategy,
    max_orbits: Option<NonZeroU64>,
    property: Option<String>,
}

fn failure(message: impl std::fmt::Display) -> String {
    format!("error[SAIL6102]: symbolic check: {message}")
}

impl Job {
    fn parse(args: &[String]) -> Result<Self, String> {
        let usage = || format!("error[SAIL6101]: usage: {USAGE}");
        let mut flags = std::collections::BTreeMap::new();
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            if !matches!(
                flag.as_str(),
                "--dive"
                    | "--session"
                    | "--module"
                    | "--config"
                    | "--time-limit"
                    | "--type-invariant"
                    | "--property-proof"
                    | "--strategy"
                    | "--max-orbits"
            ) || flags
                .insert(flag.as_str(), args.next().ok_or_else(usage)?)
                .is_some()
            {
                return Err(usage());
            }
        }
        let required = |name: &str| flags.get(name).copied().ok_or_else(usage);
        let file = |name| fs::canonicalize(required(name)?).map_err(failure);
        let seconds = required("--time-limit")?
            .parse::<u64>()
            .map_err(|_| usage())?;
        if seconds == 0 {
            return Err(usage());
        }
        let strategy = match flags.get("--strategy").map(|s| s.as_str()) {
            None | Some("symbolic") => SYMBOLIC,
            Some("occupancy") => OCCUPANCY,
            Some("finite-induction") => INDUCTION,
            _ => return Err(usage()),
        };
        let property = flags.get("--property-proof").map(|s| (*s).clone());
        if property.as_ref().is_some_and(String::is_empty)
            || (property.is_some() && strategy.option != SYMBOLIC.option)
        {
            return Err(usage());
        }
        let max_orbits = flags
            .get("--max-orbits")
            .map(|s| s.parse::<NonZeroU64>().map_err(|_| usage()))
            .transpose()?;
        if max_orbits.is_some() && strategy.option != OCCUPANCY.option {
            return Err(usage());
        }
        Ok(Self {
            strategy,
            property,
            max_orbits,
            dive: file("--dive")?,
            module: file("--module")?,
            config: file("--config")?,
            session: required("--session")?.into(),
            duration: Duration::from_secs(seconds),
            type_invariant: flags.get("--type-invariant").map(|s| (*s).clone()),
        })
    }

    fn check(
        &self,
        option: &str,
        predicate: &Path,
        hypothesis: Option<&Path>,
        started: Instant,
        cancel: &AtomicBool,
    ) -> Result<(u8, Vec<u8>), String> {
        let remaining = self.duration.saturating_sub(started.elapsed()).as_millis();
        if remaining == 0 {
            return Ok(stopped());
        }
        let mut command = Command::new(&self.dive);
        command
            .arg("check")
            .arg(&self.module)
            .arg("--config")
            .arg(&self.config)
            .args([
                "--strategy",
                self.strategy.option,
                "--output",
                "json",
                "--time-limit",
            ])
            .arg(format!("{remaining}ms"))
            .arg(option)
            .arg(predicate);
        if let Some(input) = hypothesis {
            command.arg("--verify-predicate").arg(input);
        }
        if let Some(name) = &self.type_invariant {
            command.arg("--type-invariant").arg(name);
        }
        if let Some(name) = &self.property {
            command.arg("--property-proof").arg(name);
        }
        if let Some(limit) = self.max_orbits {
            command.arg("--max-orbits").arg(limit.to_string());
        }
        process::run(command, started, self.duration, cancel)
    }
}

fn stopped() -> (u8, Vec<u8>) {
    (
        4,
        br#"{"verdict":"stopped","reason":"symbolic session time limit"}"#.to_vec(),
    )
}

// Inspect the protocol without parsing or re-serializing exact state counts.
#[derive(Deserialize)]
struct Report<'a> {
    format: Option<&'a str>,
    module: Option<PathBuf>,
    config: Option<PathBuf>,
    proof: Option<&'a str>,
    property: Option<&'a str>,
    verdict: Option<&'a str>,
    trace_validated: Option<bool>,
    #[serde(default, borrow, deserialize_with = "present")]
    trace: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    trace_file: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    occupancies: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    distinct_decimal: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    generated_decimal: Option<&'a RawValue>,
    predicate_format: Option<&'a str>,
    ending: Option<Ending>,
    #[serde(default, borrow, deserialize_with = "present")]
    failed_obligation: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    distinct: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    generated: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present")]
    depth: Option<&'a RawValue>,
}

fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<&'de RawValue>, D::Error> {
    Deserialize::deserialize(d).map(Some)
}

fn natural(number: Option<&RawValue>) -> bool {
    number.is_some_and(|n| n.get().bytes().all(|b| b.is_ascii_digit()))
}

fn same_decimal(integer: Option<&RawValue>, string: Option<&RawValue>) -> bool {
    integer.zip(string).is_some_and(|(integer, string)| {
        string
            .get()
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            == Some(integer.get())
    })
}

#[derive(Deserialize)]
struct Trace {
    format: String,
    version: u32,
    variables: Vec<String>,
    states: Vec<TraceState>,
    ending: Ending,
}

#[derive(Deserialize)]
struct TraceState {
    // The wire format is an object; only its variable keys are needed here.
    #[allow(clippy::zero_sized_map_values)]
    values: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
}

#[derive(Deserialize, PartialEq)]
struct Ending {
    kind: String,
    name: Option<String>,
}

enum Route {
    Proved,
    Retry,
    Searched,
    Terminal,
}

impl Report<'_> {
    fn valid_witness(&self) -> bool {
        let Some(trace) = self
            .trace
            .and_then(|t| serde_json::from_str::<Trace>(t.get()).ok())
        else {
            return false;
        };
        let length = self.depth.and_then(|n| n.get().parse::<usize>().ok());
        let names: std::collections::BTreeSet<_> = trace.variables.iter().collect();
        names.len() == trace.variables.len()
            && trace.format == "dive-trace"
            && trace.version == 1
            && !trace.states.is_empty()
            && Some(trace.states.len()) == length
            && self.ending.as_ref() == Some(&trace.ending)
            && trace.states.iter().all(|s| {
                s.values.len() == trace.variables.len()
                    && trace.variables.iter().all(|v| s.values.contains_key(v))
            })
    }

    fn bound_to(&self, job: &Job) -> bool {
        let format = if job.property.is_some() {
            "dive-property-proof-v1"
        } else {
            job.strategy.report
        };
        self.format == Some(format)
            && self.property == job.property.as_deref()
            && self.module.as_ref() == Some(&job.module)
            && self.config.as_ref() == Some(&job.config)
    }

    fn admit(&self, code: u8, verifying: bool, job: &Job) -> Result<Route, String> {
        let named = job.property.is_some();
        let bound = self.bound_to(job);
        let population = job.strategy.option == OCCUPANCY.option;
        if (population || named) && self.trace_file.is_some() {
            return Err(failure("unexpected external trace path on scoped report"));
        }
        let predicate_format = !population || self.predicate_format == Some(SUPPORT_FORMAT);
        let no_population_counts = !(population || named)
            || (self.occupancies.is_none()
                && self.distinct_decimal.is_none()
                && self.generated_decimal.is_none());
        let counted = natural(self.distinct)
            && natural(self.generated)
            && natural(self.depth)
            && (!population
                || (natural(self.occupancies)
                    && same_decimal(self.distinct, self.distinct_decimal)
                    && same_decimal(self.generated, self.generated_decimal)));
        let uncounted = self.distinct.is_none()
            && self.generated.is_none()
            && self.depth.is_none()
            && no_population_counts;
        match (verifying, code, self.proof, self.verdict) {
            (_, 0, Some("inductive-safety"), None)
                if (verifying || named || job.strategy.option == INDUCTION.option)
                    && bound
                    && predicate_format
                    && uncounted
                    && self.trace.is_none()
                    && self.ending.is_none()
                    && self.trace_validated.is_none()
                    && self.failed_obligation.is_some_and(|o| o.get() == "null") =>
            {
                Ok(if verifying {
                    Route::Proved
                } else {
                    Route::Searched
                })
            }
            (true, 2, Some("inconclusive"), None)
                if bound
                    && predicate_format
                    && uncounted
                    && self.trace.is_none()
                    && self.ending.is_none()
                    && self.trace_validated.is_none()
                    && self
                        .failed_obligation
                        .is_some_and(|o| o.get().starts_with('"')) =>
            {
                Ok(Route::Retry)
            }
            (true, 2, None, Some("unsupported")) => Ok(Route::Retry),
            (false, 0, None, Some("success"))
                if !named
                    && job.strategy.option != INDUCTION.option
                    && bound
                    && predicate_format
                    && counted
                    && self.trace.is_none()
                    && self.ending.is_none()
                    && self.trace_validated.is_none()
                    && self.failed_obligation.is_none() =>
            {
                Ok(Route::Searched)
            }
            (false, 1, None, Some("invariant-violation" | "deadlock"))
                if bound
                    && no_population_counts
                    && (!population || self.predicate_format.is_none())
                    && self.distinct.is_none()
                    && self.generated.is_none()
                    && self.failed_obligation.is_none()
                    && self.trace_validated == Some(true)
                    && self.valid_witness()
                    && (!named
                        || (self.verdict == Some("invariant-violation")
                            && self.ending.as_ref().and_then(|e| e.name.as_deref())
                                == job.property.as_deref()))
                    && self
                        .ending
                        .as_ref()
                        .is_some_and(|ending| match self.verdict {
                            Some("deadlock") => ending.kind == "deadlock",
                            _ => {
                                ending.kind == "invariant"
                                    && ending.name.as_deref().is_some_and(|name| !name.is_empty())
                            }
                        }) =>
            {
                Ok(Route::Terminal)
            }
            (_, 2, None, Some("unsupported"))
            | (_, 3, None, Some("error"))
            | (_, 4, None, Some("stopped")) => Ok(Route::Terminal),
            _ => Err(failure(format!("unexpected Dive report for exit {code}"))),
        }
    }
}

#[derive(Deserialize)]
struct NamedWork {
    seconds: f64,
    support_bits: u64,
    support_variables: Vec<String>,
    projected_integers: Vec<String>,
    closure_sweeps: u64,
}

#[derive(Deserialize)]
struct ProofWork<'a> {
    #[serde(borrow)]
    transition_obligations: &'a RawValue,
    reused_obligations: u64,
}

fn validate_named_work(bytes: &[u8], parsed: &Report<'_>) -> Result<(), String> {
    let work: NamedWork = serde_json::from_slice(bytes).map_err(failure)?;
    if !work.seconds.is_finite()
        || work.seconds < 0.0
        || u64::try_from(work.support_variables.len()).map_err(failure)? > work.support_bits
        || work
            .projected_integers
            .iter()
            .any(|n| work.support_variables.contains(n))
    {
        return Err(failure("invalid named-property work statistics"));
    }
    if let Some(proof) = parsed.proof {
        let counts: ProofWork<'_> = serde_json::from_slice(bytes).map_err(failure)?;
        let total: Option<u64> =
            serde_json::from_str(counts.transition_obligations.get()).map_err(failure)?;
        let valid = match proof {
            "inductive-safety" => total.is_some_and(|n| counts.reused_obligations <= n),
            "inconclusive" => {
                total.is_none() && counts.reused_obligations == 0 && work.closure_sweeps == 0
            }
            _ => false,
        };
        if !valid {
            return Err(failure("inconsistent named-property induction obligations"));
        }
    }
    Ok(())
}

fn report(bytes: &[u8], named: bool) -> Result<Report<'_>, String> {
    let parsed: Report<'_> =
        serde_json::from_slice(bytes).map_err(|e| failure(format!("invalid Dive report: {e}")))?;
    if named {
        // A scoped proof is a closed wire format. Forwarding unknown fields could
        // attach graph-exhaustion statistics or another property's witness.
        let fields: std::collections::BTreeMap<String, &RawValue> =
            serde_json::from_slice(bytes).map_err(failure)?;
        let diagnostic = matches!(parsed.verdict, Some("unsupported" | "error" | "stopped"));
        if diagnostic {
            #[derive(Deserialize)]
            struct Diagnostic {
                reason: String,
            }
            let d: Diagnostic = serde_json::from_slice(bytes).map_err(failure)?;
            if d.reason.is_empty() {
                return Err(failure("empty named-property diagnostic"));
            }
        } else {
            validate_named_work(bytes, &parsed)?;
        }
        for field in fields.keys().map(String::as_str) {
            let allowed = if diagnostic {
                matches!(field, "verdict" | "reason")
            } else {
                matches!(
                    field,
                    "format"
                        | "module"
                        | "config"
                        | "property"
                        | "projected_integers"
                        | "support_bits"
                        | "support_variables"
                        | "closure_sweeps"
                        | "seconds"
                ) || if parsed.proof.is_some() {
                    matches!(
                        field,
                        "proof"
                            | "transition_obligations"
                            | "reused_obligations"
                            | "failed_obligation"
                    )
                } else {
                    matches!(
                        field,
                        "verdict" | "depth" | "ending" | "trace_validated" | "trace"
                    )
                }
            };
            if !allowed {
                return Err(failure(format!("unexpected named-property field: {field}")));
            }
        }
    }
    Ok(parsed)
}

fn emit(out: &mut dyn Write, code: u8, bytes: &[u8]) -> Result<u8, String> {
    out.write_all(bytes).map_err(failure)?;
    if !bytes.ends_with(b"\n") {
        writeln!(out).map_err(failure)?;
    }
    Ok(code)
}

struct Stage(PathBuf);

impl Stage {
    fn create(dir: &Path) -> Result<Self, String> {
        for suffix in 0..=u32::MAX {
            let path = dir.join(format!("sail-symbolic-{}-{suffix}.tmp", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(failure(e)),
            }
        }
        Err(failure("could not allocate a search staging directory"))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("predicate.json"));
        let _ = fs::remove_dir(&self.0);
    }
}

pub(super) fn run(args: &[String], out: &mut dyn Write) -> Result<u8, String> {
    let started = Instant::now();
    let job = Job::parse(args)?;
    let cancel = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&cancel)).map_err(failure)?;
    }
    fs::create_dir_all(&job.session).map_err(failure)?;
    let dir = fs::canonicalize(&job.session).map_err(failure)?;
    let candidate = dir.join(if job.property.is_some() {
        "sail-named-property-predicate-v1.json"
    } else {
        job.strategy.candidate
    });
    if candidate == job.module || candidate == job.config || candidate == job.dive {
        return Err(failure("session candidate aliases an input file"));
    }
    // The marker belongs to the trusted worker installation. Old installations
    // retain their two-invocation protocol. The shared candidate never selects
    // a capability or establishes a verdict.
    let repair = job.property.is_some()
        && job.dive.parent().is_some_and(|bin| {
            bin.join("libexec/dive-symbolic/capabilities/named-predicate-repair-v1")
                .is_file()
        });
    let mut hypothesis = None;
    match fs::symlink_metadata(&candidate) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES && repair => {
            hypothesis = Some(candidate.as_path());
        }
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => {
            let (code, bytes) =
                job.check("--verify-predicate", &candidate, None, started, &cancel)?;
            let route = report(&bytes, job.property.is_some())?.admit(code, true, &job)?;
            if cancel.load(Ordering::Relaxed) || started.elapsed() >= job.duration {
                let (code, row) = process::stop(&cancel);
                return emit(out, code, &row);
            }
            if !matches!(route, Route::Retry) {
                return emit(out, code, &bytes);
            }
        }
        Ok(meta) if meta.is_file() => (),
        Ok(_) => return Err(failure("session candidate must be a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(failure(e)),
    }
    let stage = Stage::create(&dir)?;
    let pending = stage.0.join("predicate.json");
    let (code, bytes) = job.check("--write-predicate", &pending, hypothesis, started, &cancel)?;
    let route = report(&bytes, job.property.is_some())?.admit(code, false, &job)?;
    if cancel.load(Ordering::Relaxed) || started.elapsed() >= job.duration {
        let (code, row) = process::stop(&cancel);
        return emit(out, code, &row);
    }
    if matches!(route, Route::Searched) {
        let meta = fs::symlink_metadata(&pending).map_err(failure)?;
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_BYTES {
            return Err(failure(
                "completed search provided an invalid candidate file",
            ));
        }
        fs::rename(pending, candidate).map_err(failure)?;
    }
    emit(out, code, &bytes)
}
