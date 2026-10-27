//! In-memory candidate cache with insertion-issued completion and removal handles.
//!
//! Handles retain their original tier. A replacement gets a fresh generation,
//! even when its Arc or bytes are identical. No state certifies solver origin.

use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

use sail_acceptance::cache_candidate::{
    self, Accounting, CandidateWrite, Clock, Completion, EntryUpdate, Held, Prepared,
    Slot as CandidateSlot, Ticket,
};
use sail_key::QueryKey;
use serde::{Deserialize, Serialize};

use crate::{DiskCache, Effort, FileStamp, PutOutcome, StoreContext};

/// Original per-entry bookkeeping allowance; new retained carriers are additional.
pub const ENTRY_OVERHEAD: u64 = 128;
/// Bytes charged per slot, including its immutable eight-byte insertion generation.
pub const SLOT_OVERHEAD: u64 = ENTRY_OVERHEAD + 8;
/// The actual source-shared cache value, with the native effort representation.
pub type Value = cache_candidate::CandidateValue<Effort>;
/// The actual source-shared candidate persistence state.
pub type State = cache_candidate::SlotState<FileStamp>;
type Slot = CandidateSlot<Arc<Value>, FileStamp>;
type Update = EntryUpdate<Arc<Value>, FileStamp>;

fn value_cost(value: &Value) -> u64 {
    u64::try_from(value.reply.len())
        .unwrap_or(u64::MAX)
        .saturating_add(SLOT_OVERHEAD)
}

/// Additional retained tier metadata: two Arc strong/weak header words and
/// the owning pointer. Allocator bookkeeping and RSS are not this logical budget.
#[must_use]
pub fn tier_overhead() -> u64 {
    u64::try_from(std::mem::size_of::<usize>())
        .unwrap_or(u64::MAX)
        .saturating_mul(3)
}

/// Charge the actual queued carrier in addition to the original entry allowance.
#[must_use]
pub fn candidate_bytes(candidate: &CandidateWrite<Effort>) -> u64 {
    let extra = std::mem::size_of::<PreparedWrite>()
        .saturating_sub(std::mem::size_of::<CandidateWrite<Effort>>());
    u64::try_from(candidate.value().reply.len())
        .unwrap_or(u64::MAX)
        .saturating_add(ENTRY_OVERHEAD)
        .saturating_add(u64::try_from(extra).unwrap_or(u64::MAX))
}

#[derive(Debug)]
struct Inner {
    slots: HashMap<[u8; 32], Slot>,
    /// Least recently used first.
    order: BTreeMap<u64, [u8; 32]>,
    /// This same clock issues insertion generations and LRU touch ordinals.
    clock: Clock,
    accounting: Accounting,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            order: BTreeMap::new(),
            clock: Clock::start(),
            accounting: Accounting::empty(),
        }
    }
}

impl Inner {
    fn disable(&mut self) {
        // Preserve exhaustion forever; clearing never reconstructs Inner/Clock.
        self.clock.exhaust();
        self.slots.clear();
        self.order.clear();
        self.accounting.clear();
    }

    fn next_tick(&mut self) -> Option<u64> {
        let tick = self.clock.advance();
        if tick.is_none() {
            self.disable();
        }
        tick
    }

    fn remove(&mut self, key: &[u8; 32]) {
        if let Some(entry) = self.slots.remove_entry(key) {
            self.apply(EntryUpdate::Removed(entry));
        }
    }

    fn apply(&mut self, update: Update) {
        // Charge this exact owned slot before the shared operation returns the
        // same transition. Native maps/lock/LRU still need their separate join.
        let cost = value_cost(update.slot().value());
        match self.accounting.apply(update, cost) {
            EntryUpdate::Unchanged((key, slot)) | EntryUpdate::Written((key, slot)) => {
                self.slots.insert(key, slot);
            }
            EntryUpdate::Removed((_, slot)) => {
                self.order.remove(&slot.tick());
            }
        }
    }

    fn complete(&mut self, ticket: &Ticket, completion: Completion<FileStamp>) {
        let Some(entry) = self.slots.remove_entry(&ticket.key()) else {
            return;
        };
        self.apply(cache_candidate::complete(entry, ticket, completion));
    }

    fn remove_observed(&mut self, ticket: &Ticket) {
        let Some(entry) = self.slots.remove_entry(&ticket.key()) else {
            return;
        };
        self.apply(cache_candidate::remove_observed(entry, ticket));
    }

