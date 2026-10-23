//! Public native temporal payloads and attributed runtime jobs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Complete ITF states with explicit native claim metadata, never lossy values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItfTrace {
    /// Complete ordered variable list.
    pub vars: Vec<String>,
    /// ITF value objects, one complete variable assignment per state.
    pub states: Vec<serde_json::Map<String, Value>>,
    /// Explicit native ending claim, including zero-based loop index.
    pub ending: Value,
    /// All properties claimed false by a periodic word.
    pub properties: Vec<String>,
    /// Native action labels retained separately from ITF values.
    pub actions: Vec<Value>,
    /// Original optional claimed length.
    pub length: Option<usize>,
}

/// Exact embedded source identity. Moving it requires native conformance.
pub const DIVE_REVISION: &str = "1025cd6a0510183bc16abc1e6e3eea4f9305c5a9";
/// Worker payload protocol version.
pub const VERSION: u32 = 1;
/// Maximum encoded request or response, before JSON decoding.
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

/// Immutable closed model inputs. Paths are portable relative file names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Every supplied source and configuration byte, as strict UTF-8.
    pub files: BTreeMap<String, String>,
    /// Root TLA+ module path.
    pub root: String,
    /// Configuration path.
    pub config: String,
}

/// Native operation payload, without caller IDs or ledger metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    /// Exhaustive safety or final/short temporal checking, within limits.
    Check {
        /// Maximum admitted states; stopping is not a holding verdict.
        max_states: u64,
        /// Maximum BFS depth; stopping is not a holding verdict.
        max_depth: u64,
    },
    /// Finite seeded simulation, never an exhaustive proof.
    Simulate {
        /// Positive finite number of traces.
        traces: u64,
        /// Maximum transitions per trace.
        depth: usize,
    },
    /// Independent validation of complete states and an explicit ending.
    Validate {
        /// ITF values with a separate semantic ending claim.
        trace: ItfTrace,
    },
}

/// One worker request. The parent owns deadline and cancellation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Exactly [`VERSION`].
    pub version: u32,
    /// Frozen model closure.
    pub model: Model,
    /// Requested native work.
    pub operation: Operation,
}

/// Deciding backend result. Execution failures are never logical success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Complete finite graph establishes the requested checks.
    Holds,
    /// Concrete counterexample independently passes semantic validation.
    Refuted,
    /// Finite sampled traces, with no exhaustive proof.
    Sampled,
    /// The supplied trace satisfies its explicit semantic claim.
    Valid,
    /// The supplied trace fails its explicit semantic claim.
    Invalid,
    /// A named feature cannot be evaluated faithfully.
    Unsupported,
    /// A deterministic exploration/resource limit stopped the work.
    Stopped,
    /// Input is malformed or semantically rejected.
    Rejected,
    /// Internal or infrastructure failure, never a model verdict.
    Error,
}

/// Actual typed completion, independent of JSON discriminators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completion {
    /// No complete exploration is established.
    None,
    /// Typed safety success with an empty remaining queue.
    Safety,
    /// Typed temporal Holds with a complete captured graph.
    Temporal,
}

/// Configured checks, retained separately from the actual completion route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedChecks {
    /// Configured invariant names, in source order.
    pub invariants: Vec<String>,
    /// Configured PROPERTY names, whose actual roles the backend binds.
    pub properties: Vec<String>,
    /// Configured postconditions.
    pub postconditions: Vec<String>,
    /// Configured reachability assertions.
    pub possible: Vec<String>,
    /// Whether deadlock checking is enabled by the configuration.
    pub deadlock: bool,
}

/// Original native evidence plus a separate admitted ITF rendering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Exactly [`VERSION`].
    pub version: u32,
    /// Exact backend revision.
    pub backend_revision: String,
    /// SHA-256 of the exact canonically serialized worker request.
    pub request_sha256: String,
    /// Deciding outcome.
    pub outcome: Outcome,
    /// Actual typed complete-exploration evidence.
    pub completion: Completion,
    /// Exact configured property names, independent of JSON route labels.
    pub requested_checks: Option<RequestedChecks>,
    /// Native original report, unmodified.
    pub native_report: Value,
    /// Original native trace documents, unmodified.
    pub native_traces: Vec<Value>,
    /// Strict, round-tripped ITF renderings.
    pub itf_traces: Vec<ItfTrace>,
    /// Original independent trace validation reports.
    pub validations: Vec<Value>,
    /// Named refusal/error details, when applicable.
    pub message: Option<String>,
}

impl Response {
    /// Constructs an unevaluated native evidence record.
    #[must_use]
    pub fn new(outcome: Outcome, native_report: Value) -> Self {
        Self {
            version: VERSION,
            backend_revision: DIVE_REVISION.into(),
            request_sha256: String::new(),
            outcome,
            completion: Completion::None,
            requested_checks: None,
            native_report,
            native_traces: Vec::new(),
            itf_traces: Vec::new(),
            validations: Vec::new(),
            message: None,
        }
    }

    /// A named failure with no model evidence.
    #[must_use]
    pub fn failure(outcome: Outcome, message: impl Into<String>) -> Self {
        let mut result = Self::new(outcome, Value::Null);
        result.message = Some(message.into());
        result
    }
}

/// Exact byte SHA-256, used for worker and payload content identities.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    sail_key::sha256(bytes)
        .iter()
        .fold(String::new(), |mut value, byte| {
            let _ = write!(value, "{byte:02x}");
            value
        })
}

