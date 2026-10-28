//! COMMON-origin metadata and reuse guards for source-read child controls.
//!
//! Other driver declarations can still occur in the tracked environment. Only
//! COMMON has a separate origin in this protocol. Missing origin metadata is a
//! decoding error, never an empty driver observation.

use super::ghost_environment::Environment;

/// Whether the explicitly supplied driver-origin map contains only the known
/// COMMON bookkeeping entry. An empty map explicitly means no such driver read.
pub(super) fn known(driver: &Environment) -> bool {
    driver
        .keys()
        .all(|name| name == crate::adapter::verus_channel::COMMON)
}

/// Genuinely tracked reads of these child-controlled settings cannot reuse a
/// proof from ambient parent values. COMMON is separated from driver-only
/// bookkeeping by schema2; provenance is a source read, even when it was unset
/// before a later request enables attribution. The remaining explicit overlays
/// receive the same conservative guard. Cargo-level settings that Cargo itself
/// fingerprints, and routine Z3/Singular driver declarations, are not in this set.
const SOURCE_SENSITIVE: &[&str] = &[
    crate::adapter::verus_channel::COMMON,
    "SAIL_PROVENANCE_DIR",
    "SAIL_COMPILER_INPUTS",
    "SAIL_COMPILER_INPUTS_BINDING",
    "SAIL_ENGINE",
    "SAIL_ENGINE_EVENTS",
    "SAIL_ENGINE_CONTEXT",
    "SAIL_ENGINE_FOCUS_REPORT",
    "SAIL_ENGINE_FUNCTION",
    "SAIL_RUSTC_DEV_SOURCE_BASE",
    "SAIL_VIR_SPAN_REMAP",
];

/// Whether this explicitly identified child override needs its raw tracked
/// source value, rather than the ambient parent's context/declaration value.
pub(crate) fn source_sensitive(name: &str) -> bool {
    SOURCE_SENSITIVE.contains(&name)
}

/// Presence counts even when unset, empty, or byte-identical to bookkeeping.
/// Other ordinary declarations retain their existing currentness checks.
pub(crate) fn reusable(environment: &Environment) -> bool {
    !environment.keys().any(|name| source_sensitive(name))
}

/// Persisted write-ahead state shared by every proof context using a compiler
/// target family. It can only demand recompilation, never authorize a result.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    version: u32,
    package_id: String,
    identity: super::receipts::TargetIdentity,
}

const STATE: u32 = 2;

/// Required cleanup before Cargo may inspect this family's existing artifacts.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Cleanup {
    /// Legacy/unknown state: discard the whole family's compiler artifacts.
    Family,
    /// Packages with unfinished or reuse-ineligible compiler observations.
    Packages(PackageClean),
}

/// Exact non-authorizing observations validated under the family owner.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PackageClean {
    packages: Vec<String>,
    markers: std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
}

impl PackageClean {
    /// Full package IDs from the original validated observations.
    pub(super) fn packages(&self) -> &[String] {
        &self.packages
    }
}

/// Read under the family's owner, before invoking Cargo. Unknown state is
/// conservatively a family clean, including malformed or unreadable markers.
pub(super) fn cleanup(directory: &std::path::Path, metadata: &serde_json::Value) -> Cleanup {
    let stamp = std::fs::read(directory.join("schema"));
    if !stamp.is_ok_and(|bytes| bytes == STATE.to_string().as_bytes()) {
        return Cleanup::Family;
    }
    let Ok(entries) = std::fs::read_dir(directory.join("pending")) else {
        return Cleanup::Family;
    };
    let mut packages = std::collections::BTreeSet::new();
    let mut markers = std::collections::BTreeMap::new();
    for entry in entries {
        let Some((path, bytes, pending)) = entry.ok().and_then(|entry| {
            let path = entry.path();
            let bytes = std::fs::read(&path).ok()?;
            let pending: Pending = serde_json::from_slice(&bytes).ok()?;
            Some((path, bytes, pending))
        }) else {
            return Cleanup::Family;
        };
        let package = metadata["packages"].as_array().and_then(|all| {
            all.iter().find(|package| {
                package["id"].as_str() == Some(pending.package_id.as_str())
                    && package["manifest_path"].as_str().map(std::path::Path::new)
                        == Some(pending.identity.manifest.as_path())
            })
        });
        let Some(package) = package else {
            return Cleanup::Family;
        };
        if pending.version != STATE
            || path.file_name().and_then(std::ffi::OsStr::to_str)
                != Some(format!("{}.json", pending.identity.key()).as_str())
            || pending.identity.cargo_target(package).is_none()
        {
            return Cleanup::Family;
        }
        packages.insert(pending.package_id);
        markers.insert(path, bytes);
    }
    Cleanup::Packages(PackageClean {
        packages: packages.into_iter().collect(),
        markers,
    })
}

