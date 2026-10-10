//! Kernel-owned lease validation for a single nonrenewable owner generation.
//!
//! The helper acquires its complete inventory before readiness and never adds or
//! renews a lease afterward. ACTIVE therefore only transitions to BREAKING or
//! disappearance. A bounded sequential sweep observing every expected ACTIVE
//! lease proves they were all ACTIVE when that sweep began; it does not assume
//! that the proc seq-file is an atomic snapshot. Descheduling a reader cannot
//! renew the generation: validity depends on these kernel and namespace checks,
//! not the wall time between them. Public consumers enforce their own deadlines.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

const MAX_LOCK_BYTES: u64 = 2 * 1024 * 1024;

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
    let mut fields = text.rsplit_once(')')?.1.split_whitespace();
    // Starttime is field 22, state field 3. Stopped/zombie owners cannot certify
    // progress, even if their kernel leases still happen to block a writer.
    matches!(fields.next()?, "R" | "S" | "D").then_some(())?;
    fields.nth(18)?.parse().ok()
}
fn process(pid: u32) -> Option<u64> {
    owner(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}
enum Row {
    Other,
    Active(Key),
}
fn row(line: &str, pid: u32) -> Option<Row> {
    let mut fields = line.split_whitespace();
    fields.next()?;
    let mut kind = fields.next()?;
    let blocked = kind == "->";
    if blocked {
        kind = fields.next()?;
    }
    let state = fields.next()?;
    let access = fields.next()?;
    if fields.next()?.parse::<i64>().ok()? != i64::from(pid) {
        return Some(Row::Other);
    }
    let identity = fields.next()?;
    let start = fields.next()?;
    let end = fields.next()?;
    // Anything else owned by this private helper is an unbound generation.
    if blocked
        || kind != "LEASE"
        || state != "ACTIVE"
        || access != "READ"
        || start != "0"
        || end != "EOF"
        || fields.next().is_some()
    {
        return None;
    }
    key(identity).map(Row::Active)
}
#[cfg(test)]
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

fn kernel_rows(already_read: u64) -> Result<Vec<u8>, &'static str> {
    let remaining = MAX_LOCK_BYTES
        .checked_sub(already_read)
        .ok_or("kernel-lease-state-too-large")?;
    let mut bytes = Vec::new();
    let file = std::fs::File::open(Path::new("/proc/locks"))
        .map_err(|_| "kernel-lease-state-read-unavailable")?;
    file.take(remaining + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "kernel-lease-state-read-unavailable")?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > remaining {
        return Err("kernel-lease-state-too-large");
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err("kernel-lease-state-truncated");
    }
    Ok(bytes)
}

#[cfg(test)]
fn trace_sweep(
    stage: &'static str,
    sweep: usize,
    started: std::time::Instant,
    bytes: usize,
    descriptors: usize,
) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("SAIL_FRESHNESS_TRACE").is_some_and(|v| v == "1")) {
        eprintln!(
            "sail-kernel-lease thread={:?} elapsed_us={} stage={stage} sweep={sweep} bytes={bytes} descriptors={descriptors}",
            std::thread::current().id(),
            started.elapsed().as_micros()
        );
    }
}

