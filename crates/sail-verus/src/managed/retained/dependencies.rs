//! Independent copies of a workspace verifier's complete owned lookup graph.
//!
//! This is a byte-placement boundary, not a macro-eligibility proof. Its caller
//! must separately admit the official source/features, pinned sysroot, effective
//! Verus options and compiler-read observations. No compiler invocation uses
//! these helpers yet.

use super::super::files::digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

const FILE_LIMIT: u64 = 256 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 512 * 1024 * 1024;
const ENTRY_LIMIT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Content {
    pub sha256: String,
    pub size: u64,
}

/// Original path identities and complete non-self basename membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Inventory {
    directories: BTreeMap<PathBuf, BTreeSet<OsString>>,
    artifacts: BTreeMap<PathBuf, Content>,
}

/// The exact current unit's output family; never a broad basename prefix.
struct Unit {
    directory: PathBuf,
    stem: String,
}

impl Unit {
    fn parse(args: &[OsString]) -> Option<Self> {
        let name = value(args, "--crate-name")?.to_str()?;
        let extra = args
            .windows(2)
            .find_map(|pair| {
                (pair[0] == "-C").then(|| pair[1].to_str()?.strip_prefix("extra-filename="))?
            })
            .or_else(|| {
                args.iter()
                    .find_map(|arg| arg.to_str()?.strip_prefix("-Cextra-filename="))
            })?;
        if name.is_empty()
            || extra.is_empty()
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !extra
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return None;
        }
        Some(Self {
            directory: PathBuf::from(value(args, "--out-dir")?),
            stem: format!("{name}{extra}"),
        })
    }

    fn owns(&self, path: &Path) -> bool {
        if path.parent() != Some(self.directory.as_path()) {
            return false;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        ["d", "rmeta", "rlib", "vir", "so", "dylib"]
            .iter()
            .any(|extension| {
                name == format!("{}.{extension}", self.stem)
                    || name == format!("lib{}.{extension}", self.stem)
            })
    }
}

fn value<'a>(args: &'a [OsString], flag: &str) -> Option<&'a std::ffi::OsStr> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_os_str())
}

fn artifact(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ["rmeta", "rlib", "vir", "so", "dylib"]
            .iter()
            .any(|known| extension == *known)
    })
}

/// Read only regular, unredirected files. The held FD supplies the bytes.
pub(super) fn open(path: &Path) -> Option<std::fs::File> {
    #[cfg(unix)]
    {
        let fd = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .ok()?;
        let file = std::fs::File::from(fd);
        file.metadata().ok()?.is_file().then_some(file)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

fn copy(from: &Path, to: &Path, remaining: u64) -> Option<Content> {
    let mut input = open(from)?;
    let maximum = remaining.min(FILE_LIMIT);
    if input.metadata().ok()?.len() > maximum {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o400);
    }
    let mut output = options.open(to).ok()?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 65536];
    loop {
        let n = input.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        size = size.checked_add(n as u64)?;
        if size > maximum {
            return None;
        }
        output.write_all(&buffer[..n]).ok()?;
        hasher.update(&buffer[..n]);
    }
    Some(Content {
        sha256: format!("{:x}", hasher.finalize()),
        size,
    })
}

fn content(path: &Path, expected: &Content) -> bool {
    let Some(input) = open(path) else {
        return false;
    };
    if input
        .metadata()
        .ok()
        .is_none_or(|meta| meta.len() != expected.size)
    {
        return false;
    }
    let mut hasher = Sha256::new();
    let mut remaining = input.take(expected.size.saturating_add(1));
    let mut buffer = vec![0; 65536];
    let mut size = 0u64;
    loop {
        let Ok(n) = remaining.read(&mut buffer) else {
            return false;
        };
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buffer[..n]);
    }
    size == expected.size && format!("{:x}", hasher.finalize()) == expected.sha256
}

fn read_content(path: &Path, maximum: u64) -> Option<Content> {
    let input = open(path)?;
    if input.metadata().ok()?.len() > maximum {
        return None;
    }
    let mut input = input.take(maximum + 1);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 65536];
    loop {
        let n = input.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        size = size.checked_add(n as u64)?;
        if size > maximum {
            return None;
        }
        hash.update(&buffer[..n]);
    }
    Some(Content {
        sha256: format!("{:x}", hash.finalize()),
        size,
    })
}

