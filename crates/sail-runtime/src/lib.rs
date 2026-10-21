//! Bounded exact SMT execution, independently usable without a daemon.
#![forbid(unsafe_code)]

mod admission;
mod process;
mod store;
#[cfg(test)]
mod tests;

use sail_api::runtime::{
    BackendCapability, Capabilities, Error, Input, Limits, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    MAX_QUERIES, MAX_WALL_TIME_MS, Outcome, Output, Query, QueryResult, Request, Response,
    SolverIdentity, VERSION,
};
use sha2::{Digest as _, Sha256};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Configuration of installed tools and bounded execution; performs no installation.
#[derive(Clone, Debug)]
pub struct Config {
    /// Existing bundled Z3 executable, independently verified against the embedded pin.
    pub z3: Option<PathBuf>,
    /// Optional disk cache root; existing Verus cache namespaces are untouched.
    pub cache_directory: Option<PathBuf>,
    /// Maximum concurrent independent transcripts, from one through sixteen.
    pub workers: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            z3: None,
            cache_directory: None,
            workers: 2,
        }
    }
}

/// Clonable explicit cancellation shared by the caller and actual child processes.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    /// Cancel current and undispatched work. Cancellation is permanent for this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// An exact-input runtime; no Verus serving path is modified by its construction.
pub struct Runtime {
    config: Config,
    pin: Option<(String, String)>,
}

pub(crate) fn invalid(kind: &str, message: impl Into<String>) -> Error {
    Error {
        code: "SAIL6101".into(),
        kind: kind.into(),
        message: message.into(),
    }
}

pub(crate) fn execution(kind: &str, message: impl Into<String>) -> Error {
    Error {
        code: "SAIL6102".into(),
        kind: kind.into(),
        message: message.into(),
    }
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pin() -> Result<Option<(String, String)>, Error> {
    let pins: toml::Value = toml::from_str(include_str!("../../../corpus/toolchain.toml"))
        .map_err(|e| invalid("invalid_config", e.to_string()))?;
    let host = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        _ => return Ok(None),
    };
    let version = pins
        .get("z3")
        .and_then(|z3| z3.get("version"))
        .and_then(toml::Value::as_str);
    let digest = pins
        .get("assets")
        .and_then(|a| a.get(host))
        .and_then(|a| a.get("z3_sha256"))
        .and_then(toml::Value::as_str);
    match (version, digest) {
        (Some(version), Some(digest)) if digest.len() == 64 => {
            Ok(Some((version.into(), digest.into())))
        }
        _ => Err(invalid(
            "invalid_config",
            "embedded solver pin is incomplete",
        )),
    }
}

fn admitted(query: &Query) -> Result<admission::Plan, Error> {
    if query.obligation_id.trim().is_empty()
        || query.obligation_id.len() > 1024
        || query.requirement_ids.len() > 1024
        || query
            .requirement_ids
            .iter()
            .any(|id| id.trim().is_empty() || id.len() > 1024)
        || query
            .requirement_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "invalid_request",
            "obligation identity must be nonempty; requirement IDs must be nonempty, unique and strictly sorted",
        ));
    }
    let limits = &query.limits;
    if query.backend.is_empty()
        || limits.resource_limit == 0
        || limits.wall_time_ms == 0
        || limits.wall_time_ms > MAX_WALL_TIME_MS
        || limits.input_bytes == 0
        || limits.input_bytes > MAX_INPUT_BYTES
        || limits.output_bytes == 0
        || limits.output_bytes > MAX_OUTPUT_BYTES
        || query.script.len() > limits.input_bytes
    {
        return Err(invalid(
            "invalid_limits",
            "backend, script size or limits are outside the admitted bounds",
        ));
    }
    admission::plan(query)
}

