//! Normal adjacent release integrity, separate from compiler-input proof keys.

use super::{Manifest, SCHEMA, Unit, digest};
use crate::managed::dependency_snapshot;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Component, Path};

const INDEX: &str = "release-files.json";
const JSON_LIMIT: u64 = 8 * 1024 * 1024;
const FILE_LIMIT: u64 = 256 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const COUNT_LIMIT: usize = 16_384;

type Files = BTreeMap<String, Member>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    sha256: String,
    bytes: u64,
    mode: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    schema: u32,
    identity: String,
    runtime: String,
    runtime_manifest_sha256: String,
    manifest_sha256: String,
    #[serde(deserialize_with = "unique")]
    members: Files,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema: u32,
    proof: Proof,
    #[serde(deserialize_with = "unique")]
    files: Files,
    #[serde(deserialize_with = "unique")]
    directories: BTreeMap<String, u32>,
}

fn unique<'de, D, V>(deserializer: D) -> Result<BTreeMap<String, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct Entries<V>(std::marker::PhantomData<V>);
    impl<'de, V: Deserialize<'de>> Visitor<'de> for Entries<V> {
        type Value = BTreeMap<String, V>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded map of unique member paths")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry()? {
                if result.len() >= COUNT_LIMIT || result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate or excessive member paths",
                    ));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Entries(std::marker::PhantomData))
}

/// A checked installation inventory for this run, not part of any proof key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Admission {
    index_sha256: String,
    #[serde(deserialize_with = "unique")]
    members: Files,
}

fn bytes(path: &Path) -> Option<Vec<u8>> {
    let mut held = dependency_snapshot::open(path)?.take(JSON_LIMIT + 1);
    let mut raw = Vec::new();
    held.read_to_end(&mut raw).ok()?;
    (raw.len() as u64 <= JSON_LIMIT).then_some(raw)
}

pub(super) fn manifest(bundle: &Path) -> Option<Manifest> {
    serde_json::from_slice(&bytes(&bundle.join("manifest.json"))?).ok()
}

fn fresh_member(path: &Path, member: &Member) -> Option<()> {
    let mut held = dependency_snapshot::open(path)?;
    let metadata = held.metadata().ok()?;
    if metadata.len() != member.bytes || mode(&metadata)? != member.mode {
        return None;
    }
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0u8; 65536];
    loop {
        let used = held.read(&mut buffer).ok()?;
        if used == 0 {
            break;
        }
        size = size.checked_add(used as u64)?;
        if size > member.bytes {
            return None;
        }
        hash.update(&buffer[..used]);
    }
    (size == member.bytes
        && format!("{:x}", hash.finalize()) == member.sha256
        && mode(&held.metadata().ok()?)? == member.mode)
        .then_some(())
}

