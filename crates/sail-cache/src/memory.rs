//! The in-memory tier: a bounded, least-recently-used map from keys to
//! replies, in front of the disk tier (ADR 0014).
//!
//! A slot is either:
//! - **on disk**: its bytes were read from a verified disk entry, or were
//!   written to one, and the slot remembers that file's [`FileStamp`]. The
//!   owner serves it only while the file still has that stamp, so a memory
//!   hit is always byte-identical to what the disk tier holds; or
//! - **pending**: a `PUT` whose disk write is queued (write-behind). No
//!   entry file exists for the key yet. Once the write finishes the slot
//!   becomes *on disk* with the new file's stamp, or is dropped if the write
//!   failed or found another entry already there.
//!
//! This module only keeps the map; the daemon applies those rules.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use sail_key::QueryKey;
use serde::{Deserialize, Serialize};

use crate::{Effort, FileStamp};

/// Bytes charged for each slot on top of its reply.
pub const SLOT_OVERHEAD: u64 = 128;

/// A cached reply with the effort its entry records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// The exact reply bytes.
    pub reply: Vec<u8>,
    /// The entry's recorded effort, for `check-sat` replies.
    pub effort: Option<Effort>,
}

impl Value {
    fn cost(&self) -> u64 {
        u64::try_from(self.reply.len()).unwrap_or(u64::MAX) + SLOT_OVERHEAD
    }
}

/// Where a slot's bytes stand relative to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Identical to the entry file with this stamp.
    OnDisk(FileStamp),
    /// Queued for writing; no entry file yet.
    Pending,
}

#[derive(Debug)]
struct Slot {
    value: Arc<Value>,
    state: State,
    tick: u64,
}

#[derive(Debug, Default)]
struct Inner {
    slots: HashMap<[u8; 32], Slot>,
    /// Least recently used first.
    order: BTreeMap<u64, [u8; 32]>,
    tick: u64,
    bytes: u64,
    pending: u64,
}

impl Inner {
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    fn remove(&mut self, key: &[u8; 32]) {
        if let Some(slot) = self.slots.remove(key) {
            self.order.remove(&slot.tick);
            self.bytes = self.bytes.saturating_sub(slot.value.cost());
            if slot.state == State::Pending {
                self.pending = self.pending.saturating_sub(1);
            }
        }
    }

    fn insert(&mut self, key: [u8; 32], value: Arc<Value>, state: State, capacity: u64) {
        self.remove(&key);
        let tick = self.next_tick();
        self.bytes += value.cost();
        if state == State::Pending {
            self.pending += 1;
        }
        self.slots.insert(key, Slot { value, state, tick });
        self.order.insert(tick, key);
        while self.bytes > capacity {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            // `remove` looks the tick up again; it is already gone from
            // `order`, which is harmless.
            self.remove(&oldest);
        }
    }
}

/// Counters describing the tier's contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStats {
    /// The configured budget in bytes (0: the tier is off).
    pub capacity_bytes: u64,
    /// Slots held.
    pub entries: u64,
    /// Bytes charged for them (replies plus [`SLOT_OVERHEAD`] each).
    pub bytes: u64,
    /// Slots whose disk write is still queued.
    pub pending: u64,
}

/// The in-memory tier.
#[derive(Debug)]
pub struct MemoryTier {
    capacity: u64,
    inner: Mutex<Inner>,
}

