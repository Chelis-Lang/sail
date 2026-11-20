//! A trusted external Dive rechecks a portable predicate in each process.
use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(super) const USAGE: &str = "sail runtime symbolic-check --dive FILE --session DIR --module MODULE --config CFG --time-limit SECONDS [--type-invariant NAME]";
const MAX_BYTES: u64 = 16 << 20;
const FORMAT: &str = "dive-symbolic-report-v1";

struct Job {
    dive: PathBuf,
    module: PathBuf,
    config: PathBuf,
    session: PathBuf,
    duration: Duration,
    type_invariant: Option<String>,
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
        Ok(Self {
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
        started: Instant,
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
            .args(["--strategy", "symbolic", "--output", "json", "--time-limit"])
            .arg(format!("{remaining}ms"))
            .arg(option)
            .arg(predicate);
        if let Some(name) = &self.type_invariant {
            command.arg("--type-invariant").arg(name);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(failure)?;
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .ok_or_else(|| failure("missing Dive output pipe"))?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes);
        if read.is_err() || bytes.len() as u64 > MAX_BYTES {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failure(
                "unreadable Dive report or report larger than 16 MiB",
            ));
        }
        let status = child.wait().map_err(failure)?;
        let code = status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .ok_or_else(|| failure(format!("Dive exited with {status}")))?;
        if started.elapsed() >= self.duration {
            return Ok(stopped());
        }
        Ok((code, bytes))
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
    verdict: Option<&'a str>,
    trace_validated: Option<bool>,
    trace: Option<serde::de::IgnoredAny>,
    ending: Option<Ending>,
    #[serde(default = "missing_obligation")]
    failed_obligation: serde_json::Value,
    distinct: Option<serde::de::IgnoredAny>,
    generated: Option<serde::de::IgnoredAny>,
    depth: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct Ending {
    kind: String,
    name: Option<String>,
}

// A successful proof explicitly clears the obligation in the worker protocol.
fn missing_obligation() -> serde_json::Value {
    serde_json::Value::Bool(false)
}

enum Route {
    Proved,
    Retry,
    Searched,
    Terminal,
}

impl Report<'_> {
    fn admit(&self, code: u8, verifying: bool, job: &Job) -> Result<Route, String> {
        let bound = self.format == Some(FORMAT)
            && self.module.as_ref() == Some(&job.module)
            && self.config.as_ref() == Some(&job.config);
        let counted = self.distinct.is_some() && self.generated.is_some() && self.depth.is_some();
        let uncounted = self.distinct.is_none() && self.generated.is_none() && self.depth.is_none();
        match (verifying, code, self.proof, self.verdict) {
            (true, 0, Some("inductive-safety"), None)
                if bound && uncounted && self.failed_obligation.is_null() =>
            {
                Ok(Route::Proved)
            }
            (true, 2, Some("inconclusive"), None)
                if bound && uncounted && self.failed_obligation.is_string() =>
            {
                Ok(Route::Retry)
            }
            (true, 2, None, Some("unsupported")) => Ok(Route::Retry),
            (false, 0, None, Some("success")) if bound && counted => Ok(Route::Searched),
            (false, 1, None, Some("invariant-violation" | "deadlock"))
                if bound
                    && self.trace_validated == Some(true)
                    && self.trace.is_some()
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

fn report(bytes: &[u8]) -> Result<Report<'_>, String> {
    serde_json::from_slice(bytes).map_err(|e| failure(format!("invalid Dive report: {e}")))
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
    fs::create_dir_all(&job.session).map_err(failure)?;
    let dir = fs::canonicalize(&job.session).map_err(failure)?;
    let candidate = dir.join("sail-symbolic-predicate-v1.json");
    if candidate == job.module || candidate == job.config || candidate == job.dive {
        return Err(failure("session candidate aliases an input file"));
    }
    match fs::symlink_metadata(&candidate) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => {
            let (code, bytes) = job.check("--verify-predicate", &candidate, started)?;
            if !matches!(report(&bytes)?.admit(code, true, &job)?, Route::Retry) {
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
    let (code, bytes) = job.check("--write-predicate", &pending, started)?;
    if matches!(report(&bytes)?.admit(code, false, &job)?, Route::Searched) {
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
