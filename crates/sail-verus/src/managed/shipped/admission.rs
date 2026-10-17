//! Closed source/feature admission for the unselected vstd boundary experiment.
//!
//! Source admission is necessary, not sufficient: it does not attest arbitrary
//! compiler flags, execution-time source mutations or native artifact provenance.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../engine/verus/vstd-boundary-inputs.json"
));

#[derive(Deserialize)]
struct Policy {
    kind: String,
    schema: u32,
    vstd: String,
    packages: BTreeMap<String, Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    archive_sha256: String,
    features: Vec<String>,
    files: BTreeMap<String, String>,
    #[serde(default)]
    compiler_checksums: BTreeMap<String, CompilerChecksum>,
    #[serde(default)]
    compiler_files: BTreeMap<String, CompilerFile>,
}

#[derive(Deserialize)]
struct CompilerFile {
    checksum: String,
    size: u64,
}

#[derive(Deserialize, PartialEq, Eq)]
struct CompilerChecksum {
    blake3: String,
    size: u64,
}

#[derive(Deserialize)]
struct Checksum {
    package: Option<String>,
    files: BTreeMap<String, String>,
}

/// An exact resolved source/feature closure, bound to the embedded policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Closure {
    policy_sha256: String,
    registry: PathBuf,
    packages: BTreeSet<String>,
}

impl Closure {
    /// Reject path patches, different versions/features and extra dependencies.
    pub(super) fn resolve(metadata: &Value, registry: &Path) -> Option<Self> {
        let policy: Policy = serde_json::from_str(POLICY).ok()?;
        if policy.kind != "sail-vstd-boundary-source-policy" || policy.schema != 2 {
            return None;
        }
        let admitted = resolved_packages(metadata, registry, &policy)?;
        let closure = Self {
            policy_sha256: super::digest(POLICY.as_bytes()),
            registry: registry.to_owned(),
            packages: admitted,
        };
        closure.check_sources().then_some(closure)
    }

    /// Validate every source file, not the mutable Cargo checksum sidecar alone.
    pub(super) fn check_sources(&self) -> bool {
        if self.policy_sha256 != super::digest(POLICY.as_bytes()) {
            return false;
        }
        let Ok(policy) = serde_json::from_str::<Policy>(POLICY) else {
            return false;
        };
        if self.packages != policy.packages.keys().cloned().collect() {
            return false;
        }
        self.packages.iter().all(|name| {
            let Some(expected) = policy.packages.get(name) else {
                return false;
            };
            check_package(&self.registry.join(name), expected)
        })
    }

    pub(super) fn native_unit(
        &self,
        vars: &BTreeMap<String, String>,
        args: &[std::ffi::OsString],
    ) -> bool {
        self.unit(vars, args, false)
    }

    /// Bind the actual verified vstd feature arguments, not only Cargo metadata.
    pub(super) fn vstd_unit(
        &self,
        vars: &BTreeMap<String, String>,
        args: &[std::ffi::OsString],
    ) -> bool {
        self.unit(vars, args, true)
    }

    fn unit(
        &self,
        vars: &BTreeMap<String, String>,
        args: &[std::ffi::OsString],
        vstd: bool,
    ) -> bool {
        let Ok(policy) = serde_json::from_str::<Policy>(POLICY) else {
            return false;
        };
        let Some(name) = vars.get("CARGO_PKG_NAME") else {
            return false;
        };
        let Some(version) = vars.get("CARGO_PKG_VERSION") else {
            return false;
        };
        let identity = format!("{name}-{version}");
        let Some(package) = policy.packages.get(&identity) else {
            return false;
        };
        if (name == "vstd") != vstd
            || !self.packages.contains(&identity)
            || vars.get("CARGO_MANIFEST_DIR").map(Path::new)
                != Some(self.registry.join(&identity).as_path())
            || !check_package(&self.registry.join(&identity), package)
        {
            return false;
        }
        features(args).is_some_and(|actual| actual == package.features.iter().cloned().collect())
    }

