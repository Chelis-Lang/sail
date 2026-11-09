//! Shared schema-v1 report assembly; the existing golden stays here.

pub(crate) use sail_verus::report::{Inputs, build, setup_failure};

/// Function counts for a human summary line, with Verus's own totals.
///
/// `unknown` functions that Verus's totals clear (see
/// [`sail_api::output::CrateReport::unknown_passed_by_verus_totals`]) are
/// shown as their own category; their verdicts stay `unknown`.
///
/// A crate that Verus reports unsuccessful with no verification error and no
/// function did not compile, so nothing in it was verified or refuted. It is
/// counted in `not_compiled` and described as such, never as `0 verified, 0
/// failed, 0 unknown`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) functions: u64,
    pub(crate) verified: u64,
    pub(crate) failed: u64,
    pub(crate) unknown: u64,
    pub(crate) passed_by_verus_totals: u64,
    /// Crates that failed to compile before any function was verified.
    pub(crate) not_compiled: u64,
    verus_verified: u64,
    verus_errors: u64,
    verus_totals: bool,
}

impl Counts {
    pub(crate) fn of(report: &sail_api::output::Report) -> Self {
        let mut counts = Self::default();
        counts.add(report);
        counts
    }

    /// The counts of several reports together.
    pub(crate) fn sum<'a>(reports: impl Iterator<Item = &'a sail_api::output::Report>) -> Self {
        let mut counts = Self::default();
        for report in reports {
            counts.add(report);
        }
        counts
    }

    /// Whether nothing was verified because compilation failed first.
    pub(crate) fn compilation_failed(&self) -> bool {
        self.not_compiled > 0 && self.functions == 0
    }

    fn add(&mut self, report: &sail_api::output::Report) {
        let s = &report.summary;
        self.functions = self.functions.saturating_add(s.functions);
        self.verified = self.verified.saturating_add(s.verified);
        self.failed = self.failed.saturating_add(s.failed);
        self.unknown = self.unknown.saturating_add(s.unknown);
        for c in &report.crates {
            self.passed_by_verus_totals = self
                .passed_by_verus_totals
                .saturating_add(c.unknown_passed_by_verus_totals());
            self.verus_totals |= c.verus.verified.is_some();
            self.verus_verified = self
                .verus_verified
                .saturating_add(c.verus.verified.unwrap_or(0));
            self.verus_errors = self
                .verus_errors
                .saturating_add(c.verus.errors.unwrap_or(0));
            if c.functions.is_empty()
                && c.verus.success == Some(false)
                && c.verus.errors.unwrap_or(0) == 0
            {
                self.not_compiled = self.not_compiled.saturating_add(1);
            }
        }
    }
}

