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
        features_admitted(args, package)
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

/// A unit's features are its own `--cfg feature=` arguments. Cargo metadata
/// reports features unified over the whole workspace, which a `-p` build of
/// one member may not enable, so the closure check alone cannot bind them.
fn features_admitted(args: &[std::ffi::OsString], package: &Package) -> bool {
    features(args).is_some_and(|actual| actual == package.features.iter().cloned().collect())
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

/// How a workspace's resolved vstd closure differs from the shipped proof's.
/// This is advice for the miss note only; admission stays [`Closure::resolve`].
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Difference {
    /// One phrase per difference, in package-name order.
    pub(super) packages: Vec<String>,
    /// `cargo update` commands that select the shipped proof's versions.
    pub(super) commands: Vec<String>,
    /// vstd is resolved without its `default` feature and otherwise matches.
    pub(super) vstd_default_off: bool,
    /// The commands and the vstd feature edit align every difference.
    pub(super) alignable: bool,
}

/// Compares the closure of the workspace's crates.io vstd with the embedded
/// policy by package version, features and source. `None` without exactly
/// one vstd, or when nothing differs.
pub(super) fn closure_difference(metadata: &Value) -> Option<Difference> {
    let policy: Policy = serde_json::from_str(POLICY).ok()?;
    let wanted: BTreeMap<&str, &Package> = policy
        .packages
        .values()
        .map(|package| (package.name.as_str(), package))
        .collect();
    let packages: BTreeMap<&str, &Value> = metadata["packages"]
        .as_array()?
        .iter()
        .map(|package| Some((package["id"].as_str()?, package)))
        .collect::<Option<_>>()?;
    let nodes: BTreeMap<&str, &Value> = metadata["resolve"]["nodes"]
        .as_array()?
        .iter()
        .map(|node| Some((node["id"].as_str()?, node)))
        .collect::<Option<_>>()?;
    let version = |package: &Value| package["version"].as_str().unwrap_or_default().to_owned();
    let roots: Vec<&str> = packages
        .iter()
        .filter(|(_, package)| package["name"] == "vstd")
        .map(|(id, _)| *id)
        .collect();
    let [root] = roots[..] else {
        return None;
    };
    let shipped_vstd = wanted.get("vstd")?;
    let mut difference = Difference {
        packages: Vec::new(),
        commands: Vec::new(),
        vstd_default_off: false,
        alignable: true,
    };
    let vstd_version = version(packages.get(root)?);
    if vstd_version != shipped_vstd.version {
        // Another vstd release is another proof; its closure is not compared.
        difference.packages.push(format!(
            "this project uses vstd {vstd_version} and the proof is for vstd {}",
            shipped_vstd.version
        ));
        difference.alignable = false;
        return Some(difference);
    }
    let found = closure_members(&packages, &nodes, root)?;
    // A package name with several versions in the workspace needs a full spec.
    let versions = |wanted: &str| {
        packages
            .values()
            .filter(|package| package["name"] == wanted)
            .count()
    };
    let names: BTreeSet<&str> = wanted
        .keys()
        .copied()
        .chain(found.keys().map(String::as_str))
        .collect();
    for package in names {
        match (wanted.get(package), found.get(package).map(Vec::as_slice)) {
            (Some(shipped), Some([(actual, features, registry)])) => {
                let spec = if versions(package) > 1 {
                    format!("{package}@{actual}")
                } else {
                    package.to_owned()
                };
                compare(
                    &mut difference,
                    package,
                    shipped,
                    (actual, features, *registry),
                    &spec,
                );
            }
            (Some(_), Some(several)) => {
                let list: Vec<_> = several.iter().map(|(v, _, _)| v.as_str()).collect();
                difference
                    .packages
                    .push(format!("{package} is resolved as {}", list.join(" and ")));
                difference.alignable = false;
            }
            (Some(shipped), None) => {
                difference
                    .packages
                    .push(format!("{package} {} is absent", shipped.version));
                difference.alignable = false;
            }
            (None, Some(extra)) => {
                let list: Vec<_> = extra.iter().map(|(v, _, _)| v.as_str()).collect();
                difference.packages.push(format!(
                    "{package} {} is an extra dependency",
                    list.join(" and ")
                ));
                difference.alignable = false;
            }
            (None, None) => {}
        }
    }
    (!difference.packages.is_empty()).then_some(difference)
}

/// Each package in the closure of `root`: name to its (version, features,
/// from crates.io) resolutions.
type Members = BTreeMap<String, Vec<(String, BTreeSet<String>, bool)>>;

fn closure_members(
    packages: &BTreeMap<&str, &Value>,
    nodes: &BTreeMap<&str, &Value>,
    root: &str,
) -> Option<Members> {
    let text =
        |package: &Value, field: &str| package[field].as_str().unwrap_or_default().to_owned();
    let mut found = Members::new();
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let package = packages.get(id)?;
        let node = nodes.get(id)?;
        let features = node["features"]
            .as_array()?
            .iter()
            .map(|feature| feature.as_str().map(str::to_owned))
            .collect::<Option<_>>()?;
        let registry = package["source"].as_str()
            == Some("registry+https://github.com/rust-lang/crates.io-index");
        found.entry(text(package, "name")).or_default().push((
            text(package, "version"),
            features,
            registry,
        ));
        for dependency in node["dependencies"].as_array()? {
            pending.push(dependency.as_str()?);
        }
    }
    Some(found)
}

