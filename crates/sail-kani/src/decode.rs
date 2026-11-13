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
    let document = crate::strict_json::parse(export)?;
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
        || metadata["contract"].as_object().is_none_or(|contract| {
            contract.len() != 2
                || contract.get("contracted_function_name") != Some(&Value::Null)
                || contract.get("recursion_tracker") != Some(&Value::Null)
        })
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
    let (counts, failed) = inventory(result)?;
    let outcome = outcome(expected, &counts, &failed)?;
    completion(&document, harness, outcome)?;
    let properties = property_counts(&document, harness, &counts, checks_len(&counts)?)?;
    Ok(Observation {
        harness: harness.into(),
        outcome,
        evidence_tier: (outcome != Outcome::BoundInsufficient).then_some(EvidenceTier::Bounded),
        check_counts: counts,
        failed_checks: failed,
        harness_metadata: metadata.clone(),
        backend_configuration: cbmc["configuration"].clone(),
        property_details: properties.clone(),
    })
}

fn completion(document: &Value, harness: &str, outcome: Outcome) -> Result<(), Error> {
    let success = outcome == Outcome::BoundedSuccess;
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
    let fields = errors
        .as_object()
        .ok_or_else(|| refuse("missing native error object"))?;
    if errors["has_errors"] != !success {
        return Err(refuse(
            "native error status contradicts the selected outcome",
        ));
    }
    let expected_type = if outcome == Outcome::Counterexample {
        "assertion_failure"
    } else {
        "verification_failure"
    };
    let expected_properties = if outcome == Outcome::Counterexample {
        "PanicsOnly"
    } else {
        "Other"
    };
    if success {
        if fields.len() != 2
            || !fields.contains_key("has_errors")
            || !fields.contains_key("harness_id")
        {
            return Err(refuse(
                "successful result has contradictory native error fields",
            ));
        }
    } else if fields.len() != 5
        || errors["error_type"] != expected_type
        || errors["failed_properties_type"] != expected_properties
        || errors["exit_status"] != "properties_failed"
    {
        return Err(refuse(
            "native error details contradict the failed property categories",
        ));
    }
    Ok(())
}

fn checks_len(counts: &BTreeMap<String, usize>) -> Result<u64, Error> {
    counts.values().try_fold(0_u64, |total, count| {
        total
            .checked_add(u64::try_from(*count).map_err(|error| refuse(error.to_string()))?)
            .ok_or_else(|| refuse("native property inventory overflow"))
    })
}

fn property_counts<'a>(
    document: &'a Value,
    harness: &str,
    counts: &BTreeMap<String, usize>,
    total: u64,
) -> Result<&'a Value, Error> {
    let record = one(array(document, "property_details")?, "harness_id", harness)?;
    let values = &record["property_details"];
    let fields = values
        .as_object()
        .ok_or_else(|| refuse("missing property count object"))?;
    if fields.len() != 10 || values["total_properties"].as_u64() != Some(total) || total == 0 {
        return Err(refuse(
            "native property total differs from its complete check inventory",
        ));
    }
    for (field, status) in [
        ("passed", "Success"),
        ("failed", "Failure"),
        ("unreachable", "Unreachable"),
        ("undetermined", "Undetermined"),
    ] {
        let count = u64::try_from(counts.get(status).copied().unwrap_or(0))
            .map_err(|error| refuse(error.to_string()))?;
        if values[field].as_u64() != Some(count) {
            return Err(refuse(
                "native property status counts contradict the check inventory",
            ));
        }
    }
    for field in [
        "covered",
        "uncovered",
        "satisfied",
        "unsatisfiable",
        "solver_error",
    ] {
        if values[field].as_u64() != Some(0) {
            return Err(refuse("unsupported or erroneous native property outcome"));
        }
    }
    Ok(values)
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
            if !matches!(check["category"].as_str(), Some("assertion" | "unwind"))
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
    if undetermined != 0 || failed.iter().any(|check| check["category"] != "assertion") {
        return Err(refuse(
            "property failure has unresolved or unsupported mixed native categories",
        ));
    }
    Ok(Outcome::Counterexample)
}

#[cfg(test)]
mod tests;