pub(super) struct Inventory {
    pid: u32,
    started: u64,
    expected: BTreeMap<Key, usize>,
    expected_rows: Vec<Key>,
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
        let mut expected_rows = Vec::with_capacity(fds.len());
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
                expected_rows.push(key);
                descriptors.push((*fd, key));
                count += 1;
            }
            if count != 1 {
                return None;
            }
        }
        expected_rows.sort_unstable();
        (process(pid)? == started).then_some(Self {
            pid,
            started,
            expected,
            expected_rows,
            descriptors,
        })
    }
    pub(super) fn check(&self) -> Result<(), &'static str> {
        self.check_with(|| {})
    }
    fn check_with(&self, after_read: impl FnOnce()) -> Result<(), &'static str> {
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        // The byte cap bounds inventory work; kernel proc reads are not
        // interruptible wall-clock deadlines. A late read cannot restore an
        // invalidated owner generation, regardless of elapsed scheduling time.
        let mut consumed = 0;
        let mut after_read = Some(after_read);
        // Confirm a final independent inventory after processing the first one.
        // SIGIO can close all descriptors before the helper process exits;
        // process liveness alone cannot confirm retained lease ownership.
        #[cfg(test)]
        let started = std::time::Instant::now();
        for _sweep in 0..2 {
            let bytes = kernel_rows(consumed)?;
            #[cfg(test)]
            trace_sweep("global-read", _sweep, started, bytes.len(), 0);
            consumed += u64::try_from(bytes.len()).unwrap_or(MAX_LOCK_BYTES + 1);
            let text = std::str::from_utf8(&bytes).map_err(|_| "kernel-lease-state-malformed")?;
            if let Some(pause) = after_read.take() {
                pause();
            }
            consumed = self.check_inventory(text, consumed)?;
            #[cfg(test)]
            trace_sweep("inventory-checked", _sweep, started, bytes.len(), 0);
            if process(self.pid) != Some(self.started) {
                return Err("kernel-lease-owner-inactive");
            }
        }
        Ok(())
    }
    fn check_inventory(&self, text: &str, bytes_read: u64) -> Result<u64, &'static str> {
        let mut remaining = MAX_LOCK_BYTES
            .checked_sub(bytes_read)
            .ok_or("kernel-lease-state-too-large")?;
        // Keep the exact hardlink multiplicity while avoiding a tree node
        // allocation for each owned inode on every read. The bounded proc input
        // also bounds the observed rows; sorting never changes their identity.
        let mut found = Vec::with_capacity(self.expected_rows.len());
        for line in text.lines() {
            if let Row::Active(key) = row(line, self.pid).ok_or("kernel-lease-state-invalid")? {
                found.push(key);
            }
        }
        found.sort_unstable();
        if found == self.expected_rows {
            return Ok(bytes_read);
        }
        if found.iter().any(|key| !self.expected.contains_key(key)) {
            return Err("kernel-lease-inventory-invalidated");
        }
        // The global seq-file may repeat/omit unchanged rows if unrelated
        // owners mutate the list between seq buffer refills. Each fdinfo is
        // instead produced under the inode's lease lock. The immutable helper
        // cannot replace/add descriptors or reacquire leases after readiness.
        // Confirm this initially-bound descriptor inventory independently before
        // declaring failure; never revive an already-invalidated generation.
        #[cfg(test)]
        let started = std::time::Instant::now();
        for (fd, expected) in &self.descriptors {
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
        #[cfg(test)]
        trace_sweep("descriptor-fallback", 0, started, 0, self.descriptors.len());
        Ok(MAX_LOCK_BYTES - remaining)
    }
    #[cfg(test)]
    pub(super) fn check_after_pause_for_test(
        &self,
        after_read: impl FnOnce(),
    ) -> Result<(), &'static str> {
        self.check_with(after_read)
    }
    #[cfg(test)]
    pub(super) fn check_omitted_global_for_test(&self) -> Result<(), &'static str> {
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
        }
        self.check_inventory("", 0)?;
        if process(self.pid) != Some(self.started) {
            return Err("kernel-lease-owner-inactive");
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
    fn final_inventory_cannot_reset_the_aggregate_byte_cap() {
        let text = "1: LEASE ACTIVE READ 7 00:2a:100 0 EOF\n";
        let key = key("00:2a:100").unwrap_or_else(|| unreachable!("fixture"));
        let bound = Inventory {
            pid: 7,
            started: 0,
            expected: BTreeMap::from([(key, 1)]),
            expected_rows: vec![key],
            descriptors: Vec::new(),
        };
        assert_eq!(
            bound.check_inventory(text, MAX_LOCK_BYTES),
            Ok(MAX_LOCK_BYTES)
        );
        assert_eq!(
            bound.check_inventory(text, MAX_LOCK_BYTES + 1),
            Err("kernel-lease-state-too-large")
        );
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
    fn streaming_rows_preserve_foreign_and_owned_field_boundaries() {
        let expected = key("00:2a:100");
        for row in [
            "1: LEASE ACTIVE READ 7 00:2a:100 0 EOF",
            "  1:\tLEASE\tACTIVE READ 7\t00:2a:100 0 EOF  ",
        ] {
            assert!(matches!(super::row(row, 7), Some(Row::Active(k)) if Some(k) == expected));
        }
        for row in [
            "1: LEASE ACTIVE READ 7 00:2a:100 0",
            "1: LEASE ACTIVE READ 7 00:2a:100 0 EOF extra",
            "1: -> LEASE ACTIVE READ 7 00:2a:100 0 EOF",
            "1: LEASE ACTIVE READ 7 00:2a:100:extra 0 EOF",
            "1: LEASE ACTIVE READ not-a-pid 00:2a:100 0 EOF",
            "1: LEASE ACTIVE READ",
        ] {
            assert!(super::row(row, 7).is_none(), "{row}");
        }
        // Unrelated owners remain irrelevant even if their lease shape is not
        // ours; own blocked rows must still fail the complete inventory above.
        for row in [
            "1: POSIX ADVISORY WRITE 8",
            "1: -> LEASE BREAKER WRITE 8 malformed",
            "1: LEASE ACTIVE READ -1",
        ] {
            assert!(matches!(super::row(row, 7), Some(Row::Other)), "{row}");
        }
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
