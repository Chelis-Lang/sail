//! Own the exact native artifacts passed to this vstd compiler invocation.
//!
//! A successful Cargo producer establishes bytes at a declared path. A later
//! compiler must consume those same bytes, even if that original path changes
//! and is restored between admission and publication. These are independent
//! copies, never links to mutable Cargo outputs. The private cache directory is
//! trusted on the same basis as the retained proof records themselves.

use super::{Call, native, sha256_file};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const FILE_LIMIT: u64 = 128 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 256 * 1024 * 1024;

pub(super) struct Snapshot {
    directory: PathBuf,
    files: BTreeMap<PathBuf, (PathBuf, String)>,
    arguments: Vec<OsString>,
}

impl Snapshot {
    pub(super) fn capture(call: &Call<'_>) -> Option<Self> {
        // The admitted builtin has no non-sysroot dependencies. The two
        // admitted macro libraries contain their Rust dependencies, with only
        // platform runtime imports. Do not extend this argument to arbitrary
        // rmeta dependency graphs or additional injected externs.
        let names = call
            .externs
            .iter()
            .map(|(argument, _, _)| native::external(call, argument).map(|(name, _)| name))
            .collect::<Option<BTreeSet<_>>>()?;
        let expected = [
            "verus_builtin",
            "verus_builtin_macros",
            "verus_state_machines_macros",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect();
        if call.externs.len() != 3 || names != expected {
            return None;
        }
        let inputs = call
            .externs
            .iter()
            .map(|(argument, sha, vir)| {
                if vir.is_some() {
                    return None;
                }
                let (_, path) = native::external(call, argument)?;
                Some((path, sha.clone()))
            })
            .collect::<Option<Vec<_>>>()?;
        let parent = call
            .plan
            .target
            .join(".sail-boundary-native-inputs")
            .join(&call.plan.run_id);
        let directory = parent.join(format!("{}-{}", call.key, std::process::id()));
        Self::copy(&directory, &inputs, call.arguments)
    }

    fn copy(directory: &Path, inputs: &[(PathBuf, String)], args: &[OsString]) -> Option<Self> {
        if inputs.is_empty() || inputs.len() > 16 {
            return None;
        }
        let parent = directory.parent()?;
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.recursive(true).create(parent).ok()?;
        builder.recursive(false).create(directory).ok()?;
        let mut captured = Self {
            directory: directory.to_owned(),
            files: BTreeMap::new(),
            arguments: Vec::new(),
        };
        let mut total = 0u64;
        for (original, expected) in inputs {
            let input = open(original)?;
            if !input.metadata().ok()?.is_file() || input.metadata().ok()?.len() > FILE_LIMIT {
                return None;
            }
            let path = directory.join(original.file_name()?);
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            let mut output = options.open(&path).ok()?;
            let copied = std::io::copy(&mut input.take(FILE_LIMIT + 1), &mut output).ok()?;
            drop(output);
            total = total.checked_add(copied)?;
            if copied > FILE_LIMIT
                || total > TOTAL_LIMIT
                || sha256_file(&path)?.0 != *expected
                || captured.files.contains_key(original)
            {
                return None;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).ok()?;
            }
            captured
                .files
                .insert(original.clone(), (path, expected.clone()));
        }
        captured.arguments = captured.rewrite(args)?;
        Some(captured)
    }

    fn rewrite(&self, args: &[OsString]) -> Option<Vec<OsString>> {
        args.iter()
            .enumerate()
            .map(|(index, argument)| {
                let text = argument.to_str()?;
                let (prefix, external) = if let Some(external) = text.strip_prefix("--extern=") {
                    ("--extern=", external)
                } else if index > 0 && args[index - 1] == "--extern" {
                    ("", text)
                } else {
                    return Some(argument.clone());
                };
                if external == "proc_macro" {
                    return Some(argument.clone());
                }
                let (name, original) = external.split_once('=')?;
                let (copy, _) = self.files.get(Path::new(original))?;
                let mut replaced = OsString::from(format!("{prefix}{name}="));
                replaced.push(copy);
                Some(replaced)
            })
            .collect()
    }