fn lookups(
    target: &Path,
    unit: &Unit,
    args: &[OsString],
) -> Option<(BTreeSet<PathBuf>, Vec<PathBuf>)> {
    let mut directories = BTreeSet::new();
    let mut externs = Vec::new();
    for (index, argument) in args.iter().enumerate() {
        let text = argument.to_str()?;
        if text == "--import" || text.starts_with("--import=") {
            return None;
        }
        let search = text
            .strip_prefix("-L")
            .filter(|value| !value.is_empty())
            .or_else(|| (index > 0 && args[index - 1] == "-L").then_some(text));
        if let Some(search) = search {
            let directory = PathBuf::from(search.strip_prefix("dependency=")?);
            // Canonical equality refuses redirected parents as well as final aliases.
            if !directory.starts_with(target)
                || directory.canonicalize().ok()? != directory
                || directory.file_name()? != "deps"
                || !directory.is_dir()
            {
                return None;
            }
            directories.insert(directory);
        }
        let external = text
            .strip_prefix("--extern=")
            .or_else(|| (index > 0 && args[index - 1] == "--extern").then_some(text));
        if let Some(external) = external {
            if external == "proc_macro" {
                continue;
            }
            let (_, path) = external.split_once('=')?;
            let path = PathBuf::from(path);
            if unit.owns(&path) {
                return None;
            }
            externs.push(path);
        }
    }
    Some((directories, externs))
}

/// Own every copied directory through compiler completion/publication.
pub(super) struct Snapshot {
    root: PathBuf,
    published: bool,
    pub inventory: Inventory,
    copies: BTreeMap<PathBuf, PathBuf>,
    arguments: Vec<OsString>,
}

impl Snapshot {
    /// Reuse a graph only after full new reads of the actual mutable dependency
    /// inventory and all private bodies. There is no metadata-freshness memo.
    pub(super) fn reuse(owned_target: &Path, graphs: &Path, args: &[OsString]) -> Option<Self> {
        let target = owned_target.canonicalize().ok()?;
        let unit = Unit::parse(args)?;
        let (directories, externs) = lookups(&target, &unit, args)?;
        if directories.is_empty()
            || directories.len() > 8
            || unit.directory.canonicalize().ok()? != unit.directory
            || !unit.directory.starts_with(&target)
            || graphs.canonicalize().ok()? != graphs
            || directories
                .iter()
                .any(|directory| graphs.starts_with(directory))
        {
            return None;
        }
        let mut inventory = Inventory {
            directories: BTreeMap::new(),
            artifacts: BTreeMap::new(),
        };
        let mut remaining = TOTAL_LIMIT;
        for directory in &directories {
            let mut names = BTreeSet::new();
            for entry in std::fs::read_dir(directory).ok()? {
                let entry = entry.ok()?;
                if unit.owns(&entry.path()) {
                    continue;
                }
                if !entry.file_type().ok()?.is_file()
                    || !names.insert(entry.file_name())
                    || names.len() > ENTRY_LIMIT
                {
                    return None;
                }
                if artifact(&entry.path()) {
                    let bytes = read_content(&entry.path(), remaining.min(FILE_LIMIT))?;
                    remaining = remaining.checked_sub(bytes.size)?;
                    inventory.artifacts.insert(entry.path(), bytes);
                }
            }
            inventory.directories.insert(directory.clone(), names);
        }
        if externs
            .iter()
            .any(|path| !inventory.artifacts.contains_key(path))
        {
            return None;
        }
        let key = digest(&serde_json::to_vec(&inventory).ok()?);
        let root = graphs.join(key);
        if !graph_intact(&root, &inventory) {
            return None;
        }
        let mut snapshot = Self {
            root,
            published: true,
            inventory,
            copies: BTreeMap::new(),
            arguments: Vec::new(),
        };
        snapshot.copies = directories
            .into_iter()
            .enumerate()
            .map(|(index, path)| (path, snapshot.root.join(index.to_string())))
            .collect();
        snapshot.arguments = snapshot.rewrite(args)?;
        Some(snapshot)
    }

