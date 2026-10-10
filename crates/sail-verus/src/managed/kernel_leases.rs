//! Kernel-owned lease validation for a single nonrenewable owner generation.
//!
//! The helper acquires its complete inventory before readiness and never adds or
//! renews a lease afterward. ACTIVE therefore only transitions to BREAKING or
//! disappearance. A bounded sequential sweep observing every expected ACTIVE
//! lease proves they were all ACTIVE when that sweep began; it does not assume
//! that the proc seq-file is an atomic snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_LOCK_BYTES: u64 = 2 * 1024 * 1024;
const SWEEP_BUDGET: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    major: u64,
    minor: u64,
    inode: u64,
}
fn key(text: &str) -> Option<Key> {
    let mut fields = text.split(':');
    let key = Key {
        major: u64::from_str_radix(fields.next()?, 16).ok()?,
        minor: u64::from_str_radix(fields.next()?, 16).ok()?,
        inode: fields.next()?.parse().ok()?,
    };
    fields.next().is_none().then_some(key)
}
fn owner(text: &str) -> Option<u64> {
    let fields: Vec<_> = text.rsplit_once(')')?.1.split_whitespace().collect();
    // Starttime is field 22, state field 3. Stopped/zombie owners cannot certify
    // progress, even if their kernel leases still happen to block a writer.
    ["R", "S", "D"].contains(fields.first()?).then_some(())?;
    fields.get(19)?.parse().ok()
}
fn process(pid: u32) -> Option<u64> {
    owner(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}
enum Row {
    Other,
    Active(Key),
}
fn row(line: &str, pid: u32) -> Option<Row> {
    let fields: Vec<_> = line.split_whitespace().collect();
    let offset = usize::from(fields.get(1) == Some(&"->"));
    let fields = fields.get(offset..)?;
    if fields.get(4)?.parse::<i64>().ok()? != i64::from(pid) {
        return Some(Row::Other);
    }
    // Anything else owned by this private helper is an unbound generation.
    if offset != 0
        || fields.len() != 8
        || fields.get(1) != Some(&"LEASE")
        || fields.get(2) != Some(&"ACTIVE")
        || fields.get(3) != Some(&"READ")
        || fields.get(6) != Some(&"0")
        || fields.get(7) != Some(&"EOF")
    {
        return None;
    }
    key(fields.get(5)?).map(Row::Active)
}
fn inventory(text: &str, pid: u32) -> Option<BTreeMap<Key, usize>> {
    let mut found = BTreeMap::new();
    for line in text.lines() {
        if let Row::Active(key) = row(line, pid)? {
            *found.entry(key).or_insert(0) += 1;
        }
    }
    Some(found)
}

fn descriptor(text: &str, pid: u32) -> Option<Key> {
    if !text.ends_with('\n') {
        return None;
    }
    let mut leases = text.lines().filter_map(|line| line.strip_prefix("lock:\t"));
    let Row::Active(key) = row(leases.next()?, pid)? else {
        return None;
    };
    leases.next().is_none().then_some(key)
}

pub(super) struct Inventory {
    pid: u32,
    started: u64,
    expected: BTreeMap<Key, usize>,
    descriptors: Vec<(u32, Key)>,
}
impl Inventory {
    pub(super) fn bind(pid: u32, fds: &[u32], identities: &[(u64, u64)]) -> Option<Self> {
        if fds.is_empty()
            || fds.len() != identities.len()
            || fds.len() > 4096
            || fds.iter().copied().collect::<BTreeSet<_>>().len() != fds.len()
        {
            return None;
        }
        let started = process(pid)?;
        let mut expected = BTreeMap::new();
        let mut descriptors = Vec::new();
        for (fd, (device, inode)) in fds.iter().zip(identities) {
            let metadata = std::fs::metadata(format!("/proc/{pid}/fd/{fd}")).ok()?;
            if metadata.dev() != *device || metadata.ino() != *inode || !metadata.is_file() {
                return None;
            }
            let info = std::fs::read_to_string(format!("/proc/{pid}/fdinfo/{fd}")).ok()?;
            let rows = info.lines().filter_map(|line| line.strip_prefix("lock:\t"));
            let mut count = 0;
            for line in rows {
                let Row::Active(key) = row(line, pid)? else {
                    return None;
                };
                if key.inode != *inode {
                    return None;
                }
                *expected.entry(key).or_insert(0) += 1;
                descriptors.push((*fd, key));
                count += 1;
            }
            if count != 1 {
                return None;
            }
        }
        (process(pid)? == started).then_some(Self {
            pid,
            started,
            expected,
            descriptors,
        })
    }
    pub(super) fn check(&self) -> Result<(), &'static str> {
        let began = Instant::now();
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        // A size/work budget is fail-closed, not a claim that proc filesystem
        // reads can be forcibly interrupted within this interval.
        let mut bytes = Vec::new();
        let file = std::fs::File::open(Path::new("/proc/locks"))
            .map_err(|_| "kernel-lease-state-read-unavailable")?;
        file.take(MAX_LOCK_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "kernel-lease-state-read-unavailable")?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LOCK_BYTES {
            return Err("kernel-lease-state-too-large");
        }
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err("kernel-lease-state-truncated");
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "kernel-lease-state-malformed")?;
        self.check_inventory(text, bytes.len(), began)?;
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        if began.elapsed() > SWEEP_BUDGET {
            return Err("kernel-lease-sweep-over-budget");
        }
        Ok(())
    }
    fn check_inventory(
        &self,
        text: &str,
        bytes_read: usize,
        began: Instant,
    ) -> Result<(), &'static str> {
        let found = inventory(text, self.pid).ok_or("kernel-lease-state-invalid")?;
        if found == self.expected {
            return Ok(());
        }
        if found.keys().any(|key| !self.expected.contains_key(key)) {
            return Err("kernel-lease-inventory-invalidated");
        }
        // The global seq-file may repeat/omit unchanged rows if unrelated
        // owners mutate the list between seq buffer refills. Each fdinfo is
        // instead produced under the inode's lease lock. The immutable helper
        // cannot replace/add descriptors or reacquire leases after readiness.
        // Confirm this initially-bound descriptor inventory independently before
        // declaring failure; never revive an already-invalidated generation.
        let mut remaining = MAX_LOCK_BYTES.saturating_sub(bytes_read as u64);
        for (fd, expected) in &self.descriptors {
            if began.elapsed() > SWEEP_BUDGET {
                return Err("kernel-lease-sweep-over-budget");
            }
            let file = std::fs::File::open(format!("/proc/{}/fdinfo/{fd}", self.pid))
                .map_err(|_| "kernel-lease-descriptor-unavailable")?;
            let mut bytes = Vec::new();
            file.take(remaining.saturating_add(1))
                .read_to_end(&mut bytes)
                .map_err(|_| "kernel-lease-descriptor-unavailable")?;
            let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if size > remaining {
                return Err("kernel-lease-state-too-large");
            }
            remaining -= size;
            let text = std::str::from_utf8(&bytes).map_err(|_| "kernel-lease-state-malformed")?;
            if descriptor(text, self.pid) != Some(*expected) {
                return Err("kernel-lease-inventory-invalidated");
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn check_omitted_global_for_test(&self) -> Result<(), &'static str> {
        let began = Instant::now();
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        self.check_inventory("", 0, began)?;
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        if began.elapsed() > SWEEP_BUDGET {
            return Err("kernel-lease-sweep-over-budget");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_owner_active_multiset_rejects_breaks_extra_and_malformed_rows() {
        let valid = "1: LEASE ACTIVE READ 7 00:2a:100 0 EOF\n2: LEASE ACTIVE READ 7 00:2a:100 0 EOF\n3: POSIX ADVISORY WRITE 9 00:2a:200 0 EOF\n";
        let found = inventory(valid, 7).unwrap_or_default();
        assert_eq!(found.values().copied().collect::<Vec<_>>(), [2]);
        assert!(inventory(&valid.replace("ACTIVE", "BREAKING"), 7).is_none());
        assert!(inventory(&valid.replace("ACTIVE READ", "ACTIVE WRITE"), 7).is_none());
        assert!(inventory(&valid.replace("00:2a:100", "00:2a:bad"), 7).is_none());
        assert!(inventory("1: LEASE ACTIVE READ 7 00:2a:100 1 EOF", 7).is_none());
        assert_ne!(
            inventory(&valid.replace("READ 7", "READ 8"), 7),
            Some(found)
        );
        assert!(inventory("1: -> LEASE BREAKER WRITE 7 00:2a:100 0 EOF", 7).is_none());
    }
    #[test]
    fn descriptor_requires_one_complete_exact_owner_active_row() {
        let active = "pos:\t0\nlock:\t1: LEASE ACTIVE READ 7 00:2a:100 0 EOF\n";
        assert_eq!(descriptor(active, 7), key("00:2a:100"));
        assert!(descriptor(&active.replace("ACTIVE", "BREAKING"), 7).is_none());
        assert!(descriptor(&active.replace("READ 7", "READ 8"), 7).is_none());
        assert!(descriptor(&active.replace("ACTIVE READ", "ACTIVE WRITE"), 7).is_none());
        assert!(descriptor(&(active.to_owned() + active), 7).is_none());
        assert!(descriptor(active.trim_end(), 7).is_none());
    }
    #[test]
    fn process_binding_rejects_stopped_zombie_and_changed_starttime() {
        let rest = "0 ".repeat(18);
        let text = format!("7 (name with ) and spaces) S {rest}100 0");
        assert_eq!(owner(&text), Some(100));
        assert_eq!(owner(&text.replace(") S ", ") T ")), None);
        assert_eq!(owner(&text.replace(") S ", ") Z ")), None);
        assert_ne!(owner(&text.replace("100 0", "101 0")), Some(100));
    }
}