    fn install(&mut self, entry: ([u8; 32], Slot), capacity: u64) -> bool {
        let (key, slot) = entry;
        if !self.accounting.reserve(&slot, value_cost(slot.value())) {
            self.disable();
            return false;
        }
        let tick = slot.tick();
        self.slots.insert(key, slot);
        self.order.insert(tick, key);
        while self.accounting.bytes() > capacity {
            let Some((_, oldest)) = self.order.pop_first() else {
                self.disable();
                return false;
            };
            self.remove(&oldest);
        }
        self.slots.contains_key(&key)
    }

    fn insert_on_disk(
        &mut self,
        key: [u8; 32],
        value: Arc<Value>,
        stamp: FileStamp,
        capacity: u64,
    ) {
        self.remove(&key);
        let Some((entry, _)) = self.clock.entry(key, value, State::OnDisk(stamp)) else {
            self.disable();
            return;
        };
        self.install(entry, capacity);
    }
}

/// An opaque actual tier endpoint. Its fields and completion entry point are
/// private; public cache data cannot manufacture or redirect this capability.
#[derive(Debug)]
struct MemoryEndpoint {
    inner: Arc<Mutex<Inner>>,
}

impl MemoryEndpoint {
    fn complete(&self, ticket: &Ticket, completion: Completion<FileStamp>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.complete(ticket, completion);
        }
    }
}

/// Owns the original candidate and its insertion-issued endpoint/ticket.
/// Only the actual disk call below can dispatch completion in production.
#[derive(Debug)]
pub struct PreparedWrite {
    queued: Prepared<MemoryEndpoint, Effort>,
}

impl PreparedWrite {
    /// The exact original queue key.
    #[must_use]
    pub fn key(&self) -> QueryKey {
        QueryKey(self.queued.candidate().key())
    }

    /// The original candidate plus the actual retained queued carrier.
    #[must_use]
    pub fn charged_bytes(&self) -> u64 {
        candidate_bytes(self.queued.candidate())
    }

    /// Calls the actual disk writer with every original field, then dispatches
    /// precisely its returned Result to the original insertion endpoint.
    /// # Errors
    /// Returns the original disk-operation error unchanged.
    pub fn write(self, disk: &DiskCache) -> std::io::Result<PutOutcome> {
        let arguments = self.queued.candidate().arguments();
        let result = disk.put_scoped(
            &QueryKey(arguments.key()),
            arguments.reply(),
            arguments.command(),
            &StoreContext {
                solver: arguments.solver(),
                provenance: arguments.provenance(),
                mode: arguments.mode(),
            },
            arguments.effort(),
        );
        let (_, pending) = self.queued.into_parts();
        if let Some(held) = pending {
            held.endpoint()
                .complete(held.ticket(), cache_candidate::observed_completion(&result));
        }
        result
    }
}

/// A read observation bound to its actual tier and insertion, without another
/// owning endpoint clone on the memory-hit path.
#[derive(Debug)]
pub struct ObservedValue<'tier> {
    value: Arc<Value>,
    held: Held<&'tier MemoryTier>,
}

impl Deref for ObservedValue<'_> {
    type Target = Arc<Value>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl ObservedValue<'_> {
    /// Return the exact value observed from the actual slot.
    #[must_use]
    pub fn into_value(self) -> Arc<Value> {
        self.value
    }

    /// Remove only that same insertion from the original tier. Replacements,
    /// including reinsertion of the same Arc, are untouched.
    pub fn remove_if_current(&self) {
        self.held.endpoint().remove_observed(self.held.ticket());
    }
}

/// Counters describing the tier's contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStats {
    /// The configured budget in bytes (0: the tier is off).
    pub capacity_bytes: u64,
    /// Slots held.
    pub entries: u64,
    /// Charged replies, slots and additional retained tier metadata.
    pub bytes: u64,
    /// Slots whose disk write is still queued.
    pub pending: u64,
}

/// The actual memory tier. Its endpoint and clock are initialized once and
/// never replaced by clearing, lookup, insertion or completion.
#[derive(Debug)]
pub struct MemoryTier {
    capacity: u64,
    inner: Option<Arc<Mutex<Inner>>>,
}