impl Runtime {
    /// Construct the runtime with an existing installed solver and optional cache.
    ///
    /// # Errors
    /// Returns `SAIL6101` for invalid configuration or malformed embedded pins.
    pub fn new(mut config: Config) -> Result<Self, Error> {
        if !(1..=16).contains(&config.workers) {
            return Err(invalid(
                "invalid_config",
                "worker bound must be between one and sixteen",
            ));
        }
        // A bare relative executable name must never be hashed in the current
        // directory and then resolved to a different PATH entry at spawn time.
        config.z3 = config
            .z3
            .map(std::path::absolute)
            .transpose()
            .map_err(|e| invalid("invalid_config", e.to_string()))?;
        Ok(Self {
            config,
            pin: pin()?,
        })
    }

    fn identity(&self, cancellation: &Cancellation) -> Result<SolverIdentity, Error> {
        let path =
            self.config.z3.as_ref().ok_or_else(|| {
                execution("unavailable", "bundled Z3 is not installed or configured")
            })?;
        let (version, expected) = self
            .pin
            .as_ref()
            .ok_or_else(|| execution("unavailable", "no bundled Z3 pin exists for this host"))?;
        let digest = store::file_hash(path)
            .map_err(|e| execution("unavailable", format!("cannot read bundled Z3: {e}")))?;
        if &digest != expected {
            return Err(execution(
                "unavailable",
                "solver executable does not match the bundled pin",
            ));
        }
        let limits = Limits {
            wall_time_ms: 1000,
            output_bytes: 4096,
            ..Limits::default()
        };
        let captured = process::run(path, &["--version".into()], b"", &limits, cancellation);
        let text = std::str::from_utf8(&captured.stdout).unwrap_or_default();
        if captured.outcome != Outcome::Completed
            || !captured.stderr.is_empty()
            || text
                .split_whitespace()
                .take(3)
                .ne(["Z3", "version", version.as_str()])
        {
            return Err(execution(
                "unavailable",
                "bundled Z3 version could not be verified",
            ));
        }
        Ok(SolverIdentity {
            backend: "z3".into(),
            version: version.clone(),
            sha256: digest,
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        })
    }

