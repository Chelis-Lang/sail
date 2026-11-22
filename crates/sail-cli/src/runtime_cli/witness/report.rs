//! Decode the decision-bearing fields of the explicitly requested TLC-status check report.
use serde_json::Value;

fn text(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.is_empty())
}

pub(super) fn periodic(word: &Value) -> bool {
    let Some(rows) = word["states"].as_array().filter(|rows| !rows.is_empty()) else {
        return false;
    };
    let len = u64::try_from(rows.len()).ok();
    let ending = &word["ending"];
    let closes = match ending["kind"].as_str() {
        Some("stuttering") => true,
        Some("lasso") => ending["back_to"]
            .as_u64()
            .zip(len)
            .is_some_and(|(back, n)| back < n),
        _ => false,
    };
    closes
        && word["length"].as_u64() == len
        && word["properties"]
            .as_array()
            .is_some_and(|p| !p.is_empty() && p.iter().all(text))
        && sail_dive::native_to_itf(word).is_ok()
}

/// The human name and compact and expanded native word views come from one word.
fn temporal_violation(doc: &Value) -> bool {
    let example = &doc["counterexample"];
    let word = &example["trace"];
    if doc["check_kind"] != "temporal"
        || example["validation"] != "valid"
        || !periodic(word)
        || doc["properties"] != word["properties"]
        || example["length"] != word["length"]
    {
        return false;
    }
    let names: Vec<_> = doc["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let display = match names.as_slice() {
        [] => return false,
        [name] => (*name).to_owned(),
        [a, b] => format!("{a} and {b}"),
        [prefix @ .., last] => format!("{}, and {last}", prefix.join(", ")),
    };
    let back = if word["ending"]["kind"] == "stuttering" {
        word["length"].as_u64()
    } else {
        word["ending"]["back_to"]
            .as_u64()
            .and_then(|n| n.checked_add(1))
    };
    doc["property"] == display
        && example["ending"] == "lasso"
        && example["back_to"].as_u64() == back
        && matching_rows(example, |typed, display| typed == display)
}

fn matching_rows(example: &Value, values_match: impl Fn(&Value, &Value) -> bool) -> bool {
    let (Some(rows), Some(native)) = (
        example["states"].as_array(),
        example["trace"]["states"].as_array(),
    ) else {
        return false;
    };
    !rows.is_empty()
        && rows.len() == native.len()
        && rows
            .iter()
            .zip(native)
            .enumerate()
            .all(|(i, (view, typed))| {
                view["index"].as_u64() == u64::try_from(i + 1).ok()
                    && values_match(&typed["values"], &view["values"])
            })
}

fn finite(doc: &Value, ending: &str) -> bool {
    let example = &doc["counterexample"];
    let word = &example["trace"];
    example["ending"] == "finite"
        && word["ending"]["kind"] == ending
        && example["length"] == word["length"]
        && matching_rows(example, sail_dive::finite_row_matches)
        && sail_dive::native_to_itf(word).is_ok()
}

fn class(doc: &Value, code: u64, name: &str) -> bool {
    doc["tlc"]["code"] == code && doc["tlc"]["name"] == name
}

fn invalid_envelope(doc: &Value, status: u8) -> bool {
    let tlc = &doc["tlc"];
    doc["command"] != "check"
        || !matches!(doc["schema_version"].as_u64(), Some(3 | 4))
        || doc["options"]["exit_codes"] != "tlc"
        || doc["options"]["semantic_profile"] != "dive"
        || doc["exit_code"].as_u64() != Some(u64::from(status))
        || !matches!(doc["check_kind"].as_str(), Some("temporal" | "safety"))
        || doc.get("tlc").is_none()
        || (!tlc.is_null()
            && (tlc["exit_status"].as_u64() != Some(u64::from(status))
                || !tlc["code"].is_u64()
                || !text(&tlc["name"])
                || !tlc["verified"].is_boolean()))
}

