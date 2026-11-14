//! Per-solver command line, option admission, pin lookup and response dialect.
//!
//! Development `Backend` abstraction (ADR 0134). Z3's
//! argv, admitted options, cache-key material and response rules are the
//! exact values `lib.rs`, `admission.rs` and `store.rs` used before this
//! module existed; `tests::z3_backend_keeps_argv_and_cache_key_bytes` pins them.

use sail_api::runtime::Limits;
use sail_smtlib::{
    lex::TokenKind,
    sexpr::{SExpr, SExprKind},
};

/// One solver family the runtime can execute.
pub(crate) trait Backend: Sync + std::fmt::Debug {
    /// Wire name in `Query::backend` and `SolverIdentity::backend`.
    fn name(&self) -> &'static str;
    /// Name used in diagnostics.
    fn label(&self) -> &'static str;
    /// `(version, executable sha256)` for `host` from the embedded pins.
    /// `Ok(None)` means no pin for this host; `Err` means a malformed pin.
    fn pin(&self, pins: &toml::Value, host: &str) -> Result<Option<(String, String)>, ()>;
    /// Whether `--version` output names exactly the pinned version.
    fn version_matches(&self, stdout: &str, version: &str) -> bool;
    /// Complete argv for one transcript on stdin.
    fn args(&self, limits: &Limits) -> Vec<String>;
    /// Seed and argv policy strings bound into the cache key.
    fn key_policy(&self) -> [&'static str; 2];
    /// The closed `set-option` table.
    fn option_admitted(&self, key: &[u8], value: &[u8], limit: u64) -> bool;
    /// Shape of the value in `(:reason-unknown VALUE)`.
    fn reason_unknown_value(&self, value: &SExpr) -> bool;
}

fn numeral(value: &[u8]) -> Option<u64> {
    std::str::from_utf8(value).ok()?.parse().ok()
}

#[derive(Debug)]
pub(crate) struct Z3;

impl Backend for Z3 {
    fn name(&self) -> &'static str {
        "z3"
    }
    fn label(&self) -> &'static str {
        "bundled Z3"
    }
    fn pin(&self, pins: &toml::Value, host: &str) -> Result<Option<(String, String)>, ()> {
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
            _ => Err(()),
        }
    }
    fn version_matches(&self, stdout: &str, version: &str) -> bool {
        stdout
            .split_whitespace()
            .take(3)
            .eq(["Z3", "version", version])
    }
    fn args(&self, limits: &Limits) -> Vec<String> {
        vec![
            "-smt2".into(),
            "-in".into(),
            "smt.random_seed=0".into(),
            "sat.random_seed=0".into(),
            format!("rlimit={}", limits.resource_limit),
        ]
    }
    fn key_policy(&self) -> [&'static str; 2] {
        ["smt.random_seed=0", "sat.random_seed=0"]
    }
    fn option_admitted(&self, key: &[u8], value: &[u8], limit: u64) -> bool {
        match key {
            b":print-success" => value == b"false",
            b":produce-models" | b":produce-unsat-cores" => matches!(value, b"true" | b"false"),
            b":smt.random_seed" | b":sat.random_seed" => value == b"0",
            b":rlimit" => numeral(value) == Some(limit),
            _ => false,
        }
    }
    fn reason_unknown_value(&self, value: &SExpr) -> bool {
        matches!(value.kind, SExprKind::Atom(TokenKind::String))
    }
}

/// cvc5, executed as `cvc5 --lang=smt2 -q --incremental --seed=0
/// --sat-random-seed=0 --rlimit-per=N` with the transcript on stdin.
///
/// Measured dialect differences from Z3 at cvc5 1.4.2 (spike record):
/// - without `-q` a transcript with no `set-logic` writes a warning to stderr;
/// - an unknown option prints `unsupported` on stdout and continues, so the
///   table below is closed and Z3's dotted seed options are refused;
/// - `--rlimit` is cumulative over the whole transcript, `--rlimit-per`
///   applies to each check (the closer analogue of Z3's `rlimit`);
/// - `:reason-unknown` is a bare symbol (`resourceout`), not a string;
/// - option changes after the first check are an error with exit status 1.
#[derive(Debug)]
pub(crate) struct Cvc5;

impl Backend for Cvc5 {
    fn name(&self) -> &'static str {
        "cvc5"
    }
    fn label(&self) -> &'static str {
        "pinned cvc5"
    }
    fn pin(&self, pins: &toml::Value, host: &str) -> Result<Option<(String, String)>, ()> {
        let Some(cvc5) = pins.get("cvc5") else {
            return Ok(None);
        };
        let version = cvc5.get("version").and_then(toml::Value::as_str);
        let Some(asset) = cvc5.get("assets").and_then(|a| a.get(host)) else {
            return Ok(None);
        };
        let digest = asset.get("bin_sha256").and_then(toml::Value::as_str);
        match (version, digest) {
            (Some(version), Some(digest)) if digest.len() == 64 => {
                Ok(Some((version.into(), digest.into())))
            }
            _ => Err(()),
        }
    }
    fn version_matches(&self, stdout: &str, version: &str) -> bool {
        // "cvc5 1.4.2 [git 282d5e0 on branch HEAD]"; older releases print
        // "This is cvc5 version 1.1.2 [...]", which a 1.4 pin never matches.
        stdout.split_whitespace().take(2).eq(["cvc5", version])
    }
    fn args(&self, limits: &Limits) -> Vec<String> {
        vec![
            "--lang=smt2".into(),
            "-q".into(),
            "--incremental".into(),
            "--seed=0".into(),
            "--sat-random-seed=0".into(),
            format!("--rlimit-per={}", limits.resource_limit),
        ]
    }
    fn key_policy(&self) -> [&'static str; 2] {
        [
            "cvc5-argv-v1:--lang=smt2 -q --incremental --rlimit-per",
            "--seed=0 --sat-random-seed=0",
        ]
    }
    fn option_admitted(&self, key: &[u8], value: &[u8], limit: u64) -> bool {
        match key {
            b":print-success" => value == b"false",
            b":produce-models" | b":produce-unsat-cores" => matches!(value, b"true" | b"false"),
            b":seed" | b":random-seed" | b":sat-random-seed" => value == b"0",
            b":rlimit-per" | b":reproducible-resource-limit" => numeral(value) == Some(limit),
            _ => false,
        }
    }
    fn reason_unknown_value(&self, value: &SExpr) -> bool {
        matches!(
            value.kind,
            SExprKind::Atom(TokenKind::Symbol | TokenKind::String)
        )
    }
}

/// Every implemented backend, Z3 first.
pub(crate) const ALL: [&dyn Backend; 2] = [&Z3, &Cvc5];

/// The backend a query names, if implemented.
pub(crate) fn named(name: &str) -> Option<&'static dyn Backend> {
    ALL.into_iter().find(|backend| backend.name() == name)
}
