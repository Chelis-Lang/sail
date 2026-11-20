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
        let old = Frozen::loaded_sources(&self.previous.model)?;
        if !self.same_bytes(&old) {
            return None;
        }
        let new = Frozen::loaded_sources(&self.current.model)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn requests() -> (Request, Request) {
        let previous = Request {
            version: crate::VERSION,
            model: crate::Model {
                root: "Root.tla".into(),
                config: "Root.cfg".into(),
                files: [
                    ("Root.tla".into(), "---- MODULE Root ----\nEXTENDS Library\nVARIABLE x\nInit == x = 0\nNext == x'=x\nInv == x = Bound\n====\n".into()),
                    ("lib/Library.tla".into(), "---- MODULE Library ----\nBound == 0\n====\n".into()),
                    ("Root.cfg".into(), "INIT Init\nNEXT Next\nINVARIANT Inv\n".into()),
                ].into(),
            },
            operation: Operation::Check { max_states: 10, max_depth: 10 },
        };
        let mut current = previous.clone();
        current.model.files.insert(
            "Sibling.tla".into(),
            "---- MODULE Sibling ----\nUnused == TRUE\n====\n".into(),
        );
        (previous, current)
    }

    #[test]
    fn unrelated_source_can_change_but_a_loaded_import_cannot() -> Result<(), String> {
        let (previous, current) = requests();
        let mut query = Query { previous, current };
        let relation = query.compare()?;
        let sources = relation
            .loaded_modules
            .as_ref()
            .ok_or("missing equivalence")?;
        assert!(sources.iter().any(|p| p == "lib/Library.tla"));
        assert!(!sources.iter().any(|p| p == "Sibling.tla"));
        query.validate(&relation)?;
        query.current.model.files.insert(
            "lib/Library.tla".into(),
            "---- MODULE Library ----\nBound == 1\n====\n".into(),
        );
        assert!(query.compare()?.loaded_modules.is_none());
        assert!(query.validate(&relation).is_err());
        Ok(())
    }

    #[test]
    fn extracted_monoliths_do_not_certify_a_project_edit() -> Result<(), String> {
        let (mut previous, _) = requests();
        previous.model.files.remove("lib/Library.tla");
        let root = previous
            .model
            .files
            .get_mut("Root.tla")
            .ok_or("missing root")?;
        root.push_str("\n---- MODULE Library ----\nBound == 0\n====\n");
        let mut current = previous.clone();
        current.model.files.insert(
            "Sibling.tla".into(),
            "---- MODULE Sibling ----\nN == 0\n====\n".into(),
        );
        assert!(
            Query { previous, current }
                .compare()?
                .loaded_modules
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn module_shadowing_and_changed_limits_or_configuration_rebind() -> Result<(), String> {
        let (previous, mut current) = requests();
        current.model.files.insert(
            "other/Library.tla".into(),
            "---- MODULE Library ----\nBound == 2\n====\n".into(),
        );
        assert!(
            Query {
                previous: previous.clone(),
                current
            }
            .compare()?
            .loaded_modules
            .is_none()
        );
        let (_, mut current) = requests();
        current.operation = Operation::Check {
            max_states: 1,
            max_depth: 1,
        };
        assert!(!eligible(&previous, &current));
        let (_, mut current) = requests();
        current
            .model
            .files
            .insert("Root.cfg".into(), "INIT Init\nNEXT Next\n".into());
        assert!(!eligible(&previous, &current));
        Ok(())
    }
}
