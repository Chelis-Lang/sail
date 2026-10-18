//! Independent copies of a workspace verifier's complete owned lookup graph.
//!
//! This is a byte-placement boundary, not a macro-eligibility proof. Its caller
//! must separately admit the official source/features, pinned sysroot, effective
//! Verus options and compiler-read observations. Native callers hold each graph
//! through compiler completion; retained proofs also need it to survive for
//! their artifact generation and archive lifetime.

use super::files::digest;
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

pub(super) fn read_content(path: &Path, maximum: u64) -> Option<Content> {
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

impl Inventory {
    /// Read and hash the complete eligible lookup inventory without creating
    /// private copies. This preliminary value grants no compiler/proof authority;
    /// callers must recapture or independently reuse a complete final snapshot.
    pub(super) fn read_ready(owned_target: &Path, args: &[OsString]) -> Option<Self> {
        let target = owned_target.canonicalize().ok()?;
        let unit = Unit::parse(args)?;
        let (directories, externs) = lookups(&target, &unit, args)?;
        if directories.is_empty()
            || directories.len() > 8
            || unit.directory.canonicalize().ok()? != unit.directory
            || !unit.directory.starts_with(&target)
        {
            return None;
        }
        capture_after_directory_work(&directories, &unit, || {
            Self::read(&directories, &unit, &externs)
        })
    }

    fn read(directories: &BTreeSet<PathBuf>, unit: &Unit, externs: &[PathBuf]) -> Option<Self> {
        let mut inventory = Self {
            directories: BTreeMap::new(),
            artifacts: BTreeMap::new(),
        };
        let mut remaining = TOTAL_LIMIT;
        for directory in directories {
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
        Some(inventory)
    }

    /// Mutable original names used only to wait for producer-record availability.
    pub(super) fn original_inputs(&self) -> &BTreeMap<PathBuf, Content> {
        &self.artifacts
    }
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
    /// Wait briefly for compiler scratch directories to disappear, then
    /// run the complete capture. Any failed capture permits one fresh attempt
    /// within the same deadline; no directory is excluded or admitted.
    pub(super) fn capture_ready(
        owned_target: &Path,
        root: &Path,
        args: &[OsString],
    ) -> Option<Self> {
        let target = owned_target.canonicalize().ok()?;
        let unit = Unit::parse(args)?;
        let (directories, _) = lookups(&target, &unit, args)?;
        if directories.is_empty() || directories.len() > 8 {
            return None;
        }
        capture_after_directory_work(&directories, &unit, || {
            Self::capture(owned_target, root, args)
        })
    }

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
        let inventory = Inventory::read(&directories, &unit, &externs)?;
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

    /// Original artifact names and bytes independently copied for this invocation.
    pub(super) fn original_inputs(&self) -> &BTreeMap<PathBuf, Content> {
        &self.inventory.artifacts
    }

    pub(super) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    /// Additional durable bytes and entries for publishing this full graph.
    pub(super) fn storage(&self) -> Option<(u64, u64)> {
        let bytes = self
            .inventory
            .artifacts
            .values()
            .try_fold(0u64, |total, body| total.checked_add(body.size))?;
        let entries = self
            .inventory
            .artifacts
            .len()
            .checked_add(self.inventory.directories.len())?
            .checked_add(1)?;
        Some((bytes, u64::try_from(entries).ok()?))
    }

    /// Keep the graph at its content address through the owning artifact and
    /// retained-entry lifetime. Dep-info may name these literal paths; never
    /// rewrite it to pretend the mutable originals were consumed instead.
    /// The archive's existing reference collection owns eventual reclamation.
    pub(super) fn publish(mut self, graphs: &Path, original_args: &[OsString]) -> Option<Self> {
        let key = self.key()?;
        let destination = graphs.join(&key);
        if self.published {
            return (self.root == destination && self.intact()).then_some(self);
        }
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

fn capture_after_directory_work<T>(
    directories: &BTreeSet<PathBuf>,
    unit: &Unit,
    mut capture: impl FnMut() -> Option<T>,
) -> Option<T> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    for attempt in 0..2 {
        if attempt > 0 && std::time::Instant::now() >= deadline {
            return None;
        }
        while !lookup_ready(directories, unit)? {
            let remaining = deadline.checked_duration_since(std::time::Instant::now())?;
            std::thread::sleep(remaining.min(std::time::Duration::from_millis(10)));
        }
        // All path, shape, membership and byte checks still run on every
        // capture. The first failure may be a transient already gone before
        // another scan; at most one fresh complete attempt is allowed.
        if let Some(snapshot) = capture() {
            return Some(snapshot);
        }
    }
    None
}

/// Compiler metadata, codegen and archive writers create temporary directories
/// beside their outputs. Wait for directories rather than guessing their names.
/// Persistent directories time out; aliases and special files refuse immediately.
/// This scan reads no artifact bodies and creates none.
fn lookup_ready(directories: &BTreeSet<PathBuf>, unit: &Unit) -> Option<bool> {
    let mut ready = true;
    for directory in directories {
        let mut count = 0usize;
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            if unit.owns(&entry.path()) {
                continue;
            }
            count += 1;
            if count > ENTRY_LIMIT {
                return None;
            }
            let kind = entry.file_type().ok()?;
            if kind.is_file() {
                continue;
            }
            if !kind.is_dir() {
                return None;
            }
            ready = false;
        }
    }
    Some(ready)
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
    fn readiness_reads_complete_inventory_without_creating_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("read-inventory")?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        for (name, bytes) in [
            ("libvstd-abcd.rmeta", b"metadata A".as_slice()),
            ("libvstd-abcd.vir", b"VIR A"),
            ("libbuiltin-abcd.rlib", b"transitive A"),
            ("other.d", b"membership without body hashing"),
            ("libworkspace-abcd.rmeta", b"excluded exact self output"),
        ] {
            std::fs::write(deps.join(name), bytes)?;
        }
        let before: BTreeSet<_> = std::fs::read_dir(&root.0)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<_, _>>()?;
        let first = Inventory::read_ready(&target, &root.args()).ok_or("inventory")?;
        assert_eq!(first.original_inputs().len(), 3);
        assert_eq!(first.directories[&deps].len(), 4);
        assert_eq!(
            first.artifacts[&deps.join("libbuiltin-abcd.rlib")],
            Content {
                sha256: digest(b"transitive A"),
                size: 12,
            }
        );
        assert_eq!(
            before,
            std::fs::read_dir(&root.0)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<Result<BTreeSet<_>, _>>()?
        );
        let final_graph = Snapshot::capture(&target, &root.0.join("final"), &root.args())
            .ok_or("independent final graph")?;
        assert_eq!(first, final_graph.inventory);
        assert!(final_graph.intact());
        Ok(())
    }

    #[test]
    fn readiness_cannot_authorize_changed_originals_or_private_bodies()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("read-authority")?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        let original = deps.join("libvstd-abcd.rmeta");
        std::fs::write(&original, b"A")?;
        let first = Inventory::read_ready(&target, &root.args()).ok_or("first inventory")?;
        let modified = std::fs::metadata(&original)?.modified()?;
        std::fs::write(&original, b"B")?;
        std::fs::File::options()
            .write(true)
            .open(&original)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
        let fresh = Inventory::read_ready(&target, &root.args()).ok_or("fresh inventory")?;
        assert_ne!(first, fresh);
        let graphs = root.0.join("graphs");
        std::fs::create_dir(&graphs)?;
        let final_graph = Snapshot::capture_ready(&target, &root.0.join("final"), &root.args())
            .and_then(|graph| graph.publish(&graphs, &root.args()))
            .ok_or("final graph B")?;
        assert_eq!(final_graph.inventory, fresh);
        assert_ne!(final_graph.inventory, first);
        assert!(Snapshot::reuse(&target, &graphs, &root.args()).is_some());
        std::fs::write(&original, b"A")?;
        assert!(Snapshot::reuse(&target, &graphs, &root.args()).is_none());
        std::fs::write(&original, b"B")?;
        let private = final_graph.copied_inputs().ok_or("private inputs")?;
        let path = private.keys().next().ok_or("private input")?;
        // The fixture deliberately corrupts an owned read-only view. An earlier
        // successful preliminary inventory cannot turn it into proof authority.
        let mut permissions = std::fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            permissions.set_mode(0o600);
        }
        std::fs::set_permissions(path, permissions)?;
        std::fs::write(path, b"C")?;
        assert!(!final_graph.intact());
        assert!(Snapshot::reuse(&target, &graphs, &root.args()).is_none());
        Ok(())
    }

    #[test]
    fn readiness_waits_then_rereads_completed_producer_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("read-pipeline")?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        let artifact = deps.join("libvstd-abcd.rmeta");
        let scratch = deps.join("compiler-work");
        std::fs::write(&artifact, b"before")?;
        std::fs::create_dir(&scratch)?;
        let producer = std::thread::spawn(move || -> std::io::Result<()> {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(&artifact, b"finished")?;
            std::fs::remove_dir(&scratch)
        });
        let first = Inventory::read_ready(&target, &root.args());
        producer.join().map_err(|_| "producer fixture panicked")??;
        let first = first.ok_or("completed readiness")?;
        assert_eq!(
            first.artifacts[&deps.join("libvstd-abcd.rmeta")].sha256,
            digest(b"finished")
        );
        assert_eq!(std::fs::read_dir(&root.0)?.count(), 1, "no private view");
        Ok(())
    }

    #[test]
    fn readiness_refuses_malformed_and_missing_imports() -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("read-malformed")?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"A")?;
        for external in ["missing-equals", "vstd=/foreign/missing.rmeta"] {
            let mut args = root.args();
            *args.last_mut().ok_or("extern argument")? = external.into();
            assert!(Inventory::read_ready(&target, &args).is_none());
        }
        let mut args = root.args();
        args.push("--import=foreign=/foreign/input.vir".into());
        assert!(Inventory::read_ready(&target, &args).is_none());
        args = root.args();
        args[4] = "extra-filename=".into();
        assert!(Inventory::read_ready(&target, &args).is_none());
        assert_eq!(std::fs::read_dir(&root.0)?.count(), 1);
        Ok(())
    }

    #[test]
    fn readiness_keeps_size_entry_and_lookup_bounds() -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("read-limits")?;
        let target = root.0.join("target");
        let deps = target.join("debug/deps");
        let artifact = deps.join("libvstd-abcd.rmeta");
        std::fs::File::create(&artifact)?.set_len(FILE_LIMIT + 1)?;
        assert!(Inventory::read_ready(&target, &root.args()).is_none());
        std::fs::write(&artifact, b"A")?;
        let mut args = root.args();
        for index in 0..8 {
            let extra = target.join(format!("lookup-{index}/deps"));
            std::fs::create_dir_all(&extra)?;
            args.push(format!("-Ldependency={}", extra.display()).into());
        }
        assert!(Inventory::read_ready(&target, &args).is_none());
        for index in 0..ENTRY_LIMIT {
            std::fs::write(deps.join(format!("member-{index}.d")), b"")?;
        }
        assert!(Inventory::read_ready(&target, &root.args()).is_none());
        Ok(())
    }

    #[test]
    fn mixed_compiler_directories_disappear_before_any_graph_copy()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("metadata-ready")?;
        let deps = root.0.join("target/debug/deps");
        let scratches =
            ["rmeta4RA3xP", "rustcihBLIO", ".tmp1YZxfY.temp-archive"].map(|name| deps.join(name));
        let destination = root.0.join("snapshot");
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"before producer exit")?;
        for scratch in &scratches {
            std::fs::create_dir(scratch)?;
            std::fs::write(scratch.join("incomplete"), b"incomplete")?;
        }
        let producer_destination = destination.clone();
        let producer_deps = deps.clone();
        let producer = std::thread::spawn(move || -> std::io::Result<()> {
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(!producer_destination.exists(), "no graph copy while busy");
            std::fs::write(
                producer_deps.join("libvstd-abcd.rmeta"),
                b"completed metadata",
            )?;
            for scratch in scratches {
                std::fs::remove_dir_all(scratch)?;
            }
            Ok(())
        });
        let captured = Snapshot::capture_ready(&root.0.join("target"), &destination, &root.args());
        producer.join().map_err(|_| "producer fixture panicked")??;
        let snapshot = captured.ok_or("ready graph")?;
        assert!(snapshot.intact());
        assert_eq!(snapshot.original_inputs().len(), 1);
        assert_eq!(
            std::fs::read(destination.join("0/libvstd-abcd.rmeta"))?,
            b"completed metadata"
        );
        assert_eq!(
            snapshot.inventory.directories[&deps],
            BTreeSet::from([OsString::from("libvstd-abcd.rmeta")])
        );
        Ok(())
    }

    #[test]
    fn persistent_unknown_directory_refuses_without_partial_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("metadata-persistent")?;
        let deps = root.0.join("target/debug/deps");
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"A")?;
        std::fs::create_dir(deps.join("unknown-persistent-directory"))?;
        let destination = root.0.join("snapshot");
        assert!(
            Snapshot::capture_ready(&root.0.join("target"), &destination, &root.args()).is_none()
        );
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn metadata_directory_racing_capture_permits_at_most_one_complete_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        for fail_again in [false, true] {
            let root = Root::new(if fail_again {
                "metadata-races-twice"
            } else {
                "metadata-race"
            })?;
            let deps = root.0.join("target/debug/deps");
            let scratch = deps.join("rmeta4RA3xP");
            let destination = root.0.join("snapshot");
            std::fs::write(deps.join("libvstd-abcd.rmeta"), b"metadata")?;
            let mut captures = 0;
            let mut producer = None;
            let result = capture_after_directory_work(
                &BTreeSet::from([deps]),
                &Unit::parse(&root.args()).ok_or("unit")?,
                || {
                    captures += 1;
                    if captures == 1 || fail_again {
                        // Inject the real unsupported directory after the
                        // readiness scan, before the unchanged full capture.
                        std::fs::create_dir(&scratch).ok()?;
                    }
                    let result =
                        Snapshot::capture(&root.0.join("target"), &destination, &root.args());
                    if captures == 1 {
                        assert!(result.is_none());
                        assert!(!destination.exists(), "partial copy removed");
                        let scratch = scratch.clone();
                        producer = Some(std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(30));
                            std::fs::remove_dir(scratch)
                        }));
                    }
                    result
                },
            );
            producer
                .ok_or("producer")?
                .join()
                .map_err(|_| "producer fixture panicked")??;
            assert_eq!(captures, 2);
            if fail_again {
                assert!(result.is_none());
                assert!(!destination.exists());
            } else {
                assert!(result.is_some_and(|snapshot| snapshot.intact()));
            }
        }
        Ok(())
    }

    #[test]
    fn directory_gone_before_post_failure_scan_still_gets_one_full_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        for fail_again in [false, true] {
            let root = Root::new(if fail_again {
                "gone-twice"
            } else {
                "gone-once"
            })?;
            let deps = root.0.join("target/debug/deps");
            let scratch = deps.join("rustcihBLIO");
            let destination = root.0.join("snapshot");
            std::fs::write(deps.join("libvstd-abcd.rmeta"), b"metadata")?;
            let directories = BTreeSet::from([deps]);
            let unit = Unit::parse(&root.args()).ok_or("unit")?;
            let mut captures = 0;
            let result = capture_after_directory_work(&directories, &unit, || {
                captures += 1;
                if captures == 1 || fail_again {
                    std::fs::create_dir(&scratch).ok()?;
                }
                let result = Snapshot::capture(&root.0.join("target"), &destination, &root.args());
                if scratch.exists() {
                    assert!(result.is_none());
                    assert!(!destination.exists(), "partial copy removed");
                    std::fs::remove_dir(&scratch).ok()?;
                }
                assert_eq!(lookup_ready(&directories, &unit), Some(true));
                result
            });
            assert_eq!(captures, 2);
            if fail_again {
                assert!(result.is_none());
                assert!(!destination.exists());
            } else {
                assert!(result.is_some_and(|snapshot| snapshot.intact()));
            }
        }
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn directory_readiness_refuses_aliases_and_special_files_immediately()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Root::new("metadata-shapes")?;
        let deps = root.0.join("target/debug/deps");
        std::fs::write(deps.join("libvstd-abcd.rmeta"), b"A")?;
        std::fs::create_dir(root.0.join("elsewhere"))?;
        let alias = deps.join("rmeta4RA3xP");
        std::os::unix::fs::symlink(root.0.join("elsewhere"), &alias)?;
        let directories = BTreeSet::from([deps.clone()]);
        let unit = Unit::parse(&root.args()).ok_or("unit")?;
        assert!(lookup_ready(&directories, &unit).is_none());
        assert!(Inventory::read_ready(&root.0.join("target"), &root.args()).is_none());
        assert!(
            Snapshot::capture_ready(
                &root.0.join("target"),
                &root.0.join("snapshot"),
                &root.args()
            )
            .is_none()
        );
        std::fs::remove_file(alias)?;
        // The pinned rustix has no mkfifoat on macOS. Exercise the same real
        // filesystem shape through the host utility, as other FIFO tests do.
        let fifo = std::process::Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(deps.join("compiler-pipe"))
            .status()?;
        assert!(fifo.success());
        assert!(lookup_ready(&directories, &unit).is_none());
        assert!(Inventory::read_ready(&root.0.join("target"), &root.args()).is_none());
        assert!(
            Snapshot::capture_ready(
                &root.0.join("target"),
                &root.0.join("snapshot"),
                &root.args()
            )
            .is_none()
        );
        assert!(!root.0.join("snapshot").exists());
        Ok(())
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
        assert_eq!(first.original_inputs().len(), 3);
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
        assert!(Inventory::read_ready(&root.0.join("target"), &args).is_none());
        assert!(Snapshot::capture(&root.0.join("target"), &root.0.join("self"), &args).is_none());
        args = root.args();
        args.extend([OsString::from("-Lnative=/foreign")]);
        assert!(Inventory::read_ready(&root.0.join("target"), &args).is_none());
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
            assert!(Inventory::read_ready(&root.0.join("target"), &root.args()).is_none());
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
