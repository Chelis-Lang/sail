//! Closed producer and exact single-harness admission.
use std::collections::{BTreeMap, BTreeSet};

use sail_api::assurance::EvidenceTier;
use serde_json::Value;

use crate::{CBMC_VERSION, Error, KANI_VERSION, MAX_CAPTURE_BYTES, Observation, Outcome, refuse};

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], Error> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| refuse(format!("missing {key} inventory")))
}

fn one<'a>(values: &'a [Value], key: &str, harness: &str) -> Result<&'a Value, Error> {
    let [value] = values else {
        return Err(refuse("expected exactly one selected harness record"));
    };
    if value[key].as_str() != Some(harness) {
        return Err(refuse("selected harness identity differs"));
    }
    Ok(value)
}

pub(super) fn decode(
    harness: &str,
    exit: i32,
    stdout: &[u8],
    export: &[u8],
) -> Result<Observation, Error> {
    if harness.is_empty()
        || harness.len() > 4096
        || stdout.len() > MAX_CAPTURE_BYTES
        || export.len() > MAX_CAPTURE_BYTES
    {
        return Err(refuse("invalid harness or excessive capture"));
    }
    let document: Value =
        serde_json::from_slice(export).map_err(|error| refuse(error.to_string()))?;
    if document["metadata"]["version"] != "1.0"
        || document["metadata"]["kani_version"] != KANI_VERSION
        || document["tools"]["kani"] != KANI_VERSION
        || document["tools"]["cbmc"] != format!("{CBMC_VERSION} (cbmc-{CBMC_VERSION})")
    {
        return Err(refuse("unsupported Kani export or backend producer"));
    }
    let metadata = one(
        array(&document, "harness_metadata")?,
        "pretty_name",
        harness,
    )?;
    if metadata["attributes"]["kind"] != "Proof"
        || metadata["attributes"]["should_panic"] != false
        || metadata["is_automatically_generated"] != false
        || metadata["has_loop_contracts"] != false
        || !metadata["contract"]["contracted_function_name"].is_null()
        || !metadata["contract"]["recursion_tracker"].is_null()
    {
        return Err(refuse(
            "only ordinary hand-written proof harnesses are admitted",
        ));
    }
    let cbmc = one(array(&document, "cbmc")?, "harness_id", harness)?;
    if cbmc["cbmc_metadata"]["version"] != CBMC_VERSION
        || cbmc["configuration"]["solver"] != "cadical"
        || cbmc["configuration"]["object_bits"].as_u64().is_none()
    {
        return Err(refuse("unsupported or missing CBMC configuration"));
    }
    let result = one(
        array(&document["verification_results"], "results")?,
        "harness_id",
        harness,
    )?;
    let text = std::str::from_utf8(stdout).map_err(|error| refuse(error.to_string()))?;
    let verdicts: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("VERIFICATION:- "))
        .collect();
    let expected = match (exit, result["status"].as_str()) {
        (0, Some("Success")) => "SUCCESSFUL",
        (1, Some("Failure")) => "FAILED",
        _ => {
            return Err(refuse(
                "process and native result disagree or did not complete",
            ));
        }
    };
    if verdicts != [expected] {
        return Err(refuse(
            "textual verdict and selected native result disagree",
        ));
    }
    completion(&document, harness, expected)?;
    let (counts, failed) = inventory(result)?;
    let outcome = outcome(expected, &counts, &failed)?;
    Ok(Observation {
        harness: harness.into(),
        outcome,
        evidence_tier: (outcome != Outcome::BoundInsufficient).then_some(EvidenceTier::Bounded),
        check_counts: counts,
        failed_checks: failed,
        harness_metadata: metadata.clone(),
        backend_configuration: cbmc["configuration"].clone(),
    })
}

fn completion(document: &Value, harness: &str, verdict: &str) -> Result<(), Error> {
    let success = verdict == "SUCCESSFUL";
    let summary = &document["verification_results"]["summary"];
    if summary["total_harnesses"] != 1
        || summary["executed"] != 1
        || summary["status"] != "completed"
        || summary["successful"] != u64::from(success)
        || summary["failed"] != u64::from(!success)
    {
        return Err(refuse(
            "incomplete or contradictory native execution summary",
        ));
    }
    let errors = one(array(document, "error_details")?, "harness_id", harness)?;
    if errors["has_errors"] != !success
        || (!success && errors["exit_status"] != "properties_failed")
        || (!success
            && !matches!(
                errors["error_type"].as_str(),
                Some("assertion_failure" | "verification_failure")
            ))
    {
        return Err(refuse("missing or contradictory native error outcome"));
    }
    Ok(())
}

fn inventory(result: &Value) -> Result<(BTreeMap<String, usize>, Vec<Value>), Error> {
    let checks = array(result, "checks")?;
    if checks.is_empty() {
        return Err(refuse("empty native check inventory"));
    }
    let mut ids = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut failed = Vec::new();
    for check in checks {
        let id = check["id"]
            .as_u64()
            .filter(|id| *id > 0)
            .ok_or_else(|| refuse("missing native check identity"))?;
        if !ids.insert(id) {
            return Err(refuse("duplicate native check identity"));
        }
        let status = check["status"]
            .as_str()
            .filter(|status| {
                matches!(
                    *status,
                    "Success" | "Unreachable" | "Failure" | "Undetermined"
                )
            })
            .ok_or_else(|| refuse("unsupported, missing or erroneous native check status"))?;
        *counts.entry(status.to_owned()).or_insert(0) += 1;
        if status == "Failure" {
            if check["category"].as_str().is_none_or(str::is_empty)
                || check["description"].as_str().is_none()
            {
                return Err(refuse("failed native check lacks its original diagnostic"));
            }
            failed.push(check.clone());
        }
    }
    Ok((counts, failed))
}

fn outcome(
    verdict: &str,
    counts: &BTreeMap<String, usize>,
    failed: &[Value],
) -> Result<Outcome, Error> {
    let undetermined = counts.get("Undetermined").copied().unwrap_or(0);
    if verdict == "SUCCESSFUL" {
        if !failed.is_empty()
            || undetermined != 0
            || counts.get("Success").copied().unwrap_or(0) == 0
        {
            return Err(refuse(
                "successful verdict has unresolved or contradictory checks",
            ));
        }
        return Ok(Outcome::BoundedSuccess);
    }
    if failed.is_empty() {
        return Err(refuse("failed verdict has no failed native property"));
    }
    if failed.iter().all(|check| check["category"] == "unwind") {
        return Ok(Outcome::BoundInsufficient);
    }
    if undetermined != 0 {
        return Err(refuse("property failure also has unresolved native checks"));
    }
    Ok(Outcome::Counterexample)
}

#[cfg(test)]
mod tests;
