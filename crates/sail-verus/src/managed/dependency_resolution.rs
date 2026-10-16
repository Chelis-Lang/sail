//! Bracket pathname resolution without confusing sibling output writes with renames.

use std::collections::{BTreeMap, VecDeque};
use std::fs::Metadata;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

const MAX_PATHS: usize = 4096;
const MAX_LINKS: usize = 40;

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
        let mut current = PathBuf::from("/");
        let mut pending: VecDeque<_> = original
            .components()
            .map(|part| part.as_os_str().to_owned())
            .collect();
        let mut links = 0usize;
        self.remember(&current)?;
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
                    let metadata = self.remember(&path)?;
                    if metadata.file_type().is_symlink() {
                        links = links.checked_add(1)?;
                        if links > MAX_LINKS {
                            return None;
                        }
                        let target = std::fs::read_link(&path).ok()?;
                        target.to_str()?;
                        if stamp(&std::fs::symlink_metadata(&path).ok()?)?
                            .ne(&self.paths.get(&path)?.full)
                        {
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

    fn remember(&mut self, path: &Path) -> Option<Metadata> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if !metadata.is_file() && !metadata.is_dir() && !metadata.file_type().is_symlink() {
            return None;
        }
        let full = stamp(&metadata)?;
        if let Some(previous) = self.paths.get(path) {
            if identity(&metadata).as_ref() != Some(&previous.identity)
                || (previous.symlink && previous.full != full)
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
                    && (!before.symlink || stamp(&metadata).as_ref() == Some(&before.full))
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

#[cfg(target_os = "macos")]
mod platform {
    use super::{Identity, identity};
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
            let file = File::open(path).ok()?;
            if identity(&file.metadata().ok()?).as_ref() != Some(expected) {
                return None;
            }
            self.watcher
                .add_file(
                    &file,
                    EventFilter::EVFILT_VNODE,
                    FilterFlag::NOTE_RENAME
                        | FilterFlag::NOTE_DELETE
                        | FilterFlag::NOTE_REVOKE
                        | FilterFlag::NOTE_ATTRIB,
                )
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
    use super::{Identity, identity};
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
            let mounts = File::open("/proc/self/mounts").ok()?;
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
            let file = File::open(path).ok()?;
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
