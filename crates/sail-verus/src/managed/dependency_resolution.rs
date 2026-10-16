//! Bracket pathname resolution without confusing sibling output writes with renames.

use std::collections::{BTreeMap, VecDeque};
use std::fs::{File, Metadata};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

const MAX_PATHS: usize = 4096;
const MAX_LINKS: usize = 40;

/// Opens an input without following a last-component symlink or waiting for
/// a FIFO peer, then admits only a held regular file/directory on a local
/// filesystem with supported native observation semantics.
pub(super) fn open_input(path: &Path) -> Option<File> {
    let file = open_nonblocking(path)?;
    supported_filesystem(&file).then_some(file)
}

#[cfg(unix)]
fn open_nonblocking(path: &Path) -> Option<File> {
    use rustix::fs::{Mode, OFlags, open};
    let file = File::from(
        open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?,
    );
    let metadata = file.metadata().ok()?;
    (metadata.is_file() || metadata.is_dir()).then_some(file)
}

#[cfg(not(unix))]
fn open_nonblocking(_: &Path) -> Option<File> {
    None
}

#[cfg(any(test, target_os = "linux"))]
fn admitted_linux_type(kind: impl TryInto<u64>) -> bool {
    // ext2/3/4, XFS, Btrfs, tmpfs and overlayfs. Unknown, network, FUSE and
    // pseudo-filesystems have no qualified native-event contract here.
    kind.try_into().is_ok_and(|kind| {
        matches!(
            kind,
            0xef53 | 0x5846_5342 | 0x9123_683e | 0x0102_1994 | 0x794c_7630
        )
    })
}

#[cfg(any(test, target_os = "macos"))]
fn admitted_macos_type(name: &[u8], local: bool) -> bool {
    local && matches!(name, b"apfs" | b"hfs")
}

#[cfg(target_os = "linux")]
fn supported_filesystem(file: &File) -> bool {
    rustix::fs::fstatfs(file).is_ok_and(|stats| admitted_linux_type(stats.f_type))
}

#[cfg(target_os = "macos")]
fn supported_filesystem(file: &File) -> bool {
    let Ok(stats) = rustix::fs::fstatfs(file) else {
        return false;
    };
    let Ok(name) = stats
        .f_fstypename
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .map(u8::try_from)
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    // Darwin MNT_LOCAL (sys/mount.h): filesystem stored locally.
    admitted_macos_type(&name, stats.f_flags & 0x1000 != 0)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn supported_filesystem(_: &File) -> bool {
    false
}

/// One exact local file or directory mutation identity.
#[derive(Eq, PartialEq)]
pub(super) struct Stamp(String);

#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "The same interface conservatively refuses unsupported non-Unix hosts."
)]
pub(super) fn stamp(metadata: &Metadata) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt as _;
    Some(Stamp(format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.nlink(),
        metadata.uid(),
        metadata.gid(),
        metadata.size(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec()
    )))
}

#[cfg(not(unix))]
pub(super) fn stamp(_: &Metadata) -> Option<Stamp> {
    None
}

#[derive(Eq, PartialEq)]
struct Identity(String);

#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "The shared interface refuses platforms without a Unix inode identity."
)]
fn identity(metadata: &Metadata) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt as _;
    Some(Identity(format!(
        "{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid()
    )))
}

#[cfg(not(unix))]
fn identity(_: &Metadata) -> Option<Identity> {
    None
}

struct Endpoint {
    full: Stamp,
    identity: Identity,
    symlink: bool,
    regular: bool,
}

/// Complete resolution components and a latched native ancestor observer.
pub(super) struct Resolution {
    paths: BTreeMap<PathBuf, Endpoint>,
    canonical: BTreeMap<PathBuf, PathBuf>,
    observer: Mutex<State>,
}

struct State {
    native: Option<platform::Observer>,
    failed: bool,
}

