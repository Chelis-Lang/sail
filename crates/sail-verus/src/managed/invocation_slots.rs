//! Bounded invocation-local sidecars, under the compiler-family owner.
//!
//! Slots hold data a full run derives from its working directory/arguments.
//! They never split proof receipts or Cargo targets. Eviction only removes an
//! optimization: a replay whose slot is absent takes the full path.

use std::path::{Path, PathBuf};

pub(super) const DIRECTORY: &str = "invocations";
const RECENCY: &str = "recency.json";
const LIMIT: usize = 8;
/// Latest-context views are republished from the caller's bound slot.
pub(super) const VIEWS: [&str; 2] = ["input-state.json", "read-check.json"];

fn key(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

pub(super) fn path(context: &Path, invocation: &Path, args: &[String]) -> Option<PathBuf> {
    let invocation = std::path::absolute(invocation).ok()?;
    let hint = super::input::hint_path(context, &invocation, args);
    Some(context.join(DIRECTORY).join(hint.file_stem()?))
}

/// Best effort, called only with the workspace/family locks held. Unknown
/// children or links refuse slot use rather than being removed as an eviction.
pub(super) fn prepare(context: &Path, invocation: &Path, args: &[String]) -> Option<PathBuf> {
    let slot = path(context, invocation, args)?;
    update(&slot, true)?;
    Some(slot)
}

/// Replaying the slot counts as use. Its absence must still miss, not create a
/// replacement which could conceal the loss of the original observations.
pub(super) fn touch(slot: &Path) -> Option<()> {
    update(slot, false)
}

fn update(slot: &Path, create: bool) -> Option<()> {
    let root = slot.parent()?;
    let selected = slot.file_name()?.to_str()?;
    key(selected).then_some(())?;
    std::fs::create_dir_all(root).ok()?;
    std::fs::symlink_metadata(root)
        .ok()?
        .is_dir()
        .then_some(())?;
    let mut existing = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(root).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().into_string().ok()?;
        let kind = entry.file_type().ok()?;
        if name == RECENCY {
            kind.is_file().then_some(())?;
        } else {
            (key(&name) && kind.is_dir()).then_some(())?;
            existing.insert(name);
        }
    }
    if !existing.contains(selected) {
        create.then_some(())?;
        existing.insert(selected.to_owned());
    }
    let mut order = super::receipts::read::<Vec<String>>(&root.join(RECENCY)).unwrap_or_default();
    let mut seen = std::collections::BTreeSet::new();
    order.retain(|name| existing.contains(name) && seen.insert(name.clone()));
    // An unindexed, valid owned slot is older than the known recency list.
    let mut unindexed: Vec<_> = existing.difference(&seen).cloned().collect();
    unindexed.extend(order);
    order = unindexed;
    order.retain(|name| name != selected);
    order.push(selected.to_owned());
    while order.len() > LIMIT {
        let oldest = order.remove(0);
        std::fs::remove_dir_all(root.join(oldest)).ok()?;
    }
    if !slot.exists() {
        std::fs::create_dir(slot).ok()?;
    }
    super::files::json(&root.join(RECENCY), &order).ok()?;
    Some(())
}

/// Exact copies of the invocation's bound sidecars become the context's
/// latest views, as on a full run. The caller has already matched each source
/// digest and supplies those same completely read bytes, never a second read.
pub(super) fn publish(context: &Path, views: &[(String, Vec<u8>)]) -> crate::Result<()> {
    for (name, bytes) in views {
        if !VIEWS.contains(&name.as_str()) {
            return Err(crate::Error::new("SAIL3102", "invalid invocation view"));
        }
        super::files::bytes_if_changed(&context.join(name), bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ninth_slot_evicts_oldest_and_replay_use_refreshes_recency()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-invocation-slots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let mut slots = Vec::new();
        for index in 0..8 {
            let slot = prepare(&root, &root.join(index.to_string()), &[]).ok_or("slot refused")?;
            std::fs::write(slot.join("input-state.json"), index.to_string())?;
            slots.push(slot);
        }
        touch(&slots[0]).ok_or("touch refused")?;
        let ninth = prepare(&root, &root.join("ninth"), &[]).ok_or("ninth refused")?;
        assert!(slots[0].exists());
        assert!(!slots[1].exists());
        assert!(ninth.exists());
        assert!(touch(&slots[1]).is_none());
        assert!(!slots[1].exists());
        assert_eq!(
            std::fs::read_to_string(slots[0].join("input-state.json"))?,
            "0"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn unknown_slot_sibling_is_never_evicted() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-invocation-foreign-{}", std::process::id()));
        let foreign = root.join(DIRECTORY).join("source");
        std::fs::create_dir_all(&foreign)?;
        std::fs::write(foreign.join("keep"), "source")?;
        assert!(prepare(&root, &root, &[]).is_none());
        assert_eq!(std::fs::read_to_string(foreign.join("keep"))?, "source");
        std::fs::remove_dir_all(root)
    }
}
