//! Validate the relation between two closed safety models. This operation
//! returns an obligation subset. It cannot establish a verdict by itself.
use crate::{Operation, Request, input::Frozen};
use dive_api::safety_projection::{Limits, Program};
use serde::{Deserialize, Serialize};

/// Inputs to a conditional proof projection, dispatched to the same native worker as the basis.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Original model for which a trusted caller already has a complete safety proof.
    pub previous: Request,
    /// Current model and limits.
    pub current: Request,
}

/// Conditional result of comparing two fully bound programs in an isolated worker.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// Source revision of the Dive implementation.
    pub backend_source: String,
    /// Digest of the original input.
    pub previous_sha256: String,
    /// Digest of the edited input.
    pub current_sha256: String,
    /// When eligible, the checked current invariant inventory in configuration order.
    pub invariants: Option<Vec<String>>,
}

pub(super) fn bind<T>(
    request: &Request,
    make: impl FnOnce(&dive_api::AnalyzeReport, &dive_api::ConfigReport, Limits) -> Option<T>,
) -> Option<T> {
    if request.version != crate::VERSION {
        return None;
    }
    let Operation::Check {
        max_states,
        max_depth,
    } = request.operation
    else {
        return None;
    };
    let frozen = Frozen::create(&request.model).ok()?;
    let analysis = dive_api::analyze_file(
        &frozen.root,
        &dive_api::AnalyzeOptions {
            library_paths: frozen.libraries.clone(),
            ..Default::default()
        },
    )
    .ok()?;
    let config = dive_api::parse_config_file(&frozen.config).ok()?;
    make(
        &analysis,
        &config,
        Limits {
            max_states: Some(max_states),
            max_depth: Some(max_depth),
        },
    )
}

pub(super) fn same_scope(previous: &Request, current: &Request) -> bool {
    let (a, b) = (&previous.model, &current.model);
    a.root == b.root
        && a.config == b.config
        && a.files.len() == b.files.len()
        && a.files
            .iter()
            .all(|(name, bytes)| name == &a.config || b.files.get(name) == Some(bytes))
        && b.files.contains_key(&a.config)
}

pub(super) fn subset_inventory(previous: &Request, current: &Request) -> Option<Vec<String>> {
    if !same_scope(previous, current) || previous.operation != current.operation {
        return None;
    }
    let old = crate::requested_checks(&previous.model)?;
    let new = crate::requested_checks(&current.model)?;
    let mut names = old.invariants.iter();
    (old.properties.is_empty()
        && new.properties.is_empty()
        && new
            .invariants
            .iter()
            .all(|n| names.any(|old_name| old_name == n)))
    .then_some(new.invariants)
}

impl Query {
    fn subset(&self) -> Option<Vec<String>> {
        if !super::default_policy() || !same_scope(&self.previous, &self.current) {
            return None;
        }
        let program = |request: &Request| {
            bind(request, |a, c, l| {
                Some((Program::from_reports(a, c).ok()?, l))
            })
        };
        let (old, old_limits) = program(&self.previous)?;
        let (new, new_limits) = program(&self.current)?;
        Some(
            old.subset(&new, old_limits, new_limits)
                .ok()?
                .invariants()
                .to_vec(),
        )
    }

    /// Compare the actual bound programs. A matching pair needs separate
    /// authenticated proof of prior complete Holds before it can decide a check.
    ///
    /// # Errors
    /// A request identity could not be encoded.
    pub fn compare(&self) -> Result<Relation, String> {
        Ok(Relation {
            backend_source: super::BACKEND_SOURCE.into(),
            previous_sha256: crate::request_sha256(&self.previous).map_err(|e| e.to_string())?,
            current_sha256: crate::request_sha256(&self.current).map_err(|e| e.to_string())?,
            invariants: self.subset(),
        })
    }

    pub(crate) fn validate(&self, relation: &Relation) -> Result<(), String> {
        if relation.backend_source != super::BACKEND_SOURCE
            || relation.previous_sha256
                != crate::request_sha256(&self.previous).map_err(|e| e.to_string())?
            || relation.current_sha256
                != crate::request_sha256(&self.current).map_err(|e| e.to_string())?
        {
            return Err("projection worker source or request identity differs".into());
        }
        if let Some(names) = &relation.invariants
            && subset_inventory(&self.previous, &self.current).as_ref() != Some(names)
        {
            return Err(
                "projection worker returned a different scope, property inventory or limits".into(),
            );
        }
        Ok(())
    }
}