/// Records how one singly resolved closure member differs from the policy.
fn compare(
    difference: &mut Difference,
    package: &str,
    shipped: &Package,
    (actual, features, registry): (&str, &BTreeSet<String>, bool),
    spec: &str,
) {
    if !registry {
        difference
            .packages
            .push(format!("{package} {actual} is not from crates.io"));
        difference.alignable = false;
        return;
    }
    if actual != shipped.version {
        difference.packages.push(format!(
            "{package} is {actual} instead of {}",
            shipped.version
        ));
        difference.commands.push(format!(
            "cargo update -p {spec} --precise {}",
            shipped.version
        ));
    }
    let expected: BTreeSet<String> = shipped.features.iter().cloned().collect();
    if *features != expected {
        let mut with_default = features.clone();
        with_default.insert("default".to_owned());
        if package == "vstd" && !features.contains("default") && with_default == expected {
            difference.vstd_default_off = true;
            difference
                .packages
                .push("vstd's default features are off".to_owned());
        } else {
            let list = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>().join(", ");
            difference.packages.push(format!(
                "{package} has features [{}] instead of [{}]",
                list(features),
                list(&expected)
            ));
            difference.alignable = false;
        }
    }
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
        let metadata = policy_metadata(registry)?;
        assert!(resolved_packages(&metadata, registry, &policy).is_some());
        let mut hooks = metadata.clone();
        let hook_node = hooks["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("missing nodes")?
            .iter_mut()
            .find(|node| {
                node["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("verus_builtin_macros-"))
            })
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

/// The package named `name` (`list` "packages") or its resolve node.
#[cfg(test)]
pub(super) fn test_entry<'a>(
    metadata: &'a mut Value,
    name: &str,
    list: &str,
) -> Result<&'a mut Value, &'static str> {
    let id = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|package| package["name"] == name))
        .map(|package| package["id"].clone())
        .ok_or("no such package")?;
    let items = if list == "packages" {
        &mut metadata["packages"]
    } else {
        &mut metadata["resolve"]["nodes"]
    };
    items
        .as_array_mut()
        .and_then(|items| items.iter_mut().find(|item| item["id"] == id))
        .ok_or("no such entry")
}

#[cfg(test)]
pub(super) fn test_version(
    metadata: &mut Value,
    name: &str,
    version: &str,
) -> Result<(), &'static str> {
    test_entry(metadata, name, "packages")?["version"] = version.into();
    Ok(())
}

#[cfg(test)]
pub(super) fn test_features(
    metadata: &mut Value,
    name: &str,
    features: &[&str],
) -> Result<(), &'static str> {
    test_entry(metadata, name, "nodes")?["features"] = serde_json::json!(features);
    Ok(())
}

