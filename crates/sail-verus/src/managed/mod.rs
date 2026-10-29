//! Managed Cargo artifacts and successful per-target verification receipts.

mod absent;
pub(crate) mod build_environment;
mod build_inputs;
mod build_unit_plan;
pub mod cargo_toolchain;
pub(crate) mod compiler_origin;
mod configuration;
mod dependency_snapshot;
mod diagnostics;
mod environment;
mod files;
mod freshness;
pub(crate) mod ghost_environment;
mod input;
mod native_include_hints;
mod native_tools;
pub mod noop;
pub mod projects;
mod read_check;
mod receipts;
pub(crate) mod research;
#[expect(
    dead_code,
    reason = "ADR 0050's content-bound successor is source-only until its two-phase compiler admission and real Vest ABA gate qualify."
)]
pub(crate) mod retained;
mod runner;
pub(crate) mod shipped;
mod source_inputs;
mod store;
mod tool_digests;
pub(crate) mod transport;
pub(crate) mod visible_output;

pub use cargo_toolchain::CargoToolchain;
pub use configuration::loader_unbound;
pub(crate) use files::{digest, event, file_digest, json, stat_identity};
pub use freshness::FreshnessIndex;
pub use input::{InputSnapshot, snapshot};
pub(crate) use receipts::{Invocation, Runtime, TargetIdentity, retire as retire_receipts};
pub use runner::{
    Config, refresh_context, refresh_result, run, run_for_replay, run_showing_output,
};
pub(crate) use runner::{run_engine, run_engine_held};
pub use shipped::new_project_lockfile;

pub(crate) use receipts::complete as receipt_complete;

pub(crate) use runner::focused;

/// Use one physical spelling for managed cache paths, creating the root first.
/// If creation or resolution fails, preserve the caller's path and the normal
/// verification/error behavior. This changes no global cache or socket policy.
#[must_use]
pub fn canonical_cache_root(path: std::path::PathBuf) -> std::path::PathBuf {
    std::fs::create_dir_all(&path)
        .and_then(|()| path.canonicalize())
        .unwrap_or(path)
}

/// Whether `args` select partial verification (`--verify-function`,
/// `--verify-module`, `--verify-only-module` or `--verify-root` after `--`).
#[must_use]
pub fn partial(args: &[String]) -> bool {
    focused(args)
}

#[cfg(test)]
mod cache_path_tests {
    #[test]
    fn failed_creation_preserves_the_literal_path()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let file = std::env::temp_dir().join(format!("sail-cache-file-{}", std::process::id()));
        std::fs::write(&file, b"not a directory")?;
        let path = file.join("cache");
        assert_eq!(super::canonical_cache_root(path.clone()), path);
        std::fs::remove_file(file)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_cache_and_new_descendants_use_one_physical_root()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-cache-alias-{}", std::process::id()));
        std::fs::create_dir_all(root.join("physical"))?;
        std::os::unix::fs::symlink(root.join("physical"), root.join("alias"))?;
        let resolved = super::canonical_cache_root(root.join("alias/new/cache"));
        assert_eq!(resolved, root.join("physical/new/cache").canonicalize()?);
        assert_eq!(super::canonical_cache_root(resolved.clone()), resolved);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
