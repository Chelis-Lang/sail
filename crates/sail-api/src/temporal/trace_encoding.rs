//! A trace-local discriminator keeps native atoms separate from ordinary ITF strings.
use super::{ItfTrace, MODEL_VALUE_TAG};
use serde::ser::SerializeStruct as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

const MODEL_VALUES: &str = "sail-model-values-v1";

#[derive(Default)]
enum Encoding {
    #[default]
    Legacy,
    ModelValues,
}

impl<'de> Deserialize<'de> for Encoding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if String::deserialize(deserializer)? == MODEL_VALUES {
            Ok(Self::ModelValues)
        } else {
            Err(serde::de::Error::custom(
                "unknown Sail trace value encoding",
            ))
        }
    }
}

fn has_model_values(states: &[Map<String, Value>]) -> Result<bool, &'static str> {
    let mut found = false;
    let mut pending = Vec::new();
    for value in states.iter().flat_map(Map::values) {
        pending.push(value);
        while let Some(node) = pending.pop() {
            match node {
                Value::Object(fields) if fields.contains_key(MODEL_VALUE_TAG) => {
                    if fields.len() != 1 || !fields[MODEL_VALUE_TAG].is_string() {
                        return Err("a Sail model value requires exactly one string-valued tag");
                    }
                    found = true;
                }
                Value::Object(fields) => pending.extend(fields.values()),
                Value::Array(values) => pending.extend(values),
                _ => (),
            }
        }
    }
    Ok(found)
}

impl Serialize for ItfTrace {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let extended = has_model_values(&self.states).map_err(serde::ser::Error::custom)?;
        let mut row = serializer.serialize_struct("ItfTrace", if extended { 7 } else { 6 })?;
        row.serialize_field("vars", &self.vars)?;
        row.serialize_field("states", &self.states)?;
        row.serialize_field("ending", &self.ending)?;
        row.serialize_field("properties", &self.properties)?;
        row.serialize_field("actions", &self.actions)?;
        row.serialize_field("length", &self.length)?;
        if extended {
            row.serialize_field("value_encoding", MODEL_VALUES)?;
        }
        row.end()
    }
}

impl<'de> Deserialize<'de> for ItfTrace {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            vars: Vec<String>,
            states: Vec<Map<String, Value>>,
            ending: Value,
            properties: Vec<String>,
            actions: Vec<Value>,
            length: Option<usize>,
            #[serde(default)]
            value_encoding: Encoding,
        }

        let row = Wire::deserialize(deserializer)?;
        let extended = has_model_values(&row.states).map_err(serde::de::Error::custom)?;
        if extended != matches!(row.value_encoding, Encoding::ModelValues) {
            return Err(serde::de::Error::custom(
                "Sail model-value encoding and state tags disagree",
            ));
        }
        Ok(Self {
            vars: row.vars,
            states: row.states,
            ending: row.ending,
            properties: row.properties,
            actions: row.actions,
            length: row.length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::{Model, Operation, Request, request_sha256, sha256};
    use serde_json::json;

    const LEGACY: &str = r#"{"vars":["x"],"states":[{"x":"A"}],"ending":{"kind":"behavior"},"properties":[],"actions":[null],"length":null}"#;

    #[test]
    fn prior_trace_bytes_and_request_digest_are_stable() -> Result<(), Box<dyn std::error::Error>> {
        let trace: ItfTrace = serde_json::from_str(LEGACY)?;
        assert_eq!(serde_json::to_string(&trace)?, LEGACY);
        assert_eq!(
            sha256(LEGACY.as_bytes()),
            "23cd9b64c199a36098fe614b8b5c4dc72774de6af33a93ab1bcd0a644623ed17"
        );
        let request = Request {
            version: 1,
            model: Model {
                files: [("M.tla".into(), "---- MODULE M ----\nVARIABLE x\nInit == x = \"A\"\nNext == UNCHANGED x\n====\n".into()),
                        ("M.cfg".into(), "INIT Init\nNEXT Next\n".into())].into(),
                root: "M.tla".into(), config: "M.cfg".into(),
            },
            operation: Operation::Validate { trace },
        };
        assert_eq!(
            request_sha256(&request)?,
            "25bab6310645f2415a99d92b7448166e3c1cbbd4fa1ef0f8c010dbc23240c429"
        );
        Ok(())
    }

    #[test]
    fn marker_and_reserved_leaves_must_agree() -> Result<(), Box<dyn std::error::Error>> {
        let mut wire: Value = serde_json::from_str(LEGACY)?;
        for marker in [
            Value::Null,
            json!("legacy"),
            json!("sail-model-values-v2"),
            json!(MODEL_VALUES),
        ] {
            wire["value_encoding"] = marker;
            assert!(serde_json::from_value::<ItfTrace>(wire.clone()).is_err());
        }
        wire["states"][0]["x"] = json!({"#map": [[{(MODEL_VALUE_TAG):"A"},"A"]]});
        wire["value_encoding"] = json!({(MODEL_VALUES): null});
        assert!(serde_json::from_value::<ItfTrace>(wire.clone()).is_err());
        wire["value_encoding"] = json!(MODEL_VALUES);
        let trace: ItfTrace = serde_json::from_value(wire.clone())?;
        assert_eq!(serde_json::to_value(trace)?, wire);
        wire.as_object_mut()
            .ok_or("wire is not an object")?
            .remove("value_encoding");
        assert!(serde_json::from_value::<ItfTrace>(wire.clone()).is_err());
        wire["value_encoding"] = json!(MODEL_VALUES);
        for bad in [
            json!({(MODEL_VALUE_TAG):5}),
            json!({(MODEL_VALUE_TAG):"A","other":"A"}),
        ] {
            wire["states"][0]["x"] = json!({"#set":[{(MODEL_VALUE_TAG):"B"},bad]});
            assert!(serde_json::from_value::<ItfTrace>(wire.clone()).is_err());
        }
        Ok(())
    }
}
