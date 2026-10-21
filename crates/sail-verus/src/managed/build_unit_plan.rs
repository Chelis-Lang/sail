//! Run-local scheduling authority from the pinned Cargo's complete unit graph.
//!
//! Metadata can include platform-disabled packages. Their absent build scripts
//! need no observed contract, even when a requested proof subsequently fails.
//! This is never a saved absence claim: roots, graph closure and actual Cargo
//! reports are checked, and the ordinary full before/after input checks remain.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

type Target = (String, String, String);

/// One current Cargo plan; it cannot attest outputs or supply a proof receipt.
pub(super) struct Plan {
    packages: BTreeSet<String>,
}

impl Plan {
    /// Requires the existing exact Cargo/compiler-query authority first.
    pub(super) fn load(
        directory: &Path,
        metadata: &Value,
        expected: &BTreeSet<Target>,
    ) -> Option<Self> {
        super::build_environment::load(directory, metadata)?;
        let graph: Value =
            serde_json::from_slice(&std::fs::read(directory.join("graph.json")).ok()?).ok()?;
        Self::derive(&graph, metadata, expected)
    }

    fn derive(graph: &Value, metadata: &Value, expected: &BTreeSet<Target>) -> Option<Self> {
        (graph.get("version")?.as_u64()? == 1 && !expected.is_empty()).then_some(())?;
        let known = metadata
            .get("packages")?
            .as_array()?
            .iter()
            .map(|package| package.get("id")?.as_str())
            .collect::<Option<BTreeSet<_>>>()?;
        let units = graph.get("units")?.as_array()?;
        let mut packages = BTreeSet::new();
        for unit in units {
            let id = unit.get("pkg_id")?.as_str()?;
            known.contains(id).then_some(())?;
            packages.insert(id.to_owned());
            for dependency in unit.get("dependencies")?.as_array()? {
                let index = usize::try_from(dependency.get("index")?.as_u64()?).ok()?;
                units.get(index)?;
            }
        }
        let mut indices = BTreeSet::new();
        let mut roots = BTreeSet::new();
        for root in graph.get("roots")?.as_array()? {
            let index = usize::try_from(root.as_u64()?).ok()?;
            indices.insert(index).then_some(())?;
            let unit = units.get(index)?;
            matches!(unit.get("mode")?.as_str()?, "build" | "check").then_some(())?;
            let kinds = unit.get("target")?.get("kind")?.as_array()?;
            let [kind] = kinds.as_slice() else {
                return None;
            };
            roots.insert((
                unit.get("pkg_id")?.as_str()?.to_owned(),
                unit.get("target")?.get("name")?.as_str()?.to_owned(),
                kind.as_str()?.to_owned(),
            ));
        }
        (roots == *expected).then_some(Self { packages })
    }