impl Resolution {
    /// Resolves every symlink target component, then starts observation before
    /// the inventory reads any source bytes. Opened inode identity and final
    /// pathname identity checks bracket registration; failed setup refuses.
    pub(super) fn capture(originals: &[PathBuf]) -> Option<Self> {
        let mut result = Self {
            paths: BTreeMap::new(),
            canonical: BTreeMap::new(),
            observer: Mutex::new(State {
                native: Some(platform::Observer::new()?),
                failed: false,
            }),
        };
        for original in originals {
            original.to_str()?;
            let resolved = result.resolve(original)?;
            if original.canonicalize().ok()? != resolved {
                return None;
            }
            result.canonical.insert(original.clone(), resolved);
        }
        let state = result.observer.get_mut().ok()?;
        if let Some(observer) = &mut state.native {
            for (path, endpoint) in &result.paths {
                // Symlinks retain full inode/ctime guards. The ordinary
                // directories containing them are observed natively too.
                if !endpoint.symlink {
                    observer.add(path, &endpoint.identity)?;
                }
            }
            observer.start()?;
        }
        // Native watches are attached to opened, identity-checked inodes.
        // No source byte has been read yet: the inventory epoch begins only
        // after activation and the path/alias checks below. An ancestor ABA
        // entirely before activation cannot affect that subsequent epoch.
        // Unavailable observation refuses before any input byte is read.
        result.unchanged().then_some(result)
    }

    fn resolve(&mut self, original: &Path) -> Option<PathBuf> {
        walk(original, |path, _| self.remember(path))
    }

    /// Requires every literal and expanded intermediate component to have
    /// been observed before the guarded epoch. Only a terminal regular file
    /// can be new to this resolution map; its exact bytes must independently
    /// belong to the caller's complete, positively bound inventory. The
    /// caller must bracket this read-only walk with `unchanged()` barriers.
    #[allow(
        dead_code,
        reason = "The shared history guard uses this strict form; complete inventories also admit their positively captured directories."
    )]
    pub(super) fn covered(&self, original: &Path) -> Option<PathBuf> {
        self.covered_with(original, |_, _| false)
    }

    /// Also admits an intermediate ordinary directory only when the caller
    /// supplies its exact pre-captured native identity. Unknown symlinks still
    /// refuse; the caller must retain complete inventory mutation brackets.
    pub(super) fn covered_with(
        &self,
        original: &Path,
        mut captured_directory: impl FnMut(&Path, &Metadata) -> bool,
    ) -> Option<PathBuf> {
        original.to_str()?;
        let resolved = walk(original, |path, terminal| {
            let metadata = std::fs::symlink_metadata(path).ok()?;
            if let Some(endpoint) = self.paths.get(path) {
                (identity(&metadata).as_ref() == Some(&endpoint.identity)
                    && (!(endpoint.symlink || endpoint.regular)
                        || stamp(&metadata).as_ref() == Some(&endpoint.full)))
                .then_some(metadata)
            } else {
                ((terminal && metadata.is_file())
                    || (metadata.is_dir() && captured_directory(path, &metadata)))
                .then_some(metadata)
            }
        })?;
        if self
            .canonical(original)
            .is_some_and(|expected| expected != resolved)
            || original.canonicalize().ok()? != resolved
        {
            return None;
        }
        Some(resolved)
    }

    fn remember(&mut self, path: &Path) -> Option<Metadata> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if !metadata.is_file() && !metadata.is_dir() && !metadata.file_type().is_symlink() {
            return None;
        }
        let full = stamp(&metadata)?;
        if let Some(previous) = self.paths.get(path) {
            if identity(&metadata).as_ref() != Some(&previous.identity)
                || ((previous.symlink || previous.regular) && previous.full != full)
            {
                return None;
            }
        } else {
            if self.paths.len() == MAX_PATHS {
                return None;
            }
            self.paths.insert(
                path.to_owned(),
                Endpoint {
                    identity: identity(&metadata)?,
                    full,
                    symlink: metadata.file_type().is_symlink(),
                    regular: metadata.is_file(),
                },
            );
        }
        Some(metadata)
    }

    pub(super) fn canonical(&self, original: &Path) -> Option<&Path> {
        self.canonical.get(original).map(PathBuf::as_path)
    }

    /// False is permanent: draining an event never makes a later bracket pass.
    pub(super) fn unchanged(&self) -> bool {
        let Ok(mut state) = self.observer.lock() else {
            return false;
        };
        if state.failed {
            return false;
        }
        if let Some(observer) = &mut state.native {
            if !observer.unchanged() {
                state.failed = true;
                return false;
            }
        } else {
            state.failed = true;
            return false;
        }
        let valid = self.paths.iter().all(|(path, before)| {
            std::fs::symlink_metadata(path).is_ok_and(|metadata| {
                identity(&metadata).as_ref() == Some(&before.identity)
                    && (!(before.symlink || before.regular)
                        || stamp(&metadata).as_ref() == Some(&before.full))
            })
        }) && self
            .canonical
            .iter()
            .all(|(original, resolved)| original.canonicalize().ok().as_ref() == Some(resolved));
        if !valid {
            state.failed = true;
            return false;
        }
        if let Some(observer) = &mut state.native
            && !observer.unchanged()
        {
            state.failed = true;
            return false;
        }
        true
    }
}