/// A no-reuse/loader call keeps Cargo's own freshness and performs no origin
/// migration. Its compiler still records dirty state for a later reusable run.
pub(super) fn cleanup_for_run(
    directory: &std::path::Path,
    metadata: &serde_json::Value,
    reuse: bool,
) -> Cleanup {
    if reuse {
        cleanup(directory, metadata)
    } else {
        Cleanup::Packages(PackageClean::default())
    }
}

/// Establish schema2 state only after the whole old target was discarded (or
/// before any target existed). Receipts do not carry or remove this state.
pub(super) fn initialized(directory: &std::path::Path) -> crate::Result<()> {
    match std::fs::remove_dir_all(directory) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(crate::Error::new("SAIL3111", error.to_string())),
    }
    std::fs::create_dir_all(directory.join("pending"))
        .and_then(|()| std::fs::write(directory.join("schema"), STATE.to_string()))
        .map_err(|error| crate::Error::new("SAIL3111", error.to_string()))
}

/// Retire package facts only after their ordinary and focused owned artifacts
/// have both been cleaned successfully under the compiler-family owner. Merely
/// retiring receipts or completing another argument variant cannot call this.
pub(super) fn retired_after_clean(
    directory: &std::path::Path,
    plan: &PackageClean,
) -> crate::Result<()> {
    let retire = || -> std::result::Result<(), Box<dyn std::error::Error>> {
        let current = std::fs::read_dir(directory.join("pending"))?
            .map(|entry| {
                let path = entry?.path();
                Ok((path.clone(), std::fs::read(path)?))
            })
            .collect::<std::io::Result<std::collections::BTreeMap<_, _>>>()?;
        if current != plan.markers {
            return Err("compiler-origin observations changed during cleanup".into());
        }
        for (path, bytes) in &plan.markers {
            if std::fs::read(path)? != *bytes {
                return Err("compiler-origin observation changed before retirement".into());
            }
            std::fs::remove_file(path)?;
        }
        Ok(())
    };
    retire().map_err(|error| crate::Error::new("SAIL3111", error.to_string()))
}

/// Write before spawning a genuine verified compiler. Failure refuses that
/// spawn, so an interruption cannot leave fresh artifacts without dirty state.
pub(crate) fn started(
    runtime: &super::receipts::Runtime,
    identity: &super::receipts::TargetIdentity,
) -> crate::Result<()> {
    let package_id = runtime.packages.get(&identity.manifest).ok_or_else(|| {
        crate::Error::new(
            "SAIL3111",
            "compiler origin has no managed package identity",
        )
    })?;
    std::fs::create_dir_all(runtime.compiler_read_origins.join("pending"))
        .map_err(|error| crate::Error::new("SAIL3111", error.to_string()))?;
    super::files::json(
        &runtime
            .compiler_read_origins
            .join("pending")
            .join(format!("{}.json", identity.key())),
        &Pending {
            version: STATE,
            package_id: package_id.clone(),
            identity: identity.clone(),
        },
    )
}

