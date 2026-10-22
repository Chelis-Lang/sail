//! Whether ordinary retention can pay for itself on a target (#156).
//!
//! Retention reads and checks the target's whole dependency graph (vstd and
//! the macro crates, hundreds of megabytes) before and after its compiler
//! runs. On a crate that verifies in under a second that costs more than the
//! compile a later restore could save, on every changed run. Each verifying
//! compile records how long the compiler and, when it was engaged, retention
//! took; the next run skips retention for a target whose compile is cheap
//! next to its retention. A skipped plan compiles normally, so this decides
//! cost only: never a verdict, a receipt or what any check reads. The records
//! authorize nothing, and an unreadable or missing record keeps retention.

use super::super::{files, receipts};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const VERSION: u32 = 1;

/// Retention is kept only while it costs at most `1 / RATIO` of the compile
/// a restore would avoid.
const RATIO: u128 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Cost {
    version: u32,
    compile_ms: u64,
    /// Absent until a compile of this target ran with retention engaged.
    retention_ms: Option<u64>,
}

fn path(receipts: &Path, manifest: &Path, name: &str) -> PathBuf {
    let key = files::digest(
        &serde_json::to_vec(&(VERSION, manifest, name.replace('-', "_"))).unwrap_or_default(),
    );
    receipts
        .with_file_name("retention-costs")
        .join(format!("{key}.json"))
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Records one completed compile of a target. `retention` is the time the
/// engaged retention spent around it; without it the last measured
/// retention cost is kept. Best effort: a failure records nothing.
pub(crate) fn record(
    receipts: &Path,
    manifest: &Path,
    name: &str,
    compile: Duration,
    retention: Option<Duration>,
) {
    let file = path(receipts, manifest, name);
    let previous = receipts::read::<Cost>(&file).filter(|cost| cost.version == VERSION);
    let cost = Cost {
        version: VERSION,
        compile_ms: millis(compile),
        retention_ms: retention
            .map(millis)
            .or_else(|| previous.and_then(|cost| cost.retention_ms)),
    };
    if let Some(directory) = file.parent()
        && std::fs::create_dir_all(directory).is_ok()
    {
        let _ = files::json(&file, &cost);
    }
}

/// Whether retention may pay on any of `targets` (manifest and target name):
/// true unless every one has a recorded compile cheaper than `RATIO` times
/// its recorded retention.
pub(crate) fn pays<'a>(
    receipts: &Path,
    targets: impl IntoIterator<Item = (&'a Path, &'a str)>,
) -> bool {
    targets.into_iter().any(|(manifest, name)| {
        receipts::read::<Cost>(&path(receipts, manifest, name))
            .filter(|cost| cost.version == VERSION)
            .is_none_or(worth)
    })
}

fn worth(cost: Cost) -> bool {
    cost.retention_ms
        .is_none_or(|retention| u128::from(cost.compile_ms) >= RATIO * u128::from(retention))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipts(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("sail-retention-cost-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root.join("context").join("receipts")
    }

    #[test]
    fn unknown_costs_keep_retention() {
        let receipts = receipts("unknown");
        let manifest = Path::new("/w/Cargo.toml");
        assert!(pays(&receipts, [(manifest, "demo")]));
        // A compile without engaged retention leaves its cost unknown.
        record(&receipts, manifest, "demo", Duration::from_millis(5), None);
        assert!(pays(&receipts, [(manifest, "demo")]));
        let _ = std::fs::remove_dir_all(receipts.parent().unwrap_or(&receipts));
    }

    #[test]
    fn a_cheap_compile_skips_retention_until_it_grows() {
        let receipts = receipts("cheap");
        let manifest = Path::new("/w/Cargo.toml");
        record(
            &receipts,
            manifest,
            "demo-lib",
            Duration::from_millis(750),
            Some(Duration::from_millis(1400)),
        );
        // Names are compared as the compiler spells them.
        assert!(!pays(&receipts, [(manifest, "demo_lib")]));
        // Another target, or another manifest, has its own record.
        assert!(pays(
            &receipts,
            [(manifest, "demo_lib"), (manifest, "other")]
        ));
        assert!(pays(&receipts, [(Path::new("/x/Cargo.toml"), "demo_lib")]));
        // A later compile without retention keeps the measured retention cost.
        record(
            &receipts,
            manifest,
            "demo_lib",
            Duration::from_millis(900),
            None,
        );
        assert!(!pays(&receipts, [(manifest, "demo_lib")]));
        record(
            &receipts,
            manifest,
            "demo_lib",
            Duration::from_secs(30),
            None,
        );
        assert!(pays(&receipts, [(manifest, "demo_lib")]));
        let _ = std::fs::remove_dir_all(receipts.parent().unwrap_or(&receipts));
    }

    #[test]
    fn the_ratio_boundary_and_damaged_records() {
        assert!(worth(Cost {
            version: VERSION,
            compile_ms: 16_000,
            retention_ms: Some(1000),
        }));
        assert!(!worth(Cost {
            version: VERSION,
            compile_ms: 15_999,
            retention_ms: Some(1000),
        }));
        assert!(worth(Cost {
            version: VERSION,
            compile_ms: 0,
            retention_ms: Some(0),
        }));
        let receipts = receipts("damaged");
        let manifest = Path::new("/w/Cargo.toml");
        let file = path(&receipts, manifest, "demo");
        assert!(
            file.parent()
                .is_some_and(|d| std::fs::create_dir_all(d).is_ok())
        );
        assert!(std::fs::write(&file, b"{").is_ok());
        assert!(pays(&receipts, [(manifest, "demo")]));
        let other =
            serde_json::json!({"version": VERSION + 1, "compile_ms": 1, "retention_ms": 100});
        assert!(std::fs::write(&file, other.to_string()).is_ok());
        assert!(pays(&receipts, [(manifest, "demo")]));
        let _ = std::fs::remove_dir_all(receipts.parent().unwrap_or(&receipts));
    }
}
