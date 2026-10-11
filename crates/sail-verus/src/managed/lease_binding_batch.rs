//! Unselected per-barrier parent descriptor grouping candidate.
//!
//! Descriptors never survive a barrier. Every child retains the exact original
//! device/inode and proc pathname predicate, and every reused parent is freshly
//! resolved again before success. Unavailable grouping uses the original full
//! inventory predicate; no cache or directory metadata authorizes file bytes.

use super::{ProcBindings, current_binding_matches};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

type Binding = (PathBuf, PathBuf, u64, u64);
const MAX_PARENTS: usize = 64;

thread_local! {
    static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn enabled() -> bool {
    ENABLED.with(std::cell::Cell::get)
}
fn with_grouping<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(self.0));
        }
    }
    let _restore = Restore(ENABLED.with(|enabled| enabled.replace(true)));
    f()
}

struct Parent<'a> {
    path: &'a Path,
    descriptor: OwnedFd,
    device: u64,
    inode: u64,
}
impl<'a> Parent<'a> {
    fn open(path: &'a Path) -> Option<Self> {
        let descriptor = open_parent(path)?;
        let metadata = rustix::fs::fstat(&descriptor).ok()?;
        Some(Self {
            path,
            descriptor,
            device: metadata.st_dev,
            inode: metadata.st_ino,
        })
    }
    fn still_bound(&self) -> bool {
        open_parent(self.path)
            .and_then(|descriptor| rustix::fs::fstat(&descriptor).ok())
            .is_some_and(|metadata| metadata.st_dev == self.device && metadata.st_ino == self.inode)
    }
}
fn open_parent(path: &Path) -> Option<OwnedFd> {
    use rustix::fs::{Mode, OFlags, open};
    open(
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()
}

pub(super) fn matches(descriptors: &ProcBindings, bindings: &[Binding]) -> bool {
    matches_with(descriptors, bindings, |_| {})
}

/// The hook exists only in this unselected test module, at the precise point
/// where a reused parent would otherwise hide a changed lexical binding.
fn matches_with(
    descriptors: &ProcBindings,
    bindings: &[Binding],
    mut after_child: impl FnMut(usize),
) -> bool {
    use rustix::fs::{Mode, OFlags, openat};
    let original = || {
        bindings.iter().all(|(path, canonical, device, inode)| {
            current_binding_matches(Some(descriptors), path, canonical, *device, *inode)
        })
    };
    // Fixed count, borrowed paths: no path strings or unbounded map growth.
    // The temporary vector and at most 64 directory FDs die before return.
    let mut parents = Vec::with_capacity(MAX_PARENTS);
    for (index, (path, canonical, device, inode)) in bindings.iter().enumerate() {
        let Some((parent_path, name)) = path
            .parent()
            .zip(path.file_name())
            .filter(|_| path.is_absolute())
        else {
            return original();
        };
        let parent_index = if let Some(index) = parents
            .iter()
            .position(|parent: &Parent<'_>| parent.path == parent_path)
        {
            index
        } else if parents.len() < MAX_PARENTS {
            let Some(parent) = Parent::open(parent_path) else {
                return original();
            };
            parents.push(parent);
            parents.len() - 1
        } else {
            if !current_binding_matches(Some(descriptors), path, canonical, *device, *inode) {
                return false;
            }
            after_child(index);
            continue;
        };
        let Ok(file) = openat(
            &parents[parent_index].descriptor,
            name,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
        ) else {
            return original();
        };
        if !descriptors.descriptor_matches(&file, canonical, *device, *inode) {
            return original();
        }
        after_child(index);
    }
    // A cached parent is not a current path lookup. If any lexical parent
    // changed, redo the complete original inventory, not only that last child.
    // Existing outer namespace/mount and monotone lease barriers are unchanged.
    parents.iter().all(Parent::still_bound) || original()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt as _;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> std::io::Result<Self> {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "sail-binding-batch-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root)?;
            Ok(Self(root))
        }
        fn file(&self, path: &str) -> std::io::Result<PathBuf> {
            let path = self.0.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, b"exact original bytes")?;
            Ok(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn binding(path: &Path) -> std::io::Result<Binding> {
        let metadata = std::fs::metadata(path)?;
        Ok((
            path.to_owned(),
            path.canonicalize()?,
            metadata.dev(),
            metadata.ino(),
        ))
    }
    fn descriptors() -> std::io::Result<ProcBindings> {
        ProcBindings::open().ok_or_else(|| std::io::Error::other("real proc required"))
    }
    #[test]
    fn native_aliases_deleted_suffix_and_parent_overflow_preserve_original_predicate()
    -> std::io::Result<()> {
        use std::os::unix::ffi::OsStringExt as _;
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new()?;
        let file = fixture.file("parent/input")?;
        symlink("parent", fixture.0.join("directory"))?;
        symlink("parent/input", fixture.0.join("alias"))?;
        std::fs::hard_link(&file, fixture.0.join("hardlink"))?;
        let native = fixture
            .0
            .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
        std::fs::write(&native, b"native bytes")?;
        let suffix = fixture.file("literal (deleted)")?;
        let mut bindings = [
            file,
            fixture.0.join("directory/input"),
            fixture.0.join("alias"),
            fixture.0.join("hardlink"),
            native,
            suffix,
        ]
        .iter()
        .map(|path| binding(path))
        .collect::<std::io::Result<Vec<_>>>()?;
        for index in 0..MAX_PARENTS + 2 {
            bindings.push(binding(&fixture.file(&format!("overflow-{index}/input"))?)?);
        }
        let descriptors = descriptors()?;
        assert!(matches(&descriptors, &bindings));
        std::fs::write(fixture.0.join("replacement"), b"same bytes")?;
        std::fs::rename(fixture.0.join("replacement"), &bindings[0].0)?;
        assert!(!matches(&descriptors, &bindings));
        Ok(())
    }
    #[test]
    fn parent_alias_switch_after_child_lookup_cannot_certify_old_directory() -> std::io::Result<()>
    {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new()?;
        let file = fixture.file("old/input")?;
        fixture.file("new/input")?;
        symlink("old", fixture.0.join("selected"))?;
        let bindings = [binding(&fixture.0.join("selected/input"))?, binding(&file)?];
        let descriptors = descriptors()?;
        let mut changed = false;
        assert!(!matches_with(&descriptors, &bindings, |_| {
            if !changed {
                changed = true;
                assert!(std::fs::remove_file(fixture.0.join("selected")).is_ok());
                assert!(symlink("new", fixture.0.join("selected")).is_ok());
            }
        }));
        Ok(())
    }
    #[test]
    fn rename_and_replacement_after_last_child_requires_fresh_absolute_parent()
    -> std::io::Result<()> {
        let fixture = Fixture::new()?;
        let file = fixture.file("selected/input")?;
        let bindings = [binding(&file)?];
        let descriptors = descriptors()?;
        assert!(!matches_with(&descriptors, &bindings, |_| {
            assert!(std::fs::rename(fixture.0.join("selected"), fixture.0.join("retired")).is_ok());
            assert!(fixture.file("selected/input").is_ok());
        }));
        Ok(())
    }
    #[test]
    fn same_inode_rebinding_requires_unchanged_outer_namespace_barrier() -> std::io::Result<()> {
        let fixture = Fixture::new()?;
        let file = fixture.file("selected/input")?;
        let bindings = [binding(&file)?];
        let descriptors = descriptors()?;
        let mut namespace =
            super::super::super::precise::Observer::begin(std::slice::from_ref(&fixture.0), &[])
                .ok_or_else(|| std::io::Error::other("namespace fixture unavailable"))?;
        assert_eq!(
            matches_with(&descriptors, &bindings, |_| {
                assert!(
                    std::fs::rename(fixture.0.join("selected"), fixture.0.join("retired")).is_ok()
                );
                assert!(std::fs::create_dir(fixture.0.join("selected")).is_ok());
                assert!(std::fs::hard_link(fixture.0.join("retired/input"), &file).is_ok());
                // The newly selected hardlink has the same device/inode and exact
                // canonical path, so the original predicate permits it. The
                // namespace/lease barriers remain responsible for the rename.
                assert!(super::super::original_binding_matches(
                    &file,
                    &bindings[0].1,
                    bindings[0].2,
                    bindings[0].3
                ));
            }),
            super::super::original_binding_matches(
                &file,
                &bindings[0].1,
                bindings[0].2,
                bindings[0].3
            )
        );
        assert!(!namespace.clean());
        Ok(())
    }

    #[test]
    #[ignore = "explicit local full-operation profile, not a correctness or acceptance gate"]
    fn complete_validation_profile_preserves_every_original_barrier() -> std::io::Result<()> {
        let fixture = Fixture::new()?;
        let paths = (0..512)
            .map(|index| {
                fixture.file(&format!(
                    "deep/ordinary/toolchain/and/helper/layout/group-{}/file-{index}",
                    index % 8,
                ))
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        let mut owner = super::super::ReadLease::acquire(&paths)
            .ok_or_else(|| std::io::Error::other("full original owner unavailable"))?;
        let mut original_us = Vec::new();
        let mut grouped_us = Vec::new();
        for _ in 0..16 {
            let started = std::time::Instant::now();
            assert!(owner.clean());
            original_us.push(started.elapsed().as_micros());
        }
        for _ in 0..16 {
            let started = std::time::Instant::now();
            assert!(with_grouping(|| owner.clean()));
            grouped_us.push(started.elapsed().as_micros());
        }
        eprintln!(
            "{}",
            serde_json::json!({
                "scope": "unselected complete original ReadLease.clean comparison",
                "input_files": paths.len(), "parent_cap": MAX_PARENTS,
                "original_us": original_us, "grouped_us": grouped_us,
                "live_rpc": false, "acceptance": false,
            })
        );
        std::fs::write(&paths[0], b"changed")?;
        assert!(!with_grouping(|| owner.clean()));
        Ok(())
    }
}
