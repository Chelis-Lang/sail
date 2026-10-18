//! Exact source/feature authority for the initial real Vest dependency graph.
//!
//! Source equality is necessary, not sufficient. This does not establish what
//! a native compiler read, replace successful native producer provenance, admit
//! arbitrary macro I/O, or authorize a retained proof by itself.

use super::super::dependency_snapshot::open;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../engine/verus/retained-vest-inputs.json"
));
const PUBLIC_POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../engine/verus/retained-public-inputs.json"
));

const PUBLIC_VEST_POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../engine/verus/retained-public-vest-inputs.json"
));

/// Each profile is independently closed; path provenance never substitutes for
/// the public archive body map or vice versa.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
enum Authority {
    #[default]
    OfficialVest,
    PublicRegistry,
    PublicRegistryVest,
}

impl Authority {
    fn from_graph(
        packages: &BTreeMap<&str, &Value>,
        nodes: &BTreeMap<&str, &Value>,
    ) -> Option<Self> {
        let vstd = packages.values().find(|package| {
            package["name"] == "vstd" && package["version"] == "0.0.0-2026-10-04-0306"
        })?;
        Self::select(vstd, nodes.get(vstd["id"].as_str()?)?)
    }

    fn select(vstd: &Value, node: &Value) -> Option<Self> {
        if vstd["source"].is_null() {
            Some(Self::OfficialVest)
        } else if vstd["source"] == "registry+https://github.com/rust-lang/crates.io-index" {
            let features = node["features"]
                .as_array()?
                .iter()
                .map(|v| v.as_str())
                .collect::<Option<BTreeSet<_>>>()?;
            if features == BTreeSet::from(["alloc", "default", "std"]) {
                Some(Self::PublicRegistry)
            } else if features == BTreeSet::from(["alloc", "std"]) {
                Some(Self::PublicRegistryVest)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn bytes(self) -> &'static str {
        match self {
            Self::OfficialVest => POLICY,
            Self::PublicRegistry => PUBLIC_POLICY,
            Self::PublicRegistryVest => PUBLIC_VEST_POLICY,
        }
    }

    fn policy(self) -> Option<Policy> {
        let policy: Policy = serde_json::from_str(self.bytes()).ok()?;
        let kind = match self {
            Self::OfficialVest => "sail-retained-vest-official-inputs",
            Self::PublicRegistry => "sail-retained-public-registry-inputs",
            Self::PublicRegistryVest => "sail-retained-public-vest-inputs",
        };
        (policy.kind == kind
            && policy.schema == 1
            && policy.verus_source_commit == crate::toolchain::Pins::embedded().ok()?.verus.commit)
            .then_some(policy)
    }
}

const VSTD: &str = "vstd-0.0.0-2026-10-04-0306";

#[derive(Deserialize)]
struct Policy {
    kind: String,
    schema: u32,
    verus_source_commit: String,
    packages: BTreeMap<String, Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    features: BTreeSet<String>,
    origin: Origin,
    files: BTreeMap<String, String>,
    compiler_files: BTreeMap<String, CompilerSource>,
}

#[derive(Deserialize)]
struct CompilerSource {
    checksum: String,
    size: u64,
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum Origin {
    #[serde(rename = "pinned-verus-source")]
    Git { commit: String, path: PathBuf },
    #[serde(rename = "crates-io")]
    Registry { archive_sha256: String },
}

/// Literal provider directories, never inferred from a basename in `-L`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Closure {
    #[serde(default)]
    authority: Authority,
    directories: BTreeMap<String, PathBuf>,
}

impl Closure {
    /// Cheap ordinary-mode routing only, never source or proof authority.
    ///
    /// Avoid provider reads and graph/native side effects for profiles outside
    /// the measured ordinary scope. Full admission below remains mandatory for
    /// a matching profile, including every dependency's exact source/features.
    pub(super) fn automatic_profile_candidate(metadata: &Value) -> bool {
        let authority = || {
            let packages = metadata["packages"]
                .as_array()?
                .iter()
                .map(|package| Some((package["id"].as_str()?, package)))
                .collect::<Option<BTreeMap<_, _>>>()?;
            let nodes = metadata["resolve"]["nodes"]
                .as_array()?
                .iter()
                .map(|node| Some((node["id"].as_str()?, node)))
                .collect::<Option<BTreeMap<_, _>>>()?;
            Authority::from_graph(&packages, &nodes)
        };
        matches!(authority(), Some(Authority::PublicRegistryVest))
    }

    /// Initial scope is a verified workspace library with exactly the pinned
    /// vstd normal dependency, no custom build script or macro provider. Dev
    /// packages elsewhere in Cargo's metadata do not silently widen this set.
    pub(super) fn admit(
        metadata: &Value,
        manifest: &Path,
        verus_source: &Path,
        registry: &Path,
    ) -> Option<Self> {
        let packages: BTreeMap<_, _> = metadata["packages"]
            .as_array()?
            .iter()
            .map(|package| Some((package["id"].as_str()?, package)))
            .collect::<Option<_>>()?;
        let nodes: BTreeMap<_, _> = metadata["resolve"]["nodes"]
            .as_array()?
            .iter()
            .map(|node| Some((node["id"].as_str()?, node)))
            .collect::<Option<_>>()?;
        let (root_id, root) = packages.iter().find(|(_, package)| {
            package["manifest_path"].as_str().map(Path::new) == Some(manifest)
        })?;
        if !root["source"].is_null()
            || !root.pointer("/metadata/verus/verify")?.as_bool()?
            || !metadata["workspace_members"]
                .as_array()?
                .iter()
                .any(|id| id.as_str() == Some(*root_id))
            || root["targets"].as_array()?.iter().any(|target| {
                target["kind"].as_array().is_none_or(|kinds| {
                    kinds
                        .iter()
                        .any(|kind| matches!(kind.as_str(), Some("proc-macro" | "custom-build")))
                })
            })
        {
            return None;
        }
        let authority = Authority::from_graph(&packages, &nodes)?;
        let policy = authority.policy()?;
        let root_deps = normal_dependencies(nodes.get(root_id)?)?;
        if root_deps.len() != 1 {
            return None;
        }
        let mut pending = root_deps;
        let mut seen = BTreeSet::new();
        let mut directories = BTreeMap::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let package = packages.get(id)?;
            let key = format!(
                "{}-{}",
                package["name"].as_str()?,
                package["version"].as_str()?
            );
            if directories.is_empty() && key != VSTD {
                return None;
            }
            let expected = policy.packages.get(&key)?;
            let directory = match &expected.origin {
                Origin::Git { commit, path } => {
                    if commit != &policy.verus_source_commit || !package["source"].is_null() {
                        return None;
                    }
                    verus_source.join(path)
                }
                Origin::Registry { .. } => {
                    if package["source"].as_str()?
                        != "registry+https://github.com/rust-lang/crates.io-index"
                    {
                        return None;
                    }
                    registry.join(&key)
                }
            };
            if package["manifest_path"].as_str().map(Path::new)
                != Some(directory.join("Cargo.toml").as_path())
                || directory.canonicalize().ok()? != directory
                || nodes.get(id)?["features"]
                    .as_array()?
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned))
                    .collect::<Option<BTreeSet<_>>>()?
                    != expected.features
            {
                return None;
            }
            directories.insert(key, directory);
            pending.extend(normal_dependencies(nodes.get(id)?)?);
        }
        if directories.keys().ne(policy.packages.keys()) {
            return None;
        }
        let closure = Self {
            authority,
            directories,
        };
        closure.check_sources().then_some(closure)
    }

    /// Actual loaded native `SourceMap` bytes must match independent SHA-authorized
    /// source bodies. The later mutable path is not the evidence of consumption.
    pub(super) fn native_sources<'a>(
        &self,
        sources: impl Iterator<Item = (&'a Path, &'a str, u64)>,
    ) -> bool {
        let Some(policy) = self.authority.policy() else {
            return false;
        };
        let mut count = 0;
        for (path, checksum, size) in sources {
            let Some(path) = super::lexical(path) else {
                return false;
            };
            count += 1;
            if count > 16_384 {
                return false;
            }
            let matches = self
                .directories
                .iter()
                .filter_map(|(key, root)| {
                    let relative = path.strip_prefix(root).ok()?.to_str()?;
                    let package = policy.packages.get(key)?;
                    let expected = package.compiler_files.get(relative)?;
                    (package.files.contains_key(relative)
                        && expected.checksum == checksum
                        && expected.size == size)
                        .then_some(())
                })
                .count();
            if matches != 1 {
                return false;
            }
        }
        count > 0
    }

    pub(super) fn policy_digest(&self) -> String {
        super::super::files::digest(self.authority.bytes().as_bytes())
    }

    pub(super) fn source_roots(&self) -> BTreeSet<PathBuf> {
        self.directories.values().cloned().collect()
    }

    pub(super) fn check_sources(&self) -> bool {
        let Some(policy) = self.authority.policy() else {
            return false;
        };
        self.directories.keys().eq(policy.packages.keys())
            && self.directories.iter().all(|(key, path)| {
                policy
                    .packages
                    .get(key)
                    .is_some_and(|expected| check_package(path, expected))
            })
    }

    /// Actual Cargo package identity and feature flags must agree too. The
    /// caller still applies the independently reviewed compiler-option policy.
    pub(super) fn unit(&self, environment: &BTreeMap<String, String>, args: &[OsString]) -> bool {
        let Some(name) = environment.get("CARGO_PKG_NAME") else {
            return false;
        };
        let Some(version) = environment.get("CARGO_PKG_VERSION") else {
            return false;
        };
        let key = format!("{name}-{version}");
        let Some(policy) = self.authority.policy() else {
            return false;
        };
        let Some(expected) = policy.packages.get(&key) else {
            return false;
        };
        let Some(directory) = self.directories.get(&key) else {
            return false;
        };
        if name != &expected.name
            || version != &expected.version
            || environment.get("CARGO_MANIFEST_DIR").map(Path::new) != Some(directory.as_path())
        {
            return false;
        }
        let Some(features) = actual_features(args) else {
            return false;
        };
        features == expected.features && check_package(directory, expected)
    }
}