fn relative(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && name.split('/').all(|part| !part.is_empty())
        && Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "The common API explicitly refuses unsupported platform permission semantics."
)]
fn mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt as _;
    Some(metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn mode(_: &std::fs::Metadata) -> Option<u32> {
    None
}

fn expected_files(bundle: &Path, manifest: &Manifest, members: &Files) -> Option<BTreeSet<String>> {
    let mut expected = BTreeSet::from(["manifest.json".to_owned(), "Cargo.lock".to_owned()]);
    // A release may carry its default proof and one independently recorded
    // raised-limit proof. Both keep exact keys and complete member admission.
    if !(1..=2).contains(&manifest.units.len()) {
        return None;
    }
    for key in manifest.units.keys() {
        if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let prefix = format!("units/{key}/");
        let name = format!("{prefix}unit.json");
        let raw = bytes(&bundle.join(&name))?;
        let bound = members.get(&name)?;
        if digest(&raw) != bound.sha256 || raw.len() as u64 != bound.bytes {
            return None;
        }
        let unit: Unit = serde_json::from_slice(&raw).ok()?;
        if unit.schema != SCHEMA || unit.key != *key || !unit.verifying || unit.crate_name != "vstd"
        {
            return None;
        }
        expected.insert(name);
        for file in &unit.files {
            let name = format!("{prefix}files/{}", file.name);
            if file.name.contains('/')
                || file.name.starts_with('.')
                || !relative(&name)
                || !expected.insert(name)
            {
                return None;
            }
        }
        for file in [&unit.stdout, &unit.stderr] {
            let name = format!("{prefix}{}", file.name);
            if !relative(&name) || !expected.insert(name) {
                return None;
            }
        }
        // The optional rlimit witness (ADR 0060), when the index binds one.
        let witness = format!("{prefix}{}", super::RLIMIT_WITNESS);
        if members.contains_key(&witness) && !expected.insert(witness) {
            return None;
        }
    }
    Some(expected)
}

/// Full new-target admission. Warm naming does not call this inventory scan.
pub(super) fn admit(bundle: &Path, runtime: &Path, manifest: &Manifest) -> Option<Admission> {
    let release = bundle.parent()?.parent()?;
    let variant = runtime.file_name()?.to_str()?;
    let raw = bytes(&release.join(INDEX))?;
    let index: Index = serde_json::from_slice(&raw).ok()?;
    let prefix = format!("deps/{variant}/");
    let manifest_bytes = bytes(&bundle.join("manifest.json"))?;
    if index.schema != 1
        || index.proof.schema != SCHEMA
        || index.proof.identity != manifest.identity
        || index.proof.runtime != variant
        || index.proof.manifest_sha256 != digest(&manifest_bytes)
        || index.proof.runtime_manifest_sha256
            != super::sha256_file(&runtime.join("sail-build.json"))?.0
        || index
            .files
            .get(&format!("engine/{variant}/sail-build.json"))?
            .sha256
            != index.proof.runtime_manifest_sha256
        || index.files.len() > COUNT_LIMIT
        || index.directories.len() > COUNT_LIMIT
        || index
            .files
            .keys()
            .chain(index.directories.keys())
            .any(|name| !relative(name))
        || !std::fs::symlink_metadata(bundle.parent()?).ok()?.is_dir()
        || !std::fs::symlink_metadata(bundle).ok()?.is_dir()
    {
        return None;
    }
    let expected = expected_files(bundle, manifest, &index.proof.members)?;
    if index.proof.members.keys().cloned().collect::<BTreeSet<_>>() != expected
        || index
            .files
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .count()
            != expected.len()
    {
        return None;
    }
    let mut expected_dirs = BTreeSet::new();
    let mut total = 0u64;
    for (name, member) in &index.proof.members {
        if !relative(name)
            || index.files.get(&format!("{prefix}{name}")) != Some(member)
            || member.bytes > FILE_LIMIT
        {
            return None;
        }
        total = total.checked_add(member.bytes)?;
        if total > TOTAL_LIMIT {
            return None;
        }
        let path = bundle.join(name);
        let mut parent = path.parent()?;
        while parent != bundle {
            expected_dirs.insert(parent.to_owned());
            parent = parent.parent()?;
        }
        fresh_member(&path, member)?;
    }
    complete_shape(bundle, &index, &expected, &expected_dirs)?;
    Some(Admission {
        index_sha256: digest(&raw),
        members: index.proof.members,
    })
}

fn complete_shape(
    bundle: &Path,
    index: &Index,
    expected_files: &BTreeSet<String>,
    expected_dirs: &BTreeSet<std::path::PathBuf>,
) -> Option<()> {
    let release = bundle.parent()?.parent()?;
    let prefix = format!("deps/{}/", bundle.file_name()?.to_str()?);
    let mut stack = vec![bundle.to_owned()];
    let mut actual_files = BTreeSet::new();
    let mut actual_dirs = BTreeSet::new();
    while let Some(directory) = stack.pop() {
        let relative_dir = directory.strip_prefix(release).ok()?.to_str()?;
        if mode(&std::fs::symlink_metadata(&directory).ok()?)?
            != *index.directories.get(relative_dir)?
        {
            return None;
        }
        for entry in std::fs::read_dir(&directory).ok()? {
            let path = entry.ok()?.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if metadata.is_dir() {
                actual_dirs.insert(path.clone());
                stack.push(path);
            } else if metadata.is_file() {
                actual_files.insert(path.strip_prefix(bundle).ok()?.to_str()?.to_owned());
            } else {
                return None;
            }
            if actual_files.len() + actual_dirs.len() > COUNT_LIMIT {
                return None;
            }
        }
    }
    (actual_files == *expected_files
        && actual_dirs == *expected_dirs
        && index
            .directories
            .keys()
            .filter(|name| name.starts_with(&prefix))
            .count()
            == actual_dirs.len()
        && mode(&std::fs::symlink_metadata(bundle.parent()?).ok()?)?
            == *index.directories.get("deps")?)
    .then_some(())
}

impl Admission {
    /// Bind the running cache alias to the admitted installation's Sail bytes.
    pub(super) fn executable_matches(&self, bundle: &Path, executable: &Path) -> bool {
        let check = || {
            let root = bundle.parent()?.parent()?;
            let raw = bytes(&root.join(INDEX))?;
            if digest(&raw) != self.index_sha256 {
                return None;
            }
            let index: Index = serde_json::from_slice(&raw).ok()?;
            fresh_member(executable, index.files.get("bin/sail")?)
        };
        check().is_some()
    }

    pub(super) fn intact(&self, bundle: &Path) -> bool {
        bundle
            .parent()
            .and_then(Path::parent)
            .and_then(|root| bytes(&root.join(INDEX)))
            .is_some_and(|raw| digest(&raw) == self.index_sha256)
            && bytes(&bundle.join("manifest.json")).is_some_and(|raw| {
                self.members.get("manifest.json").is_some_and(|member| {
                    member.bytes == raw.len() as u64 && member.sha256 == digest(&raw)
                })
            })
    }

    pub(super) fn read_unit(&self, bundle: &Path, key: &str) -> Option<Vec<u8>> {
        let raw = bytes(&bundle.join("units").join(key).join("unit.json"))?;
        self.unit_matches(bundle, key, &raw).then_some(raw)
    }

    /// Another file stored beside a unit, read only if the index binds it.
    pub(super) fn read_unit_file(&self, bundle: &Path, key: &str, name: &str) -> Option<Vec<u8>> {
        let raw = bytes(&bundle.join("units").join(key).join(name))?;
        (self.intact(bundle)
            && self
                .members
                .get(&format!("units/{key}/{name}"))
                .is_some_and(|member| {
                    member.bytes == raw.len() as u64 && member.sha256 == digest(&raw)
                }))
        .then_some(raw)
    }

    /// Bind precisely the bytes `try_serve` will deserialize, never a second read.
    pub(super) fn unit_matches(&self, bundle: &Path, key: &str, raw: &[u8]) -> bool {
        self.intact(bundle)
            && self
                .members
                .get(&format!("units/{key}/unit.json"))
                .is_some_and(|member| {
                    member.bytes == raw.len() as u64 && member.sha256 == digest(raw)
                })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// Small installation metadata fixture, not a native proof qualification.
    struct Installation {
        root: PathBuf,
        bundle: PathBuf,
        runtime: PathBuf,
        files: Files,
        directories: BTreeMap<String, u32>,
    }

    impl Drop for Installation {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Installation {
        fn new() -> Result<Self> {
            let root = std::env::temp_dir().join(format!(
                "sail-release-index-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root)?;
            let root = root.canonicalize()?;
            let mut fixture = Self {
                bundle: root.join("deps/variant"),
                runtime: root.join("engine/variant"),
                root,
                files: BTreeMap::new(),
                directories: BTreeMap::new(),
            };
            fixture.add("bin/sail", b"qualified Sail")?;
            fixture.add("engine/variant/sail-build.json", b"runtime manifest")?;
            let manifest = json!({
                "schema":SCHEMA, "kind":super::super::KIND,
                "identity":"identity", "runtime":"variant", "virtual_base":"/rustc-dev/test",
                "units":{KEY:"vstd registry"}
            });
            fixture.add(
                "deps/variant/manifest.json",
                &serde_json::to_vec(&manifest)?,
            )?;
            fixture.add("deps/variant/Cargo.lock", b"lock")?;
            let file = |name: &str, raw: &[u8]| {
                json!({
                    "name":name, "sha256":digest(raw), "size":raw.len(), "executable":false
                })
            };
            let unit = json!({
                "schema":SCHEMA, "key":KEY, "crate_name":"vstd", "package":"vstd registry",
                "verifying":true, "args":[], "environment":{}, "source_tree":{},
                "establishing_run":"original", "compiler_reads":null,
                "files":[file("libvstd.rmeta", b"metadata")], "dep_info":"", "sources":[],
                "declared":[], "stdout":file("stdout", b"stdout"), "stderr":file("stderr", b"stderr")
            });
            fixture.add(
                &format!("deps/variant/units/{KEY}/unit.json"),
                &serde_json::to_vec(&unit)?,
            )?;
            for (name, raw) in [
                ("files/libvstd.rmeta", b"metadata".as_slice()),
                ("stdout", b"stdout"),
                ("stderr", b"stderr"),
            ] {
                fixture.add(&format!("deps/variant/units/{KEY}/{name}"), raw)?;
            }
            fixture.write_index()?;
            Ok(fixture)
        }

        fn add(&mut self, name: &str, raw: &[u8]) -> Result {
            let path = self.root.join(name);
            let parent = path.parent().ok_or("parent")?;
            std::fs::create_dir_all(parent)?;
            std::fs::write(&path, raw)?;
            self.files.insert(
                name.into(),
                Member {
                    sha256: digest(raw),
                    bytes: raw.len() as u64,
                    mode: mode(&path.metadata()?).ok_or("file mode")?,
                },
            );
            let mut directory = parent;
            while directory != self.root {
                self.directories.insert(
                    directory
                        .strip_prefix(&self.root)?
                        .to_str()
                        .ok_or("path")?
                        .into(),
                    mode(&directory.metadata()?).ok_or("directory mode")?,
                );
                directory = directory.parent().ok_or("directory parent")?;
            }
            Ok(())
        }

        fn write_index(&self) -> Result {
            let members: Files = self
                .files
                .iter()
                .filter_map(|(name, member)| {
                    name.strip_prefix("deps/variant/")
                        .map(|name| (name.into(), member.clone()))
                })
                .collect();
            let index = json!({
                "schema":1, "proof":{
                    "schema":SCHEMA, "identity":"identity", "runtime":"variant",
                    "runtime_manifest_sha256":digest(b"runtime manifest"),
                    "manifest_sha256":self.files["deps/variant/manifest.json"].sha256,
                    "members":members
                }, "files":self.files, "directories":self.directories
            });
            std::fs::write(self.root.join(INDEX), serde_json::to_vec(&index)?)?;
            Ok(())
        }

        fn admit(&self) -> Option<Admission> {
            admit(&self.bundle, &self.runtime, &manifest(&self.bundle)?)
        }

        fn child_plan(&self) -> Option<super::super::Plan> {
            Some(super::super::Plan {
                mode: super::super::Mode::Serve {
                    bundle: self.bundle.clone(),
                },
                identity: "identity".into(),
                registry: self.root.join("registry"),
                target: self.root.join("target"),
                virtual_base: "/rustc-dev/test".into(),
                solver_sha256: "solver".into(),
                solver_adapter: None,
                run_id: "run".into(),
                closure: None,
                native_records: self.root.join("records"),
                native_compiler_sha256: "compiler".into(),
                native_launcher: self.runtime.join("verus/source/target-verus/release/verus"),
                native_launcher_sha256: "launcher".into(),
                compiler_source: None,
                release: Some(self.admit()?),
            })
        }
    }

    #[test]
    fn bound_rlimit_witness_is_admitted_and_read_only_while_bound() -> Result {
        let mut fixture = Installation::new()?;
        let name = format!("deps/variant/units/{KEY}/{}", super::super::RLIMIT_WITNESS);
        // Present but not bound by the index: the whole installation refuses.
        std::fs::write(fixture.root.join(&name), b"witness")?;
        assert!(fixture.admit().is_none());
        fixture.add(&name, b"witness")?;
        fixture.write_index()?;
        let admission = fixture.admit().ok_or("bound witness admitted")?;
        assert_eq!(
            admission.read_unit_file(&fixture.bundle, KEY, super::super::RLIMIT_WITNESS),
            Some(b"witness".to_vec())
        );
        std::fs::write(fixture.root.join(&name), b"changed")?;
        assert_eq!(
            admission.read_unit_file(&fixture.bundle, KEY, super::super::RLIMIT_WITNESS),
            None
        );
        Ok(())
    }

    #[test]
    fn retained_release_witness_rechecks_unit_output_and_index() -> Result {
        let fixture = Installation::new()?;
        let plan = fixture.child_plan().ok_or("admitted plan")?;
        std::fs::create_dir_all(&plan.target)?;
        let output = plan.target.join("libvstd.rmeta");
        std::fs::write(&output, b"metadata")?;
        let body = dependency_snapshot::read_content(&output, 1024).ok_or("body")?;
        let document = fixture.bundle.join("units").join(KEY).join("unit.json");
        let raw = std::fs::read(&document)?;
        let proof = super::super::RetainedProof {
            plan,
            key: KEY.into(),
            unit_sha256: digest(&raw),
            outputs: BTreeMap::from([(output.clone(), body.clone())]),
        };
        // Storage integrity only: this fixture has no compiler/source/native
        // authority and deliberately cannot authorize a retained proof.
        assert!(proof.release_output_intact(&output, &body));
        assert!(!proof.intact(&output, &body));
        assert!(proof.for_run("run", "compiler", &fixture.root.join("target")));
        assert!(!proof.for_run("other", "compiler", &fixture.root.join("target")));
        assert!(!proof.for_run("run", "foreign", &fixture.root.join("target")));
        assert!(!proof.for_run("run", "compiler", &fixture.root.join("other")));
        std::fs::write(&output, b"metadatz")?;
        assert!(!proof.release_output_intact(&output, &body));
        std::fs::write(&output, b"metadata")?;
        let foreign = fixture.root.join("libvstd.rmeta");
        std::fs::write(&foreign, b"metadata")?;
        assert!(!proof.release_output_intact(&foreign, &body));
        std::fs::write(&document, [raw.as_slice(), b" "].concat())?;
        assert!(!proof.release_output_intact(&output, &body));
        std::fs::write(&document, raw)?;
        assert!(proof.release_output_intact(&output, &body));
        let index = fixture.root.join(INDEX);
        std::fs::write(&index, b"changed index")?;
        assert!(!proof.release_output_intact(&output, &body));
        Ok(())
    }

    #[test]
    fn only_indexed_complete_installation_is_admitted() -> Result {
        let fixture = Installation::new()?;
        let accepted = fixture.admit().ok_or("positive admission")?;
        let raw = accepted
            .read_unit(&fixture.bundle, KEY)
            .ok_or("unit bytes")?;
        assert!(accepted.unit_matches(&fixture.bundle, KEY, &raw));
        let mut changed = raw.clone();
        changed.push(b' ');
        assert!(!accepted.unit_matches(&fixture.bundle, KEY, &changed));
        std::fs::remove_file(fixture.root.join(INDEX))?;
        assert!(
            manifest(&fixture.bundle).is_some(),
            "benign warm naming survives index loss"
        );
        assert!(
            fixture.admit().is_none(),
            "an unindexed research bundle cannot auto-serve"
        );
        assert!(!accepted.intact(&fixture.bundle));
        fixture.write_index()?;
        let indexed = std::fs::read(fixture.root.join(INDEX))?;
        let mut old: Value = serde_json::from_slice(&indexed)?;
        old["proof"]["schema"] = json!(SCHEMA - 1);
        std::fs::write(fixture.root.join(INDEX), serde_json::to_vec(&old)?)?;
        assert!(fixture.admit().is_none());
        std::fs::write(fixture.root.join(INDEX), b"{}")?;
        assert!(fixture.admit().is_none());
        Ok(())
    }

    #[test]
    fn changed_missing_extra_or_aliased_members_refuse() -> Result {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let fixture = Installation::new()?;
        let member = fixture
            .bundle
            .join(format!("units/{KEY}/files/libvstd.rmeta"));
        std::fs::write(&member, b"different bytes")?;
        assert!(fixture.admit().is_none());
        std::fs::write(&member, b"metadata")?;
        let permissions = member.metadata()?.permissions();
        std::fs::set_permissions(&member, std::fs::Permissions::from_mode(0o700))?;
        assert!(fixture.admit().is_none());
        std::fs::set_permissions(&member, permissions)?;
        let directory = member.parent().ok_or("member parent")?;
        let permissions = directory.metadata()?.permissions();
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        assert!(fixture.admit().is_none());
        std::fs::set_permissions(directory, permissions)?;
        std::fs::write(fixture.bundle.join("extra"), b"extra")?;
        assert!(fixture.admit().is_none());
        std::fs::remove_file(fixture.bundle.join("extra"))?;
        std::fs::remove_file(&member)?;
        assert!(fixture.admit().is_none());
        let other = fixture.root.join("other");
        std::fs::write(&other, b"metadata")?;
        symlink(&other, &member)?;
        assert!(fixture.admit().is_none());
        Ok(())
    }

    #[test]
    fn stale_manifest_unit_and_runtime_refuse() -> Result {
        let fixture = Installation::new()?;
        let accepted = fixture.admit().ok_or("positive")?;
        let unit_path = fixture.bundle.join(format!("units/{KEY}/unit.json"));
        let original = std::fs::read(&unit_path)?;
        let mut changed: Value = serde_json::from_slice(&original)?;
        changed["establishing_run"] = json!("foreign");
        std::fs::write(&unit_path, serde_json::to_vec(&changed)?)?;
        assert!(fixture.admit().is_none());
        assert!(accepted.read_unit(&fixture.bundle, KEY).is_none());
        std::fs::write(&unit_path, original)?;
        std::fs::write(fixture.runtime.join("sail-build.json"), b"other runtime")?;
        assert!(fixture.admit().is_none());
        std::fs::write(fixture.runtime.join("sail-build.json"), b"runtime manifest")?;
        std::fs::write(fixture.bundle.join("manifest.json"), b"{}")?;
        assert!(!accepted.intact(&fixture.bundle));
        Ok(())
    }

    #[test]
    fn duplicate_or_unsafe_index_names_refuse() -> Result {
        let fixture = Installation::new()?;
        let path = fixture.root.join(INDEX);
        let original = std::fs::read_to_string(&path)?;
        let duplicate = original.replacen(
            "\"files\":{",
            "\"files\":{\"duplicate\":{\"sha256\":\"x\",\"bytes\":0,\"mode\":420},\"duplicate\":{\"sha256\":\"x\",\"bytes\":0,\"mode\":420},",
            1,
        );
        std::fs::write(&path, duplicate)?;
        assert!(fixture.admit().is_none());
        let mut index: Value = serde_json::from_str(&original)?;
        index["files"]["../foreign"] = json!({"sha256":"x", "bytes":0, "mode":420});
        std::fs::write(&path, serde_json::to_vec(&index)?)?;
        assert!(fixture.admit().is_none());
        Ok(())
    }

    #[test]
    fn compiler_alias_admission_uses_bound_runtime_instead_of_alias_parent() -> Result {
        let fixture = Installation::new()?;
        let plan = fixture.child_plan().ok_or("admitted child fixture")?;
        let cache = fixture.root.join("cache/compiler-artifacts/family");
        std::fs::create_dir_all(&cache)?;
        let hard = cache.join("sail-verus-driver");
        let copy = cache.join("sail-cargo");
        std::fs::hard_link(fixture.root.join("bin/sail"), &hard)?;
        std::fs::copy(fixture.root.join("bin/sail"), &copy)?;
        for alias in [&hard, &copy] {
            let named = alias.canonicalize()?;
            assert_ne!(named, fixture.root.join("bin/sail"));
            // The former current_exe-parent calculation selects this wrong root.
            assert_ne!(
                named
                    .parent()
                    .and_then(Path::parent)
                    .ok_or("alias parents")?
                    .join("deps/variant"),
                fixture.bundle
            );
            assert!(super::super::ordinary_child(&plan, &fixture.runtime, alias));
        }
        let modified = std::fs::metadata(&copy)?.modified()?;
        std::fs::write(&copy, b"Qualified Sail")?;
        std::fs::File::options()
            .write(true)
            .open(&copy)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
        assert!(!super::super::ordinary_child(
            &plan,
            &fixture.runtime,
            &copy
        ));
        assert!(super::super::ordinary_child(&plan, &fixture.runtime, &hard));
        Ok(())
    }

    #[test]
    fn compiler_alias_admission_refuses_missing_foreign_roots_and_changed_index() -> Result {
        let fixture = Installation::new()?;
        let plan = fixture.child_plan().ok_or("admitted child fixture")?;
        let executable = fixture.root.join("bin/sail");
        assert!(super::super::ordinary_child(
            &plan,
            &fixture.runtime,
            &executable
        ));
        let missing = fixture.runtime.with_file_name("missing");
        assert!(!super::super::ordinary_child(&plan, &missing, &executable));
        let foreign = fixture.root.join("foreign/engine/variant");
        std::fs::create_dir_all(&foreign)?;
        std::fs::write(foreign.join("sail-build.json"), b"runtime manifest")?;
        assert!(!super::super::ordinary_child(&plan, &foreign, &executable));
        let wrong_parent = fixture.root.join("other/variant");
        std::fs::create_dir_all(&wrong_parent)?;
        std::fs::write(wrong_parent.join("sail-build.json"), b"runtime manifest")?;
        assert!(!super::super::ordinary_child(
            &plan,
            &wrong_parent,
            &executable
        ));
        let path = fixture.root.join(INDEX);
        let raw = std::fs::read(&path)?;
        let modified = std::fs::metadata(&path)?.modified()?;
        let mut changed = raw.clone();
        let at = changed
            .windows(b"\"schema\":1".len())
            .position(|bytes| bytes == b"\"schema\":1")
            .ok_or("schema field")?;
        changed[at + b"\"schema\":".len()] = b'2';
        std::fs::write(&path, changed)?;
        std::fs::File::options()
            .write(true)
            .open(&path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
        assert!(!super::super::ordinary_child(
            &plan,
            &fixture.runtime,
            &executable
        ));
        std::fs::remove_file(&path)?;
        assert!(!super::super::ordinary_child(
            &plan,
            &fixture.runtime,
            &executable
        ));
        std::fs::write(&path, raw)?;
        assert!(super::super::ordinary_child(
            &plan,
            &fixture.runtime,
            &executable
        ));
        Ok(())
    }

    #[test]
    fn two_profiles_bind_each_unit_and_reject_a_third() -> Result {
        let mut fixture = Installation::new()?;
        for key in ["b".repeat(64), "c".repeat(64)] {
            let original = fixture.bundle.join("units").join(KEY);
            for name in ["files/libvstd.rmeta", "stdout", "stderr"] {
                fixture.add(
                    &format!("deps/variant/units/{key}/{name}"),
                    &std::fs::read(original.join(name))?,
                )?;
            }
            let mut unit: serde_json::Value =
                serde_json::from_slice(&std::fs::read(original.join("unit.json"))?)?;
            unit["key"] = json!(key);
            let name = format!("deps/variant/units/{key}/unit.json");
            let raw = serde_json::to_vec(&unit)?;
            fixture.add(&name, &raw)?;
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(fixture.bundle.join("manifest.json"))?)?;
            manifest["units"][&key] = json!("vstd registry");
            fixture.add(
                "deps/variant/manifest.json",
                &serde_json::to_vec(&manifest)?,
            )?;
            fixture.write_index()?;
            if key.starts_with('b') {
                assert!(fixture.admit().is_some());
                unit["establishing_run"] = json!("unbound");
                std::fs::write(fixture.root.join(&name), serde_json::to_vec(&unit)?)?;
                assert!(fixture.admit().is_none());
                std::fs::write(fixture.root.join(&name), raw)?;
                assert!(fixture.admit().is_some());
            } else {
                assert!(fixture.admit().is_none());
            }
        }
        Ok(())
    }
}