fn walk(
    original: &Path,
    mut visit: impl FnMut(&Path, bool) -> Option<Metadata>,
) -> Option<PathBuf> {
    if !original.is_absolute() {
        return None;
    }
    let mut current = PathBuf::from("/");
    let mut pending: VecDeque<_> = original
        .components()
        .map(|part| part.as_os_str().to_owned())
        .collect();
    let mut links = 0usize;
    visit(&current, false)?;
    while let Some(part) = pending.pop_front() {
        match Path::new(&part).components().next()? {
            Component::RootDir => current = PathBuf::from("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                current.pop();
                if current.as_os_str().is_empty() {
                    current.push("/");
                }
            }
            Component::Normal(name) => {
                let path = current.join(name);
                let metadata = visit(&path, pending.is_empty())?;
                if metadata.file_type().is_symlink() {
                    links = links.checked_add(1)?;
                    if links > MAX_LINKS {
                        return None;
                    }
                    let before = stamp(&metadata)?;
                    let target = std::fs::read_link(&path).ok()?;
                    target.to_str()?;
                    if stamp(&std::fs::symlink_metadata(&path).ok()?)?.ne(&before) {
                        return None;
                    }
                    let mut expanded: VecDeque<_> = target
                        .components()
                        .map(|part| part.as_os_str().to_owned())
                        .collect();
                    expanded.append(&mut pending);
                    pending = expanded;
                } else {
                    if !metadata.is_dir() && !pending.is_empty() {
                        return None;
                    }
                    current = path;
                }
            }
            Component::Prefix(_) => return None,
        }
    }
    Some(current)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{Identity, identity, open_input};
    use kqueue::{EventData, EventFilter, FilterFlag, Vnode, Watcher};
    use std::fs::File;
    use std::path::Path;

    pub(super) struct Observer {
        // Queue drops before its borrowed descriptors, per kqueue's contract.
        watcher: Watcher,
        files: Vec<File>,
    }
    impl Observer {
        pub(super) fn new() -> Option<Self> {
            let mut watcher = Watcher::new().ok()?;
            // XNU EVFILT_FS ignores ident and latches global VQ_MOUNT (0x8)
            // and VQ_UNMOUNT (0x10). Overmounting need not rename a vnode.
            // kqueue 1.2.1 reports this filter as EventData::Error, which is
            // intentionally a refusal in unchanged(), never a dropped event.
            watcher
                .add_fd(
                    0,
                    EventFilter::EVFILT_FS,
                    FilterFlag::from_bits_retain(0x18),
                )
                .ok()?;
            Some(Self {
                watcher,
                files: Vec::new(),
            })
        }
        pub(super) fn add(&mut self, path: &Path, expected: &Identity) -> Option<()> {
            let file = open_input(path)?;
            if identity(&file.metadata().ok()?).as_ref() != Some(expected) {
                return None;
            }
            let mut flags =
                FilterFlag::NOTE_RENAME | FilterFlag::NOTE_DELETE | FilterFlag::NOTE_REVOKE;
            // Executing a regular Mach-O file advances its access time and
            // delivers NOTE_ATTRIB without changing any guarded input. Full
            // regular-file stamps bind ctime, permissions and identity instead;
            // rename/delete/revoke remain latched. Ancestor directories need
            // ATTRIB because transient permission changes can affect lookup.
            if file.metadata().ok()?.is_dir() {
                flags |= FilterFlag::NOTE_ATTRIB;
            }
            self.watcher
                .add_file(&file, EventFilter::EVFILT_VNODE, flags)
                .ok()?;
            self.files.push(file);
            Some(())
        }
        pub(super) fn start(&mut self) -> Option<()> {
            self.watcher.watch().ok()
        }
        pub(super) fn unchanged(&mut self) -> bool {
            // Finite coalesced vnode events, bounded by the watched path cap.
            for _ in 0..super::MAX_PATHS * 8 {
                let Some(event) = self.watcher.poll(None) else {
                    return true;
                };
                if !matches!(
                    event.data,
                    EventData::Vnode(Vnode::Write | Vnode::Extend | Vnode::Link)
                ) {
                    return false;
                }
            }
            false
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Identity, identity, open_input};
    use inotify::{Inotify, WatchMask};
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use std::fs::File;
    use std::os::fd::AsRawFd as _;
    use std::path::Path;
    pub(super) struct Observer {
        queue: Inotify,
        mounts: File,
        files: Vec<File>,
    }
    impl Observer {
        pub(super) fn new() -> Option<Self> {
            let mounts = super::open_nonblocking(Path::new("/proc/self/mounts"))?;
            if rustix::fs::fstatfs(&mounts).ok()?.f_type != rustix::fs::PROC_SUPER_MAGIC {
                return None;
            }
            Some(Self {
                queue: Inotify::init().ok()?,
                mounts,
                files: Vec::new(),
            })
        }
        pub(super) fn add(&mut self, path: &Path, expected: &Identity) -> Option<()> {
            let file = open_input(path)?;
            if identity(&file.metadata().ok()?).as_ref() != Some(expected) {
                return None;
            }
            // Follow this held descriptor, not a second lookup of the original
            // pathname which could briefly name a different inode. Missing
            // procfs or unsupported watching refuses registration.
            self.queue
                .watches()
                .add(
                    format!("/proc/self/fd/{}", file.as_raw_fd()),
                    WatchMask::MOVE_SELF | WatchMask::DELETE_SELF | WatchMask::ATTRIB,
                )
                .ok()?;
            self.files.push(file);
            Some(())
        }
        #[allow(
            clippy::unnecessary_wraps,
            reason = "Linux registers immediately; the shared API also covers fallible kqueue activation."
        )]
        pub(super) fn start(&mut self) -> Option<()> {
            Some(())
        }
        pub(super) fn unchanged(&mut self) -> bool {
            // A namespace mount generation change reports PRI/ERR even when
            // mounts were restored before this poll. Never read/reset the
            // descriptor between brackets; the outer State latches refusal.
            let mut mount_events = [PollFd::new(&self.mounts, PollFlags::PRI)];
            if poll(&mut mount_events, Some(&Timespec::default())) != Ok(0) {
                return false;
            }
            let mut bytes = [0; 4096];
            loop {
                match self.queue.read_events(&mut bytes) {
                    Ok(mut events) => return events.next().is_none(),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return true,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => return false,
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use std::path::Path;
    pub(super) struct Observer;
    impl Observer {
        pub(super) fn new() -> Option<Self> {
            None
        }
        pub(super) fn add(&mut self, _: &Path, _: &super::Identity) -> Option<()> {
            None
        }
        pub(super) fn start(&mut self) -> Option<()> {
            None
        }
        pub(super) fn unchanged(&mut self) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn executing_observed_regular_file_keeps_input_guard_current() -> Result<(), Box<dyn Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-resolution-execute-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let executable = root.join("echo");
        let source = root.join("echo.rs");
        std::fs::write(&source, "fn main() { println!(\"observed execution\"); }")?;
        let compiled = std::process::Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()?;
        assert!(compiled.status.success());
        let result = (|| {
            let resolution =
                Resolution::capture(std::slice::from_ref(&executable)).ok_or("capture")?;
            let before = stamp(&std::fs::metadata(&executable)?).ok_or("stamp")?;
            let output = std::process::Command::new(&executable)
                .arg("observed execution")
                .output()?;
            assert!(output.status.success());
            assert_eq!(output.stdout, b"observed execution\n");
            assert!(stamp(&std::fs::metadata(&executable)?).as_ref() == Some(&before));
            assert!(resolution.unchanged());
            assert_eq!(
                resolution.covered(&executable),
                Some(executable.canonicalize()?)
            );
            Ok::<_, Box<dyn Error>>(())
        })();
        std::fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn regular_content_and_permission_aba_refuse_every_read_path() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt as _;
        let root =
            std::env::temp_dir().join(format!("sail-resolution-regular-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let file = root.join("input");
        std::fs::write(&file, "correct")?;
        let result = (|| {
            for change_permission in [false, true] {
                let resolution =
                    Resolution::capture(std::slice::from_ref(&file)).ok_or("capture")?;
                if change_permission {
                    let permissions = std::fs::metadata(&file)?.permissions();
                    std::fs::set_permissions(
                        &file,
                        std::fs::Permissions::from_mode(permissions.mode() ^ 0o100),
                    )?;
                    std::fs::set_permissions(&file, permissions)?;
                } else {
                    std::fs::write(&file, "bad input")?;
                    assert_eq!(std::fs::read_to_string(&file)?, "bad input");
                    std::fs::write(&file, "correct")?;
                }
                assert!(resolution.covered(&file).is_none());
                assert!(!resolution.unchanged());
                assert!(!resolution.unchanged());
            }
            Ok::<_, Box<dyn Error>>(())
        })();
        std::fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn observed_regular_file_replacement_aba_is_latched() -> Result<(), Box<dyn Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-resolution-file-aba-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let file = root.join("input");
        let saved = root.join("saved");
        let bad = root.join("bad");
        std::fs::write(&file, "correct")?;
        std::fs::write(&bad, "bad input")?;
        let result = (|| {
            let resolution = Resolution::capture(std::slice::from_ref(&file)).ok_or("capture")?;
            std::fs::rename(&file, &saved)?;
            std::fs::rename(&bad, &file)?;
            assert_eq!(std::fs::read_to_string(&file)?, "bad input");
            std::fs::rename(&file, &bad)?;
            std::fs::rename(&saved, &file)?;
            assert!(!resolution.unchanged());
            assert!(!resolution.unchanged());
            Ok::<_, Box<dyn Error>>(())
        })();
        std::fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn filesystem_admission_refuses_unknown_network_and_pseudo_types() {
        for kind in [
            0xef53_u64,
            0x5846_5342,
            0x9123_683e,
            0x0102_1994,
            0x794c_7630,
        ] {
            assert!(admitted_linux_type(kind));
        }
        for kind in [0_u64, 0x6969, 0x6573_5546, 0x9fa0, 0x6265_6572, u64::MAX] {
            assert!(!admitted_linux_type(kind));
        }
        assert!(admitted_macos_type(b"apfs", true));
        assert!(admitted_macos_type(b"hfs", true));
        for name in [b"nfs".as_slice(), b"smbfs", b"macfuse", b"unknown"] {
            assert!(!admitted_macos_type(name, true));
        }
        assert!(!admitted_macos_type(b"apfs", false));
    }

    #[cfg(unix)]
    #[test]
    fn fifo_or_last_component_symlink_refuses_without_waiting() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-dependency-resolution-special-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let fifo = root.join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()?
                .success()
        );
        let regular = root.join("regular");
        std::fs::write(&regular, "regular input")?;
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&regular, &alias)?;
        assert!(open_input(&regular).is_some());
        assert!(
            open_input(&alias).is_none(),
            "last-component symlink cannot be followed"
        );
        let expected = identity(&std::fs::metadata(&regular)?).ok_or("regular identity")?;
        let mut observer = platform::Observer::new().ok_or("native observer")?;
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let refused = open_input(&fifo).is_none() && observer.add(&fifo, &expected).is_none();
            let _ = send.send(refused);
        });
        let refused = receive.recv_timeout(std::time::Duration::from_secs(2));
        std::fs::remove_dir_all(root)?;
        assert_eq!(
            refused.ok(),
            Some(true),
            "FIFO open must refuse without a peer"
        );
        assert!(worker.join().is_ok());
        Ok(())
    }

    #[test]
    fn watch_registration_refuses_an_unexpected_opened_inode() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-dependency-resolution-open-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let first = root.join("first");
        let second = root.join("second");
        std::fs::write(&first, "first inode")?;
        std::fs::write(&second, "second inode")?;
        let result = (|| {
            let expected = identity(&std::fs::metadata(first)?).ok_or("identity")?;
            let mut observer = platform::Observer::new().ok_or("native observer")?;
            assert!(observer.add(&second, &expected).is_none());
            Ok::<_, Box<dyn Error>>(())
        })();
        std::fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn unavailable_native_observation_refuses() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-dependency-resolution-unavailable-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let file = root.join("input");
        std::fs::write(&file, "source")?;
        let result = (|| {
            let mut resolution = Resolution::capture(&[file]).ok_or("capture")?;
            resolution
                .observer
                .get_mut()
                .map_err(|_| "poisoned observer")?
                .native = None;
            assert!(
                !resolution.unchanged(),
                "unavailable observation must refuse"
            );
            assert!(!resolution.unchanged());
            Ok::<_, Box<dyn Error>>(())
        })();
        std::fs::remove_dir_all(root)?;
        result
    }
}