pub(super) fn ordinary(doc: &Value, status: u8) -> bool {
    if invalid_envelope(doc, status) {
        return false;
    }
    let tlc = &doc["tlc"];
    let named = text(&doc["property"]);
    let no_claim =
        doc["property"].is_null() && doc["properties"].is_null() && doc["counterexample"].is_null();
    let named_finite = |kind| {
        named
            && finite(doc, kind)
            && doc["counterexample"]["trace"]["ending"]["name"] == doc["property"]
    };
    let ec = tlc["code"].as_u64();
    match doc["verdict"].as_str() {
        Some("success") => {
            status == 0
                && class(doc, 2193, "TLC_SUCCESS")
                && no_claim
                && doc["statistics"]["distinct"].is_u64()
                && (doc["check_kind"] == "safety"
                    || (doc["execution"]["exhausted"] == true
                        && doc["execution"]["capture_completed"] == true))
        }
        Some("liveness-violation") => {
            status == 13
                && class(doc, 2116, "TLC_TEMPORAL_PROPERTY_VIOLATED")
                && temporal_violation(doc)
        }
        Some("invariant-violation") => {
            status == 12
                && (class(doc, 2107, "TLC_INVARIANT_VIOLATED_INITIAL")
                    || class(doc, 2110, "TLC_INVARIANT_VIOLATED_BEHAVIOR"))
                && named_finite("invariant")
        }
        Some("property-violation") => {
            status == 13
                && class(doc, 2108, "TLC_PROPERTY_VIOLATED_INITIAL")
                && named_finite("property")
        }
        Some("action-property-violation") => {
            status == 13
                && class(doc, 2112, "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR")
                && named_finite("property")
        }
        Some("deadlock") => {
            status == 11
                && class(doc, 2114, "TLC_DEADLOCK_REACHED")
                && !named
                && finite(doc, "deadlock")
        }
        Some("assumption-failure") => {
            status == 10 && class(doc, 2104, "TLC_ASSUMPTION_FALSE") && named
        }
        Some("postcondition-failure") => {
            status == 10 && class(doc, 2404, "TLC_POSTCONDITION_FALSE") && named
        }
        Some("unsupported" | "resource-limit") => {
            status == 3 && tlc.is_null() && no_claim && text(&doc["message"])
        }
        Some("internal-error") => status == 4 && tlc.is_null() && no_claim && text(&doc["message"]),
        Some("stopped") => {
            status == 5 && tlc.is_null() && no_claim && text(&doc["stopped"]["resource"])
        }
        Some("rejected") => {
            no_claim
                && doc["diagnostics"].as_array().is_some_and(|d| !d.is_empty())
                && ((status == 150 && class(doc, 3002, "TLC_PARSING_FAILED"))
                    || (status == 151 && tlc.is_null()))
        }
        Some("assertion-failure") => {
            status == 14 && class(doc, 2132, "TLC_VALUE_ASSERT_FAILED") && text(&doc["message"])
        }
        Some("overflow") => {
            status == 255 && matches!(ec, Some(2178 | 2265)) && text(&doc["message"])
        }
        Some("incomplete-state") => {
            matches!((status, ec), (75, Some(2109)) | (255, Some(2102))) && text(&doc["message"])
        }
        Some("model-error" | "evaluation-error") => {
            text(&doc["message"])
                && (matches!(status, 4) && tlc.is_null()
                    || matches!(status, 10 | 14 | 75..=77 | 150 | 151 | 255)
                        && ec.is_some_and(|n| {
                            !matches!(
                                n,
                                2104 | 2107 | 2108 | 2110 | 2112 | 2114 | 2116 | 2193 | 2404
                            )
                        }))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finite_views_use_the_tlc_value_renderer() {
        let mut example = json!({"states":[{"index":1,"values":{"x":["p", {"ok":true}]}}],
            "trace":{"states":[{"values":{"x":{"seq":[{"model":"p"},{"record":{"ok":true}}]}}}]}});
        assert!(matching_rows(&example, sail_dive::finite_row_matches));
        example["states"][0]["values"]["x"] = json!(true);
        assert!(!matching_rows(&example, sail_dive::finite_row_matches));
        example["states"][0] = Value::Null;
        assert!(!matching_rows(&example, sail_dive::finite_row_matches));
    }
}
