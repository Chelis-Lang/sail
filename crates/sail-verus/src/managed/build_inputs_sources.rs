//! Bounded declared path inventories, never shared content or proof authority.

use super::{BuildInput, bytes, digest, encode};
use crate::managed::source_inputs::Inputs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MAX_PATHS: usize = 4096;
const MAX_PACKAGES: usize = 128;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inventory {
    packages: BTreeMap<String, Package>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    identity: String,
    paths: Vec<PathBuf>,
}

fn package<'a>(data: &'a Value, id: &str) -> Option<&'a Value> {
    let mut matched = data["packages"]
        .as_array()?
        .iter()
        .filter(|p| p["id"] == id);
    let package = matched.next()?;
    if matched.next().is_some() {
        return None;
    }
    Some(package)
}

fn identity(package: &Value) -> Option<String> {
    // Preserve the actual Cargo role, including target/source/manifest identity.
    // Bounded encoding precedes any digest-sized retained allocation.
    encode(&(
        &package["id"],
        &package["name"],
        &package["version"],
        &package["source"],
        &package["manifest_path"],
        &package["targets"],
    ))
    .map(|bytes| digest(&bytes))
}

fn canonical(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().len() <= MAX_PATH_BYTES
        && path.canonicalize().is_ok_and(|actual| actual == path)
        && path.is_file()
}

fn supported(
    path: &Path,
    package: &Value,
    records: &[BuildInput],
    target: &Path,
    data: &Value,
) -> bool {
    if !canonical(path) {
        return false;
    }
    let Some(directory) = package["manifest_path"]
        .as_str()
        .map(Path::new)
        .and_then(Path::parent)
        .filter(|p| p.is_absolute() && p.canonicalize().is_ok_and(|actual| actual == *p))
    else {
        return false;
    };
    // An excluded target path needs its exact consuming package's already
    // validated script output unit. Containment in the workspace alone cannot
    // authorize arbitrary compiler artifacts as generated source declarations.
    let excluded = data["sail_cache_directory"]
        .as_str()
        .map(Path::new)
        .is_some_and(|cache| path.starts_with(cache));
    if !excluded && !path.starts_with(target) && path.starts_with(directory) {
        return true;
    }
    records.iter().any(|record| {
        package["id"] == record.package_id
            && record.output.as_ref().is_some_and(|(output, _)| {
                output.parent().is_some_and(|unit| {
                    let generated = unit.join("out");
                    generated.starts_with(target)
                        && generated
                            .canonicalize()
                            .is_ok_and(|actual| actual == generated)
                        && path.starts_with(generated)
                })
            })
    })
}

impl Inventory {
    pub(super) fn capture(
        local: &Path,
        data: &Value,
        records: &[BuildInput],
        target: &Path,
    ) -> Option<Self> {
        if target.canonicalize().ok()?.as_path() != target {
            return None;
        }
        let inputs: Inputs = serde_json::from_slice(&bytes(local)?).ok()?;
        if !inputs.covered || inputs.paths.len() > MAX_PATHS {
            return None;
        }
        let scoped = inputs.by_package?;
        if scoped.is_empty() || scoped.len() > MAX_PACKAGES {
            return None;
        }
        let mut union = BTreeSet::new();
        let mut count = 0_usize;
        let mut packages = BTreeMap::new();
        for (id, paths) in scoped {
            count = count.checked_add(paths.len())?;
            if count > MAX_PATHS || paths.is_empty() {
                return None;
            }
            let metadata = package(data, &id)?;
            if paths
                .iter()
                .any(|p| !supported(p, metadata, records, target, data))
            {
                return None;
            }
            union.extend(paths.iter().cloned());
            packages.insert(
                id,
                Package {
                    identity: identity(metadata)?,
                    paths,
                },
            );
        }
        let declared: BTreeSet<_> = inputs.paths.into_iter().collect();
        if union != declared {
            return None;
        }
        Some(Self { packages })
    }

