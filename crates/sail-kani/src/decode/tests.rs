//! Contradictions and unsupported input never establish bounded success.
use serde_json::{Value, json};

use crate::{Outcome, decode};

const HARNESS: &str = "pilot::handwritten";

fn export() -> Value {
    json!({
        "metadata":{"version":"1.0","kani_version":"0.68.0"},
        "tools":{"kani":"0.68.0","cbmc":"6.11.0 (cbmc-6.11.0)"},
        "harness_metadata":[{"pretty_name":HARNESS,"attributes":{"kind":"Proof","should_panic":false},
            "is_automatically_generated":false,"has_loop_contracts":false,
            "contract":{"contracted_function_name":null,"recursion_tracker":null}}],
        "cbmc":[{"harness_id":HARNESS,"cbmc_metadata":{"version":"6.11.0"},
            "configuration":{"solver":"cadical","object_bits":16}}],
        "error_details":[{"harness_id":HARNESS,"has_errors":false}],
        "property_details":[{"harness_id":HARNESS,"property_details":{"total_properties":2,"passed":1,"failed":0,"unreachable":1,"undetermined":0,"covered":0,"uncovered":0,"satisfied":0,"unsatisfiable":0,"solver_error":0}}],
        "verification_results":{"summary":{"total_harnesses":1,"executed":1,"status":"completed","successful":1,"failed":0},"results":[{"harness_id":HARNESS,"status":"Success","checks":[
            {"id":1,"status":"Success","category":"assertion","description":"owned check"},
            {"id":2,"status":"Unreachable","category":"assertion","description":"unreachable check"}]}]}
    })
}

fn parsed(value: &Value, exit: i32, text: &[u8]) -> Result<crate::Observation, crate::Error> {
    let bytes = serde_json::to_vec(value).map_err(|error| crate::refuse(error.to_string()))?;
    decode(HARNESS, exit, text, &bytes)
}

#[test]
fn bounded_success_is_distinct_from_verified_and_preserves_counts() -> Result<(), crate::Error> {
    let result = parsed(&export(), 0, b"VERIFICATION:- SUCCESSFUL\n")?;
    assert_eq!(result.outcome(), Outcome::BoundedSuccess);
    assert_eq!(result.harness(), HARNESS);
    assert_eq!(
        result.evidence_tier(),
        Some(sail_api::assurance::EvidenceTier::Bounded)
    );
    assert_eq!(result.check_counts().get("Success"), Some(&1));
    assert_eq!(result.check_counts().get("Unreachable"), Some(&1));
    assert_eq!(result.failed_checks(), &[] as &[Value]);
    let serialized =
        serde_json::to_string(&result).map_err(|error| crate::refuse(error.to_string()))?;
    assert!(!serialized.contains("verified"));
    assert!(!serialized.contains("smt-verified"));
    Ok(())
}

#[test]
fn native_refutation_and_insufficient_bound_are_separate() -> Result<(), crate::Error> {
    let mut value = export();
    value["verification_results"]["results"][0]["status"] = json!("Failure");
    value["verification_results"]["summary"]["successful"] = json!(0);
    value["verification_results"]["summary"]["failed"] = json!(1);
    value["error_details"][0] = json!({"harness_id":HARNESS,"has_errors":true,"error_type":"assertion_failure","failed_properties_type":"PanicsOnly","exit_status":"properties_failed"});
    value["verification_results"]["results"][0]["checks"][0]["status"] = json!("Failure");
    value["property_details"][0]["property_details"]["passed"] = json!(0);
    value["property_details"][0]["property_details"]["failed"] = json!(1);
    let result = parsed(&value, 1, b"VERIFICATION:- FAILED\n")?;
    assert_eq!(result.outcome(), Outcome::Counterexample);
    assert_eq!(result.failed_checks().len(), 1);
    assert_eq!(result.failed_checks()[0]["description"], "owned check");
    value["verification_results"]["results"][0]["checks"][0]["category"] = json!("unwind");
    value["verification_results"]["results"][0]["checks"][1]["status"] = json!("Undetermined");
    value["property_details"][0]["property_details"]["unreachable"] = json!(0);
    value["property_details"][0]["property_details"]["undetermined"] = json!(1);
    value["error_details"][0]["error_type"] = json!("verification_failure");
    value["error_details"][0]["failed_properties_type"] = json!("Other");
    let result = parsed(&value, 1, b"VERIFICATION:- FAILED\n")?;
    assert_eq!(result.outcome(), Outcome::BoundInsufficient);
    assert_eq!(result.evidence_tier(), None);
    Ok(())
}

