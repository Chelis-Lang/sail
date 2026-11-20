//! Qualify the first temporal claim of an authenticated complete ordinary check.
use super::{SearchCounts, default_policy, projection};
use crate::{Completion, Operation, Outcome, Request, Response};
use dive_api::temporal_projection::Program;
use serde::{Deserialize, Serialize};

/// An assertion about the original ordinary check, carried with its two closed inputs.
/// The caller must authenticate the producer and its receipt before using the relation.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Input whose ordinary result was authenticated.
    pub previous: Request,
    /// Current request retaining the first temporal claim alone.
    pub current: Request,
    /// Genuine response to the previous request from this worker build.
    pub previous_response: Response,
}

/// The current property qualified by the worker, with statistics of the original search.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// Current singleton temporal property name.
    pub property: String,
    /// Exact native payload digest of the ordinary establishing request.
    pub basis_sha256: String,
    /// Statistics reported by the previous ordinary exhaustive check.
    pub basis_counts: SearchCounts,
}

/// Both input identities and the conditional result. The relation cannot create a receipt.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// Immutable Dive source linked into the worker.
    pub backend_source: String,
    /// Original native payload digest.
    pub previous_sha256: String,
    /// Current native payload digest.
    pub current_sha256: String,
    /// Qualification of the current property or a non-deciding miss.
    pub claim: Option<Claim>,
}

pub(super) fn candidate(request: &Request, response: &Response) -> bool {
    let Operation::Check {
        max_states,
        max_depth,
    } = request.operation
    else {
        return false;
    };
    let limits = dive_api::temporal_projection::Limits {
        max_states: Some(max_states),
        max_depth: Some(max_depth),
    };
    response.outcome == Outcome::Holds
        && response.completion == Completion::Temporal
        && response.requested_checks.as_ref().is_some_and(|c| {
            c.properties.len() >= 2
                && c.invariants.is_empty()
                && c.possible.is_empty()
                && c.postconditions.is_empty()
        })
        && dive_api::temporal_projection::qualify_execution(&response.native_report, limits).is_ok()
        && response.native_traces.is_empty()
        && response.itf_traces.is_empty()
        && response.validations.is_empty()
}

/// Check the cheap input and receipt boundary before dispatching any model binding.
#[must_use]
pub fn eligible(previous: &Request, current: &Request, response: &Response) -> bool {
    inventory(previous, current, response).is_some()
}

fn inventory(previous: &Request, current: &Request, response: &Response) -> Option<Claim> {
    if previous.version != crate::VERSION
        || current.version != crate::VERSION
        || previous.operation != current.operation
        || !candidate(previous, response)
        || !projection::same_scope(previous, current)
    {
        return None;
    }
    let old = response.requested_checks.as_ref()?;
    let new = crate::requested_checks(&current.model)?;
    let [property] = new.properties.as_slice() else {
        return None;
    };
    if !new.invariants.is_empty()
        || !new.possible.is_empty()
        || !new.postconditions.is_empty()
        || old.properties.first() != Some(property)
    {
        return None;
    }
    crate::validate_response(previous, response).ok()?;
    let stats = &response.native_report["statistics"];
    Some(Claim {
        basis_sha256: response.request_sha256.clone(),
        property: property.clone(),
        basis_counts: SearchCounts {
            initial_generated: stats["initial_generated"].as_u64()?,
            generated: stats["generated"].as_u64()?,
            distinct: stats["distinct"].as_u64()?,
            depth: stats["depth"].as_u64()?,
            left_on_queue: stats["left_on_queue"].as_u64()?,
        },
    })
}

fn bind(request: &Request) -> Option<Program> {
    projection::bind(request, |analysis, config, limits| {
        Program::from_reports(analysis, config, limits).ok()
    })
}

pub(super) fn qualify(
    previous: &Request,
    current: &Request,
    response: &Response,
    current_program: impl FnOnce() -> Option<Program>,
) -> Option<Claim> {
    if !default_policy() {
        return None;
    }
    let claim = inventory(previous, current, response)?;
    let old = bind(previous)?;
    let new = current_program()?;
    let property = old.first_claim(&new, &response.native_report).ok()?;
    (property == claim.property).then_some(claim)
}

