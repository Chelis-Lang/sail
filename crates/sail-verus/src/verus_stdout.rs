//! What Verus would have printed on stdout, for managed runs.
//!
//! The managed driver always asks Verus for `--output-json --time`: the
//! per-target report and receipt are built from that document. Stock
//! `cargo verus verify` without those flags prints a human summary instead,
//! and with them prints the document itself. So that `sail verify` prints
//! what plain `cargo verus verify` prints (charter rule 1, ADR 0045), the
//! driver adds only the flags the user did not pass, and rebuilds the user's
//! stdout from the document:
//!
//! - with the user's own `--output-json`: the document as Verus printed it,
//!   without the `times-ms` object when the user did not ask for `--time`;
//! - otherwise: any other text Verus printed, the `verification results::`
//!   line under exactly the conditions Verus prints it, and with the user's
//!   `--time` (or `--time-expanded`), Verus's timing table, rebuilt from
//!   `times-ms`.
//!
//! The summary line is Verus's own format (`rust_verify/src/verifier.rs`,
//! `finish_verus`); the timing table follows `rust_verify/src/main.rs`.

use serde_json::Value;

/// The Verus flags the user passed (after `--`) that change Verus's stdout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one independent bool per Verus command-line flag"
)]
pub(crate) struct UserFlags {
    /// `--output-json`.
    pub json: bool,
    /// `--time` itself (not `--time-expanded`).
    pub time: bool,
    /// `--time-expanded`, which implies `--time` in Verus.
    pub time_expanded: bool,
    /// `--no-verify`: Verus prints no summary line.
    pub no_verify: bool,
}

impl UserFlags {
    /// The flags among `args` after the first `--`, where `cargo verus`
    /// arguments end and the Verus arguments it forwards begin.
    pub(crate) fn of(args: &[String]) -> Self {
        let mut flags = Self::default();
        for arg in args.iter().skip_while(|a| *a != "--").skip(1) {
            match arg.as_str() {
                "--output-json" => flags.json = true,
                "--time" => flags.time = true,
                "--time-expanded" => flags.time_expanded = true,
                "--no-verify" => flags.no_verify = true,
                _ => {}
            }
        }
        flags
    }

    /// Whether Verus measures and reports time for the user.
    fn timed(self) -> bool {
        self.time || self.time_expanded
    }

    /// The structured-output flags the driver adds: only those the user did
    /// not pass, since Verus rejects an option given twice.
    pub(crate) fn sail_flags(self) -> Vec<&'static str> {
        let mut flags = Vec::new();
        if !self.json {
            flags.push("--output-json");
        }
        if !self.time {
            flags.push("--time");
        }
        flags
    }
}

/// The bytes plain Verus would have written to stdout, given what it wrote
/// with Sail's structured-output flags.
pub(crate) fn rebuild(captured: &[u8], flags: UserFlags) -> Vec<u8> {
    if flags.json {
        if flags.timed() {
            return captured.to_vec();
        }
        return without_times(captured);
    }
    let (docs, rest) = crate::attribution::json_documents(captured);
    let mut out = rest;
    for doc in &docs {
        if !flags.no_verify
            && let Some(line) = summary(doc)
        {
            out.extend_from_slice(line.as_bytes());
        }
        if flags.timed()
            && let Some(table) = doc.get("times-ms").and_then(|t| timing(t, doc, flags))
        {
            out.extend_from_slice(table.as_bytes());
        }
    }
    out
}

/// Verus's `verification results::` line for one document, if Verus prints
/// one. Verus prints it unless verification never ran: a VIR error (the
/// document then has no `verified` count) or a Rust error before
/// verification. The document records the latter only as
/// `encountered-error` with nothing verified and no errors, which is how it
/// is recognised here.
fn summary(doc: &Value) -> Option<String> {
    let results = doc.get("verification-results")?;
    let verified = results.get("verified")?.as_u64()?;
    let errors = results.get("errors")?.as_u64()?;
    let encountered = results
        .get("encountered-error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if encountered && verified == 0 && errors == 0 {
        return None;
    }
    let entire = results
        .get("is-verifying-entire-crate")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Some(format!(
        "verification results:: {verified} verified, {errors} errors{}\n",
        if entire {
            ""
        } else {
            " (partial verification with `--verify-*`)"
        }
    ))
}

/// The pretty-printed document without its top-level `times-ms` object.
/// Verus inserts `times-ms` before its build information, so it is never
/// the last member. Anything unexpected leaves the bytes unchanged.
fn without_times(captured: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(captured);
    if text.as_bytes() != captured {
        return captured.to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    let mut removed = false;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches('\n');
        if skipping {
            if bare == "  }," {
                skipping = false;
            }
            continue;
        }
        if bare == "  \"times-ms\": {" {
            skipping = true;
            removed = true;
            continue;
        }
        out.push_str(line);
    }
    if skipping || !removed {
        return captured.to_vec();
    }
    out.into_bytes()
}

