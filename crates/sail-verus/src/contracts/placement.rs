//! Placement of owned neutral shadow workspaces away from inherited Cargo configuration.
//!
//! Cargo loads `.cargo/config[.toml]` from every parent of the directory it runs
//! in, and neutral translation refuses any such unreviewed file. A Sail cache
//! under a home directory with `~/.cargo/config.toml` would otherwise refuse
//! every neutral job. Shadows stay in the configured cache when no parent is
//! configured; otherwise they move to a private per-user directory with no
//! configured parent. The refusal policy itself is unchanged: the admission
//! check still runs on the chosen placement, and when no candidate qualifies the
//! default placement is returned so that check reports an actionable refusal.

use std::path::{Path, PathBuf};

use super::trust::ancestor_configured;
use crate::Result;

/// Directory name of the neutral shadow root inside the configured cache.
pub(super) const SHADOWS: &str = "neutral-contracts-v1";

/// The shadow root's parent to use for `cache`, before the root is created.
///
/// `runtime` is the caller's private runtime directory (`XDG_RUNTIME_DIR`) and
/// `temporary` the system temporary directory; both are tried only when the
/// configured cache has a configured parent.
pub(super) fn shadow_root(
    cache: &Path,
    runtime: Option<PathBuf>,
    temporary: &Path,
) -> Result<PathBuf> {
    let preferred = cache.join(SHADOWS);
    if !ancestor_configured(cache)? {
        return Ok(preferred);
    }
    let candidates = runtime
        .filter(|path| path.is_absolute())
        .map(|path| path.join(format!("sail-{SHADOWS}")))
        .into_iter()
        .chain(private_name().map(|name| temporary.join(name)));
    for candidate in candidates {
        if let Some(parent) = candidate.parent()
            && parent.is_dir()
            && !ancestor_configured(parent)?
            && private(&candidate)
        {
            return Ok(candidate);
        }
    }
    Ok(preferred)
}

/// A shared temporary directory needs a per-user name.
#[cfg(unix)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Platforms without user identities have no private temporary name."
)]
fn private_name() -> Option<String> {
    Some(format!(
        "sail-{}-{SHADOWS}",
        rustix::process::getuid().as_raw()
    ))
}

#[cfg(not(unix))]
const fn private_name() -> Option<String> {
    None
}

/// Create `path` if absent; accept only a real directory owned by this user
/// that no other user can write.
#[cfg(unix)]
fn private(path: &Path) -> bool {
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
    let _ = std::fs::DirBuilder::new().mode(0o700).create(path);
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == rustix::process::getuid().as_raw()
            && metadata.mode() & 0o022 == 0
    })
}

#[cfg(not(unix))]
const fn private(_: &Path) -> bool {
    false
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn shadows_leave_a_configured_parent_only_for_a_private_unconfigured_directory() -> TestResult {
        let base = Directory(
            std::env::temp_dir().join(format!("sail-neutral-placement-{}", std::process::id())),
        );
        let home = base.0.join("home");
        let cache = home.join("cache/sail");
        let runtime = base.0.join("runtime");
        let temporary = base.0.join("tmp");
        for directory in [&cache, &runtime, &temporary] {
            std::fs::create_dir_all(directory)?;
        }
        let place = |runtime: Option<PathBuf>| {
            shadow_root(&cache, runtime, &temporary).map_err(|error| error.message)
        };
        // An unconfigured cache keeps its existing placement.
        assert_eq!(place(Some(runtime.clone())), Ok(cache.join(SHADOWS)));
        std::fs::create_dir_all(home.join(".cargo"))?;
        std::fs::write(home.join(".cargo/config.toml"), "[build]\n")?;
        let in_runtime = runtime.join(format!("sail-{SHADOWS}"));
        let in_temporary = temporary.join(private_name().unwrap_or_default());
        for (runtime, expected) in [
            (Some(runtime.clone()), in_runtime.clone()),
            (None, in_temporary.clone()),
            (Some(PathBuf::from("relative")), in_temporary.clone()),
        ] {
            assert_eq!(place(runtime), Ok(expected.clone()));
            let mode = std::fs::metadata(&expected)?.permissions().mode();
            assert_eq!(mode & 0o077, 0, "{mode:o}");
        }
        // A writable-by-others directory or a link is never adopted.
        std::fs::set_permissions(&in_runtime, std::fs::Permissions::from_mode(0o777))?;
        std::fs::remove_dir(&in_temporary)?;
        std::os::unix::fs::symlink(&cache, &in_temporary)?;
        assert_eq!(place(Some(runtime.clone())), Ok(cache.join(SHADOWS)));
        // A configured candidate parent is skipped too.
        std::fs::set_permissions(&in_runtime, std::fs::Permissions::from_mode(0o700))?;
        std::fs::create_dir(runtime.join(".cargo"))?;
        std::fs::write(runtime.join(".cargo/config"), "")?;
        assert_eq!(place(Some(runtime)), Ok(cache.join(SHADOWS)));
        Ok(())
    }
}