    /// Admit actual native compiler source reads only from the complete exact
    /// official closure. Non-Rust files such as included README text still
    /// need their compiler-retained checksum; endpoint reads cannot attest it.
    pub(super) fn native_sources<'a>(
        &self,
        sources: impl Iterator<Item = (&'a Path, &'a str, u64)>,
    ) -> bool {
        let Ok(policy) = serde_json::from_str::<Policy>(POLICY) else {
            return false;
        };
        let mut count = 0;
        for (path, checksum, size) in sources {
            // Rust retains src/../README.md for include_str!. Resolve only
            // the policy lookup; preserve literal documents and commands.
            let Some(path) = local_path(path) else {
                return false;
            };
            let Some((package, relative)) = self.packages.iter().find_map(|name| {
                path.strip_prefix(self.registry.join(name))
                    .ok()
                    .map(|relative| (name, relative))
            }) else {
                return false;
            };
            let Some(relative) = relative.to_str() else {
                return false;
            };
            let Some(expected) = policy
                .packages
                .get(package)
                .and_then(|package| package.compiler_files.get(relative))
            else {
                return false;
            };
            if expected.checksum != checksum || expected.size != size {
                return false;
            }
            count += 1;
        }
        count > 0
    }

    pub(super) fn policy_sha256(&self) -> &str {
        &self.policy_sha256
    }

    /// Check the compiler's hashes of the bytes it actually loaded, rather
    /// than another filesystem read after compilation. This deliberately
    /// admits only the closed vstd Rust source set, never arbitrary tracked
    /// proc-macro files (rustc hashes those separately at the end).
    pub(super) fn compiler_sources(&self, dep_info: &str) -> bool {
        let Ok(policy) = serde_json::from_str::<Policy>(POLICY) else {
            return false;
        };
        let Some(package) = policy.packages.get(&policy.vstd) else {
            return false;
        };
        compiler_sources(
            dep_info,
            &self.registry.join(&policy.vstd),
            &package.compiler_checksums,
        )
    }

    /// Validate hashes retained directly by each actual compiler phase.
    pub(super) fn loaded_sources<'a>(
        &self,
        sources: impl Iterator<Item = (&'a str, &'a str, u64)>,
    ) -> bool {
        let Ok(policy) = serde_json::from_str::<Policy>(POLICY) else {
            return false;
        };
        let Some(package) = policy.packages.get(&policy.vstd) else {
            return false;
        };
        let root = self.registry.join(&policy.vstd);
        let mut count = 0;
        for (path, checksum, size) in sources {
            let Some(relative) = Path::new(path)
                .strip_prefix(&root)
                .ok()
                .and_then(Path::to_str)
            else {
                return false;
            };
            let Some(expected) = package.compiler_checksums.get(relative) else {
                return false;
            };
            if Path::new(relative)
                .extension()
                .is_none_or(|extension| extension != "rs")
                || checksum.strip_prefix("blake3=") != Some(expected.blake3.as_str())
                || size != expected.size
            {
                return false;
            }
            count += 1;
        }
        count > 0
    }
}

fn features(args: &[std::ffi::OsString]) -> Option<BTreeSet<String>> {
    let text = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()?;
    let mut features = BTreeSet::new();
    for (index, arg) in text.iter().enumerate() {
        let cfg = if *arg == "--cfg" {
            Some(*text.get(index + 1)?)
        } else {
            arg.strip_prefix("--cfg=")
        };
        if let Some(cfg) = cfg.filter(|cfg| cfg.contains("feature")) {
            // Cargo emits this exact spelling. Rust also accepts spaces and
            // raw identifiers; silently ignoring those would omit an enabled
            // feature from admission. Unsupported spellings simply miss.
            let feature = cfg.strip_prefix("feature=\"")?.strip_suffix('"')?;
            if feature.is_empty()
                || feature
                    .chars()
                    .any(|c| c.is_whitespace() || matches!(c, '"' | '\\'))
            {
                return None;
            }
            features.insert(feature.to_owned());
        }
    }
    Some(features)
}