impl Claim {
    pub(super) fn matches(
        &self,
        previous: &Request,
        current: &Request,
        response: &Response,
    ) -> bool {
        inventory(previous, current, response).as_ref() == Some(self)
    }
}

impl Query {
    /// Bind both closed models and compare their actual first temporal products.
    ///
    /// # Errors
    /// Request identity encoding failed.
    pub fn compare(&self) -> Result<Relation, String> {
        Ok(Relation {
            backend_source: super::BACKEND_SOURCE.into(),
            previous_sha256: crate::request_sha256(&self.previous).map_err(|e| e.to_string())?,
            current_sha256: crate::request_sha256(&self.current).map_err(|e| e.to_string())?,
            claim: qualify(
                &self.previous,
                &self.current,
                &self.previous_response,
                || bind(&self.current),
            ),
        })
    }

    pub(crate) fn validate(&self, relation: &Relation) -> Result<(), String> {
        if relation.backend_source != super::BACKEND_SOURCE
            || relation.previous_sha256
                != crate::request_sha256(&self.previous).map_err(|e| e.to_string())?
            || relation.current_sha256
                != crate::request_sha256(&self.current).map_err(|e| e.to_string())?
            || relation
                .claim
                .as_ref()
                .is_some_and(|c| !c.matches(&self.previous, &self.current, &self.previous_response))
        {
            return Err(
                "first-claim worker returned different source, scope, receipt, property or limits"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::{Batch, BatchResult, Entry, Evidence};

    #[test]
    fn transport_requires_the_claim_of_the_original_ordinary_search() -> Result<(), String> {
        let cfg = "INIT Init\nNEXT Next\nPROPERTY P Q";
        let make = |config: &str| {
            Request {
            version: crate::VERSION,
            operation: Operation::Check {max_states:64, max_depth:30},
            model: crate::Model {
                root:"M.tla".into(), config:"M.cfg".into(),
                files:[("M.tla".into(), "---- MODULE M ----\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == (x=0) ~> (x=0)\nQ == []<>(x=0)\n====".into()),("M.cfg".into(),config.into())].into(),
            },
        }
        };
        let previous = make(cfg);
        let current = make(&cfg.replace("P Q", "P"));
        let mut query = Query {
            previous_response: crate::execute(&previous),
            previous,
            current,
        };
        let mut relation = query.compare()?;
        query.validate(&relation)?;
        let mut claim = relation.claim.take().ok_or("missing claim")?;
        let batch = Batch {
            version: 1,
            wall_time_ms: 1000,
            checks: vec![query.previous.clone(), query.current.clone()],
        };
        let batch_result = |claim, response| BatchResult {
            version: 1,
            entries: vec![
                Entry {
                    elapsed_us: 0,
                    request_sha256: relation.previous_sha256.clone(),
                    evidence: Evidence::Fresh {
                        response: Box::new(response),
                    },
                },
                Entry {
                    elapsed_us: 0,
                    request_sha256: relation.current_sha256.clone(),
                    evidence: Evidence::EstablishedFirstTemporalClaim(claim),
                },
            ],
        };
        claim.basis_counts.generated += 1;
        assert!(
            batch_result(claim, query.previous_response.clone())
                .validate(&batch)
                .is_err()
        );
        let true_claim = query.compare()?.claim.ok_or("missing qualified claim")?;
        let valid = batch_result(true_claim, query.previous_response.clone());
        valid.validate(&batch)?;
        // A first claim is never transported as the ordinary proof for another claim.
        let mut value = serde_json::to_value(&valid).map_err(|e| e.to_string())?;
        value["entries"][0]["evidence"] = value["entries"][1]["evidence"].clone();
        let chained: BatchResult = serde_json::from_value(value).map_err(|e| e.to_string())?;
        assert!(chained.validate(&batch).is_err());
        relation.claim = query.compare()?.claim;
        relation.backend_source.push_str("-changed");
        assert!(query.validate(&relation).is_err());
        relation.backend_source = crate::edits::BACKEND_SOURCE.into();
        query.previous_response.native_report["core"]["metrics"]["liveness"]["atoms"]["fallback"] =
            1.into();
        assert!(query.validate(&relation).is_err());
        Ok(())
    }
}
