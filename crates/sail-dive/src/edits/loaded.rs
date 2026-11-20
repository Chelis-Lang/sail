//! Compare the exact modules resolved by the same admitted ordinary checker.
use crate::{Operation, Request, input::Frozen};
use serde::{Deserialize, Serialize};

/// Two immutable project envelopes submitted to the selected native worker.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Input that established an authenticated, complete ordinary Holds result.
    pub previous: Request,
    /// Input whose selected root and configuration the caller wants to recheck.
    pub current: Request,
}

/// The worker certifies only the relation between these two inputs.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// Native backend source identity.
    pub backend_source: String,
    /// Canonical digest of the earlier complete request.
    pub previous_sha256: String,
    /// Canonical digest of the current request.
    pub current_sha256: String,
    /// Ordered loaded source paths, including qualified standard modules, on equivalence.
    pub loaded_modules: Option<Vec<String>>,
}

/// Cheap parent-checkable prerequisites for a comparison of loaded source graphs.
#[must_use]
pub fn eligible(previous: &Request, current: &Request) -> bool {
    let (a, b) = (&previous.model, &current.model);
    previous.version == crate::VERSION
        && current.version == crate::VERSION
        && matches!(previous.operation, Operation::Check { .. })
        && previous.operation == current.operation
        && a.root == b.root
        && a.config == b.config
        && a.files
            .get(&a.root)
            .is_some_and(|v| b.files.get(&a.root) == Some(v))
        && a.files
            .get(&a.config)
            .is_some_and(|v| b.files.get(&a.config) == Some(v))
        && a.files != b.files
}

impl Query {
    fn same_bytes(&self, modules: &[String]) -> bool {
        let (a, b) = (&self.previous.model, &self.current.model);
        modules.contains(&a.root)
            && modules.iter().all(|name| {
                name.starts_with("<standard>/")
                    || a.files
                        .get(name)
                        .is_some_and(|bytes| b.files.get(name) == Some(bytes))
            })
    }

    fn resolve(&self) -> Option<Vec<String>> {
        if !eligible(&self.previous, &self.current) {
            return None;
        }
        let old = Frozen::create(&self.previous.model)
            .ok()?
            .loaded_sources()?;
        if !self.same_bytes(&old) {
            return None;
        }
        let new = Frozen::create(&self.current.model).ok()?.loaded_sources()?;
        (old == new).then_some(new)
    }

    /// Resolve both programs in private directories under ordinary host admission.
    ///
    /// # Errors
    /// Canonical input identity encoding failed.
    pub fn compare(&self) -> Result<Relation, String> {
        Ok(Relation {
            backend_source: super::BACKEND_SOURCE.to_owned(),
            previous_sha256: crate::request_sha256(&self.previous).map_err(|e| e.to_string())?,
            current_sha256: crate::request_sha256(&self.current).map_err(|e| e.to_string())?,
            loaded_modules: self.resolve(),
        })
    }

    pub(crate) fn validate(&self, relation: &Relation) -> Result<(), String> {
        if relation.backend_source != super::BACKEND_SOURCE
            || relation.previous_sha256
                != crate::request_sha256(&self.previous).map_err(|e| e.to_string())?
            || relation.current_sha256
                != crate::request_sha256(&self.current).map_err(|e| e.to_string())?
            || relation.loaded_modules.as_ref().is_some_and(|modules| {
                !eligible(&self.previous, &self.current) || !self.same_bytes(modules)
            })
        {
            return Err(
                "loaded-module comparison has mismatched inputs, scope or source bytes".into(),
            );
        }
        Ok(())
    }
}
