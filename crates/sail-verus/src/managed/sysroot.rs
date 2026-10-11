//! Run-local sysroot digests protected before their first content read. Namespace
//! observation and kernel leases remain owned until the final receipt seal.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use super::{lease::ReadLease, precise::Observer};

#[derive(PartialEq, Eq)]
struct Inventory {
    paths: Vec<PathBuf>,
    complete: bool,
}

struct Enumeration {
    visits: usize,
    bytes: usize,
    failed: bool,
    #[cfg(target_os = "linux")]
    cancelled: Option<Cancellation>,
}
impl Enumeration {
    fn visit(&mut self) -> bool {
        self.visits += 1;
        #[cfg(target_os = "linux")]
        {
            self.failed |= self.cancelled.as_ref().is_some_and(Cancellation::cancelled);
        }
        self.failed |= self.visits > 16384;
        !self.failed
    }
    fn insert(&mut self, path: &PathBuf, count: usize) -> bool {
        self.bytes = self.bytes.saturating_add(path.capacity());
        self.failed |= count >= 4096
            || self.bytes > 512 * 1024
            || path.as_os_str().len() > 4096
            || path.capacity() > 8192
            || path.to_str().is_none();
        !self.failed
    }
}
fn file(
    mut path: PathBuf,
    files: &mut BTreeSet<PathBuf>,
    complete: &mut bool,
    limit: &mut Option<Enumeration>,
) {
    if limit.is_some() {
        path.shrink_to_fit();
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            if !files.contains(&path)
                && limit
                    .as_mut()
                    .is_some_and(|limit| !limit.insert(&path, files.len()))
            {
                *complete = false;
                return;
            }
            files.insert(path);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => *complete = false,
    }
}

fn directory(
    path: &Path,
    files: &mut BTreeSet<PathBuf>,
    complete: &mut bool,
    limit: &mut Option<Enumeration>,
) {
    match std::fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                if limit.as_mut().is_some_and(|limit| !limit.visit()) {
                    *complete = false;
                    break;
                }
                match entry {
                    Ok(entry) => file(entry.path(), files, complete, limit),
                    Err(_) => *complete = false,
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => *complete = false,
    }
}