    /// Only whole packages absent from this graph can exempt an unseen script.
    pub(super) fn absent_scripts(&self, metadata: &Value) -> BTreeSet<String> {
        let Some(selection) = super::build_inputs::selection_closure(metadata) else {
            return BTreeSet::new();
        };
        metadata["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|package| {
                let id = package["id"].as_str()?;
                (selection.contains(id)
                    && !self.packages.contains(id)
                    && package["targets"].as_array()?.iter().any(|target| {
                        target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
                    }))
                .then(|| id.to_owned())
            })
            .collect()
    }

    /// A completed Cargo report may fail its proof, but must not schedule any
    /// package outside the plan. Missing, malformed or truncated reports refuse.
    pub(super) fn confirms(&self, stdout: &Path, artifacts: &Path) -> bool {
        let checked = || -> Option<()> {
            let mut finished = 0;
            for line in std::fs::read_to_string(stdout).ok()?.lines() {
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    // Verus's forwarded pretty JSON also uses this stream.
                    // Only a malformed Cargo event with its reason tag is an
                    // interrupted scheduling report, not a verifier fragment.
                    if line.trim_start().starts_with('{') && line.contains("\"reason\":") {
                        return None;
                    }
                    continue;
                };
                if value["reason"] == "build-finished" {
                    value["success"].as_bool()?;
                    finished += 1;
                }
                if let Some(id) = value.get("package_id") {
                    self.packages.contains(id.as_str()?).then_some(())?;
                }
            }
            (finished == 1).then_some(())?;
            for line in std::fs::read_to_string(artifacts).ok()?.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let value: Value = serde_json::from_str(line).ok()?;
                self.packages
                    .contains(value["package_id"].as_str()?)
                    .then_some(())?;
            }
            Some(())
        };
        checked().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Value, Value, BTreeSet<Target>) {
        let metadata = json!({
            "sail_selected_package_ids":["root"],
            "packages":[
                {"id":"root","targets":[{"kind":["lib"]}]},
                {"id":"active","targets":[{"kind":["custom-build"]}]},
                {"id":"windows","targets":[{"kind":["custom-build"]}]}
            ],
            "resolve":{"nodes":[
                {"id":"root","dependencies":["active","windows"]},
                {"id":"active","dependencies":[]},
                {"id":"windows","dependencies":[]}
            ]}
        });
        let graph = json!({"version":1,"roots":[0],"units":[
            {"pkg_id":"root","mode":"check","target":{"name":"root","kind":["lib"]},
             "dependencies":[{"index":1}]},
            {"pkg_id":"active","mode":"run-custom-build","dependencies":[]}
        ]});
        (
            metadata,
            graph,
            BTreeSet::from([("root".into(), "root".into(), "lib".into())]),
        )
    }

    #[test]
    fn absent_packages_do_not_exempt_planned_scripts() -> Result<(), Box<dyn std::error::Error>> {
        let (metadata, graph, expected) = fixture();
        let plan = Plan::derive(&graph, &metadata, &expected).ok_or("plan refused")?;
        assert_eq!(
            plan.absent_scripts(&metadata),
            BTreeSet::from(["windows".into()])
        );
        let mut unknown = metadata;
        unknown["sail_selected_package_ids"] = Value::Null;
        assert!(plan.absent_scripts(&unknown).is_empty());
        Ok(())
    }

    #[test]
    fn unknown_or_incomplete_graphs_supply_no_absence() {
        let (metadata, graph, expected) = fixture();
        for broken in [
            "version",
            "empty-roots",
            "duplicate-root",
            "unknown-package",
            "edge",
            "root-name",
        ] {
            let mut graph = graph.clone();
            match broken {
                "version" => graph["version"] = json!(2),
                "empty-roots" => graph["roots"] = json!([]),
                "duplicate-root" => graph["roots"] = json!([0, 0]),
                "unknown-package" => graph["units"][1]["pkg_id"] = json!("foreign"),
                "edge" => graph["units"][0]["dependencies"][0]["index"] = json!(99),
                _ => graph["units"][0]["target"]["name"] = json!("foreign"),
            }
            assert!(
                Plan::derive(&graph, &metadata, &expected).is_none(),
                "{broken}"
            );
        }
    }

    #[test]
    fn failed_proof_needs_a_complete_report_with_no_unplanned_units()
    -> Result<(), Box<dyn std::error::Error>> {
        let (metadata, graph, expected) = fixture();
        let plan = Plan::derive(&graph, &metadata, &expected).ok_or("plan refused")?;
        let directory = std::env::temp_dir().join(format!("sail-unit-plan-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        let stdout = directory.join("cargo.stdout");
        let artifacts = directory.join("artifacts.jsonl");
        let finished = "{\"reason\":\"build-finished\",\"success\":false}\n";
        let original = "{\"package_id\":\"root\"}\n{\"package_id\":\"active\"}\n";
        std::fs::write(&stdout, finished)?;
        std::fs::write(&artifacts, original)?;
        assert!(plan.confirms(&stdout, &artifacts));
        std::fs::write(
            &stdout,
            format!("{{\n  \"verification-results\": {{}}\n}}\n{finished}"),
        )?;
        assert!(plan.confirms(&stdout, &artifacts));
        std::fs::write(
            &stdout,
            format!("{{\"reason\":\"compiler-message\",\n{finished}"),
        )?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::write(
            &stdout,
            format!(
                "{{\"reason\":\"build-script-executed\",\"package_id\":\"windows\"}}\n{finished}"
            ),
        )?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::write(&stdout, finished)?;
        std::fs::write(&artifacts, "{\"package_id\":\"windows\"}\n")?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::write(&artifacts, original)?;
        std::fs::write(&stdout, "")?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::write(&stdout, format!("{finished}{finished}"))?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::write(&stdout, finished)?;
        std::fs::write(&artifacts, "broken record")?;
        assert!(!plan.confirms(&stdout, &artifacts));
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