    /// `root` must be a fresh private sibling outside every source lookup dir.
    /// `owned_target` is the actual managed target, not a source-derived path.
    pub(super) fn capture(owned_target: &Path, root: &Path, args: &[OsString]) -> Option<Self> {
        let target = owned_target.canonicalize().ok()?;
        let unit = Unit::parse(args)?;
        let (directories, externs) = lookups(&target, &unit, args)?;
        if directories.is_empty()
            || directories.len() > 8
            || root.exists()
            || directories
                .iter()
                .any(|directory| root.starts_with(directory))
            || unit.directory.canonicalize().ok()? != unit.directory
            || !unit.directory.starts_with(&target)
        {
            return None;
        }
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(root).ok()?;
        let mut snapshot = Self {
            root: root.to_owned(),
            published: false,
            inventory: Inventory {
                directories: BTreeMap::new(),
                artifacts: BTreeMap::new(),
            },
            copies: BTreeMap::new(),
            arguments: Vec::new(),
        };
        let mut remaining = TOTAL_LIMIT;
        for (index, directory) in directories.iter().enumerate() {
            let destination = root.join(index.to_string());
            builder.create(&destination).ok()?;
            let mut names = BTreeSet::new();
            for entry in std::fs::read_dir(directory).ok()? {
                let entry = entry.ok()?;
                if unit.owns(&entry.path()) {
                    continue;
                }
                if !entry.file_type().ok()?.is_file()
                    || !names.insert(entry.file_name())
                    || names.len() > ENTRY_LIMIT
                {
                    return None;
                }
                if artifact(&entry.path()) {
                    let copied = destination.join(entry.file_name());
                    let bytes = copy(&entry.path(), &copied, remaining)?;
                    remaining = remaining.checked_sub(bytes.size)?;
                    if !content(&copied, &bytes) {
                        return None;
                    }
                    snapshot.inventory.artifacts.insert(entry.path(), bytes);
                }
            }
            snapshot
                .inventory
                .directories
                .insert(directory.clone(), names);
            snapshot.copies.insert(directory.clone(), destination);
        }
        if externs
            .iter()
            .any(|path| !snapshot.inventory.artifacts.contains_key(path))
        {
            return None;
        }
        snapshot.arguments = snapshot.rewrite(args)?;
        Some(snapshot)
    }

    fn rewrite(&self, args: &[OsString]) -> Option<Vec<OsString>> {
        args.iter()
            .enumerate()
            .map(|(index, argument)| {
                let text = argument.to_str()?;
                let search = text
                    .strip_prefix("-Ldependency=")
                    .map(|path| ("-Ldependency=", path))
                    .or_else(|| {
                        (index > 0 && args[index - 1] == "-L").then(|| {
                            text.strip_prefix("dependency=")
                                .map(|path| ("dependency=", path))
                        })?
                    });
                if let Some((prefix, path)) = search {
                    let mut replaced = OsString::from(prefix);
                    replaced.push(self.copies.get(Path::new(path))?);
                    return Some(replaced);
                }
                let external = text
                    .strip_prefix("--extern=")
                    .map(|external| ("--extern=", external))
                    .or_else(|| (index > 0 && args[index - 1] == "--extern").then_some(("", text)));
                if let Some((prefix, external)) = external {
                    if external == "proc_macro" {
                        return Some(argument.clone());
                    }
                    let (name, path) = external.split_once('=')?;
                    let path = Path::new(path);
                    let parent = self.copies.get(path.parent()?)?;
                    let mut replaced = OsString::from(format!("{prefix}{name}="));
                    replaced.push(parent.join(path.file_name()?));
                    return Some(replaced);
                }
                Some(argument.clone())
            })
            .collect()
    }

    pub(super) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    /// Keep the graph at its content address through the owning artifact and
    /// retained-entry lifetime. Dep-info may name these literal paths; never
    /// rewrite it to pretend the mutable originals were consumed instead.
    /// The archive's existing reference collection owns eventual reclamation.
    pub(super) fn publish(mut self, graphs: &Path, original_args: &[OsString]) -> Option<Self> {
        let key = self.key()?;
        let destination = graphs.join(&key);
        if graphs.canonicalize().ok()? != graphs
            || destination == self.root
            || self
                .inventory
                .directories
                .keys()
                .any(|directory| graphs.starts_with(directory))
        {
            return None;
        }
        if destination.exists() {
            if !graph_intact(&destination, &self.inventory) {
                return None;
            }
            std::fs::remove_dir_all(&self.root).ok()?;
        } else if std::fs::rename(&self.root, &destination).is_err() {
            // Another owned publisher may have installed the identical graph.
            if !graph_intact(&destination, &self.inventory) {
                return None;
            }
            std::fs::remove_dir_all(&self.root).ok()?;
        }
        // From here this may be another live generation's shared graph. A
        // later argument/readback refusal must not delete it in Drop.
        self.published = true;
        self.root = destination;
        self.copies = self
            .inventory
            .directories
            .keys()
            .enumerate()
            .map(|(index, directory)| (directory.clone(), self.root.join(index.to_string())))
            .collect();
        self.arguments = self.rewrite(original_args)?;
        if !self.intact() {
            return None;
        }
        Some(self)
    }