impl MemoryTier {
    /// A tier holding at most `capacity` charged bytes (0 turns it off).
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            inner: (capacity > tier_overhead()).then(|| Arc::new(Mutex::new(Inner::default()))),
        }
    }

    fn lock(&self) -> Option<MutexGuard<'_, Inner>> {
        self.inner.as_ref()?.lock().ok()
    }

    fn usable_capacity(&self) -> u64 {
        self.capacity.saturating_sub(tier_overhead())
    }

    fn admits(&self, value: &Value) -> bool {
        self.inner.is_some() && value_cost(value) <= self.usable_capacity() / 4
    }

    /// Observe `key` and touch its LRU position while retaining its original
    /// insertion generation and original tier. Exhaustion is a permanent miss.
    #[must_use]
    pub fn lookup(&self, key: &QueryKey) -> Option<(ObservedValue<'_>, State)> {
        let mut inner = self.lock()?;
        let tick = inner.next_tick()?;
        let inner = &mut *inner;
        let slot = inner.slots.get_mut(&key.0)?;
        inner.order.remove(&slot.tick());
        slot.touch(tick);
        inner.order.insert(tick, key.0);
        Some((
            ObservedValue {
                value: Arc::clone(slot.value()),
                held: slot.observe_bound(self, key.0),
            },
            *slot.state(),
        ))
    }

    /// Retain a candidate read from disk under a newly issued insertion generation.
    pub fn insert_on_disk(&self, key: &QueryKey, value: Arc<Value>, stamp: FileStamp) {
        if !self.admits(&value) {
            self.remove(key);
            return;
        }
        if let Some(mut inner) = self.lock() {
            inner.insert_on_disk(key.0, value, stamp, self.usable_capacity());
        }
    }

    /// Prepare the actual queued candidate. Memory refusal still permits the
    /// original disk write, without creating a pending capability.
    #[must_use]
    pub fn prepare(&self, candidate: CandidateWrite<Effort>) -> PreparedWrite {
        let without_memory = |candidate| PreparedWrite {
            queued: Prepared::without_memory(candidate),
        };
        if !self.admits(candidate.value()) {
            return without_memory(candidate);
        }
        let Some(endpoint) = self.inner.as_ref() else {
            return without_memory(candidate);
        };
        let Ok(mut inner) = endpoint.lock() else {
            return without_memory(candidate);
        };
        if inner.slots.contains_key(&candidate.key()) {
            return without_memory(candidate);
        }
        let issued = inner.clock.prepare::<_, _, FileStamp>(
            MemoryEndpoint {
                inner: Arc::clone(endpoint),
            },
            candidate,
        );
        match issued {
            Ok((entry, queued)) => {
                if inner.install(entry, self.usable_capacity()) {
                    PreparedWrite { queued }
                } else {
                    let (candidate, _) = queued.into_parts();
                    without_memory(candidate)
                }
            }
            Err(candidate) => {
                inner.disable();
                without_memory(candidate)
            }
        }
    }

    fn remove_observed(&self, ticket: &Ticket) {
        if let Some(mut inner) = self.lock() {
            inner.remove_observed(ticket);
        }
    }

    /// Drop any slot for `key`; this administrative removal does not reset time.
    pub fn remove(&self, key: &QueryKey) {
        if let Some(mut inner) = self.lock() {
            inner.remove(&key.0);
        }
    }

    /// Current charged occupancy, including the retained endpoint metadata.
    #[must_use]
    pub fn stats(&self) -> MemoryStats {
        let fixed = if self.inner.is_some() {
            tier_overhead()
        } else {
            0
        };
        let Some(inner) = self.lock() else {
            return MemoryStats {
                capacity_bytes: self.capacity,
                bytes: fixed,
                ..MemoryStats::default()
            };
        };
        MemoryStats {
            capacity_bytes: self.capacity,
            entries: u64::try_from(inner.slots.len()).unwrap_or(u64::MAX),
            bytes: inner.accounting.bytes().saturating_add(fixed),
            pending: inner.accounting.pending(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(n: usize, b: u8) -> Arc<Value> {
        Arc::new(Value {
            mode: "default".to_owned(),
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
        // Room for the original five 100-byte slots plus the actual new tier metadata.
        let t = MemoryTier::new(5 * (100 + SLOT_OVERHEAD) + tier_overhead() + 10);
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

    // These fixtures dispatch a supplied outcome only after real insertion;
    // actual DiskCache return dispatch is exercised in separate native tests.
    fn insert_pending(
        tier: &MemoryTier,
        key: &QueryKey,
        value: Arc<Value>,
    ) -> Option<Held<MemoryEndpoint>> {
        let candidate = CandidateWrite::new(
            key.0,
            value,
            "check-sat".to_owned(),
            "memory-fixture".to_owned(),
            "fixture-solver".to_owned(),
        );
        let (_, pending) = tier.prepare(candidate).queued.into_parts();
        pending
    }

    fn settle(pending: Held<MemoryEndpoint>, stamp: Option<FileStamp>) {
        let result: std::io::Result<PutOutcome> = Ok(match stamp {
            Some(stamp) => PutOutcome::Written(stamp),
            None => PutOutcome::Exists,
        });
        pending.endpoint().complete(
            pending.ticket(),
            cache_candidate::observed_completion(&result),
        );
    }

    fn required_pending(
        pending: Option<Held<MemoryEndpoint>>,
    ) -> std::io::Result<Held<MemoryEndpoint>> {
        pending.ok_or_else(|| std::io::Error::other("expected an insertion-issued pending handle"))
    }

    #[test]
    fn off_and_oversized_values_are_not_held() {
        let off = MemoryTier::new(0);
        off.insert_on_disk(&QueryKey([1; 32]), val(1, 1), stamp());
        assert!(insert_pending(&off, &QueryKey([2; 32]), val(1, 1)).is_none());
        assert_eq!(off.stats().entries, 0);
        let small = MemoryTier::new(1000);
        assert!(insert_pending(&small, &QueryKey([2; 32]), val(500, 1)).is_none());
        assert!(insert_pending(&small, &QueryKey([2; 32]), val(10, 1)).is_some());
    }

    #[test]
    fn pending_slots_settle_only_for_their_own_value() -> std::io::Result<()> {
        let t = MemoryTier::new(1 << 20);
        let k = QueryKey([5; 32]);
        let (a, b) = (val(10, b'a'), val(10, b'b'));
        let first = insert_pending(&t, &k, Arc::clone(&a));
        assert!(first.is_some());
        let first = required_pending(first)?;
        // A second pending insert for the key is refused.
        assert!(insert_pending(&t, &k, Arc::clone(&b)).is_none());
        assert_eq!(t.stats().pending, 1);
        // Another insertion's handle cannot target the original key or endpoint.
        let unrelated = required_pending(insert_pending(&t, &QueryKey([9; 32]), Arc::clone(&b)))?;
        settle(unrelated, None);
        assert!(matches!(t.lookup(&k), Some((_, State::Pending))));
        settle(first, Some(stamp()));
        assert!(matches!(t.lookup(&k), Some((_, State::OnDisk(_)))));
        assert_eq!(t.stats().pending, 0);
        // A refused write drops the pending slot.
        let k2 = QueryKey([6; 32]);
        let second = insert_pending(&t, &k2, Arc::clone(&b));
        assert!(second.is_some());
        settle(required_pending(second)?, None);
        assert!(t.lookup(&k2).is_none());
        // A disk read replaces a pending slot; late completion/removal are ignored.
        let k3 = QueryKey([7; 32]);
        let third = insert_pending(&t, &k3, Arc::clone(&a));
        assert!(third.is_some());
        let old = t
            .lookup(&k3)
            .ok_or_else(|| std::io::Error::other("missing old observation"))?
            .0;
        t.insert_on_disk(&k3, Arc::clone(&b), stamp());
        let current = t
            .lookup(&k3)
            .ok_or_else(|| std::io::Error::other("missing replacement observation"))?
            .0;
        settle(required_pending(third)?, None);
        assert!(t.lookup(&k3).is_some_and(|(v, _)| Arc::ptr_eq(&v, &b)));
        old.remove_if_current();
        assert!(t.lookup(&k3).is_some());
        current.remove_if_current();
        assert!(t.lookup(&k3).is_none());
        Ok(())
    }

    #[test]
    fn same_arc_and_equal_bytes_do_not_reuse_an_insertion() -> std::io::Result<()> {
        let t = MemoryTier::new(1 << 20);
        let key = QueryKey([31; 32]);
        let original = val(10, b'a');
        let pending = required_pending(insert_pending(&t, &key, Arc::clone(&original)))?;
        let read = t
            .lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing original"))?
            .0;
        t.remove(&key);
        let replacement = required_pending(insert_pending(&t, &key, Arc::clone(&original)))?;
        settle(pending, Some(stamp()));
        assert!(matches!(t.lookup(&key), Some((_, State::Pending))));
        read.remove_if_current();
        assert!(
            t.lookup(&key)
                .is_some_and(|(value, _)| Arc::ptr_eq(&value, &original))
        );
        settle(replacement, None);
        assert!(t.lookup(&key).is_none());
        let pending = required_pending(insert_pending(&t, &key, Arc::clone(&original)))?;
        let equal = val(10, b'a');
        assert_eq!(original.as_ref(), equal.as_ref());
        assert!(!Arc::ptr_eq(&original, &equal));
        t.insert_on_disk(&key, Arc::clone(&equal), stamp());
        settle(pending, None);
        assert!(
            t.lookup(&key)
                .is_some_and(|(value, _)| Arc::ptr_eq(&value, &equal))
        );
        Ok(())
    }

    #[test]
    fn actual_owned_accounting_preserves_stale_replacement_and_retirement() -> std::io::Result<()> {
        let t = MemoryTier::new(1 << 20);
        let key = QueryKey([71; 32]);
        let original = val(6, b'a');
        let pending = required_pending(insert_pending(&t, &key, original))?;
        let old = t
            .lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing accounting observation"))?
            .0;
        assert_eq!(t.stats().bytes, tier_overhead() + SLOT_OVERHEAD + 6);
        assert_eq!(t.stats().pending, 1);
        // Replacement first retires the original actual pending slot, then
        // reserves the different-sized actual on-disk slot.
        t.insert_on_disk(&key, val(17, b'b'), stamp());
        assert_eq!(t.stats().bytes, tier_overhead() + SLOT_OVERHEAD + 17);
        assert_eq!(t.stats().pending, 0);
        settle(pending, None);
        old.remove_if_current();
        assert_eq!(t.stats().entries, 1);
        assert_eq!(t.stats().bytes, tier_overhead() + SLOT_OVERHEAD + 17);
        assert_eq!(t.stats().pending, 0);
        t.lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing accounting replacement"))?
            .0
            .remove_if_current();
        assert_eq!(t.stats().entries, 0);
        assert_eq!(t.stats().bytes, tier_overhead());
        assert_eq!(t.stats().pending, 0);
        Ok(())
    }

    #[test]
    fn retained_metadata_is_charged_before_quarter_budget_admission() {
        let key = QueryKey([61; 32]);
        let slot = SLOT_OVERHEAD + 1;
        let t = MemoryTier::new(tier_overhead() + 4 * slot);
        assert!(insert_pending(&t, &key, val(1, b'a')).is_some());
        assert_eq!(t.stats().bytes, tier_overhead() + slot);
        // A second key one byte above the actual quarter limit is not retained.
        assert!(insert_pending(&t, &QueryKey([62; 32]), val(2, b'a')).is_none());
        assert_eq!(t.stats().entries, 1);
        assert_eq!(t.stats().pending, 1);
        assert!(t.stats().bytes <= t.stats().capacity_bytes);
    }

    #[test]
    fn handles_keep_their_original_tier_with_equal_local_generations() -> std::io::Result<()> {
        let first = MemoryTier::new(1 << 20);
        let second = MemoryTier::new(1 << 20);
        let key = QueryKey([33; 32]);
        let a = required_pending(insert_pending(&first, &key, val(10, b'a')))?;
        let b = required_pending(insert_pending(&second, &key, val(10, b'a')))?;
        // Both fresh tiers issued generation one; each handle acts only on its endpoint.
        settle(a, Some(stamp()));
        assert!(matches!(first.lookup(&key), Some((_, State::OnDisk(_)))));
        assert!(matches!(second.lookup(&key), Some((_, State::Pending))));
        settle(b, None);
        assert!(second.lookup(&key).is_none());
        assert!(first.lookup(&key).is_some());
        second.insert_on_disk(&key, val(10, b'a'), stamp());
        let read = first
            .lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing first-tier read"))?
            .0;
        read.remove_if_current();
        assert!(first.lookup(&key).is_none());
        assert!(second.lookup(&key).is_some());
        Ok(())
    }

    #[test]
    fn exhausted_clock_never_recovers_after_clearing() -> std::io::Result<()> {
        let t = MemoryTier::new(1 << 20);
        let key = QueryKey([34; 32]);
        let pending = required_pending(insert_pending(&t, &key, val(10, b'a')))?;
        let read = t
            .lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing read before exhaustion"))?
            .0;
        {
            let mut inner = t
                .lock()
                .ok_or_else(|| std::io::Error::other("poisoned test tier"))?;
            inner.clock.exhaust();
        }
        assert!(t.lookup(&key).is_none());
        settle(pending, Some(stamp()));
        read.remove_if_current();
        t.insert_on_disk(&key, val(10, b'a'), stamp());
        assert!(insert_pending(&t, &key, val(10, b'a')).is_none());
        assert!(t.lookup(&key).is_none());
        assert_eq!(t.stats().entries, 0);
        assert_eq!(t.stats().pending, 0);
        let inner = t
            .lock()
            .ok_or_else(|| std::io::Error::other("poisoned exhausted tier"))?;
        // An exhausted clock refuses forever; clearing did not replace it.
        drop(inner);
        assert!(t.lookup(&QueryKey([35; 32])).is_none());
        Ok(())
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sail-memory-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::remove_file(&path);
        path
    }

    fn candidate(key: QueryKey, value: Arc<Value>) -> CandidateWrite<Effort> {
        CandidateWrite::new(
            key.0,
            value,
            "check-sat".to_owned(),
            "original-fixture".to_owned(),
            "original-solver".to_owned(),
        )
    }

    #[test]
    fn prepared_write_preserves_actual_disk_fields_and_stamp() -> std::io::Result<()> {
        let path = scratch("written");
        let disk = DiskCache::new(&path);
        let t = MemoryTier::new(1 << 20);
        let key = QueryKey([41; 32]);
        let effort = Effort {
            limit: Some(100),
            used: Some(7),
            reason: Some("original-reason".to_owned()),
        };
        let original = Arc::new(Value {
            mode: "default".to_owned(),
            reply: b"unsat\n".to_vec(),
            effort: Some(effort.clone()),
        });
        let prepared = t.prepare(candidate(key, Arc::clone(&original)));
        assert!(matches!(t.lookup(&key), Some((_, State::Pending))));
        let result = prepared.write(&disk)?;
        let PutOutcome::Written(stamp) = result else {
            return Err(std::io::Error::other("expected actual write"));
        };
        assert_eq!(disk.stamp_for_hit(&key), Some(stamp));
        let (observed, state) = t
            .lookup(&key)
            .ok_or_else(|| std::io::Error::other("missing written insertion"))?;
        assert_eq!(state, State::OnDisk(stamp));
        assert!(Arc::ptr_eq(&observed, &original));
        let (metadata, reply) = disk
            .get(&key)
            .ok_or_else(|| std::io::Error::other("missing persisted candidate"))?;
        assert_eq!(metadata.key, key.to_string());
        assert_eq!(metadata.mode, "default");
        assert_eq!(metadata.command, "check-sat");
        assert_eq!(metadata.solver, "original-solver");
        assert_eq!(metadata.provenance, "original-fixture");
        assert_eq!(metadata.effort, Some(effort));
        assert_eq!(reply, b"unsat\n");
        assert_eq!(t.stats().pending, 0);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    #[test]
    fn prepared_write_exists_and_error_remove_only_original_pending_slot() -> std::io::Result<()> {
        let path = scratch("refusal");
        let disk = DiskCache::new(&path);
        let t = MemoryTier::new(1 << 20);
        let key = QueryKey([42; 32]);
        t.prepare(candidate(
            key,
            Arc::new(Value {
                mode: "default".to_owned(),
                reply: b"unsat\n".to_vec(),
                effort: None,
            }),
        ))
        .write(&disk)?;
        t.remove(&key);
        let refused = t.prepare(candidate(
            key,
            Arc::new(Value {
                mode: "default".to_owned(),
                reply: b"sat\n".to_vec(),
                effort: None,
            }),
        ));
        assert!(matches!(t.lookup(&key), Some((_, State::Pending))));
        assert!(matches!(refused.write(&disk)?, PutOutcome::Exists));
        assert!(t.lookup(&key).is_none());
        assert_eq!(
            disk.get(&key)
                .ok_or_else(|| std::io::Error::other("lost original disk record"))?
                .1,
            b"unsat\n"
        );
        let invalid = scratch("invalid-root");
        std::fs::write(&invalid, b"not a directory")?;
        let invalid_disk = DiskCache::new(&invalid);
        let failed_key = QueryKey([43; 32]);
        let failed = t.prepare(candidate(failed_key, val(10, b'a')));
        assert!(matches!(t.lookup(&failed_key), Some((_, State::Pending))));
        assert!(failed.write(&invalid_disk).is_err());
        assert!(t.lookup(&failed_key).is_none());
        assert_eq!(t.stats().pending, 0);
        std::fs::remove_file(invalid)?;
        std::fs::remove_dir_all(path)?;
        Ok(())
    }
}