    /// Inspect actual installation readiness; this does not establish any theorem.
    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        let identity = self.identity(&Cancellation::default());
        Capabilities {
            version: VERSION,
            backends: vec![BackendCapability {
                backend: "z3".into(),
                implemented: true,
                installed: identity.is_ok(),
                solver: identity.as_ref().ok().cloned(),
                error: identity.err(),
            }],
            incremental_transcripts: true,
            batches: true,
            max_queries: MAX_QUERIES,
        }
    }

    /// Execute a fully admitted bounded batch, preserving caller order and metadata.
    ///
    /// # Errors
    /// Returns `SAIL6101` before dispatch if any query or protocol version is invalid.
    /// Worker infrastructure failures use `SAIL6102` and never become solver answers.
    pub fn execute(
        &self,
        request: &Request,
        cancellation: &Cancellation,
    ) -> Result<Response, Error> {
        let Input::Smt { queries } = &request.input;
        if request.version != VERSION
            || queries.is_empty()
            || queries.len() > MAX_QUERIES
            || queries
                .iter()
                .try_fold(0_usize, |sum, query| sum.checked_add(query.script.len()))
                .is_none_or(|size| size > MAX_INPUT_BYTES)
            || queries
                .iter()
                .try_fold(0_usize, |sum, query| {
                    query
                        .limits
                        .output_bytes
                        .checked_mul(3)
                        .and_then(|bound| sum.checked_add(bound))
                })
                .is_none_or(|bound| bound > sail_api::runtime::MAX_RETAINED_OUTPUT_BYTES)
        {
            return Err(invalid(
                "invalid_request",
                "unsupported version, empty or oversized batch",
            ));
        }
        let plans = queries
            .iter()
            .map(admitted)
            .collect::<Result<Vec<_>, _>>()?;
        // Join each bounded wave before starting another; no task outlives its request.
        let mut results = Vec::with_capacity(queries.len());
        for (query_wave, plan_wave) in queries
            .chunks(self.config.workers)
            .zip(plans.chunks(self.config.workers))
        {
            let mut wave = std::thread::scope(|scope| {
                let mut workers = Vec::new();
                let mut spawn_error = None;
                for (query, plan) in query_wave.iter().zip(plan_wave) {
                    match std::thread::Builder::new()
                        .name("sail-runtime-query".into())
                        .spawn_scoped(scope, || self.query(query, plan, cancellation))
                    {
                        Ok(worker) => workers.push(worker),
                        Err(e) => {
                            spawn_error = Some(execution("infrastructure", e.to_string()));
                            break;
                        }
                    }
                }
                let mut wave = Vec::new();
                for worker in workers {
                    match worker.join() {
                        Ok(result) => wave.push(result),
                        Err(_) => {
                            spawn_error =
                                Some(execution("infrastructure", "query worker panicked"));
                        }
                    }
                }
                spawn_error.map_or(Ok(wave), Err)
            })?;
            results.append(&mut wave);
        }
        Ok(Response {
            version: VERSION,
            output: Output::Smt { results },
        })
    }

    fn query(
        &self,
        query: &Query,
        plan: &admission::Plan,
        cancellation: &Cancellation,
    ) -> QueryResult {
        let mut result = empty_result(query);
        if cancellation.is_cancelled() {
            result.outcome = Outcome::Cancelled;
            result.error = Some(execution("cancelled", "cancelled before dispatch"));
            return result;
        }
        if query.backend != "z3" {
            result.error = Some(execution(
                "unavailable",
                "requested backend is not implemented",
            ));
            return result;
        }
        let identity = match self.identity(cancellation) {
            Ok(identity) => identity,
            Err(e) => {
                if cancellation.is_cancelled() {
                    result.outcome = Outcome::Cancelled;
                }
                result.error = Some(e);
                return result;
            }
        };
        result.solver = Some(identity.clone());
        if let Some(cached) = store::read(&self.config, query, &identity, plan) {
            result.stdout = cached;
            if let Ok(checks) = admission::responses(plan, &result.stdout) {
                result.checks = checks;
                result.outcome = Outcome::Completed;
                result.cache_hit = true;
                return result;
            }
        }
        let Some(path) = &self.config.z3 else {
            return result;
        };
        let args = vec![
            "-smt2".into(),
            "-in".into(),
            "smt.random_seed=0".into(),
            "sat.random_seed=0".into(),
            format!("rlimit={}", query.limits.resource_limit),
        ];
        let captured = process::run(
            path,
            &args,
            query.script.as_bytes(),
            &query.limits,
            cancellation,
        );
        result.outcome = captured.outcome;
        result.stdout = captured.stdout;
        result.stderr = captured.stderr;
        if result.outcome == Outcome::Completed {
            let checked = if !result.stderr.is_empty() {
                Err(execution(
                    "infrastructure",
                    "backend produced stderr diagnostics",
                ))
            } else if store::file_hash(path).ok().as_ref() != Some(&identity.sha256) {
                Err(execution(
                    "infrastructure",
                    "backend executable changed during execution",
                ))
            } else {
                admission::responses(plan, &result.stdout)
            };
            match checked {
                Ok(checks) => {
                    result.checks = checks;
                    store::write(
                        &self.config,
                        query,
                        &identity,
                        &result.stdout,
                        &result.checks,
                    );
                }
                Err(e) => {
                    result.outcome = Outcome::Infrastructure;
                    result.error = Some(execution("invalid_response", e.to_string()));
                }
            }
        } else {
            result.error = Some(execution(
                match result.outcome {
                    Outcome::Cancelled => "cancelled",
                    Outcome::Timeout => "timeout",
                    Outcome::OutputLimit => "output_limit",
                    _ => "infrastructure",
                },
                captured
                    .error
                    .unwrap_or_else(|| "backend execution did not complete".into()),
            ));
        }
        result
    }
}

fn empty_result(query: &Query) -> QueryResult {
    QueryResult {
        obligation_id: query.obligation_id.clone(),
        requirement_ids: query.requirement_ids.clone(),
        backend: query.backend.clone(),
        outcome: Outcome::Unavailable,
        checks: Vec::new(),
        stdout: Vec::new(),
        stderr: Vec::new(),
        solver: None,
        script_sha256: hash(query.script.as_bytes()),
        limits: query.limits.clone(),
        cache_hit: false,
        error: None,
    }
}