    /// Actual private paths accepted for compiler-marked binary imports.
    pub(super) fn copied_inputs(&self) -> Option<BTreeMap<PathBuf, Content>> {
        self.inventory
            .artifacts
            .iter()
            .map(|(path, content)| {
                let copy = self.copies.get(path.parent()?)?.join(path.file_name()?);
                Some((copy, content.clone()))
            })
            .collect()
    }

    pub(super) fn key(&self) -> Option<String> {
        Some(digest(&serde_json::to_vec(&self.inventory).ok()?))
    }

    pub(super) fn intact(&self) -> bool {
        let membership = self.copies.iter().all(|(original, copy)| {
            let expected: BTreeSet<_> = self
                .inventory
                .artifacts
                .keys()
                .filter(|path| path.parent() == Some(original.as_path()))
                .filter_map(|path| path.file_name().map(std::ffi::OsStr::to_owned))
                .collect();
            let Some(actual) = std::fs::read_dir(copy).ok().and_then(|entries| {
                entries
                    .map(|entry| {
                        let entry = entry.ok()?;
                        entry.file_type().ok()?.is_file().then(|| entry.file_name())
                    })
                    .collect::<Option<BTreeSet<_>>>()
            }) else {
                return false;
            };
            actual == expected
        });
        membership
            && self.inventory.artifacts.iter().all(|(path, expected)| {
                path.parent()
                    .and_then(|parent| self.copies.get(parent))
                    .zip(path.file_name())
                    .is_some_and(|(directory, name)| content(&directory.join(name), expected))
            })
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn graph_intact(root: &Path, inventory: &Inventory) -> bool {
    if root.canonicalize().ok().as_deref() != Some(root) {
        return false;
    }
    let Some(children) = std::fs::read_dir(root).ok().and_then(|children| {
        children
            .map(|child| {
                let child = child.ok()?;
                child.file_type().ok()?.is_dir().then(|| child.file_name())
            })
            .collect::<Option<BTreeSet<_>>>()
    }) else {
        return false;
    };
    if children
        != (0..inventory.directories.len())
            .map(|index| OsString::from(index.to_string()))
            .collect()
    {
        return false;
    }
    inventory
        .directories
        .keys()
        .enumerate()
        .all(|(index, original)| {
            let directory = root.join(index.to_string());
            let expected: BTreeSet<_> = inventory
                .artifacts
                .keys()
                .filter(|path| path.parent() == Some(original))
                .filter_map(|path| path.file_name().map(std::ffi::OsStr::to_owned))
                .collect();
            let Some(actual) = std::fs::read_dir(&directory).ok().and_then(|entries| {
                entries
                    .map(|entry| {
                        let entry = entry.ok()?;
                        entry.file_type().ok()?.is_file().then(|| entry.file_name())
                    })
                    .collect::<Option<BTreeSet<_>>>()
            }) else {
                return false;
            };
            actual == expected
                && inventory
                    .artifacts
                    .iter()
                    .filter(|(path, _)| path.parent() == Some(original))
                    .all(|(path, expected)| {
                        path.file_name()
                            .is_some_and(|name| content(&directory.join(name), expected))
                    })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root(PathBuf);
    impl Root {
        fn new(name: &str) -> std::io::Result<Self> {
            let root = std::env::temp_dir().join(format!(
                "sail-retained-dependencies-{}-{name}",
                std::process::id()
            ));
            std::fs::create_dir(&root)?;
            std::fs::create_dir_all(root.join("target/debug/deps"))?;
            Ok(Self(root.canonicalize()?))
        }
        fn args(&self) -> Vec<OsString> {
            let deps = self.0.join("target/debug/deps");
            vec![
                "rustc".into(),
                "--crate-name".into(),
                "workspace".into(),
                "-C".into(),
                "extra-filename=-abcd".into(),
                "--out-dir".into(),
                deps.clone().into(),
                "-L".into(),
                format!("dependency={}", deps.display()).into(),
                "--extern".into(),
                format!("vstd={}/libvstd-abcd.rmeta", deps.display()).into(),
            ]
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn whole_lookup_graph_is_owned_and_self_family_does_not_change_revert_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("whole-graph")?;
        let deps = root.0.join("target/debug/deps");
        for (name, bytes) in [
            ("libvstd-abcd.rmeta", b"metadata A".as_slice()),
            ("libvstd-abcd.vir", b"VIR A"),
            ("libbuiltin-abcd.rmeta", b"transitive A"),
            ("libworkspace-abcd.rmeta", b"old self A"),
        ] {
            std::fs::write(deps.join(name), bytes)?;
        }
        let first = Snapshot::capture(&root.0.join("target"), &root.0.join("one"), &root.args())
            .ok_or("snapshot")?;
        assert_eq!(first.inventory.artifacts.len(), 3);
        assert!(first.intact());
        // The output directory is intentionally unchanged. Only the lookup
        // directory and explicit input path must leave mutable Cargo outputs.
        assert_eq!(first.arguments()[6], deps.as_os_str());
        assert_eq!(
            first.arguments()[8],
            format!("dependency={}/one/0", root.0.display()).as_str()
        );
        assert_eq!(
            first.arguments()[10],
            format!("vstd={}/one/0/libvstd-abcd.rmeta", root.0.display()).as_str()
        );
        std::fs::write(deps.join("libworkspace-abcd.rmeta"), b"self B")?;
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"metadata B")?;
        assert!(first.intact());
        assert_eq!(
            std::fs::read(root.0.join("one/0/libvstd-abcd.rmeta"))?,
            b"metadata A"
        );
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"metadata A")?;
        let second = Snapshot::capture(&root.0.join("target"), &root.0.join("two"), &root.args())
            .ok_or("second")?;
        assert_eq!(first.key(), second.key());
        std::fs::write(deps.join("libworkspace-abcd-other.rmeta"), b"not self")?;
        let third = Snapshot::capture(&root.0.join("target"), &root.0.join("three"), &root.args())
            .ok_or("third")?;
        assert_ne!(second.key(), third.key());
        std::fs::write(root.0.join("one/0/libunexpected.rmeta"), b"extra")?;
        assert!(!first.intact());
        Ok(())
    }

    #[test]
    fn self_extern_foreign_search_and_aliases_refuse() -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("refusals")?;
        let deps = root.0.join("target/debug/deps");
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"A")?;
        let mut args = root.args();
        let last = args.len() - 1;
        args[last] = format!("workspace={}/libworkspace-abcd.rmeta", deps.display()).into();
        assert!(Snapshot::capture(&root.0.join("target"), &root.0.join("self"), &args).is_none());
        args = root.args();
        args.extend([OsString::from("-Lnative=/foreign")]);
        assert!(
            Snapshot::capture(&root.0.join("target"), &root.0.join("foreign"), &args).is_none()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                deps.join("libvstd-abcd.rmeta"),
                deps.join("libalias.rmeta"),
            )?;
            assert!(
                Snapshot::capture(&root.0.join("target"), &root.0.join("alias"), &root.args())
                    .is_none()
            );
            assert!(!root.0.join("alias").exists());
        }
        Ok(())
    }