impl MemoryTier {
    /// A tier holding at most `capacity` bytes (0 turns it off).
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> Option<MutexGuard<'_, Inner>> {
        // A poisoned lock means a thread panicked mid-update: treat the
        // tier as empty from then on (every lookup is a miss).
        self.inner.lock().ok()
    }

    /// Whether `value` is small enough to hold: at most a quarter of the
    /// budget, so one large reply cannot flush the whole tier.
    fn admits(&self, value: &Value) -> bool {
        self.capacity > 0 && value.cost() <= self.capacity / 4
    }

    /// The slot for `key`, marking it most recently used.
    #[must_use]
    pub fn lookup(&self, key: &QueryKey) -> Option<(Arc<Value>, State)> {
        let mut inner = self.lock()?;
        let tick = inner.next_tick();
        let inner = &mut *inner;
        let slot = inner.slots.get_mut(&key.0)?;
        inner.order.remove(&slot.tick);
        slot.tick = tick;
        inner.order.insert(tick, key.0);
        Some((Arc::clone(&slot.value), slot.state))
    }

    /// Holds `value`, read from or written to the entry file with `stamp`,
    /// replacing any slot for `key`.
    pub fn insert_on_disk(&self, key: &QueryKey, value: Arc<Value>, stamp: FileStamp) {
        if !self.admits(&value) {
            self.remove(key);
            return;
        }
        if let Some(mut inner) = self.lock() {
            inner.insert(key.0, value, State::OnDisk(stamp), self.capacity);
        }
    }

    /// Holds `value` as pending, unless `key` already has a slot or the
    /// value is too large. Returns whether it was inserted.
    pub fn insert_pending(&self, key: &QueryKey, value: Arc<Value>) -> bool {
        if !self.admits(&value) {
            return false;
        }
        let Some(mut inner) = self.lock() else {
            return false;
        };
        if inner.slots.contains_key(&key.0) {
            return false;
        }
        inner.insert(key.0, value, State::Pending, self.capacity);
        true
    }

    /// Records the end of `value`'s disk write: with `Some(stamp)` the slot
    /// becomes on-disk with that stamp; with `None` (the write failed, or
    /// another entry was already there) it is dropped. Only a pending slot
    /// still holding this very `value` is affected.
    pub fn settle(&self, key: &QueryKey, value: &Arc<Value>, stamp: Option<FileStamp>) {
        let Some(mut inner) = self.lock() else {
            return;
        };
        let mine = inner
            .slots
            .get(&key.0)
            .is_some_and(|s| s.state == State::Pending && Arc::ptr_eq(&s.value, value));
        if !mine {
            return;
        }
        match stamp {
            Some(stamp) => {
                inner.pending = inner.pending.saturating_sub(1);
                if let Some(slot) = inner.slots.get_mut(&key.0) {
                    slot.state = State::OnDisk(stamp);
                }
            }
            None => inner.remove(&key.0),
        }
    }

    /// Drops `key`'s slot if it still holds `value`.
    pub fn remove_if(&self, key: &QueryKey, value: &Arc<Value>) {
        if let Some(mut inner) = self.lock() {
            let same = inner
                .slots
                .get(&key.0)
                .is_some_and(|s| Arc::ptr_eq(&s.value, value));
            if same {
                inner.remove(&key.0);
            }
        }
    }

    /// Drops `key`'s slot.
    pub fn remove(&self, key: &QueryKey) {
        if let Some(mut inner) = self.lock() {
            inner.remove(&key.0);
        }
    }

    /// The tier's counters.
    #[must_use]
    pub fn stats(&self) -> MemoryStats {
        let (entries, bytes, pending) = self.lock().map_or((0, 0, 0), |i| {
            (
                u64::try_from(i.slots.len()).unwrap_or(u64::MAX),
                i.bytes,
                i.pending,
            )
        });
        MemoryStats {
            capacity_bytes: self.capacity,
            entries,
            bytes,
            pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(n: usize, b: u8) -> Arc<Value> {
        Arc::new(Value {
            reply: vec![b; n],
            effort: None,
        })
    }

    fn stamp() -> FileStamp {
        match std::fs::metadata(std::env::temp_dir()) {
            Ok(m) => FileStamp::of(&m),
            Err(e) => unreachable!("no temp dir: {e}"),
        }
    }

    #[test]
    fn evicts_least_recently_used_within_budget() {
        // Room for five 100-byte replies.
        let t = MemoryTier::new(5 * (100 + SLOT_OVERHEAD) + 10);
        for i in 0..5u8 {
            t.insert_on_disk(&QueryKey([i; 32]), val(100, i), stamp());
        }
        // Touch key 0, so key 1 is the oldest.
        assert!(t.lookup(&QueryKey([0; 32])).is_some());
        t.insert_on_disk(&QueryKey([5; 32]), val(100, 5), stamp());
        assert!(t.lookup(&QueryKey([1; 32])).is_none());
        for i in [0u8, 2, 3, 4, 5] {
            assert!(t.lookup(&QueryKey([i; 32])).is_some(), "{i}");
        }
        let s = t.stats();
        assert_eq!(s.entries, 5);
        assert!(s.bytes <= s.capacity_bytes);
    }

    #[test]
    fn off_and_oversized_values_are_not_held() {
        let off = MemoryTier::new(0);
        off.insert_on_disk(&QueryKey([1; 32]), val(1, 1), stamp());
        assert!(!off.insert_pending(&QueryKey([2; 32]), val(1, 1)));
        assert_eq!(off.stats().entries, 0);
        let small = MemoryTier::new(1000);
        assert!(!small.insert_pending(&QueryKey([2; 32]), val(500, 1)));
        assert!(small.insert_pending(&QueryKey([2; 32]), val(10, 1)));
    }

    #[test]
    fn pending_slots_settle_only_for_their_own_value() {
        let t = MemoryTier::new(1 << 20);
        let k = QueryKey([5; 32]);
        let (a, b) = (val(10, b'a'), val(10, b'b'));
        assert!(t.insert_pending(&k, Arc::clone(&a)));
        // A second pending insert for the key is refused.
        assert!(!t.insert_pending(&k, Arc::clone(&b)));
        assert_eq!(t.stats().pending, 1);
        // Settling another value does nothing.
        t.settle(&k, &b, None);
        assert!(matches!(t.lookup(&k), Some((_, State::Pending))));
        t.settle(&k, &a, Some(stamp()));
        assert!(matches!(t.lookup(&k), Some((_, State::OnDisk(_)))));
        assert_eq!(t.stats().pending, 0);
        // A failed write drops the pending slot.
        let k2 = QueryKey([6; 32]);
        assert!(t.insert_pending(&k2, Arc::clone(&b)));
        t.settle(&k2, &b, None);
        assert!(t.lookup(&k2).is_none());
        // A disk read replaces a pending slot; the late settle is ignored.
        let k3 = QueryKey([7; 32]);
        assert!(t.insert_pending(&k3, Arc::clone(&a)));
        t.insert_on_disk(&k3, Arc::clone(&b), stamp());
        t.settle(&k3, &a, None);
        assert!(t.lookup(&k3).is_some_and(|(v, _)| Arc::ptr_eq(&v, &b)));
        t.remove_if(&k3, &a);
        assert!(t.lookup(&k3).is_some());
        t.remove_if(&k3, &b);
        assert!(t.lookup(&k3).is_none());
    }
}