#[cfg(test)]
fn inventory(sysroots: &BTreeSet<PathBuf>) -> Inventory {
    inventory_with(sysroots, false)
}
#[cfg(test)]
fn inventory_with(sysroots: &BTreeSet<PathBuf>, bounded: bool) -> Inventory {
    inventory_controlled(
        sysroots,
        bounded,
        #[cfg(target_os = "linux")]
        None,
    )
}
fn inventory_controlled(
    sysroots: &BTreeSet<PathBuf>,
    bounded: bool,
    #[cfg(target_os = "linux")] cancelled: Option<&Cancellation>,
) -> Inventory {
    let mut limit = bounded.then_some(Enumeration {
        visits: 0,
        bytes: 0,
        failed: false,
        #[cfg(target_os = "linux")]
        cancelled: cancelled.cloned(),
    });
    let mut files = BTreeSet::new();
    let mut complete = true;
    for sysroot in sysroots {
        for name in ["rustc", "cargo"] {
            file(
                sysroot.join("bin").join(name),
                &mut files,
                &mut complete,
                &mut limit,
            );
        }
        directory(&sysroot.join("lib"), &mut files, &mut complete, &mut limit);
        match std::fs::read_dir(sysroot.join("lib/rustlib")) {
            Ok(entries) => {
                for entry in entries {
                    if limit.as_mut().is_some_and(|limit| !limit.visit()) {
                        complete = false;
                        break;
                    }
                    match entry {
                        Ok(entry) => {
                            let library = entry.path().join("lib");
                            match std::fs::metadata(&library) {
                                Ok(metadata) if metadata.is_dir() => {
                                    directory(&library, &mut files, &mut complete, &mut limit);
                                }
                                Ok(_) => {}
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        std::io::ErrorKind::NotFound
                                            | std::io::ErrorKind::NotADirectory
                                    ) => {}
                                Err(_) => complete = false,
                            }
                        }
                        Err(_) => complete = false,
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => complete = false,
        }
    }
    Inventory {
        paths: files.into_iter().collect(),
        complete,
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct Cancellation {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    started: std::time::Instant,
}
#[cfg(target_os = "linux")]
impl Cancellation {
    fn cancelled(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Acquire)
            || self.started.elapsed() >= std::time::Duration::from_secs(120)
    }
}

#[cfg(target_os = "linux")]
struct Protection {
    namespace: Observer,
    files: Observer,
    lease: ReadLease,
}

pub(super) struct Baseline {
    paths: Vec<PathBuf>,
    eligible: BTreeSet<PathBuf>,
    captured: BTreeMap<PathBuf, String>,
    failure: Option<&'static str>,
    failure_detail: Option<(&'static str, &'static str)>,
    #[cfg(target_os = "linux")]
    protection: Option<Protection>,
}

impl Baseline {
    pub(super) fn begin(
        sysroots: &BTreeSet<PathBuf>,
        already_hashed: impl Fn(&Path) -> bool,
    ) -> Self {
        Self::begin_with(
            sysroots,
            already_hashed,
            #[cfg(target_os = "linux")]
            None,
        )
    }
    #[cfg(target_os = "linux")]
    pub(super) fn begin_bounded(
        sysroots: &BTreeSet<PathBuf>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        started: std::time::Instant,
    ) -> Option<Self> {
        if sysroots.is_empty()
            || sysroots.len() > 8
            || sysroots.iter().any(|path| {
                !path.is_absolute() || path.as_os_str().len() > 4096 || path.to_str().is_none()
            })
        {
            return None;
        }
        let cancellation = Cancellation { stop, started };
        let mut owner = Self::begin_with(sysroots, |_| false, Some(&cancellation));
        (owner.complete()
            && !cancellation.cancelled()
            && owner.retained_bytes()? <= 16 * 1024 * 1024)
            .then_some(owner)
    }
    fn begin_with(
        sysroots: &BTreeSet<PathBuf>,
        already_hashed: impl Fn(&Path) -> bool,
        #[cfg(target_os = "linux")] bounded: Option<&Cancellation>,
    ) -> Self {
        #[cfg(target_os = "linux")]
        let observe = |roots: &[PathBuf], excluded: &[PathBuf]| {
            if let Some(bounded) = bounded {
                Observer::begin_bounded_with(
                    roots,
                    excluded,
                    std::sync::Arc::clone(&bounded.stop),
                    bounded.started,
                )
            } else {
                Observer::begin(roots, excluded)
            }
        };
        #[cfg(target_os = "linux")]
        let namespace = observe(
            &sysroots
                .iter()
                .flat_map(|root| {
                    [
                        root.join("lib"),
                        root.join("lib/rustlib"),
                        root.join("bin/rustc"),
                        root.join("bin/cargo"),
                    ]
                })
                .collect::<Vec<_>>(),
            &[],
        );
        // The namespace watcher is installed before enumeration, including the
        // parent of absent directories and every existing target directory.
        let found = inventory_controlled(
            sysroots,
            #[cfg(target_os = "linux")]
            bounded.is_some(),
            #[cfg(not(target_os = "linux"))]
            false,
            #[cfg(target_os = "linux")]
            bounded,
        );
        let baseline = Self {
            eligible: found
                .paths
                .iter()
                .filter(|path| !already_hashed(path))
                .cloned()
                .collect(),
            paths: found.paths.clone(),
            captured: BTreeMap::new(),
            failure: (!found.complete).then_some("sysroot-inventory-incomplete"),
            failure_detail: None,
            #[cfg(target_os = "linux")]
            protection: None,
        };
        #[cfg(target_os = "linux")]
        {
            let mut baseline = baseline;
            if baseline.paths.iter().any(|path| path.to_str().is_none()) {
                baseline.failure = Some("sysroot-runtime-identity-path-unrepresentable");
            }
            baseline.protection = (|| {
                if baseline.failure.is_some() {
                    return None;
                }
                let namespace = namespace?;
                // Explicit file roots cover symlinks and reserved-looking
                // directory names which the general walk need not descend.
                let files = observe(&found.paths, &[])?;
                let lease = if let Some(bounded) = bounded {
                    ReadLease::acquire_bounded(
                        &found.paths,
                        &|| bounded.cancelled(),
                        std::sync::Arc::clone(&bounded.stop),
                        bounded.started,
                    )?
                } else {
                    ReadLease::acquire(&found.paths)?
                };
                (inventory_controlled(sysroots, bounded.is_some(), bounded) == found).then_some(
                    Protection {
                        namespace,
                        files,
                        lease,
                    },
                )
            })();
            if baseline.protection.is_none() && baseline.failure.is_none() {
                baseline.failure = Some("sysroot-kernel-coverage-unavailable");
            }
            let _ = baseline.clean();
            baseline
        }
        #[cfg(not(target_os = "linux"))]
        {
            baseline
        }
    }

    #[allow(dead_code, reason = "Used by bounded daemon sysroot admission")]
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let Self {
            paths,
            eligible,
            captured,
            failure: _,
            failure_detail: _,
            #[cfg(target_os = "linux")]
            protection,
        } = self;
        let used = std::mem::size_of::<Self>()
            .checked_add(
                paths
                    .capacity()
                    .checked_mul(std::mem::size_of::<PathBuf>())?,
            )?
            .checked_add(eligible.len().checked_add(1)?.checked_mul(1024)?)?
            .checked_add(captured.len().checked_add(1)?.checked_mul(1024)?)?;
        let used = paths
            .iter()
            .chain(eligible)
            .try_fold(used, |used, path| used.checked_add(path.capacity()))?;
        let used = captured.iter().try_fold(used, |used, (path, hash)| {
            used.checked_add(path.capacity())?
                .checked_add(hash.capacity())
        })?;
        #[cfg(target_os = "linux")]
        {
            let used = if let Some(Protection {
                namespace,
                files,
                lease,
            }) = protection
            {
                used.checked_add(namespace.retained_bytes()?)?
                    .checked_add(files.retained_bytes()?)?
                    .checked_add(lease.retained_bytes()?)?
            } else {
                used
            };
            Some(used)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Some(used)
        }
    }
    pub(super) fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub(super) fn capture(&mut self, path: &Path, hash: &str) {
        if self.failure.is_none() && self.eligible.contains(path) {
            self.captured.insert(path.to_owned(), hash.to_owned());
        }
    }

    pub(super) fn captured(&self, path: &Path) -> Option<&String> {
        self.captured.get(path)
    }

    #[cfg(target_os = "linux")]
    #[allow(
        dead_code,
        reason = "Runner activation follows independent handoff qualification"
    )]
    pub(super) fn import(
        &mut self,
        scope: &sail_api::sysroot::Scope,
        snapshot: &sail_api::sysroot::Snapshot,
    ) -> bool {
        // This caller acquired its original protection BEFORE the RPC. A wire
        // shape or generation alone cannot prove a complete inventory or grant
        // authority to pre-guard executable/engine aliases.
        if !self.captured.is_empty()
            || !snapshot.valid(scope)
            || snapshot.rows.len() != self.paths.len()
            || !super::sysroot_cache::actual(scope)
            || !self.clean()
        {
            return false;
        }
        for (path, row) in self.paths.iter().zip(&snapshot.rows) {
            let Some(binding) = super::sysroot_cache::binding(path) else {
                return false;
            };
            if row.path != *path
                || row.canonical != binding.canonical
                || row.device != binding.device
                || row.inode != binding.inode
            {
                return false;
            }
        }
        if !super::sysroot_cache::actual(scope) || !self.clean() {
            return false;
        }
        for row in &snapshot.rows {
            self.capture(&row.path, &row.sha256);
        }
        if !super::sysroot_cache::actual(scope) || !self.clean() {
            self.captured.clear();
            return false;
        }
        true
    }

    pub(super) fn clean(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            if self.failure.is_some() {
                return false;
            }
            let Some(protection) = self.protection.as_mut() else {
                return false;
            };
            if !protection.namespace.clean() {
                self.failure = Some("sysroot-namespace-invalidated");
                self.failure_detail = Some(("namespace", protection.namespace.failure_reason()));
                return false;
            }
            if !protection.files.clean() {
                self.failure = Some("sysroot-namespace-invalidated");
                self.failure_detail = Some(("explicit-files", protection.files.failure_reason()));
                return false;
            }
            if !protection.lease.clean() {
                self.failure = Some(protection.lease.failure_reason());
                return false;
            }
            true
        }
        #[cfg(not(target_os = "linux"))]
        {
            // Unsupported platforms never retain a reusable digest baseline.
            self.captured.clear();
            false
        }
    }

    pub(super) fn complete(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.clean()
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.failure.is_none()
        }
    }

    pub(super) fn failure_reason(&self) -> &'static str {
        self.failure
            .unwrap_or("sysroot-kernel-coverage-unavailable")
    }

    // Read only static labels saved by the original observation barriers.
    // No extra native drain, metadata read, lease check or hashing is performed.
    pub(super) fn failure_description(&self) -> String {
        let reason = self.failure_reason();
        match self.failure_detail {
            Some((barrier, detail)) => format!("{reason}; barrier={barrier}; detail={detail}"),
            None => reason.to_owned(),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    struct Mapper(std::process::Child);

    impl Drop for Mapper {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn fixture() -> std::io::Result<PathBuf> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from("/tmp").join(format!(
            "sail-sysroot-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("lib/rustlib/host/lib"))?;
        std::fs::create_dir_all(root.join("bin"))?;
        for name in [
            "bin/rustc",
            "bin/cargo",
            "lib/compiler.so",
            "lib/rustlib/host/lib/std.rlib",
        ] {
            std::fs::write(root.join(name), b"good")?;
        }
        Ok(root)
    }

    fn baseline(root: &Path) -> Baseline {
        Baseline::begin(&BTreeSet::from([root.to_owned()]), |_| false)
    }

    #[test]
    fn new_targets_and_restored_namespace_changes_break_owned_coverage() -> TestResult {
        let root = fixture()?;
        let mut owner = baseline(&root);
        assert!(owner.complete(), "fixture did not acquire kernel coverage");
        let library = root.join("lib/compiler.so");
        let moved = root.join("lib/moved.so");
        std::fs::rename(&library, &moved)?;
        std::fs::rename(&moved, &library)?;
        assert!(!owner.complete());
        assert_eq!(owner.failure_reason(), "sysroot-namespace-invalidated");
        let description = owner.failure_description();
        assert!(description.contains("barrier=namespace; detail=namespace-"));
        assert!(!owner.complete());
        assert_eq!(
            owner.failure_description(),
            description,
            "first failure stays sticky"
        );
        drop(owner);
        let mut owner = baseline(&root);
        assert!(owner.complete());
        std::fs::create_dir_all(root.join("lib/rustlib/new-target/lib"))?;
        std::fs::write(root.join("lib/rustlib/new-target/lib/std.rlib"), b"new")?;
        assert!(
            !owner.complete(),
            "new installed target was silently omitted"
        );
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn explicit_file_failure_retains_its_first_observer_detail() -> TestResult {
        let root = fixture()?;
        let disjoint = root.join("disjoint");
        std::fs::write(&disjoint, b"unchanged")?;
        let mut owner = baseline(&root);
        assert!(owner.complete(), "fixture did not acquire kernel coverage");
        // Isolate the second real observer in this control. Production roots
        // and acquisition remain unchanged; no observer result is fabricated.
        owner
            .protection
            .as_mut()
            .ok_or("fixture protection unavailable")?
            .namespace = Observer::begin(std::slice::from_ref(&disjoint), &[])
            .ok_or("disjoint native observation unavailable")?;
        assert!(owner.complete());
        std::fs::write(root.join("lib/compiler.so"), b"changed")?;
        assert!(!owner.complete());
        assert_eq!(owner.failure_reason(), "sysroot-namespace-invalidated");
        assert_eq!(
            owner.failure_description(),
            "sysroot-namespace-invalidated; barrier=explicit-files; detail=namespace-tracked-input-event"
        );
        assert!(!owner.complete());
        assert_eq!(
            owner.failure_description(),
            "sysroot-namespace-invalidated; barrier=explicit-files; detail=namespace-tracked-input-event",
            "first failed observer detail must remain sticky"
        );
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn absent_inventory_and_symlink_retargeting_break_owned_coverage() -> TestResult {
        use std::os::unix::fs::symlink;
        let root = fixture()?;
        std::fs::remove_dir_all(root.join("lib/rustlib"))?;
        let mut owner = baseline(&root);
        assert!(owner.complete());
        std::fs::create_dir_all(root.join("lib/rustlib/new/lib"))?;
        std::fs::write(root.join("lib/rustlib/new/lib/std.rlib"), b"good")?;
        assert!(!owner.complete(), "initially absent inventory was reused");
        drop(owner);
        let first = root.join("lib/rustlib/new/lib");
        let second = root.join("alternate");
        std::fs::create_dir(&second)?;
        std::fs::write(second.join("std.rlib"), b"good")?;
        let link = root.join("lib/rustlib/alias");
        symlink(root.join("lib/rustlib/new"), &link)?;
        let mut owner = baseline(&root);
        assert!(owner.complete());
        std::fs::remove_file(&link)?;
        symlink(&second, &link)?;
        assert!(!owner.complete(), "target symlink retarget was reused");
        drop(owner);
        std::fs::remove_file(&link)?;
        symlink(&first, &link)?;
        let mut owner = baseline(&root);
        assert!(owner.complete());
        let moved = root.with_extension("moved");
        std::fs::rename(&root, &moved)?;
        std::fs::rename(&moved, &root)?;
        assert!(!owner.complete(), "ancestor replacement was reused");
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn previously_hashed_rustc_is_ineligible_for_digest_reuse() -> TestResult {
        use std::os::unix::fs::symlink;
        let root = fixture()?;
        let rustc = root.join("bin/rustc");
        let alias = root.join("bin/prior-compiler");
        symlink(&rustc, &alias)?;
        let mut hashes = super::super::files::FileDigests::default();
        let prior = hashes.hash(&alias)?;
        let mut owner = Baseline::begin(&BTreeSet::from([root.clone()]), |path| {
            hashes.contains(path)
        });
        assert!(owner.complete());
        let cargo = root.join("bin/cargo");
        owner.capture(&rustc, &prior);
        owner.capture(&cargo, "read-under-guard");
        assert_eq!(owner.captured(&rustc), None);
        assert_eq!(
            owner.captured(&cargo).map(String::as_str),
            Some("read-under-guard")
        );
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn incomplete_directory_enumeration_disables_reuse() -> TestResult {
        let root = fixture()?;
        std::fs::remove_dir_all(root.join("lib/rustlib"))?;
        std::fs::write(root.join("lib/rustlib"), b"not a directory")?;
        let mut owner = baseline(&root);
        assert!(!owner.complete());
        assert_eq!(owner.failure_reason(), "sysroot-inventory-incomplete");
        assert_eq!(owner.failure_description(), "sysroot-inventory-incomplete");
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn restored_writes_and_helper_loss_never_restore_a_baseline() -> TestResult {
        let root = fixture()?;
        let library = root.join("lib/compiler.so");
        let original = std::fs::metadata(&library)?.modified()?;
        let mut owner = baseline(&root);
        assert!(owner.complete());
        std::fs::write(&library, b"fail")?;
        std::fs::write(&library, b"good")?;
        std::fs::File::open(&library)?
            .set_times(std::fs::FileTimes::new().set_modified(original))?;
        assert!(!owner.complete(), "restored bytes/mtime were accepted");
        assert!(!owner.complete(), "invalidated baseline recovered");
        drop(owner);
        let mut owner = baseline(&root);
        assert!(owner.complete());
        owner
            .protection
            .as_mut()
            .ok_or("missing fixture owner")?
            .lease
            .stop_owner_for_test()?;
        assert!(!owner.complete(), "dead owner authorized hash reuse");
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn preexisting_writable_mapping_and_non_utf8_identity_disable_reuse() -> TestResult {
        use std::os::unix::ffi::OsStringExt as _;
        let root = fixture()?;
        let library = root.join("lib/compiler.so");
        let ready = root.join("mapper-ready");
        let mut mapper = Mapper(Command::new("python3").args(["-I", "-u", "-c",
            "import mmap,os,sys; f=os.open(sys.argv[1],os.O_RDWR); m=mmap.mmap(f,0); os.close(f); open(sys.argv[2],'wb').close(); sys.stdin.readline(); m[:]=b'fail'; m.flush(); m[:]=b'good'; m.flush(); m.close()"])
            .arg(&library).arg(&ready).stdin(Stdio::piped()).stdout(Stdio::null()).spawn()?);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !ready.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            ready.exists(),
            "mapping fixture did not start within two seconds"
        );
        let mut owner = baseline(&root);
        assert!(
            !owner.complete(),
            "preexisting writable mapping was trusted"
        );
        mapper
            .0
            .stdin
            .as_mut()
            .ok_or("missing fixture stdin")?
            .write_all(b"run\n")?;
        assert!(mapper.0.wait()?.success());
        assert!(!owner.complete(), "closed mapper revived invalid owner");
        drop(owner);
        let odd = root
            .join("lib")
            .join(std::ffi::OsString::from_vec(b"nonutf-\xff.rlib".to_vec()));
        std::fs::write(odd, b"good")?;
        let mut owner = baseline(&root);
        assert!(!owner.complete());
        assert_eq!(
            owner.failure_reason(),
            "sysroot-runtime-identity-path-unrepresentable"
        );
        drop(owner);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::{Enumeration, inventory, inventory_with};
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn optional_inventory_cap_never_truncates_original_context_inventory() -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "sail-sysroot-inventory-bound-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("lib"))?;
        for file in 0..4097 {
            std::fs::write(root.join("lib").join(format!("{file:04}.rlib")), [])?;
        }
        let roots = BTreeSet::from([root.clone()]);
        let original = inventory(&roots);
        let bounded = inventory_with(&roots, true);
        assert!(original.complete);
        assert_eq!(original.paths.len(), 4097);
        assert!(!bounded.complete);
        assert!(bounded.paths.len() <= 4096);
        assert_eq!(original.paths, inventory_with(&roots, false).paths);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn path_bytes_and_directory_visits_refuse_before_growth() {
        let mut budget = Enumeration {
            visits: 16384,
            bytes: 512 * 1024,
            failed: false,
            #[cfg(target_os = "linux")]
            cancelled: None,
        };
        assert!(!budget.visit());
        assert!(!budget.insert(&PathBuf::from("/extra"), 1));
        assert!(budget.failed);
    }
}