    #[test]
    fn graph_paths_survive_the_driver_and_are_stable_across_own_body_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("lifetime")?;
        let deps = root.0.join("target/debug/deps");
        let graphs = root.0.join("graphs");
        std::fs::create_dir(&graphs)?;
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"metadata")?;
        std::fs::write(deps.join("libvstd-abcd.vir"), b"VIR")?;
        let args = root.args();
        let first = Snapshot::capture(&root.0.join("target"), &root.0.join("temporary-a"), &args)
            .and_then(|snapshot| snapshot.publish(&graphs, &args))
            .ok_or("published graph A")?;
        let first_args = first.arguments().to_vec();
        let copies = first.copied_inputs().ok_or("copies")?;
        let key = first.key().ok_or("key")?;
        drop(first);
        assert!(copies.keys().all(|path| path.is_file()));
        std::fs::write(deps.join("libworkspace-abcd.rmeta"), b"body B")?;
        let second = Snapshot::capture(&root.0.join("target"), &root.0.join("temporary-b"), &args)
            .and_then(|snapshot| snapshot.publish(&graphs, &args))
            .ok_or("published graph B")?;
        assert_eq!(second.key().as_ref(), Some(&key));
        assert_eq!(second.arguments(), first_args);
        assert!(second.intact());
        assert!(!root.0.join("temporary-b").exists());
        let reused = Snapshot::reuse(&root.0.join("target"), &graphs, &args)
            .ok_or("reused without copying")?;
        assert_eq!(reused.arguments(), first_args);
        std::fs::write(deps.join("libvstd-abcd.vir"), b"different import")?;
        assert!(Snapshot::reuse(&root.0.join("target"), &graphs, &args).is_none());
        Ok(())
    }
}