fn local_path(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::Normal(name) => resolved.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

fn compiler_sources(
    dep_info: &str,
    root: &Path,
    expected: &BTreeMap<String, CompilerChecksum>,
) -> bool {
    let mut sources = BTreeSet::new();
    for source in super::dep_info_sources(dep_info) {
        let Ok(relative) = Path::new(&source).strip_prefix(root) else {
            return false;
        };
        if relative
            .extension()
            .is_none_or(|extension| extension != "rs")
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return false;
        }
        let Some(relative) = relative.to_str() else {
            return false;
        };
        if !expected.contains_key(relative) || !sources.insert(relative.to_owned()) {
            return false;
        }
    }
    if sources.is_empty() {
        return false;
    }
    let mut observed = BTreeSet::new();
    for line in dep_info
        .lines()
        .filter(|line| line.starts_with("# checksum"))
    {
        let Some((hash, rest)) = line
            .strip_prefix("# checksum:blake3=")
            .and_then(|rest| rest.split_once(" file_len:"))
        else {
            return false;
        };
        let Some((size, path)) = rest.split_once(' ') else {
            return false;
        };
        let Ok(size) = size.parse::<u64>() else {
            return false;
        };
        let path = super::unescape(path);
        let Some(relative) = Path::new(&path)
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
        else {
            return false;
        };
        let Some(expected) = expected.get(relative) else {
            return false;
        };
        if !sources.contains(relative)
            || !observed.insert(relative.to_owned())
            || hash != expected.blake3
            || size != expected.size
        {
            return false;
        }
    }
    observed == sources
}

fn resolved_packages(
    metadata: &Value,
    registry: &Path,
    policy: &Policy,
) -> Option<BTreeSet<String>> {
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
    let root = packages.iter().find_map(|(id, package)| {
        (format!(
            "{}-{}",
            package["name"].as_str()?,
            package["version"].as_str()?
        ) == policy.vstd)
            .then_some(*id)
    })?;
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut admitted = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let package = packages.get(id)?;
        let node = nodes.get(id)?;
        let name = format!(
            "{}-{}",
            package["name"].as_str()?,
            package["version"].as_str()?
        );
        let expected = policy.packages.get(&name)?;
        if package["name"].as_str()? != expected.name
            || package["version"].as_str()? != expected.version
            || package["source"].as_str()?
                != "registry+https://github.com/rust-lang/crates.io-index"
            || Path::new(package["manifest_path"].as_str()?)
                != registry.join(&name).join("Cargo.toml")
        {
            return None;
        }
        let features: BTreeSet<_> = node["features"]
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect::<Option<_>>()?;
        if features != expected.features.iter().map(String::as_str).collect() {
            return None;
        }
        admitted.insert(name);
        for dependency in node["dependencies"].as_array()? {
            pending.push(dependency.as_str()?);
        }
    }
    if admitted != policy.packages.keys().cloned().collect() {
        return None;
    }
    Some(admitted)
}