/// Clear only after a genuine complete successful schema2 compiler observation
/// proves that this same target reads no source-sensitive child setting. On any failure,
/// leaving the marker only causes another real compile.
pub(crate) fn completed(runtime: &super::receipts::Runtime, key: &str) {
    if !runtime.reuse {
        return;
    }
    let _ = std::fs::remove_file(
        runtime
            .compiler_read_origins
            .join("pending")
            .join(format!("{key}.json")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_consumes_only_the_original_complete_observation_set()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-origin-capture-{}", std::process::id()));
        initialized(&root)?;
        let identity = super::super::receipts::TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "original".into(),
            cargo_name: None,
        };
        let metadata = serde_json::json!({"packages":[{"id":"original-package","name":"demo",
            "manifest_path":identity.manifest,"targets":[{"name":"demo","kind":["lib"]}]}]});
        let marker = root
            .join("pending")
            .join(format!("{}.json", identity.key()));
        let mut pending = Pending {
            version: STATE,
            package_id: "original-package".into(),
            identity,
        };
        super::super::files::json(&marker, &pending)?;
        let original = std::fs::read(&marker)?;
        let Cleanup::Packages(plan) = cleanup(&root, &metadata) else {
            return Err("original cleanup plan".into());
        };
        pending.package_id = "different-package".into();
        super::super::files::json(&marker, &pending)?;
        let changed = std::fs::read(&marker)?;
        assert!(retired_after_clean(&root, &plan).is_err());
        assert_eq!(
            std::fs::read(&marker)?,
            changed,
            "a rewritten package identity cannot retire a fact"
        );
        std::fs::write(&marker, &original)?;
        let added = root.join("pending/new-observation.json");
        std::fs::write(&added, &original)?;
        assert!(retired_after_clean(&root, &plan).is_err());
        assert_eq!(std::fs::read(&marker)?, original);
        assert!(added.is_file(), "new unknown observations remain intact");
        std::fs::remove_file(added)?;
        retired_after_clean(&root, &plan)?;
        assert!(!marker.exists());
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn source_reads_are_ineligible_independently_of_value() {
        assert!(reusable(&Environment::new()));
        assert!(reusable(&Environment::from([(
            "SAIL_APPLICATION_INPUT".into(),
            Some("bound-by-parent".into())
        )])));
        for name in [
            crate::adapter::verus_channel::COMMON,
            "SAIL_PROVENANCE_DIR",
            "SAIL_COMPILER_INPUTS",
            "SAIL_COMPILER_INPUTS_BINDING",
            "SAIL_ENGINE",
            "SAIL_ENGINE_EVENTS",
            "SAIL_ENGINE_CONTEXT",
            "SAIL_ENGINE_FOCUS_REPORT",
            "SAIL_ENGINE_FUNCTION",
            "SAIL_RUSTC_DEV_SOURCE_BASE",
            "SAIL_VIR_SPAN_REMAP",
        ] {
            for value in [None, Some(String::new()), Some("same-as-driver".into())] {
                let source = Environment::from([(name.into(), value)]);
                assert!(!reusable(&source));
            }
        }
        assert!(known(&Environment::from([(
            crate::adapter::verus_channel::COMMON.into(),
            None
        )])));
        assert!(!known(&Environment::from([("UNCLASSIFIED".into(), None)])));
    }
    #[test]
    fn pending_origin_survives_receipts_contexts_and_package_clean_until_fresh_observation()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-origin-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let state = root.join("family/origins");
        let manifest = root.join("Cargo.toml");
        let metadata = serde_json::json!({"packages":[{"id":"package-id","name":"demo",
            "manifest_path":manifest,"targets":[{"name":"demo","kind":["lib"]}]}]});
        let identity = super::super::receipts::TargetIdentity {
            manifest,
            name: "demo".into(),
            crate_type: "rlib".into(),
            rustc_args_sha256: "arguments".into(),
            cargo_name: None,
        };
        assert_eq!(
            cleanup(&state, &metadata),
            Cleanup::Family,
            "legacy origin is unknown"
        );
        assert_eq!(
            cleanup_for_run(&state, &metadata, false),
            Cleanup::Packages(PackageClean::default()),
            "loader runs do not clean or initialize legacy state"
        );
        assert!(!state.exists());
        assert_eq!(
            cleanup_for_run(&state, &metadata, true),
            Cleanup::Family,
            "later reusable call still migrates"
        );
        initialized(&state)?;
        assert_eq!(
            cleanup(&state, &metadata),
            Cleanup::Packages(PackageClean::default())
        );
        let marker = state
            .join("pending")
            .join(format!("{}.json", identity.key()));
        let mut runtime = super::super::receipts::Runtime {
            run_id: "run".into(),
            args: Vec::new(),
            z3: root.join("z3"),
            z3_sha256: String::new(),
            daemon: false,
            reuse: false,
            sidecars: root.join("sidecars"),
            invocations: root.join("invocations"),
            original_wrapper: root.join("wrapper"),
            receipts: root.join("context-a/receipts"),
            compiler_read_origins: state.clone(),
            context_sha256: "context-a".into(),
            input_revision: "sources".into(),
            packages: std::collections::BTreeMap::from([(
                identity.manifest.clone(),
                "package-id".into(),
            )]),
            target_kinds: std::collections::BTreeMap::new(),
            engine: None,
            retained: None,
            attribution_logs: false,
        };
        started(&runtime, &identity)?;
        completed(&runtime, &identity.key());
        assert!(
            marker.is_file(),
            "no-reuse verification cannot clear the dirty marker"
        );
        // Receipts are per context and Cargo clean affects target only. Neither
        // may erase the write-ahead marker when no reusable receipt was emitted.
        for relative in ["context-a/receipts", "context-b/receipts", "family/target"] {
            let path = root.join(relative);
            std::fs::create_dir_all(&path)?;
            std::fs::remove_dir_all(path)?;
            let Cleanup::Packages(plan) = cleanup(&state, &metadata) else {
                return Err("validated package cleanup refused".into());
            };
            assert_eq!(plan.packages(), ["package-id"]);
        }
        runtime.reuse = true;
        completed(&runtime, &identity.key());
        assert_eq!(
            cleanup(&state, &metadata),
            Cleanup::Packages(PackageClean::default()),
            "fresh no-source-read completion returns the same target to routine reuse"
        );
        started(&runtime, &identity)?;
        std::fs::write(&marker, b"malformed")?;
        assert_eq!(cleanup(&state, &metadata), Cleanup::Family);
        // Only after discarding the whole family may unknown state be reset.
        initialized(&state)?;
        assert_eq!(
            cleanup(&state, &metadata),
            Cleanup::Packages(PackageClean::default())
        );
        std::fs::write(state.join("schema"), b"1")?;
        assert_eq!(cleanup(&state, &metadata), Cleanup::Family);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
