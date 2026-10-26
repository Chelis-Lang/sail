//! Content-bound retained-proof building blocks (proposed ADR 0050).
//!
//! The explicit research request joins native certificates, archive publication
//! and checked replay. Native qualification remains pending. No old event
//! observer is imported.

mod archive;
mod budget;
mod compiler;
pub(crate) mod cost;
mod driver;
mod eligibility;
mod namespace;
mod native;
mod producers;
mod selection;
pub(crate) use driver::Plan;
#[cfg(test)]
pub(crate) use driver::actual_late_guard;
pub(crate) use producers::Native;
pub(crate) use producers::Running;
pub(crate) use producers::publish_shipped;
pub(in crate::managed) use producers::publish_shipped_native;

/// The stable code of the notes that say why automatic retention (instant
/// revert) did not apply. Printed in human output only with `SAIL_EXPLAIN=1`.
pub(crate) const SKIPPED: &str = "SAIL3146";

/// Sidecar marker a target's compiler child leaves when its earlier verified
/// revision was evicted from the arena; it holds the target's name.
const EVICTED_MARKER: &str = "retained-evicted";

/// The note saying automatic retention did not apply because of `reason`.
pub(crate) fn skipped_note(reason: &str) -> String {
    format!("{SKIPPED}: instant revert did not apply because {reason}; verification is unaffected")
}

/// The note when retention would be requested on this platform and mode but
/// every selected target's compile is too cheap for it ([`cost::pays`]).
pub(crate) fn cost_note<'a>(
    receipts: &std::path::Path,
    targets: impl IntoIterator<Item = (&'a std::path::Path, &'a str)>,
) -> Option<String> {
    let mode = super::research::selected().ok()?;
    if !selection::requested(mode) {
        return None;
    }
    cost::skipped(receipts, targets).map(|reason| skipped_note(&reason))
}

/// One note per target of this run whose earlier verified revision had been
/// evicted from the retained arena, from the run's sidecar markers.
pub(crate) fn evicted_notes(sidecars: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sidecars)
        .into_iter()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path().join(EVICTED_MARKER)).ok())
        .collect();
    names.sort();
    names.dedup();
    names
        .iter()
        .map(|name| {
            skipped_note(&format!(
                "{name}'s earlier verified revision had been evicted from the retained arena (its revision or byte limit), so it was compiled again"
            ))
        })
        .collect()
}

/// Resolve lexical dot components only. Never reopen a path to manufacture
/// consumed-byte evidence; compiler checksums provide that separate authority.
fn lexical(path: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut result = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => result.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(result.components().next_back(), Some(Component::Normal(_)))
                    || !result.pop()
                {
                    return None;
                }
            }
            Component::Prefix(_) => return None,
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn eviction_markers_become_one_note_per_target() -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = std::env::temp_dir().join(format!(
            "sail-retained-evicted-notes-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&sidecars);
        for (key, name) in [("a", "vest_lib"), ("b", "vest_lib"), ("c", "other")] {
            std::fs::create_dir_all(sidecars.join(key))?;
            std::fs::write(sidecars.join(key).join(super::EVICTED_MARKER), name)?;
        }
        std::fs::create_dir_all(sidecars.join("d"))?;
        let notes = super::evicted_notes(&sidecars);
        let _ = std::fs::remove_dir_all(&sidecars);
        assert_eq!(notes.len(), 2);
        assert!(notes.iter().all(|note| {
            note.starts_with("SAIL3146: instant revert did not apply because ")
                && note.ends_with("; verification is unaffected")
        }));
        assert!(notes[1].contains("vest_lib's earlier verified revision had been evicted"));
        assert_eq!(super::evicted_notes(&sidecars), Vec::<String>::new());
        Ok(())
    }
}