/// Canonical payload identity. Model maps are ordered and trace values use
/// serde's sorted object representation; no timestamps enter this identity.
///
/// # Errors
/// A payload value cannot be serialized.
pub fn request_sha256(request: &Request) -> std::result::Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(request)?;
    value.sort_all_objects();
    serde_json::to_vec(&value).map(|bytes| sha256(&bytes))
}

/// One attributed temporal invocation through the shared runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// Stable caller obligation identity.
    pub obligation_id: String,
    /// Strictly sorted unique caller requirement identities.
    pub requirement_ids: Vec<String>,
    /// Exact native payload, without executable choices.
    pub request: Request,
    /// Whole-worker deadline, positive and at most five minutes.
    pub wall_time_ms: u64,
}

/// Attributed worker result; execution and native outcomes remain separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Result {
    /// Caller obligation identity, unchanged.
    pub obligation_id: String,
    /// Caller requirement identities, unchanged.
    pub requirement_ids: Vec<String>,
    /// Canonical identity of the exact worker request.
    pub request_sha256: String,
    /// Exact backend source revision.
    pub backend_revision: String,
    /// Authenticated worker bytes, absent when unavailable.
    pub worker_sha256: Option<String>,
    /// Effective whole-worker deadline.
    pub wall_time_ms: u64,
    /// Process outcome, independent of native logical evidence.
    pub outcome: crate::runtime::Outcome,
    /// Original complete worker evidence, absent on execution failure.
    pub response: Option<Response>,
    /// Explicit execution diagnostic.
    pub error: Option<crate::runtime::Error>,
    /// This slice never caches temporal evidence.
    pub cache_hit: bool,
}

/// Implemented temporal operations and authenticated installation readiness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Native check, finite simulation and trace validation are implemented.
    pub implemented: bool,
    /// Trusted worker bytes were authenticated for this installation.
    pub installed: bool,
    /// Exact embedded backend source revision.
    pub backend_revision: String,
    /// Actual authenticated worker identity, when installed.
    pub worker_sha256: Option<String>,
    /// Actionable unavailable diagnostic.
    pub error: Option<crate::runtime::Error>,
}

/// Checks a returned worker's request identity and conclusive evidence shape.
/// This supplements the parent's pinned worker-byte admission; it does not
/// authenticate an arbitrary caller-supplied verdict as a native result.
///
/// # Errors
/// Mismatched protocol/input or absent/incompatible conclusive evidence.
pub fn validate_response(
    request: &Request,
    response: &Response,
) -> std::result::Result<(), String> {
    if response.version != VERSION
        || response.backend_revision != DIVE_REVISION
        || response.request_sha256 != request_sha256(request).map_err(|e| e.to_string())?
    {
        return Err("SAIL4213: worker protocol, backend or request identity differs".into());
    }
    let report = &response.native_report;
    let conclusive = matches!(
        response.outcome,
        Outcome::Holds | Outcome::Refuted | Outcome::Sampled | Outcome::Valid | Outcome::Invalid
    );
    if conclusive && report["schema_version"] != 2 {
        return Err("SAIL4213: conclusive outcome lacks native schema-2 evidence".into());
    }
    let compatible = match response.outcome {
        Outcome::Holds => {
            matches!(request.operation, Operation::Check { .. })
                && report["command"] == "check"
                && report["verdict"] == "success"
                && report["options"]["profile"] == "dive"
                && report["options"]["store"] == "exact"
                && report["statistics"]["left_on_queue"] == 0
                && (response.completion == Completion::Safety
                    || (response.completion == Completion::Temporal
                        && report["execution"]["capture_completed"] == true))
        }
        Outcome::Refuted => {
            matches!(
                request.operation,
                Operation::Check { .. } | Operation::Simulate { .. }
            ) && !response.native_traces.is_empty()
                && response.native_traces.len() == response.itf_traces.len()
                && response.validations.len() == response.native_traces.len()
                && response.validations.iter().all(|v| v["result"] == "valid")
        }
        Outcome::Sampled => {
            matches!(request.operation, Operation::Simulate { .. })
                && report["result"] == "sampled"
                && report["model_proved"] == false
                && report["exhaustive"] == false
                && !response.native_traces.is_empty()
                && response.native_traces.len() == response.itf_traces.len()
                && response.validations.len() == response.native_traces.len()
                && response.validations.iter().all(|v| v["result"] == "valid")
        }
        Outcome::Valid => {
            matches!(request.operation, Operation::Validate { .. }) && report["result"] == "valid"
        }
        Outcome::Invalid => {
            matches!(request.operation, Operation::Validate { .. }) && report["result"] == "invalid"
        }
        _ => true,
    };
    if !compatible {
        return Err("SAIL4213: outcome lacks matching native operation evidence".into());
    }
    Ok(())
}

use serde::Deserializer;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::Map;
use std::fmt;

struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct Any;
        impl<'de> Visitor<'de> for Any {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object fields")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(Value::Number)
                    .map(Unique)
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(value)) = a.next_element()? {
                    values.push(value);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut fields = Map::new();
                while let Some((key, Unique(value))) = a.next_entry::<String, Unique>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(de::Error::custom("SAIL4214: duplicate JSON object field"));
                    }
                }
                Ok(Unique(Value::Object(fields)))
            }
        }
        decoder.deserialize_any(Any)
    }
}

/// Strict whole-document JSON decode, including unique nested object fields.
///
/// # Errors
/// Rejects duplicates, trailing input or an incompatible typed payload.
pub fn decode<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> std::result::Result<T, serde_json::Error> {
    let Unique(value) = serde_json::from_slice(bytes)?;
    serde_json::from_value(value)
}