fn ms(value: &Value, path: &[&str]) -> u64 {
    path.iter()
        .try_fold(value, |v, key| v.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// The three slowest `(module, milliseconds, rlimit)` rows of a `times-ms`
/// array, ordered by `order` (Verus orders SMT rows by microseconds), as
/// Verus lists them.
fn slowest(times: &Value, path: &[&str], order: &str) -> Vec<(String, u64, Option<u64>)> {
    let mut rows: Vec<(u64, String, u64, Option<u64>)> = path
        .iter()
        .try_fold(times, |v, k| v.get(k))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|row| {
            (
                row[order].as_u64().unwrap_or(0),
                row["module"].as_str().unwrap_or_default().to_owned(),
                row["time"].as_u64().unwrap_or(0),
                row["rlimit"].as_u64(),
            )
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    rows.into_iter()
        .take(3)
        .map(|(_, module, time, count)| (module, time, count))
        .collect()
}

fn rlimit(count: Option<u64>) -> String {
    count.map_or_else(String::new, |rc| format!(", {rc:>8} rlimit"))
}

/// Verus's human `--time` report, rebuilt from the `times-ms` object and the
/// document's build information. Timings vary from run to run in Verus
/// itself; the layout is Verus's.
fn timing(t: &Value, doc: &Value, flags: UserFlags) -> Option<String> {
    use std::fmt::Write as _;
    let build = doc.get("verus")?;
    let field = |key: &str| build.get(key).and_then(Value::as_str).unwrap_or_default();
    let platform = |key: &str| {
        build
            .get("platform")
            .and_then(|p| p.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
    };
    let threads = ms(t, &["num-threads"]);
    let total = ms(t, &["total"]);
    let rust = ms(t, &["rust", "total"]);
    let verify = ms(t, &["verification", "total"]);
    let vir = ms(t, &["verification", "vir", "total"]);
    let mut s = String::new();
    let _ = writeln!(s, "(use --output-json for machine-readable output)");
    let _ = writeln!(s, "verus-build-info");
    let _ = writeln!(s, "Verus");
    let _ = writeln!(s, "  Version: {}", field("version"));
    let _ = writeln!(s, "  Profile: {}", field("profile"));
    let _ = writeln!(s, "  Platform: {}_{}", platform("os"), platform("arch"));
    let _ = writeln!(s, "  Toolchain: {}", field("toolchain"));
    let _ = writeln!(s);
    let _ = write!(s, "total-time:      {total:>10} ms");
    if threads > 1 {
        let _ = writeln!(
            s,
            "    (estimated total cpu time {} ms)",
            ms(t, &["estimated-cpu-time"])
        );
    } else {
        let _ = writeln!(s);
    }
    let _ = writeln!(s, "    rust-time:          {rust:>10} ms");
    for (label, key) in [
        ("init-and-types:     ", "init-and-types"),
        ("trait-conflicts:    ", "trait-conflicts"),
        ("compile-time:       ", "compile"),
    ] {
        let _ = writeln!(s, "        {label}{:>10} ms", ms(t, &["rust", key]));
    }
    let _ = writeln!(s, "    verification-time:  {verify:>10} ms");
    let _ = writeln!(s, "        vir-time:           {vir:>10} ms");
    for (label, key) in [
        ("hir-time:           ", "hir"),
        ("import-time:        ", "import"),
        ("rust-to-vir:        ", "rust-to-vir"),
    ] {
        let _ = writeln!(
            s,
            "            {label}{:>10} ms",
            ms(t, &["verification", "vir", key])
        );
    }
    let _ = writeln!(
        s,
        "        verify-crate-time:  {:>10} ms",
        verify.saturating_sub(vir)
    );
    let _ = writeln!(
        s,
        "    unaccounted-time:   {:>10} ms",
        total.saturating_sub(rust + verify)
    );
    breakdown(&mut s, t, threads, flags.time_expanded);
    Some(s)
}

/// The `verify-crate-time-breakdown` part of Verus's `--time` report, with
/// the three slowest modules per line under `--time-expanded`.
fn breakdown(s: &mut String, t: &Value, threads: u64, expanded_rows: bool) {
    use std::fmt::Write as _;
    let _ = writeln!(s, "\nverify-crate-time-breakdown");
    let _ = writeln!(
        s,
        "    total verify-time:     {:>10} ms   ({threads} threads)",
        ms(t, &["total-verify"])
    );
    let expanded =
        |s: &mut String, rows: Vec<(String, u64, Option<u64>)>, indent: &str, rl: bool| {
            if expanded_rows {
                for (i, (module, time, count)) in rows.into_iter().enumerate() {
                    let _ = writeln!(
                        s,
                        "{indent}{}. {module:<40} {time:>10} ms{}",
                        i + 1,
                        if rl { rlimit(count) } else { String::new() }
                    );
                }
            }
        };
    expanded(
        s,
        slowest(t, &["total-verify-module-times"], "time"),
        "      ",
        false,
    );
    let _ = writeln!(
        s,
        "        total air-time:        {:>10} ms   ({threads} threads)",
        ms(t, &["air", "total"])
    );
    expanded(
        s,
        slowest(t, &["air", "module-times"], "time"),
        "      ",
        false,
    );
    if let Some(smt) = t.get("smt") {
        let _ = writeln!(
            s,
            "        total smt-time:        {:>10} ms   ({threads} threads)",
            ms(smt, &["total"])
        );
        let _ = writeln!(
            s,
            "            total smt-init:        {:>10} ms{} ({threads} threads)",
            ms(smt, &["smt-init"]),
            rlimit(smt.get("rlimit-init").and_then(Value::as_u64))
        );
        expanded(
            s,
            slowest(smt, &["smt-init-module-times"], "time-micros"),
            "                ",
            false,
        );
        let _ = writeln!(
            s,
            "            total smt-run:         {:>10} ms{} ({threads} threads)",
            ms(smt, &["smt-run"]),
            rlimit(smt.get("rlimit-run").and_then(Value::as_u64))
        );
        expanded(
            s,
            slowest(smt, &["smt-run-module-times"], "time-micros"),
            "                ",
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{UserFlags, rebuild};

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    const DOC: &str = r#"{
  "func-details": {},
  "verification-results": {
    "encountered-error": true,
    "encountered-vir-error": false,
    "success": false,
    "verified": 1,
    "errors": 1,
    "is-verifying-entire-crate": true
  },
  "times-ms": {
    "num-threads": 1,
    "total": 10
  },
  "verus": {
    "profile": "release"
  }
}
"#;

    #[test]
    fn only_missing_flags_are_added() {
        let flags = UserFlags::of(&strings(&["-p", "x", "--", "--time", "--rlimit", "5"]));
        assert_eq!(flags.sail_flags(), ["--output-json"]);
        let flags = UserFlags::of(&strings(&["--output-json", "--", "--time-expanded"]));
        assert_eq!(flags.sail_flags(), ["--output-json", "--time"]);
        let flags = UserFlags::of(&strings(&["--", "--output-json", "--time"]));
        assert_eq!(flags.sail_flags(), Vec::<&str>::new());
    }

    #[test]
    fn the_summary_line_is_verus_own() {
        let out = rebuild(DOC.as_bytes(), UserFlags::default());
        assert_eq!(out, b"verification results:: 1 verified, 1 errors\n");
        let partial = DOC.replace(
            "\"is-verifying-entire-crate\": true",
            "\"is-verifying-entire-crate\": false",
        );
        assert_eq!(
            rebuild(partial.as_bytes(), UserFlags::default()),
            b"verification results:: 1 verified, 1 errors (partial verification with `--verify-*`)\n"
        );
        // A Rust error before verification: Verus prints no line.
        let early = DOC
            .replace("\"verified\": 1", "\"verified\": 0")
            .replace("\"errors\": 1", "\"errors\": 0");
        assert_eq!(
            rebuild(early.as_bytes(), UserFlags::default()),
            Vec::<u8>::new()
        );
        // A VIR error: no counts, no line.
        let vir = DOC
            .replace("    \"verified\": 1,\n", "")
            .replace("    \"errors\": 1,\n", "");
        assert_eq!(
            rebuild(vir.as_bytes(), UserFlags::default()),
            Vec::<u8>::new()
        );
        // Other text Verus printed is kept.
        let mut noisy = b"note\n".to_vec();
        noisy.extend_from_slice(DOC.as_bytes());
        assert_eq!(
            rebuild(&noisy, UserFlags::default()),
            b"note\nverification results:: 1 verified, 1 errors\n"
        );
        let flags = UserFlags {
            no_verify: true,
            ..UserFlags::default()
        };
        assert_eq!(rebuild(DOC.as_bytes(), flags), Vec::<u8>::new());
    }

    #[test]
    fn a_requested_document_is_passed_through_without_unrequested_times() {
        let json = UserFlags {
            json: true,
            ..UserFlags::default()
        };
        let out = String::from_utf8(rebuild(DOC.as_bytes(), json)).unwrap_or_default();
        assert!(
            !out.contains("times-ms") && out.contains("\"verus\""),
            "{out}"
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(&out).is_ok(),
            "{out}"
        );
        let timed = UserFlags {
            json: true,
            time: true,
            ..UserFlags::default()
        };
        assert_eq!(rebuild(DOC.as_bytes(), timed), DOC.as_bytes());
    }

    #[test]
    fn time_prints_verus_table() {
        let flags = UserFlags {
            time: true,
            ..UserFlags::default()
        };
        let out = String::from_utf8(rebuild(DOC.as_bytes(), flags)).unwrap_or_default();
        assert!(
            out.starts_with(
                "verification results:: 1 verified, 1 errors\n(use --output-json for machine-readable output)\nverus-build-info\nVerus\n"
            ) && out.contains("  Profile: release\n") && out.contains("\n\ntotal-time:              10 ms\n")
                && out.contains("\nverify-crate-time-breakdown\n"),
            "{out}"
        );
    }
}