impl std::fmt::Display for Counts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.compilation_failed() {
            return write!(f, "compilation failed before verification");
        }
        let cleared = self.passed_by_verus_totals;
        let unknown = self.unknown.saturating_sub(cleared);
        if self.failed > 0 {
            // A failing result leads with what failed, and does not open
            // with `0 verified` when Verus's totals cleared the rest.
            write!(f, "{} failed, ", self.failed)?;
            if cleared > 0 {
                write!(
                    f,
                    "{cleared} passed by Verus's totals (not attributed one by one), "
                )?;
            }
            if self.verified > 0 || cleared == 0 {
                write!(f, "{} verified, ", self.verified)?;
            }
            write!(f, "{unknown} unknown")?;
        } else {
            write!(f, "{} verified, 0 failed, {unknown} unknown", self.verified)?;
        }
        if self.verus_totals
            && self.verus_verified.saturating_add(self.verus_errors) != self.functions
        {
            write!(
                f,
                " (functions; Verus counts verification units: {} verified, {} errors)",
                self.verus_verified, self.verus_errors
            )?;
        }
        if self.not_compiled > 0 {
            write!(
                f,
                "; compilation failed before verification in {} crate{}",
                self.not_compiled,
                if self.not_compiled == 1 { "" } else { "s" }
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Golden test: the fixture under `tests/fixtures/attr` is a real Verus
    //! run on `input.rs` (pinned release, `--output-json --time --log
    //! smt-transcript --log air`), with its logs trimmed to their structure
    //! (each batch keeps only `push`, `pop`, `check-sat`, `get-info`,
    //! `get-model`, `set-option` and `echo`; replies are verbatim; AIR logs
    //! keep their comments and `check-valid` lines). Regenerate the golden
    //! file with `SAIL_BLESS=1 cargo test -p sail-solve`.
    use super::*;
    use sail_api::output::SourceSummaryName as S;
    use sail_api::output::{AttributionStatus, FunctionReport, Report};
    use sail_verus::attribution::{LogFile, Provenance, SessionRecord, attribute, read_logs};
    use sail_verus::report::incomplete_failures;
    use std::path::{Path, PathBuf};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/attr")
            .join(name)
    }

    #[test]
    fn summary_counts_name_what_verus_totals_clear_and_the_counting_unit() {
        let mut r: Report =
            serde_json::from_str(include_str!("../tests/fixtures/attr/report.golden.json"))
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert_eq!(
            super::Counts::of(&r).to_string(),
            "2 failed, 6 verified, 0 unknown (functions; Verus counts verification units: 9 verified, 2 errors)"
        );
        for f in r.crates.iter_mut().flat_map(|c| &mut c.functions) {
            if f.verdict == sail_api::output::VerdictName::Verified {
                f.verdict = sail_api::output::VerdictName::Unknown;
            }
        }
        r.summary = sail_api::output::Summary::of(&r.crates);
        assert_eq!(
            super::Counts::of(&r).to_string(),
            "2 failed, 6 passed by Verus's totals (not attributed one by one), 0 unknown (functions; Verus counts verification units: 9 verified, 2 errors)"
        );
        // A passing report keeps its wording.
        for c in &mut r.crates {
            c.functions
                .retain(|f| f.verdict != sail_api::output::VerdictName::Failed);
            for f in &mut c.functions {
                f.verdict = sail_api::output::VerdictName::Verified;
            }
            c.verus.errors = Some(0);
            c.verus.verified = Some(6);
        }
        r.summary = sail_api::output::Summary::of(&r.crates);
        assert_eq!(
            super::Counts::of(&r).to_string(),
            "6 verified, 0 failed, 0 unknown"
        );
    }

    /// The crate entry is the one a real Rust type error produced (pinned
    /// Verus, `sail verify --output json`): Verus reports no verification
    /// error, no function and `success: false`.
    #[test]
    fn summary_counts_say_when_compilation_failed_before_verification() {
        let mut r: Report =
            serde_json::from_str(include_str!("../tests/fixtures/attr/report.golden.json"))
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        let verified = r.clone();
        let krate = &mut r.crates[0];
        krate.functions.clear();
        krate.verus = sail_api::output::VerusResults {
            verified: Some(0),
            errors: Some(0),
            success: Some(false),
            encountered_vir_error: Some(false),
        };
        r.summary = sail_api::output::Summary::of(&r.crates);
        let counts = super::Counts::of(&r);
        assert!(counts.compilation_failed());
        assert_eq!(counts.to_string(), "compilation failed before verification");
        // Beside a crate that was verified, the counts stay and say so.
        let both = super::Counts::sum([&r, &verified].into_iter());
        assert!(!both.compilation_failed());
        assert_eq!(
            both.to_string(),
            "2 failed, 6 verified, 0 unknown (functions; Verus counts verification units: 9 verified, 2 errors); compilation failed before verification in 1 crate"
        );
        // A crate with no function that Verus reports successful compiled.
        r.crates[0].verus.success = Some(true);
        assert_eq!(
            super::Counts::of(&r).to_string(),
            "0 verified, 0 failed, 0 unknown"
        );
    }

    fn logs() -> Vec<LogFile> {
        read_logs(&fixture("logs")).unwrap_or_default()
    }

    fn report(provenance: &Provenance) -> Report {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, rest) = sail_verus::attribution::json_documents(&verus);
        assert_eq!(rest, b"");
        let logs = logs();
        let attribution = attribute(&docs, Some(&logs), provenance, None);
        let stderr = std::fs::read_to_string(fixture("stderr.txt")).unwrap_or_default();
        let diagnostics = sail_verus::diagnostics::parse(&stderr);
        build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: matches!(provenance, Provenance::Records(_)),
            attribution: &attribution,
            diagnostics: &diagnostics,
        })
    }

    fn function<'a>(r: &'a Report, name: &str) -> Option<&'a FunctionReport> {
        r.crates
            .iter()
            .flat_map(|c| &c.functions)
            .find(|f| f.function == name)
    }

    #[test]
    fn golden_report() {
        let mut r = report(&Provenance::NoCache);
        // The golden must not change with every release version bump.
        r.sail_version = "0.0.0".to_owned();
        let text = serde_json::to_string_pretty(&r).unwrap_or_default() + "\n";
        let golden = fixture("report.golden.json");
        if std::env::var_os("SAIL_BLESS").is_some() {
            assert!(std::fs::write(&golden, &text).is_ok());
        }
        let want = std::fs::read_to_string(&golden).unwrap_or_default();
        assert!(
            text == want,
            "report differs from {}; rerun with SAIL_BLESS=1 to update:\n{text}",
            golden.display()
        );
        let schema: serde_json::Value =
            serde_json::from_str(sail_api::output::JSON_SCHEMA).unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        assert_eq!(
            sail_api::schema::validate(&schema, &value),
            Vec::<String>::new()
        );
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn verdicts_agree_with_verus_and_close_the_bit_vector_gap() {
        let r = report(&Provenance::NoCache);
        assert_eq!(
            r.attribution.status,
            AttributionStatus::Complete,
            "{:?}",
            r.attribution.notes
        );
        for f in r.crates.iter().flat_map(|c| &c.functions) {
            let verified = f.verdict == sail_api::output::VerdictName::Verified;
            if f.function == "input::arith::bv_bad" {
                // Verus's JSON says success, but its bit-vector query failed
                // (docs/verus-issues/VI-0003); its diagnostics and exit code
                // agree with sail.
                assert_eq!(f.verus_success, Some(true));
                assert!(!verified);
            } else {
                assert_eq!(Some(verified), f.verus_success, "{}", f.function);
            }
            assert!(
                f.obligations.as_ref().is_some_and(|o| !o.is_empty()),
                "{}",
                f.function
            );
        }
        let bad = function(&r, "input::bad");
        let ids: Vec<&str> = bad
            .into_iter()
            .flat_map(|f| f.obligations.iter().flatten())
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "input::bad#Function-Def/0",
                "input::bad#Function-Recommends/0"
            ]
        );
        let def = bad
            .and_then(|f| f.obligations.as_ref())
            .and_then(|o| o.first());
        assert!(def.is_some_and(|o| {
            o.counterexample
                .as_ref()
                .is_some_and(|c| c.assignments.iter().any(|a| a.name == "x!"))
        }));
        let mul = function(&r, "input::arith::mul_pos");
        let ids: Vec<&str> = mul
            .into_iter()
            .flat_map(|f| f.obligations.iter().flatten())
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "input::arith::mul_pos#Function-Def/0",
                "input::arith::mul_pos#Function-Def/spinoff-1"
            ]
        );
    }

    #[test]
    fn incomplete_attribution_never_confirms_hidden_bit_vector_failures() {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, _) = sail_verus::attribution::json_documents(&verus);
        let attribution = attribute(&docs, None, &Provenance::NoCache, None);
        let r = build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        });
        assert_eq!(r.summary.verified, 0);
        let bad = function(&r, "input::arith::bv_bad");
        assert!(bad.is_some_and(|f| f.verus_success == Some(true)
            && f.verdict == sail_api::output::VerdictName::Unknown));
        assert!(r.attribution.notes.iter().any(|n| n.contains("VI-0003")));
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn successful_crates_keep_confirmed_verus_results_without_logs() {
        let mut r = report(&Provenance::NoCache);
        r.attribution.status = AttributionStatus::Unavailable;
        for c in &mut r.crates {
            c.verus.errors = Some(0);
            c.verus.success = Some(true);
        }
        let verified = r.summary.verified;
        incomplete_failures(&mut r);
        assert_eq!(r.summary.verified, verified);
    }

    #[test]
    fn cache_sources_come_from_session_records() {
        // A record for the root module's process saying the cache served its
        // first 20 replies: checks before reply 20 are cached, later ones
        // solved; processes without a record are unrecorded.
        let root = logs().into_iter().find(|l| l.stem == "root");
        let Some(root) = root else {
            unreachable!("no root log in the fixture");
        };
        let records = vec![
            SessionRecord {
                version: 1,
                input_sha256: root.input_sha256.clone(),
                replies_served: 20,
                solver_started: true,
            },
            SessionRecord {
                version: 1,
                input_sha256: "00".repeat(32),
                replies_served: 3,
                solver_started: false,
            },
        ];
        let r = report(&Provenance::Records(records));
        assert_eq!(r.attribution.unmatched_sessions, 1);
        assert_eq!(r.attribution.status, AttributionStatus::Partial);
        let source = |name: &str| function(&r, name).map(|f| f.provenance.source);

        assert_eq!(source("input::Counter::incr"), Some(S::Cache));
        assert_eq!(source("input::main"), Some(S::Fresh));
        assert_eq!(source("input::arith::mul_pos"), Some(S::Unrecorded));
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn without_logs_raw_verus_results_are_preserved_without_false_proofs() {
        let verus = std::fs::read(fixture("verus.json")).unwrap_or_default();
        let (docs, _) = sail_verus::attribution::json_documents(&verus);
        let a = attribute(&docs, None, &Provenance::NoCache, Some("multiple packages"));
        let r = build(&Inputs {
            args: &[],
            exit_code: Some(1),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &a,
            diagnostics: &[],
        });
        assert_eq!(r.attribution.status, AttributionStatus::Unavailable);
        assert_eq!(
            r.attribution.notes.first().map(String::as_str),
            Some("multiple packages")
        );
        assert!(r.attribution.notes.iter().any(|n| n.contains("VI-0003")));
        assert_eq!(r.summary.functions, 8);
        assert_eq!(r.summary.failed, 1, "the known raw failure is retained");
        assert_eq!(r.summary.unknown, 7);
        assert_eq!(r.summary.verified, 0);
        assert!(r.crates.iter().flat_map(|c| &c.functions).all(|f| {
            match f.verus_success {
                Some(true) => f.verdict == sail_api::output::VerdictName::Unknown,
                Some(false) => f.verdict == sail_api::output::VerdictName::Failed,
                None => false,
            }
        }));
        assert!(
            r.crates
                .iter()
                .flat_map(|c| &c.functions)
                .all(|f| f.obligations.is_none())
        );
        assert_eq!(r.invariant_violations(), Vec::<String>::new());
    }

    #[test]
    fn setup_failures_are_valid_documents() {
        let r = setup_failure(&["x".to_owned()], "SAIL0101", "no cargo-verus");
        let schema: serde_json::Value =
            serde_json::from_str(sail_api::output::JSON_SCHEMA).unwrap_or_default();
        let value = serde_json::to_value(&r).unwrap_or_default();
        assert_eq!(
            sail_api::schema::validate(&schema, &value),
            Vec::<String>::new()
        );
    }
}