/// Cargo emits one exact spelling. Rust also accepts whitespace and raw
/// identifiers; silently ignoring those would admit an unapproved feature.
fn actual_features(args: &[OsString]) -> Option<BTreeSet<String>> {
    let text = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()?;
    let mut features = BTreeSet::new();
    for (index, argument) in text.iter().enumerate() {
        let cfg = argument.strip_prefix("--cfg=").or_else(|| {
            (*argument == "--cfg")
                .then(|| text.get(index + 1).copied())
                .flatten()
        });
        if *argument == "--cfg" && cfg.is_none() {
            return None;
        }
        let Some(cfg) = cfg else {
            continue;
        };
        if let Some(name) = cfg
            .strip_prefix("feature=\"")
            .and_then(|value| value.strip_suffix('"'))
        {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                return None;
            }
            features.insert(name.to_owned());
        } else if cfg.contains("feature") {
            return None;
        }
    }
    Some(features)
}

fn normal_dependencies(node: &Value) -> Option<Vec<&str>> {
    let mut result = Vec::new();
    for dependency in node["deps"].as_array()? {
        let kinds = dependency["dep_kinds"].as_array()?;
        if kinds.is_empty()
            || kinds.iter().any(|kind| {
                !kind["kind"].is_null() && !matches!(kind["kind"].as_str(), Some("dev" | "build"))
            })
        {
            return None;
        }
        if kinds
            .iter()
            .any(|kind| kind["kind"].as_str() != Some("dev"))
        {
            result.push(dependency["pkg"].as_str()?);
        }
    }
    Some(result)
}