fn check_package(root: &Path, expected: &Package) -> bool {
    let Some(mut actual) = super::package_tree(root) else {
        return false;
    };
    // Cargo creates these bookkeeping files after authenticated extraction.
    // Their contents are checked independently; they are not compiler sources.
    actual.remove(".cargo-ok");
    actual.remove(".cargo-checksum.json");
    if actual != expected.files {
        return false;
    }
    // New Cargo extraction omits this legacy sidecar. The independently pinned
    // full source map above is authoritative in both layouts.
    match std::fs::read(root.join(".cargo-checksum.json")) {
        Ok(bytes) => {
            let Ok(checksum) = serde_json::from_slice::<Checksum>(&bytes) else {
                return false;
            };
            if checksum.package.as_deref() != Some(expected.archive_sha256.as_str())
                || checksum.files != expected.files
            {
                return false;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return false,
    }
    std::fs::read(root.join(".cargo-ok")).is_ok_and(|bytes| bytes == b"{\"v\":1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_compiler_source_maps_require_exact_admitted_read_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy: Policy = serde_json::from_str(POLICY)?;
        let closure = Closure {
            policy_sha256: super::super::digest(POLICY.as_bytes()),
            registry: "/registry".into(),
            packages: policy.packages.keys().cloned().collect(),
        };
        let path = closure.registry.join(&policy.vstd).join("vstd.rs");
        let path = path.to_str().ok_or("path")?;
        let checksum = "blake3=835f517957115052871472aac54967dad2706e21d67ffa5c5d7014e489c5507d";
        assert!(closure.loaded_sources([(path, checksum, 5040)].into_iter()));
        assert!(!closure.loaded_sources([(path, checksum, 5041)].into_iter()));
        assert!(!closure.loaded_sources([(path, "blake3=changed", 5040)].into_iter()));
        assert!(!closure.loaded_sources([("/foreign/vstd.rs", checksum, 5040)].into_iter()));
        assert!(!closure.loaded_sources(std::iter::empty()));
        Ok(())
    }

    #[test]
    fn compiler_loaded_checksums_refuse_restored_and_unattested_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy: Policy = serde_json::from_str(POLICY)?;
        let package = policy.packages.get(&policy.vstd).ok_or("missing vstd")?;
        let root = Path::new("/registry").join(&policy.vstd);
        let path = root.join("vstd.rs").display().to_string();
        let checksum = package
            .compiler_checksums
            .get("vstd.rs")
            .ok_or("missing compiler checksum")?;
        let sources = format!("{path}:\n");
        let comment = format!(
            "# checksum:blake3={} file_len:{} {path}\n",
            checksum.blake3, checksum.size
        );
        let valid = format!("{sources}{comment}");
        assert!(compiler_sources(&valid, &root, &package.compiler_checksums));
        // Restoring the disk file after a different compiler read cannot
        // restore the compiler's retained checksum. No disk reread is used.
        let changed = valid.replace(&checksum.blake3, &"0".repeat(64));
        assert!(!compiler_sources(
            &changed,
            &root,
            &package.compiler_checksums
        ));
        assert!(!compiler_sources(
            &sources,
            &root,
            &package.compiler_checksums
        ));
        assert!(!compiler_sources(
            &comment,
            &root,
            &package.compiler_checksums
        ));
        for rejected in [
            valid.replace("blake3=", "sha256="),
            valid.replace("file_len:5040", "file_len:5041"),
            valid.replace("vstd.rs", "../vstd.rs"),
            valid.replace("vstd.rs", "foreign.rs"),
            format!("{valid}{comment}"),
            format!("{valid}{sources}"),
            format!("{valid}# checksum:malformed\n"),
            format!("{valid}/generated/macro-input.rs:\n"),
        ] {
            assert!(
                !compiler_sources(&rejected, &root, &package.compiler_checksums),
                "accepted unattested compiler input: {rejected}"
            );
        }
        Ok(())
    }

    #[test]
    fn source_policy_refuses_changed_added_and_redirected_files() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-boundary-admission-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let body = b"pub fn macro_input() {}";
        let expected = Package {
            name: "macro".into(),
            version: "1".into(),
            archive_sha256: "trusted-archive".into(),
            features: Vec::new(),
            files: BTreeMap::from([("lib.rs".into(), super::super::digest(body))]),
            compiler_checksums: BTreeMap::new(),
            compiler_files: BTreeMap::new(),
        };
        std::fs::write(root.join("lib.rs"), body)?;
        std::fs::write(root.join(".cargo-ok"), b"{\"v\":1}")?;
        std::fs::write(
            root.join(".cargo-checksum.json"),
            serde_json::to_vec(
                &serde_json::json!({"package":"trusted-archive","files":expected.files}),
            )?,
        )?;
        assert!(check_package(&root, &expected));
        std::fs::remove_file(root.join(".cargo-checksum.json"))?;
        assert!(
            check_package(&root, &expected),
            "fresh Cargo layout retains the same pinned source check"
        );
        std::fs::write(root.join("lib.rs"), b"foreign macro")?;
        assert!(!check_package(&root, &expected));
        std::fs::write(root.join("lib.rs"), body)?;
        std::fs::write(root.join("provider.rs"), b"foreign provider")?;
        assert!(!check_package(&root, &expected));
        std::fs::remove_file(root.join("provider.rs"))?;
        #[cfg(unix)]
        {
            std::fs::remove_file(root.join("lib.rs"))?;
            std::os::unix::fs::symlink(root.join(".cargo-ok"), root.join("lib.rs"))?;
            assert!(!check_package(&root, &expected));
        }
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn resolved_policy_refuses_hooks_path_patches_and_foreign_dependencies()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy: Policy = serde_json::from_str(POLICY)?;
        let registry = Path::new("/registry");
        let packages: Vec<_> = policy
            .packages
            .iter()
            .map(|(id, package)| {
                serde_json::json!({"id":id,"name":package.name,"version":package.version,
                "source":"registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path":registry.join(id).join("Cargo.toml")})
            })
            .collect();
        let nodes: Vec<_> = policy
            .packages
            .iter()
            .map(|(id, package)| {
                let dependencies: Vec<_> = if *id == policy.vstd {
                    policy
                        .packages
                        .keys()
                        .filter(|name| *name != id)
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                serde_json::json!({"id":id,"features":package.features,"dependencies":dependencies})
            })
            .collect();
        let metadata = serde_json::json!({"packages":packages,"resolve":{"nodes":nodes}});
        assert!(resolved_packages(&metadata, registry, &policy).is_some());
        let mut hooks = metadata.clone();
        let hook_node = hooks["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("missing nodes")?
            .iter_mut()
            .find(|node| node["id"] == "verus_builtin_macros-0.0.0-2026-09-20-0158")
            .ok_or("missing macros")?;
        hook_node["features"] = serde_json::json!(["contrib-hooks"]);
        assert!(resolved_packages(&hooks, registry, &policy).is_none());
        let mut patch = metadata.clone();
        patch["packages"][0]["source"] = Value::Null;
        assert!(resolved_packages(&patch, registry, &policy).is_none());
        let mut foreign = metadata.clone();
        let root = foreign["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("missing nodes")?
            .iter_mut()
            .find(|node| node["id"] == policy.vstd)
            .ok_or("missing root")?;
        root["dependencies"]
            .as_array_mut()
            .ok_or("missing dependencies")?
            .push(Value::String("foreign-macro".into()));
        assert!(resolved_packages(&foreign, registry, &policy).is_none());
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn native_test_fixture(registry: &Path) -> Option<(Closure, PathBuf, String, u64)> {
    let policy: Policy = serde_json::from_str(POLICY).ok()?;
    let package = policy.packages.get("convert_case-0.4.0")?;
    let source = package.compiler_files.get("src/lib.rs")?;
    Some((
        Closure {
            policy_sha256: super::digest(POLICY.as_bytes()),
            registry: registry.to_owned(),
            packages: policy.packages.keys().cloned().collect(),
        },
        registry.join("convert_case-0.4.0/src/lib.rs"),
        source.checksum.clone(),
        source.size,
    ))
}

#[cfg(test)]
mod native_source_tests {
    use super::*;

    #[test]
    fn feature_admission_refuses_alternate_rust_cfg_spellings() {
        let args = [
            "rustc",
            "--cfg",
            "feature=\"std\"",
            "--cfg=feature=\"alloc\"",
        ]
        .map(std::ffi::OsString::from);
        assert_eq!(
            features(&args),
            Some(BTreeSet::from(["std".to_owned(), "alloc".to_owned()]))
        );
        for flag in [
            "feature = \"foreign\"",
            "r#feature=\"foreign\"",
            "feature=r#\"foreign\"#",
        ] {
            assert!(features(&["rustc".into(), "--cfg".into(), flag.into()]).is_none());
        }
    }

    #[test]
    fn native_consumed_source_requires_the_official_body_and_complete_size()
    -> Result<(), Box<dyn std::error::Error>> {
        let (closure, path, checksum, size) =
            native_test_fixture(Path::new("/registry")).ok_or("native policy fixture")?;
        assert!(closure.native_sources(std::iter::once((path.as_path(), checksum.as_str(), size))));
        assert!(!closure.native_sources(std::iter::once((path.as_path(), "blake3=foreign", size))));
        assert!(!closure.native_sources(std::iter::once((
            path.as_path(),
            checksum.as_str(),
            size + 1
        ))));
        assert!(!closure.native_sources(std::iter::once((
            Path::new("/foreign/lib.rs"),
            checksum.as_str(),
            size
        ))));
        assert!(!closure.native_sources(std::iter::empty()));
        // Included text is a real SourceMap input too, not an end-of-run hash.
        let policy: Policy = serde_json::from_str(POLICY)?;
        let package = policy.packages.get("hashbrown-0.17.1").ok_or("hashbrown")?;
        let readme = package.compiler_files.get("README.md").ok_or("README")?;
        assert!(closure.native_sources(std::iter::once((
            Path::new("/registry/hashbrown-0.17.1/src/../README.md"),
            readme.checksum.as_str(),
            readme.size
        ))));
        Ok(())
    }
}
