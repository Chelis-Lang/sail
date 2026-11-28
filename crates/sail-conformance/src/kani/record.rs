//! One run of one harness by one arm, and the outcome taxonomy (ADR 0181).
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which side produced a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    /// The original Kani driver, run without Sail.
    Direct,
    /// Sail's Kani backend as users run it.
    Sail,
}

/// Why a run made no claim. Every no-claim outcome is never cached and never
/// reported as success (ADR 0181).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoClaim {
    /// A deadline fired (Sail's own, or Kani's `--harness-timeout`).
    Timeout,
    /// The kernel killed a process for memory.
    OutOfMemory,
    /// A tool died by a signal or panicked.
    Crash,
    /// The user cancelled.
    Cancelled,
    /// The harness kind or tool configuration is not admitted yet.
    Unsupported,
    /// The project did not build.
    BuildFailure,
    /// Tool identity or output integrity did not hold.
    Integrity,
    /// Refused for another stated reason.
    Refused,
}

/// What a run claims about a harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "reason")]
pub enum Claim {
    /// Every check succeeds within the bounds (tier Bounded).
    BoundedSuccess,
    /// A non-unwinding property fails within the bounds (tier Bounded).
    Counterexample,
    /// Only unwinding checks fail: complete, with no tier.
    BoundInsufficient,
    /// No claim, with the reason.
    NoClaim(NoClaim),
}

impl Claim {
    /// The direct driver's claim, read from its own exit status, verdict line,
    /// export result and failed-check categories. It never consults Sail code.
    #[must_use]
    pub fn of_direct(record: &KaniRunRecord) -> Self {
        if record.signal.is_some() {
            return Self::NoClaim(NoClaim::Crash);
        }
        let verdicts: Vec<&str> = record
            .stdout
            .lines()
            .filter_map(|line| line.strip_prefix("VERIFICATION:- "))
            .collect();
        let result = record
            .export
            .as_ref()
            .and_then(|e| e["verification_results"]["results"].as_array())
            .and_then(|r| r.first());
        let Some(result) = result else {
            return Self::NoClaim(if record.stderr.contains("could not compile") {
                NoClaim::BuildFailure
            } else {
                NoClaim::Refused
            });
        };
        let failed: Vec<&Value> = result["checks"]
            .as_array()
            .map(|c| c.iter().filter(|c| c["status"] == "Failure").collect())
            .unwrap_or_default();
        match (record.exit, verdicts.as_slice(), result["status"].as_str()) {
            (Some(0), ["SUCCESSFUL"], Some("Success")) if failed.is_empty() => Self::BoundedSuccess,
            (Some(1), ["FAILED"], Some("Failure")) if !failed.is_empty() => {
                if failed.iter().all(|c| c["category"] == "unwind") {
                    Self::BoundInsufficient
                } else {
                    Self::Counterexample
                }
            }
            _ if record
                .stdout
                .contains("CBMC appears to have run out of memory") =>
            {
                Self::NoClaim(NoClaim::OutOfMemory)
            }
            _ if record.stdout.contains("CBMC timed out") => Self::NoClaim(NoClaim::Timeout),
            _ => Self::NoClaim(NoClaim::Refused),
        }
    }

    /// Whether this claims an outcome.
    #[must_use]
    pub fn is_claim(self) -> bool {
        !matches!(self, Self::NoClaim(_))
    }
}

/// One harness run by one arm, with everything the comparison reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KaniRunRecord {
    /// The arm.
    pub arm: Arm,
    /// Corpus project.
    pub project: String,
    /// Exact harness name.
    pub harness: String,
    /// Unwind bound passed.
    pub unwind: u32,
    /// The driver's argv (after the program).
    pub argv: Vec<String>,
    /// The driver's complete environment.
    pub environment: BTreeMap<String, String>,
    /// This run's private directory; its target, home, tmp and export live
    /// under it, so paths below it are compared relative to it.
    pub capture: PathBuf,
    /// Driver exit status, if it exited.
    pub exit: Option<i32>,
    /// Signal that ended the driver, if any.
    pub signal: Option<i32>,
    /// Complete driver stdout.
    pub stdout: String,
    /// Complete driver stderr.
    pub stderr: String,
    /// The `--export-json` document, strictly parsed, when one was written.
    pub export: Option<Value>,
    /// The arm's claim: for Sail what Sail reported, for direct what the
    /// driver's own output states.
    pub claim: Claim,
    /// Sail's refusal message, when it made no claim.
    pub refusal: Option<String>,
    /// Sail's process exit status (Sail arm only).
    pub sail_exit: Option<i32>,
    /// Wall time of the arm, milliseconds.
    pub wall_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn record(exit: i32, verdict: &str, status: &str, checks: &Value) -> KaniRunRecord {
        KaniRunRecord {
            arm: Arm::Direct,
            project: "p".into(),
            harness: "h".into(),
            unwind: 1,
            argv: Vec::new(),
            environment: BTreeMap::new(),
            capture: PathBuf::from("/c"),
            exit: Some(exit),
            signal: None,
            stdout: format!("x\nVERIFICATION:- {verdict}\n"),
            stderr: String::new(),
            export: Some(
                json!({"verification_results":{"results":[{"status":status,"checks":checks}]}}),
            ),
            claim: Claim::NoClaim(NoClaim::Refused),
            refusal: None,
            sail_exit: None,
            wall_ms: 0,
        }
    }

    #[test]
    fn direct_claims_follow_exit_verdict_result_and_categories() {
        let ok = json!([{"id":1,"status":"Success","category":"assertion"}]);
        let cex = json!([{"id":1,"status":"Failure","category":"assertion"}]);
        let unwind = json!([{"id":1,"status":"Failure","category":"unwind"}]);
        assert_eq!(
            Claim::of_direct(&record(0, "SUCCESSFUL", "Success", &ok)),
            Claim::BoundedSuccess
        );
        assert_eq!(
            Claim::of_direct(&record(1, "FAILED", "Failure", &cex)),
            Claim::Counterexample
        );
        assert_eq!(
            Claim::of_direct(&record(1, "FAILED", "Failure", &unwind)),
            Claim::BoundInsufficient
        );
        // A success verdict with a failing exit, or a failure with no failed
        // check, is not a claim.
        assert!(!Claim::of_direct(&record(1, "SUCCESSFUL", "Success", &ok)).is_claim());
        assert!(!Claim::of_direct(&record(1, "FAILED", "Failure", &ok)).is_claim());
        let mut killed = record(0, "SUCCESSFUL", "Success", &ok);
        killed.signal = Some(9);
        assert_eq!(Claim::of_direct(&killed), Claim::NoClaim(NoClaim::Crash));
    }
}
