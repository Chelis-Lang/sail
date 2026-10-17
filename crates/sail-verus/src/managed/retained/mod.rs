//! Content-bound retained-proof building blocks (proposed ADR 0050).
//!
//! The explicit driver request stages only a source-read witness. Native
//! producer joining, archive publication and replay remain disconnected until
//! the closed input boundary qualifies. No old event observer is imported.

mod archive;
mod compiler;
mod driver;
mod eligibility;
mod namespace;
mod native;
mod producers;
pub(crate) use driver::Plan;
pub(crate) use producers::Native;

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