#[test]
fn missing_duplicate_and_contradictory_native_authority_refuse() -> Result<(), crate::Error> {
    let success = b"VERIFICATION:- SUCCESSFUL\n";
    let original = export();
    for text in [
        b"".as_slice(),
        b"VERIFICATION:- FAILED\n",
        b"VERIFICATION:- SUCCESSFUL\nVERIFICATION:- SUCCESSFUL\n",
        b"\xff",
    ] {
        assert!(parsed(&original, 0, text).is_err());
    }
    for exit in [-1, 1, 2, 101] {
        assert!(parsed(&original, exit, success).is_err());
    }
    let mutations = [
        ("/verification_results/summary/status", json!("incomplete")),
        ("/verification_results/summary/executed", json!(0)),
        ("/error_details/0/has_errors", json!(true)),
        ("/metadata/kani_version", json!("0.68.1")),
        ("/tools/cbmc", json!("6.11.1 (cbmc-6.11.1)")),
        (
            "/harness_metadata/0/pretty_name",
            json!("other::handwritten"),
        ),
        ("/harness_metadata/0/attributes/should_panic", json!(true)),
        (
            "/harness_metadata/0/attributes/kind",
            json!("ProofForContract"),
        ),
        (
            "/harness_metadata/0/is_automatically_generated",
            json!(true),
        ),
        ("/harness_metadata/0/has_loop_contracts", json!(true)),
        (
            "/harness_metadata/0/contract/contracted_function_name",
            json!("foreign"),
        ),
        ("/cbmc/0/harness_id", json!("other::handwritten")),
        ("/cbmc/0/configuration/solver", json!("z3")),
        (
            "/verification_results/results/0/harness_id",
            json!("other::handwritten"),
        ),
        ("/verification_results/results/0/checks", json!([])),
        ("/verification_results/results/0/checks/1/id", json!(1)),
        (
            "/verification_results/results/0/checks/0/status",
            json!("Error"),
        ),
        (
            "/verification_results/results/0/checks/0/status",
            json!("Undetermined"),
        ),
        (
            "/verification_results/results/0/checks/0/status",
            json!("Failure"),
        ),
    ];
    for (path, changed) in mutations {
        let mut value = original.clone();
        if let Some(field) = value.pointer_mut(path) {
            *field = changed;
        } else {
            return Err(crate::refuse(format!("invalid fixture pointer {path}")));
        }
        assert!(parsed(&value, 0, success).is_err(), "accepted {path}");
    }
    let mut duplicate = original.clone();
    if let Some(results) = duplicate["verification_results"]["results"].as_array_mut() {
        results.push(original["verification_results"]["results"][0].clone());
    } else {
        return Err(crate::refuse("missing fixture results"));
    }
    assert!(parsed(&duplicate, 0, success).is_err());
    assert!(decode(HARNESS, 0, success, b"{").is_err());
    Ok(())
}

#[test]
fn independent_review_inventory_error_and_duplicate_key_failures_refuse() -> Result<(), crate::Error>
{
    let original = export();
    let text = b"VERIFICATION:- SUCCESSFUL\n";
    let mutations = [
        ("/property_details", json!([])),
        ("/property_details/0/harness_id", json!("foreign::harness")),
        (
            "/property_details/0/property_details/total_properties",
            json!(0),
        ),
        ("/property_details/0/property_details/failed", json!(1)),
        (
            "/property_details/0/property_details/solver_error",
            json!(1),
        ),
        ("/harness_metadata/0/contract", json!("missing object")),
    ];
    for (path, value) in mutations {
        let mut changed = original.clone();
        let field = changed
            .pointer_mut(path)
            .ok_or_else(|| crate::refuse("invalid regression fixture"))?;
        *field = value;
        assert!(parsed(&changed, 0, text).is_err(), "accepted {path}");
    }
    let mut changed = original.clone();
    changed["error_details"][0]["error_type"] = json!("tool_error");
    changed["error_details"][0]["exit_status"] = json!("timeout");
    assert!(parsed(&changed, 0, text).is_err());
    let bytes =
        serde_json::to_string(&original).map_err(|error| crate::refuse(error.to_string()))?;
    let duplicate = bytes.replacen(
        "\"status\":\"Success\"",
        "\"status\":\"Failure\",\"status\":\"Success\"",
        1,
    );
    assert_ne!(bytes, duplicate);
    assert!(decode(HARNESS, 0, text, duplicate.as_bytes()).is_err());
    let mut missing = original.clone();
    missing["harness_metadata"][0]
        .as_object_mut()
        .ok_or_else(|| crate::refuse("fixture metadata"))?
        .remove("contract");
    assert!(parsed(&missing, 0, text).is_err());
    let mut lost = original.clone();
    lost["verification_results"]["results"][0]["checks"]
        .as_array_mut()
        .ok_or_else(|| crate::refuse("fixture checks"))?
        .pop();
    assert!(parsed(&lost, 0, text).is_err());
    Ok(())
}