    pub(super) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub(super) fn intact(&self) -> bool {
        self.files
            .values()
            .all(|(copy, expected)| sha256_file(copy).is_some_and(|(sha, _)| sha == *expected))
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[cfg(unix)]
fn open(path: &Path) -> Option<std::fs::File> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    Some(std::fs::File::from(fd))
}

#[cfg(not(unix))]
fn open(_: &Path) -> Option<std::fs::File> {
    None
}

#[cfg(test)]
mod tests {
    use super::Snapshot;
    use crate::managed::shipped::sha256_file;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "sail-native-snapshot-{}-{name}",
                std::process::id()
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn original_substitution_cannot_change_captured_compiler_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new("substitution")?;
        let library = root.path().join("libfixture.so");
        std::fs::write(&library, b"native library A")?;
        let digest = sha256_file(&library).ok_or("hash A")?.0;
        let separate: Vec<OsString> = vec![
            "rustc".into(),
            "--extern".into(),
            format!("fixture={}", library.display()).into(),
            "--cfg".into(),
            "unmodified".into(),
        ];
        let directory = root.path().join("private/run/one");
        let snapshot = Snapshot::copy(&directory, &[(library.clone(), digest.clone())], &separate)
            .ok_or("capture A")?;
        std::fs::write(&library, b"native library B")?;
        let copied = &snapshot.files.get(&library).ok_or("captured library")?.0;
        assert_eq!(std::fs::read(copied)?, b"native library A");
        assert!(snapshot.intact());
        assert_eq!(&snapshot.arguments()[..2], &separate[..2]);
        assert_eq!(&snapshot.arguments()[3..], &separate[3..]);
        assert_eq!(
            snapshot.arguments()[2],
            OsString::from(format!("fixture={}", copied.display()))
        );
        let attached = vec![OsString::from(format!(
            "--extern=fixture={}",
            library.display()
        ))];
        assert_eq!(
            snapshot.rewrite(&attached).ok_or("rewrite attached")?[0],
            OsString::from(format!("--extern=fixture={}", copied.display()))
        );
        std::fs::write(&library, b"native library A")?;
        assert_eq!(sha256_file(&library).ok_or("restored A")?.0, digest);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(copied, std::fs::Permissions::from_mode(0o600))?;
            std::fs::write(copied, b"damaged private input")?;
            assert!(!snapshot.intact());
        }
        drop(snapshot);
        assert!(!directory.exists());
        assert_eq!(std::fs::read(&library)?, b"native library A");
        Ok(())
    }

    #[test]
    fn different_bytes_and_incomplete_argument_binding_refuse_and_remove_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new("mismatch")?;
        let library = root.path().join("libfixture.so");
        std::fs::write(&library, b"native library A")?;
        let expected = sha256_file(&library).ok_or("hash A")?.0;
        let args = vec![OsString::from(format!(
            "--extern=fixture={}",
            library.display()
        ))];
        let directory = root.path().join("private/run/one");
        std::fs::write(&library, b"native library B")?;
        assert!(
            Snapshot::copy(&directory, &[(library.clone(), expected.clone())], &args).is_none()
        );
        assert!(!directory.exists());
        std::fs::write(&library, b"native library A")?;
        let missing = vec![OsString::from("--extern=unbound=/no/such/artifact")];
        assert!(Snapshot::copy(&directory, &[(library, expected)], &missing).is_none());
        assert!(!directory.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn redirected_native_input_is_never_followed() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new("redirect")?;
        let library = root.path().join("libfixture.so");
        let redirected = root.path().join("redirected.so");
        std::fs::write(&library, b"native library A")?;
        let expected = sha256_file(&library).ok_or("hash A")?.0;
        std::os::unix::fs::symlink(&library, &redirected)?;
        let args = vec![OsString::from(format!(
            "--extern=fixture={}",
            redirected.display()
        ))];
        let directory = root.path().join("private/run/one");
        assert!(Snapshot::copy(&directory, &[(redirected, expected)], &args).is_none());
        assert!(!directory.exists());
        Ok(())
    }
}