    pub(super) fn admit(
        &self,
        data: &Value,
        records: &[BuildInput],
        target: &Path,
    ) -> Option<Inputs> {
        if target.canonicalize().ok()?.as_path() != target {
            return None;
        }
        if self.packages.is_empty() || self.packages.len() > MAX_PACKAGES {
            return None;
        }
        let mut count = 0_usize;
        let mut paths = BTreeSet::new();
        let mut scoped = BTreeMap::new();
        for (id, entry) in &self.packages {
            count = count.checked_add(entry.paths.len())?;
            if count > MAX_PATHS || entry.paths.is_empty() {
                return None;
            }
            let metadata = package(data, id)?;
            if identity(metadata).as_ref() != Some(&entry.identity)
                || entry
                    .paths
                    .iter()
                    .any(|p| !supported(p, metadata, records, target, data))
            {
                return None;
            }
            paths.extend(entry.paths.iter().cloned());
            scoped.insert(id.clone(), entry.paths.clone());
        }
        Some(Inputs {
            paths: paths.into_iter().collect(),
            covered: true,
            by_package: Some(scoped),
        })
    }
}

pub(super) fn merge(local: &Path, mut incoming: Inputs) -> Option<Inputs> {
    match std::fs::symlink_metadata(local) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(incoming),
        Ok(_) => {}
        Err(_) => return None,
    }
    // Never drop earlier private declarations or upgrade an incomplete private
    // ledger merely because a different selector completed its own transaction.
    let previous: Inputs = serde_json::from_slice(&bytes(local)?).ok()?;
    incoming.covered &= previous.covered && previous.by_package.is_some();
    let mut scoped = incoming.by_package.take()?;
    let mut paths: BTreeSet<_> = incoming.paths.into_iter().collect();
    for (id, old) in previous.by_package.unwrap_or_default() {
        let current = scoped.entry(id).or_default();
        current.extend(old);
        current.sort();
        current.dedup();
    }
    paths.extend(previous.paths);
    if paths.len() > MAX_PATHS
        || scoped.len() > MAX_PACKAGES
        || paths.iter().any(|p| p.as_os_str().len() > MAX_PATH_BYTES)
        || scoped.values().map(Vec::len).sum::<usize>() > MAX_PATHS
    {
        return None;
    }
    incoming.paths = paths.into_iter().collect();
    incoming.by_package = Some(scoped);
    encode(&incoming)?;
    Some(incoming)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = Result<(), Box<dyn std::error::Error>>;
    struct Fixture {
        root: PathBuf,
        target: PathBuf,
        local: PathBuf,
        data: Value,
        records: Vec<BuildInput>,
        generated: PathBuf,
    }
    impl Fixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "sail-family-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            let target = root.join("cache/target");
            std::fs::create_dir_all(target.join("unit/out"))?;
            let root = root.canonicalize()?;
            let target = root.join("cache/target");
            let generated = target.join("unit/out/private.rs");
            std::fs::write(&generated, b"pub const VALUE: u8 = 1;\n")?;
            let source = root.join("src/lib.rs");
            std::fs::create_dir_all(source.parent().ok_or("source parent")?)?;
            std::fs::write(&source, b"include!(\"generated\");\n")?;
            let manifest = root.join("Cargo.toml");
            std::fs::write(&manifest, "[package]\nname='fixture'\n")?;
            let data = serde_json::json!({"sail_cache_directory":root.join("cache"),"packages":[{
                "id":"fixture", "name":"fixture", "version":"0.0.0", "source":null,
                "manifest_path":manifest, "targets":[{"kind":["lib"]},{"kind":["custom-build"]}]
            }]});
            let local = root.join("cache/source-inputs.json");
            let paths = vec![source, generated.clone()];
            crate::managed::files::json(
                &local,
                &Inputs {
                    paths: paths.clone(),
                    covered: true,
                    by_package: Some(BTreeMap::from([("fixture".to_owned(), paths)])),
                },
            )?;
            let record = BuildInput {
                package_id: "fixture".to_owned(),
                package_name: "fixture".to_owned(),
                manifest,
                paths: Vec::new(),
                env: BTreeMap::new(),
                digest: String::new(),
                covered: true,
                output: Some((target.join("unit/output"), "seal".to_owned())),
                unit_out_dir: None,
            };
            Ok(Self {
                root,
                target,
                local,
                data,
                records: vec![record],
                generated,
            })
        }
        fn capture(&self) -> Option<Inventory> {
            Inventory::capture(&self.local, &self.data, &self.records, &self.target)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn typed_source_inventory_refuses_unknown_packages_roles_aliases_and_limits() -> TestResult {
        let fixture = Fixture::new()?;
        let inventory = fixture.capture().ok_or("inventory refused")?;
        assert!(
            inventory
                .admit(&fixture.data, &fixture.records, &fixture.target)
                .is_some()
        );
        let mut changed = fixture.data.clone();
        changed["packages"][0]["version"] = "0.0.1".into();
        assert!(
            inventory
                .admit(&changed, &fixture.records, &fixture.target)
                .is_none()
        );
        changed["packages"] = serde_json::json!([]);
        assert!(
            inventory
                .admit(&changed, &fixture.records, &fixture.target)
                .is_none()
        );
        assert!(
            inventory
                .admit(&fixture.data, &[], &fixture.target)
                .is_none()
        );
        let mut changed = fixture.data.clone();
        let duplicate = changed["packages"][0].clone();
        changed["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .push(duplicate);
        assert!(
            inventory
                .admit(&changed, &fixture.records, &fixture.target)
                .is_none()
        );
        std::fs::remove_file(&fixture.generated)?;
        std::os::unix::fs::symlink(fixture.root.join("src/lib.rs"), &fixture.generated)?;
        assert!(
            inventory
                .admit(&fixture.data, &fixture.records, &fixture.target)
                .is_none()
        );
        std::fs::remove_file(&fixture.generated)?;
        std::fs::write(&fixture.generated, b"restored")?;
        let escape = fixture.target.join("unit/executable");
        std::fs::write(&escape, b"not a generated source role")?;
        let mut value = serde_json::to_value(&inventory)?;
        value["packages"]["fixture"]["paths"] = serde_json::json!([escape]);
        let invalid: Inventory = serde_json::from_value(value)?;
        assert!(
            invalid
                .admit(&fixture.data, &fixture.records, &fixture.target)
                .is_none()
        );
        let mut invalid = inventory;
        invalid.packages.get_mut("fixture").ok_or("package")?.paths =
            vec![fixture.generated.clone(); MAX_PATHS + 1];
        assert!(
            invalid
                .admit(&fixture.data, &fixture.records, &fixture.target)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn incomplete_or_unattributed_sources_cannot_export_and_private_roots_survive() -> TestResult {
        let fixture = Fixture::new()?;
        let inventory = fixture.capture().ok_or("inventory")?;
        let mut inputs: Inputs = serde_json::from_slice(&std::fs::read(&fixture.local)?)?;
        inputs.covered = false;
        crate::managed::files::json(&fixture.local, &inputs)?;
        assert!(fixture.capture().is_none());
        let admitted = inventory
            .admit(&fixture.data, &fixture.records, &fixture.target)
            .ok_or("admission")?;
        let merged = merge(&fixture.local, admitted).ok_or("merge")?;
        assert!(!merged.covered);
        assert_eq!(
            merged.paths,
            inputs
                .paths
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        inputs.covered = true;
        inputs.by_package = None;
        crate::managed::files::json(&fixture.local, &inputs)?;
        assert!(fixture.capture().is_none());
        inputs.by_package = Some(BTreeMap::new());
        crate::managed::files::json(&fixture.local, &inputs)?;
        assert!(fixture.capture().is_none());
        std::fs::write(&fixture.local, "corrupt")?;
        let admitted = inventory
            .admit(&fixture.data, &fixture.records, &fixture.target)
            .ok_or("admission")?;
        assert!(merge(&fixture.local, admitted).is_none());
        Ok(())
    }

    #[test]
    fn imported_paths_read_changed_source_bytes_independently_before_acquisition() -> TestResult {
        let fixture = Fixture::new()?;
        let inventory = fixture.capture().ok_or("inventory")?;
        let before = inventory
            .admit(&fixture.data, &fixture.records, &fixture.target)
            .ok_or("first admission")?;
        let original = crate::managed::input::snapshot_paths(&before.paths);
        assert!(original.1);
        std::fs::write(&fixture.generated, b"pub const VALUE: u8 = 2;\n")?;
        std::fs::write(fixture.root.join("src/lib.rs"), b"different actual Rust\n")?;
        let after = inventory
            .admit(&fixture.data, &fixture.records, &fixture.target)
            .ok_or("path inventory should not cache content")?;
        assert_eq!(before.paths, after.paths);
        let actual = crate::managed::input::snapshot_paths(&after.paths);
        assert!(actual.1);
        assert_ne!(original.0, actual.0);
        Ok(())
    }
}
