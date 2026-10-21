//! Strict reversible ITF values, with separate native trace claim metadata.
use dive_api::trace::{Ending, TraceDoc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// Complete ITF states with explicit native claim metadata, never lossy values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItfTrace {
    /// Complete ordered variable list.
    pub vars: Vec<String>,
    /// ITF value objects, one complete variable assignment per state.
    pub states: Vec<Map<String, Value>>,
    /// Explicit native ending claim, including zero-based loop index.
    pub ending: Value,
    /// All properties claimed false by a periodic word.
    pub properties: Vec<String>,
    /// Native action labels retained separately from ITF values.
    pub actions: Vec<Value>,
    /// Original optional claimed length.
    pub length: Option<usize>,
}

fn decimal(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0"
}

fn convert(value: &Value, native: bool, depth: usize) -> Result<Value, String> {
    if depth > 64 {
        return Err("SAIL4203: trace value nesting exceeds 64".into());
    }
    let recurse = |v: &Value| convert(v, native, depth + 1);
    match value {
        Value::Bool(_) | Value::String(_) => Ok(value.clone()),
        Value::Number(n) if native => n
            .as_i64()
            .map(|i| json!({"#bigint": i.to_string()}))
            .ok_or_else(|| "SAIL4203: native number is not an i64 integer".into()),
        Value::Object(object) => {
            if !native && object.keys().all(|k| !k.starts_with('#')) {
                return object
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), recurse(v)?)))
                    .collect::<Result<Map<_, _>, String>>()
                    .map(|fields| json!({"record":fields}));
            }
            if object.len() != 1 {
                return Err("SAIL4203: value tag must have exactly one field".into());
            }
            let (tag, body) = object.iter().next().ok_or("SAIL4203: missing value tag")?;
            let integer = if native { "int" } else { "#bigint" };
            if tag == integer {
                let digits = body
                    .as_str()
                    .filter(|s| decimal(s))
                    .ok_or("SAIL4203: integer requires canonical decimal digits")?;
                return if native {
                    Ok(json!({"#bigint": digits}))
                } else if let Ok(small) = digits.parse::<i64>() {
                    Ok(json!(small))
                } else {
                    Ok(json!({"int": digits}))
                };
            }
            if native && tag == "record" {
                let fields = body
                    .as_object()
                    .ok_or("SAIL4203: record requires an object")?;
                if fields.keys().any(|k| k.starts_with('#')) {
                    return Err("SAIL4203: record collides with reserved ITF tags".into());
                }
                return fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), recurse(v)?)))
                    .collect::<Result<Map<_, _>, String>>()
                    .map(Value::Object);
            }
            let target = match (native, tag.as_str()) {
                (true, "set") => "#set",
                (true, "seq") => "#tup",
                (true, "fcn") => "#map",
                (false, "#set") => "set",
                (false, "#tup") => "seq",
                (false, "#map") => "fcn",
                _ => return Err(format!("SAIL4203: unsupported value tag {tag}")),
            };
            let items = body
                .as_array()
                .ok_or("SAIL4203: collection tag requires an array")?;
            let mapped = if matches!(tag.as_str(), "fcn" | "#map") {
                items
                    .iter()
                    .map(|pair| {
                        let pair = pair
                            .as_array()
                            .filter(|p| p.len() == 2)
                            .ok_or("SAIL4203: function requires argument/value pairs")?;
                        Ok(json!([recurse(&pair[0])?, recurse(&pair[1])?]))
                    })
                    .collect::<Result<Vec<_>, String>>()?
            } else {
                items.iter().map(recurse).collect::<Result<Vec<_>, _>>()?
            };
            Ok(json!({target: mapped}))
        }
        _ => Err("SAIL4203: unsupported or ambiguous trace value".into()),
    }
}

fn complete(trace: &ItfTrace) -> Result<(), String> {
    let vars = trace.vars.iter().collect::<BTreeSet<_>>();
    if vars.len() != trace.vars.len()
        || vars.is_empty()
        || trace.states.is_empty()
        || trace.actions.len() != trace.states.len()
        || trace.length.is_some_and(|n| n != trace.states.len())
        || trace.properties.iter().collect::<BTreeSet<_>>().len() != trace.properties.len()
        || trace
            .states
            .iter()
            .any(|state| state.keys().collect::<BTreeSet<_>>() != vars)
    {
        return Err("SAIL4203: trace requires complete nonempty states and exact metadata".into());
    }
    Ending::from_json(&trace.ending)?;
    Ok(())
}

/// Converts a native original document to exact ITF values, preserving claims.
///
/// # Errors
/// Refuses unknown native fields, versions and unrepresentable values.
pub fn native_to_itf(native: &Value) -> Result<ItfTrace, String> {
    let document = TraceDoc::from_json(native)?;
    if document.to_json() != *native {
        return Err("SAIL4203: native trace is not an exact canonical document".into());
    }
    let trace = ItfTrace {
        vars: document.variables.clone(),
        states: document
            .states
            .iter()
            .map(|s| {
                s.iter()
                    .map(|(k, v)| Ok((k.clone(), convert(v, true, 0)?)))
                    .collect::<Result<Map<_, _>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?,
        ending: document
            .ending
            .as_ref()
            .map(Ending::to_json)
            .ok_or("SAIL4203: trace ending is required")?,
        properties: document.properties.clone(),
        actions: native["states"]
            .as_array()
            .ok_or("SAIL4203: missing states")?
            .iter()
            .map(|s| s["action"].clone())
            .collect(),
        length: document.length,
    };
    complete(&trace)?;
    if itf_to_native(&trace)? != *native {
        return Err("SAIL4203: trace values do not round-trip exactly".into());
    }
    Ok(trace)
}

/// Converts admitted ITF values to a native document for semantic validation.
///
/// # Errors
/// Refuses partial traces, unknown tags, noncanonical numbers and invalid claims.
pub fn itf_to_native(trace: &ItfTrace) -> Result<Value, String> {
    complete(trace)?;
    let states = trace
        .states
        .iter()
        .zip(&trace.actions)
        .map(|(state, action)| {
            let values = state
                .iter()
                .map(|(k, v)| Ok((k.clone(), convert(v, false, 0)?)))
                .collect::<Result<Map<_, _>, String>>()?;
            Ok(json!({"action": action, "values": values}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut native = json!({"format":"dive-trace", "version":1,
        "variables":trace.vars,"states":states,"ending":trace.ending});
    if !trace.properties.is_empty() {
        native["properties"] = json!(trace.properties);
    }
    if let Some(length) = trace.length {
        native["length"] = json!(length);
    }
    if TraceDoc::from_json(&native)?.to_json() != native {
        return Err("SAIL4203: trace claim metadata is not canonical".into());
    }
    Ok(native)
}