/// Cargo metadata resolving exactly the policy's closure under `registry`.
#[cfg(test)]
pub(super) fn policy_metadata(registry: &Path) -> Result<Value, serde_json::Error> {
    let policy: Policy = serde_json::from_str(POLICY)?;
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
    Ok(serde_json::json!({"packages":packages,"resolve":{"nodes":nodes}}))
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
mod closure_difference_tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn policy() -> Result<Policy, serde_json::Error> {
        serde_json::from_str(POLICY)
    }

    #[test]
    fn the_shipped_closure_has_no_difference() -> TestResult {
        assert_eq!(closure_difference(&policy_metadata(Path::new("/r"))?), None);
        Ok(())
    }

    #[test]
    fn version_and_default_feature_differences_are_alignable() -> TestResult {
        // Vest's lock moved to the shipped vstd (#237): older host crates and
        // `default-features = false`.
        let policy = policy()?;
        let shipped = |name: &str| {
            policy
                .packages
                .values()
                .find(|package| package.name == name)
                .map(|package| package.version.clone())
        };
        let mut metadata = policy_metadata(Path::new("/r"))?;
        test_version(&mut metadata, "unicode-ident", "1.0.24")?;
        test_version(&mut metadata, "indexmap", "2.14.1")?;
        test_features(&mut metadata, "vstd", &["alloc", "std"])?;
        let difference = closure_difference(&metadata).ok_or("no difference")?;
        let unicode = shipped("unicode-ident").ok_or("unicode-ident")?;
        let indexmap = shipped("indexmap").ok_or("indexmap")?;
        assert_eq!(
            difference.packages,
            [
                format!("indexmap is 2.14.1 instead of {indexmap}"),
                format!("unicode-ident is 1.0.24 instead of {unicode}"),
                "vstd's default features are off".to_owned(),
            ]
        );
        assert_eq!(
            difference.commands,
            [
                format!("cargo update -p indexmap --precise {indexmap}"),
                format!("cargo update -p unicode-ident --precise {unicode}"),
            ]
        );
        assert!(difference.vstd_default_off && difference.alignable);
        Ok(())
    }

    #[test]
    fn a_name_with_several_versions_gets_a_full_spec() -> TestResult {
        let mut metadata = policy_metadata(Path::new("/r"))?;
        test_version(&mut metadata, "unicode-ident", "1.0.24")?;
        // Another unicode-ident elsewhere in the workspace, outside the closure.
        metadata["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .push(serde_json::json!({"id":"unicode-ident-0.9.0","name":"unicode-ident",
                "version":"0.9.0","source":"registry+https://github.com/rust-lang/crates.io-index"}));
        let difference = closure_difference(&metadata).ok_or("no difference")?;
        assert!(
            difference.commands[0].starts_with("cargo update -p unicode-ident@1.0.24 --precise "),
            "{:?}",
            difference.commands
        );
        Ok(())
    }

    #[test]
    fn other_releases_features_and_sources_are_reported_without_commands() -> TestResult {
        let mut other = policy_metadata(Path::new("/r"))?;
        test_version(&mut other, "vstd", "0.0.0-2026-09-06-0133")?;
        let difference = closure_difference(&other).ok_or("no difference")?;
        assert_eq!(difference.packages.len(), 1, "{:?}", difference.packages);
        assert!(difference.packages[0].starts_with(
            "this project uses vstd 0.0.0-2026-09-06-0133 and the proof is for vstd "
        ));
        assert!(!difference.alignable && difference.commands.is_empty());

        let mut other_features = policy_metadata(Path::new("/r"))?;
        test_features(&mut other_features, "vstd", &["alloc", "allow_panic"])?;
        let difference = closure_difference(&other_features).ok_or("no difference")?;
        assert!(!difference.alignable && !difference.vstd_default_off);

        let mut path = policy_metadata(Path::new("/r"))?;
        test_entry(&mut path, "syn", "packages")?["source"] = Value::Null;
        let difference = closure_difference(&path).ok_or("no difference")?;
        assert!(!difference.alignable);
        assert!(
            difference
                .packages
                .iter()
                .any(|p| p.ends_with("is not from crates.io"))
        );

        let mut extra = policy_metadata(Path::new("/r"))?;
        test_entry(&mut extra, "vstd", "nodes")?["dependencies"]
            .as_array_mut()
            .ok_or("dependencies")?
            .push("foreign-1.0.0".into());
        extra["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .push(serde_json::json!({
            "id":"foreign-1.0.0","name":"foreign","version":"1.0.0",
            "source":"registry+https://github.com/rust-lang/crates.io-index"}));
        extra["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("nodes")?
            .push(serde_json::json!({"id":"foreign-1.0.0","features":[],"dependencies":[]}));
        let difference = closure_difference(&extra).ok_or("no difference")?;
        assert!(!difference.alignable);
        assert!(
            difference
                .packages
                .contains(&"foreign 1.0.0 is an extra dependency".to_owned())
        );
        Ok(())
    }

    #[test]
    fn unit_features_bind_the_invocation_not_workspace_metadata() -> TestResult {
        // Workspace-unified metadata admits vstd with the policy's features
        // even when the selected `-p` member enables fewer. The invocation's
        // own `--cfg feature=` arguments decide.
        let policy = policy()?;
        let registry = Path::new("/r");
        let metadata = policy_metadata(registry)?;
        assert!(resolved_packages(&metadata, registry, &policy).is_some());
        let vstd = policy.packages.get(&policy.vstd).ok_or("vstd")?;
        let cfgs = |features: &[&str]| {
            let mut args = vec![std::ffi::OsString::from("rustc")];
            for feature in features {
                args.push("--cfg".into());
                args.push(format!("feature=\"{feature}\"").into());
            }
            args
        };
        let exact: Vec<&str> = vstd.features.iter().map(String::as_str).collect();
        assert!(features_admitted(&cfgs(&exact), vstd));
        let narrower: Vec<&str> = exact.iter().copied().filter(|f| *f != "default").collect();
        assert!(!features_admitted(&cfgs(&narrower), vstd));
        Ok(())
    }
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