fn check_package(root: &Path, expected: &Package) -> bool {
    fn visit(
        root: &Path,
        directory: &Path,
        files: &mut BTreeMap<String, String>,
        total: &mut u64,
        expected: &Package,
    ) -> Option<()> {
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                let relative = entry.path().strip_prefix(root).ok()?.to_str()?.to_owned() + "/";
                if !expected
                    .files
                    .keys()
                    .any(|name| name.starts_with(&relative))
                {
                    return None;
                }
                visit(root, &entry.path(), files, total, expected)?;
            } else {
                let mut file = open(&entry.path())?;
                let mut hash = Sha256::new();
                let mut buffer = vec![0; 65536];
                loop {
                    let n = file.read(&mut buffer).ok()?;
                    if n == 0 {
                        break;
                    }
                    *total = total.checked_add(n as u64)?;
                    if *total > 64 * 1024 * 1024 {
                        return None;
                    }
                    hash.update(&buffer[..n]);
                }
                if files.len() >= 16_384 {
                    return None;
                }
                files.insert(
                    entry.path().strip_prefix(root).ok()?.to_str()?.to_owned(),
                    format!("{:x}", hash.finalize()),
                );
            }
        }
        Some(())
    }
    let mut actual = BTreeMap::new();
    if visit(root, root, &mut actual, &mut 0, expected).is_none() {
        return false;
    }
    match &expected.origin {
        Origin::Git { .. } => actual == expected.files,
        Origin::Registry { archive_sha256 } => {
            actual.remove(".cargo-ok");
            actual.remove(".cargo-checksum.json");
            if actual != expected.files
                || std::fs::read(root.join(".cargo-ok")).ok().as_deref() != Some(b"{\"v\":1}")
            {
                return false;
            }
            match std::fs::read(root.join(".cargo-checksum.json")) {
                Ok(bytes) => serde_json::from_slice::<Value>(&bytes).is_ok_and(|checksum| {
                    checksum["package"].as_str() == Some(archive_sha256)
                        && serde_json::from_value::<BTreeMap<String, String>>(
                            checksum["files"].clone(),
                        )
                        .is_ok_and(|files| files == expected.files)
                }),
                Err(error) => error.kind() == std::io::ErrorKind::NotFound,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_prefilter_uses_the_existing_exact_public_feature_profile() {
        let metadata = |source: Value, features: Value| {
            serde_json::json!({
                "packages": [{"id": "vstd", "name": "vstd", "version": "0.0.0-2026-10-04-0306", "source": source}],
                "resolve": {"nodes": [{"id": "vstd", "features": features}]}
            })
        };
        for (source, features, expected) in [
            (
                Value::from("registry+https://github.com/rust-lang/crates.io-index"),
                serde_json::json!(["alloc", "std"]),
                true,
            ),
            (
                Value::from("registry+https://github.com/rust-lang/crates.io-index"),
                serde_json::json!(["alloc", "default", "std"]),
                false,
            ),
            (Value::Null, serde_json::json!(["alloc", "std"]), false),
            (
                Value::from("registry+https://foreign.invalid/index"),
                serde_json::json!(["alloc", "std"]),
                false,
            ),
            (
                Value::from("registry+https://github.com/rust-lang/crates.io-index"),
                serde_json::json!(["alloc", "std", "foreign"]),
                false,
            ),
        ] {
            let data = metadata(source, features);
            assert_eq!(Closure::automatic_profile_candidate(&data), expected);
            // Matching metadata is only a routing hint. It has no workspace,
            // source paths or actual provider bodies and cannot admit a plan.
            assert!(
                Closure::admit(
                    &data,
                    Path::new("/missing"),
                    Path::new("/missing"),
                    Path::new("/missing")
                )
                .is_none()
            );
        }
        let mut data = metadata(
            Value::from("registry+https://github.com/rust-lang/crates.io-index"),
            serde_json::json!(["alloc", "std"]),
        );
        data["packages"][0]["version"] = Value::from("0.0.0-2026-09-20-0500");
        assert!(!Closure::automatic_profile_candidate(&data));
        assert!(!Closure::automatic_profile_candidate(&Value::Null));
    }

    #[test]
    fn public_profiles_are_separate_complete_archive_authorities()
    -> Result<(), Box<dyn std::error::Error>> {
        for authority in [Authority::PublicRegistry, Authority::PublicRegistryVest] {
            let policy = authority.policy().ok_or("public policy")?;
            assert_eq!(policy.packages.len(), 15);
            assert_eq!(
                policy
                    .packages
                    .values()
                    .map(|p| p.files.len())
                    .sum::<usize>(),
                662
            );
            assert!(policy.packages.values().all(|p| {
                matches!(p.origin, Origin::Registry { .. })
                    && p.files.keys().eq(p.compiler_files.keys())
            }));
            assert_ne!(
                super::super::super::files::digest(authority.bytes().as_bytes()),
                super::super::super::files::digest(POLICY.as_bytes())
            );
        }
        Ok(())
    }

    #[test]
    fn actual_public_metadata_source_admission() -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = std::env::var_os("SAIL_RETAINED_PUBLIC_METADATA") else {
            return Ok(());
        };
        // External local correctness control supplies actual Cargo metadata
        // and cached SHA-authorized public packages. No compiler is doubled
        // into a successful certificate or proof by this source-only check.
        let metadata: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let manifest =
            PathBuf::from(std::env::var_os("SAIL_RETAINED_PUBLIC_MANIFEST").ok_or("manifest")?);
        let registry = metadata["packages"]
            .as_array()
            .ok_or("packages")?
            .iter()
            .find(|p| p["name"] == "proc-macro2")
            .ok_or("proc macro")?["manifest_path"]
            .as_str()
            .map(PathBuf::from)
            .ok_or("provider manifest")?;
        let registry = registry.parent().and_then(Path::parent).ok_or("registry")?;
        let admit = |data: &Value| {
            Closure::admit(
                data,
                &manifest,
                Path::new("/not-an-official-source-tree"),
                registry,
            )
        };
        let closure = admit(&metadata).ok_or("actual public admission")?;
        assert!(closure.check_sources());
        let policy = closure.authority.policy().ok_or("selected policy")?;
        for (key, provider) in &closure.directories {
            let package = &policy.packages[key];
            let variables = BTreeMap::from([
                ("CARGO_PKG_NAME".into(), package.name.clone()),
                ("CARGO_PKG_VERSION".into(), package.version.clone()),
                (
                    "CARGO_MANIFEST_DIR".into(),
                    provider.to_str().ok_or("UTF8 provider")?.into(),
                ),
            ]);
            let args = package
                .features
                .iter()
                .flat_map(|feature| {
                    [
                        OsString::from("--cfg"),
                        format!("feature=\"{feature}\"").into(),
                    ]
                })
                .collect::<Vec<_>>();
            assert!(closure.unit(&variables, &args));
        }
        let mut changed = metadata.clone();
        let node = changed["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("nodes")?
            .iter_mut()
            .find(|n| {
                metadata["packages"]
                    .as_array()
                    .is_some_and(|ps| ps.iter().any(|p| p["name"] == "vstd" && p["id"] == n["id"]))
            })
            .ok_or("vstd")?;
        node["features"]
            .as_array_mut()
            .ok_or("features")?
            .push(Value::String("foreign".into()));
        assert!(admit(&changed).is_none());
        let mut changed = metadata.clone();
        let provider = changed["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .iter_mut()
            .find(|p| p["name"] == "vstd")
            .ok_or("vstd")?;
        provider["source"] = Value::String("git+https://foreign.invalid/custom".into());
        assert!(admit(&changed).is_none());
        let mut changed = metadata.clone();
        let provider = changed["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .iter_mut()
            .find(|p| p["name"] == "verus_builtin_macros")
            .ok_or("macros")?;
        provider["manifest_path"] = Value::String("/foreign/provider/Cargo.toml".into());
        assert!(admit(&changed).is_none());
        assert!(!closure.native_sources(std::iter::once((
            Path::new("/unknown/read.rs"),
            "blake3=foreign",
            1
        ))));
        Ok(())
    }

    #[test]
    fn exact_pinned_vest_policy_has_only_the_declared_official_closure()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy: Policy = serde_json::from_str(POLICY)?;
        assert_eq!(policy.packages.len(), 15);
        assert_eq!(
            policy.verus_source_commit,
            crate::toolchain::Pins::embedded()?.verus.commit
        );
        assert_eq!(
            policy.packages[VSTD].features,
            BTreeSet::from(["alloc".into(), "std".into()])
        );
        assert!(policy.packages.values().all(|package| {
            !package.files.is_empty()
                && package.files.keys().eq(package.compiler_files.keys())
                && package
                    .compiler_files
                    .values()
                    .all(|source| source.checksum.starts_with("blake3="))
        }));
        let git = policy
            .packages
            .values()
            .filter(|package| matches!(package.origin, Origin::Git { .. }))
            .count();
        assert_eq!(git, 6);
        Ok(())
    }

    #[test]
    fn noncanonical_rust_feature_cfgs_are_refused_instead_of_ignored() {
        assert_eq!(
            actual_features(&["--cfg".into(), "feature=\"std\"".into()]),
            Some(BTreeSet::from(["std".into()]))
        );
        assert_eq!(
            actual_features(&["--cfg=feature=\"std\"".into()]),
            Some(BTreeSet::from(["std".into()]))
        );
        for cfg in [
            "feature = \"foreign\"",
            "r#feature=\"foreign\"",
            "feature",
            "feature=\"foreign\" ",
            "feature=\"st\\x64\"",
            "/*comment*/feature=\"foreign\"",
        ] {
            assert!(actual_features(&["--cfg".into(), cfg.into()]).is_none());
        }
        assert!(actual_features(&["--cfg".into()]).is_none());
    }

    #[test]
    fn missing_or_unknown_dependency_kinds_never_disappear_from_the_closure() {
        let ordinary =
            serde_json::json!({"deps":[{"pkg":"vstd","dep_kinds":[{"kind":null,"target":null}]}]});
        assert_eq!(normal_dependencies(&ordinary), Some(vec!["vstd"]));
        for bad in [
            serde_json::json!({"deps":[{"pkg":"custom"}]}),
            serde_json::json!({"deps":[{"pkg":"custom","dep_kinds":[]}]}),
            serde_json::json!({"deps":[{"pkg":"custom","dep_kinds":[{"kind":"foreign"}]}]}),
        ] {
            assert!(normal_dependencies(&bad).is_none());
        }
    }

    #[test]
    fn source_equality_is_literal_and_unexpected_files_or_aliases_refuse()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "sail-retained-provider-policy-{}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::write(directory.join("lib.rs"), b"official A")?;
            let expected = Package {
                name: "official".into(),
                version: "1".into(),
                features: BTreeSet::new(),
                origin: Origin::Git {
                    commit: "pin".into(),
                    path: "source/official".into(),
                },
                compiler_files: BTreeMap::new(),
                files: BTreeMap::from([(
                    "lib.rs".into(),
                    super::super::super::files::digest(b"official A"),
                )]),
            };
            assert!(check_package(&directory, &expected));
            std::fs::write(directory.join("lib.rs"), b"custom B")?;
            assert!(!check_package(&directory, &expected));
            std::fs::write(directory.join("lib.rs"), b"official A")?;
            std::fs::write(directory.join("extra.rs"), b"new provider input")?;
            assert!(!check_package(&directory, &expected));
            std::fs::remove_file(directory.join("extra.rs"))?;
            std::fs::create_dir(directory.join("unknown-empty-directory"))?;
            assert!(!check_package(&directory, &expected));
            std::fs::remove_dir(directory.join("unknown-empty-directory"))?;
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink("lib.rs", directory.join("alias.rs"))?;
                assert!(!check_package(&directory, &expected));
            }
            Ok(())
        })();
        std::fs::remove_dir_all(directory)?;
        outcome
    }
    #[test]
    fn native_wire_is_distinct_and_binds_the_actual_child_and_loaded_authority()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-native-wire-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
            let policy: Policy = serde_json::from_str(POLICY)?;
            let (package, authority) = policy.packages.iter().next().ok_or("package")?;
            let (relative, source) = authority.compiler_files.iter().next().ok_or("source")?;
            let source_root = root.join("provider");
            let closure = Closure {
                authority: Authority::OfficialVest,
                directories: BTreeMap::from([(package.clone(), source_root.clone())]),
            };
            let mut child = std::process::Command::new("/usr/bin/true").spawn()?;
            let mut observation =
                super::super::native::Observation::begin(&root.join("request"), "run", "unit")
                    .ok_or("request")?;
            let pid = child.id();
            observation.started(pid);
            let success = child.wait()?.success();
            let document = serde_json::json!({
                "kind":"sail-native-compiler-read-inputs", "schema":1,
                "binding":super::super::super::files::digest(b"run\0unit"),
                "pid":pid, "complete":true,
                "inputs":{"sources":{source_root.join(relative).to_string_lossy().as_ref():{
                    "checksum":source.checksum,"size":source.size}},
                    "extra_tracked_files":[],"tracked_environment":{}}
            });
            let path = root.join("request/inputs.json");
            let admit = |value: &Value, exited| -> Result<bool, Box<dyn std::error::Error>> {
                std::fs::write(&path, serde_json::to_vec(value)?)?;
                Ok(observation
                    .finish(exited, &root, &closure, &BTreeMap::new(), &BTreeMap::new())
                    .is_some())
            };
            assert!(admit(&document, success)?);
            let mut included = document.clone();
            let original = source_root.join(relative);
            let name = original.file_name().ok_or("filename")?;
            let lexical = original.parent().ok_or("parent")?.join("dot/..").join(name);
            let sources = included["inputs"]["sources"]
                .as_object_mut()
                .ok_or("sources")?;
            let value = sources
                .remove(original.to_str().ok_or("path")?)
                .ok_or("source")?;
            sources.insert(lexical.to_string_lossy().into_owned(), value);
            included["inputs"]["extra_tracked_files"] = serde_json::json!([lexical]);
            assert!(admit(&included, true)?);
            assert!(!admit(&document, false)?);
            for (field, value) in [
                ("kind", serde_json::json!("sail-compiler-read-inputs")),
                ("pid", serde_json::json!(pid + 1)),
                ("complete", serde_json::json!(false)),
                ("binding", serde_json::json!("foreign")),
            ] {
                let mut changed = document.clone();
                changed[field] = value;
                assert!(!admit(&changed, true)?);
            }
            let mut changed = document.clone();
            let sources = changed["inputs"]["sources"]
                .as_object_mut()
                .ok_or("sources")?;
            let loaded = sources.values_mut().next().ok_or("loaded")?;
            loaded["checksum"] = serde_json::json!("blake3=changed");
            assert!(!admit(&changed, true)?);
            let mut changed = document.clone();
            changed["inputs"]["extra_tracked_files"] =
                serde_json::json!([root.join("mutable-extra")]);
            assert!(!admit(&changed, true)?);
            let mut changed = document.clone();
            changed["unexpected"] = serde_json::json!(true);
            assert!(!admit(&changed, true)?);
            Ok(())
        })();
        std::fs::remove_dir_all(root)?;
        outcome
    }
}
