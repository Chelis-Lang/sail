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
pub(in crate::managed) mod phase_fields;
mod producers;
mod selection;
pub(crate) use driver::Plan;
#[cfg(test)]
pub(crate) use driver::actual_late_guard;
pub(crate) use producers::Native;
pub(crate) use producers::Running;
pub(crate) use producers::publish_shipped;
pub(in crate::managed) use producers::publish_shipped_native;

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
