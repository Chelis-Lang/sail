//! Bounded immutable wire frames. Freshness and the exact stdout seal are
//! validated before every lookup; these bytes never authorize Currentness.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use sail_api::jobs::Currentness;

const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;
const ARC_HEADER_BYTES: usize = 2 * std::mem::size_of::<usize>();
const MAP_NODE_BYTES: usize = 1024;

// Rust 1.99 keeps the empty leaf root after remove/pop_first, and Arc frees its
// outer allocation only after dropping Frames (including Charge). These bytes
// therefore cannot use the refundable per-family quota. Reserve their maximum
// overlap permanently from the same 128 MiB ceiling, including fixed owners and
// each endpoint worker's off-lock retired Arc vector. No frame bytes are exempt.
const ADMISSION_MARGIN: usize = super::output_service::FIXED_BYTES
    + MAP_NODE_BYTES
    + std::mem::size_of::<ResultCache>()
    + std::mem::size_of::<Budget>()
    + ARC_HEADER_BYTES
    + (super::MAX_CLIENTS + 1)
        * (std::mem::size_of::<Frames>()
            + ARC_HEADER_BYTES
            + MAX_ENTRIES * std::mem::size_of::<Arc<Frames>>());

pub(super) struct Frames {
    current: Arc<Vec<u8>>,
    historical: Arc<Vec<u8>>,
    unknown: Arc<Vec<u8>>,
    pub(super) output: Mutex<Option<Arc<Mutex<super::output_service::OutputOwner>>>>,
    pub(super) offered: std::sync::atomic::AtomicBool,
    // Last field: physical buffers are disposed before their charge is released.
    charge: Option<Charge>,
}
impl Frames {
    pub fn new(current: Arc<Vec<u8>>, historical: Arc<Vec<u8>>, unknown: Arc<Vec<u8>>) -> Self {
        Self {
            current,
            historical,
            unknown,
            output: Mutex::new(None),
            offered: std::sync::atomic::AtomicBool::new(false),
            charge: None,
        }
    }
    fn size(&self) -> usize {
        self.current.len() + self.historical.len() + self.unknown.len()
    }
    fn retained_size(&self, key_capacity: usize) -> Option<usize> {
        // Include actual Vec capacities, all Arc allocations, the inline owner,
        // key storage and a conservative 1 KiB per indexed BTree node. The
        // pinned map layout is also audited by sail-api's ownership controls.
        let mut size = std::mem::size_of::<Self>()
            .checked_add(ARC_HEADER_BYTES)?
            .checked_add(std::mem::size_of::<String>() + key_capacity)?
            .checked_add(MAP_NODE_BYTES)?;
        for frame in [&self.current, &self.historical, &self.unknown] {
            size = size
                .checked_add(frame.capacity())?
                .checked_add(std::mem::size_of::<Vec<u8>>() + ARC_HEADER_BYTES)?;
        }
        Some(size)
    }
    pub(super) fn charged(&self) -> bool {
        self.charge.is_some()
    }
    pub fn select(self: &Arc<Self>, currentness: Currentness) -> WireFrame {
        let bytes = Arc::clone(match currentness {
            Currentness::Current => &self.current,
            Currentness::Historical => &self.historical,
            Currentness::Unknown => &self.unknown,
        });
        WireFrame {
            bytes,
            _owner: self.charge.as_ref().map(|_| Arc::clone(self)),
        }
    }
}

/// A selected immutable frame keeps the complete charged family alive through
/// socket output, even after eviction. Uncached responses retain no cache charge.
pub(super) struct WireFrame {
    bytes: Arc<Vec<u8>>,
    _owner: Option<Arc<Frames>>,
}
impl From<Arc<Vec<u8>>> for WireFrame {
    fn from(bytes: Arc<Vec<u8>>) -> Self {
        Self {
            bytes,
            _owner: None,
        }
    }
}
impl Deref for WireFrame {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.bytes.as_slice()
    }
}

pub(super) struct Budget {
    used: AtomicUsize,
    limit: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            used: AtomicUsize::new(0),
            limit: MAX_BYTES - ADMISSION_MARGIN,
        }
    }
}
impl Budget {
    #[cfg(test)]
    pub(super) fn used_for_test(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }
    pub(super) fn reserve(self: &Arc<Self>, bytes: usize) -> Option<Charge> {
        self.used
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|total| *total <= self.limit)
            })
            .ok()?;
        Some(Charge {
            budget: Arc::clone(self),
            bytes,
        })
    }
}
pub(super) struct Charge {
    budget: Arc<Budget>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
        #[cfg(test)]
        REFUND_GATE.with(|gate| {
            if let Some(gate) = gate.borrow().as_ref() {
                gate.0.wait();
                gate.1.wait();
            }
        });
    }
}

#[cfg(test)]
thread_local! {
    // Pauses the actual refund inside Frames destruction, while its outer Arc
    // allocation is still live. Only the explicitly owned test thread opts in.
    pub(super) static REFUND_GATE: std::cell::RefCell<Option<Arc<(std::sync::Barrier, std::sync::Barrier)>>> = const { std::cell::RefCell::new(None) };
}
#[derive(Default)]
struct Entries {
    values: BTreeMap<String, Arc<Frames>>,
    bytes: usize,
}
#[derive(Default)]
pub(super) struct ResultCache {
    entries: Mutex<Entries>,
    budget: Arc<Budget>,
}
impl ResultCache {
    pub(super) fn budget(&self) -> Arc<Budget> {
        Arc::clone(&self.budget)
    }
    pub fn get(&self, key: &str) -> Option<Arc<Frames>> {
        self.entries.lock().ok()?.values.get(key).cloned()
    }
    pub fn remove(&self, key: &str) {
        let old = self.entries.lock().ok().and_then(|mut entries| {
            let old = entries.values.remove(key)?;
            entries.bytes -= old.size();
            Some(old)
        });
        drop(old);
    }
    pub fn insert(&self, key: String, frames: Frames) -> Arc<Frames> {
        self.insert_bounded(key, frames, MAX_BYTES, MAX_ENTRIES)
    }
    fn insert_bounded(
        &self,
        key: String,
        mut frames: Frames,
        cap: usize,
        count: usize,
    ) -> Arc<Frames> {
        if frames.size() > cap || count == 0 {
            return Arc::new(frames);
        }
        let Some(size) = frames.retained_size(key.capacity()) else {
            return Arc::new(frames);
        };
        if size > self.budget.limit {
            return Arc::new(frames);
        }
        // Remove old indexed ownership first. An output writer's selected
        // frame continues holding its charge; eviction cannot refund it.
        self.remove(&key);
        let mut charge = self.budget.reserve(size);
        for _ in 0..MAX_ENTRIES {
            if charge.is_some() {
                break;
            }
            let old = self.entries.lock().ok().and_then(|mut entries| {
                let (_, old) = entries.values.pop_first()?;
                entries.bytes -= old.size();
                Some(old)
            });
            let Some(old) = old else { break };
            drop(old); // Never dispose large buffers under the index lock.
            charge = self.budget.reserve(size);
        }
        let Some(charge) = charge else {
            return Arc::new(frames);
        };
        frames.charge = Some(charge);
        let frames = Arc::new(frames);
        let Ok(mut entries) = self.entries.lock() else {
            return frames;
        };
        let mut retired = Vec::new();
        if let Some(old) = entries.values.remove(&key) {
            entries.bytes -= old.size();
            retired.push(old);
        }
        while entries.bytes + frames.size() > cap || entries.values.len() >= count {
            let Some(first) = entries.values.keys().next().cloned() else {
                break;
            };
            if let Some(old) = entries.values.remove(&first) {
                entries.bytes -= old.size();
                retired.push(old);
            }
        }
        entries.bytes += frames.size();
        entries.values.insert(key, Arc::clone(&frames));
        drop(entries);
        drop(retired);
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frames(n: usize) -> Frames {
        Frames::new(
            Arc::new(vec![1; n]),
            Arc::new(vec![2; n]),
            Arc::new(vec![3; n]),
        )
    }
    #[test]
    fn bounded_eviction_and_oversized_replies_never_retain_unbounded_entries() {
        let cache = ResultCache::default();
        cache.insert_bounded("a".to_owned(), frames(4), 24, 2);
        cache.insert_bounded("b".to_owned(), frames(4), 24, 2);
        cache.insert_bounded("c".to_owned(), frames(4), 24, 2);
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        let oversized = cache.insert_bounded("d".to_owned(), frames(9), 24, 2);
        assert_eq!(oversized.select(Currentness::Unknown).len(), 9);
        assert!(cache.get("d").is_none());
        cache.insert_bounded("b".to_owned(), frames(1), 24, 2);
        assert_eq!(cache.get("b").map(|f| f.size()), Some(3));
        assert_eq!(
            cache.entries.lock().ok().map(|entries| entries.bytes),
            Some(15)
        );
    }

    #[test]
    fn spare_capacity_is_charged_even_when_indexed_payload_is_tiny() {
        let cache = ResultCache {
            budget: Arc::new(Budget {
                used: AtomicUsize::new(0),
                limit: 8192,
            }),
            ..ResultCache::default()
        };
        let mut bytes = Vec::with_capacity(8192);
        bytes.push(7);
        let oversized = Frames::new(Arc::new(bytes), Arc::new(vec![8]), Arc::new(vec![9]));
        let response = cache.insert_bounded("large-capacity".to_owned(), oversized, 24, 2);
        assert_eq!(&*response.select(Currentness::Current), &[7]);
        assert!(cache.get("large-capacity").is_none());
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn evicted_in_flight_output_keeps_charge_until_its_last_physical_owner_drops() {
        let cache = ResultCache {
            budget: Arc::new(Budget {
                used: AtomicUsize::new(0),
                limit: 2000,
            }),
            ..ResultCache::default()
        };
        let first = cache.insert_bounded("a".to_owned(), frames(100), 1000, 1);
        assert!(cache.get("a").is_some());
        let output = first.select(Currentness::Current);
        drop(first);
        cache.remove("a");
        assert!(cache.budget.used.load(Ordering::Acquire) > 0);
        let refused = cache.insert_bounded("b".to_owned(), frames(100), 1000, 1);
        assert!(cache.get("b").is_none());
        assert_eq!(refused.select(Currentness::Historical).len(), 100);
        drop(refused);
        drop(output);
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
        let admitted = cache.insert_bounded("c".to_owned(), frames(100), 1000, 1);
        assert!(cache.get("c").is_some());
        assert!(cache.budget.used.load(Ordering::Acquire) <= 2000);
        drop(admitted);
        cache.remove("c");
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn concurrent_eviction_cannot_reuse_storage_held_by_an_output_writer() {
        let cache = ResultCache {
            budget: Arc::new(Budget {
                used: AtomicUsize::new(0),
                limit: 2000,
            }),
            ..ResultCache::default()
        };
        let entered = std::sync::Barrier::new(5);
        let release = std::sync::Barrier::new(5);
        let (blocked, charged, correct) = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|index| {
                    let cache = &cache;
                    let entered = &entered;
                    let release = &release;
                    scope.spawn(move || {
                        let frame = cache.insert_bounded(index.to_string(), frames(100), 1000, 4);
                        let output = frame.select(Currentness::Current);
                        drop(frame);
                        entered.wait();
                        release.wait();
                        output.iter().all(|byte| *byte == 1) && output.len() == 100
                    })
                })
                .collect();
            entered.wait();
            let probe = cache.insert_bounded("probe".to_owned(), frames(100), 1000, 4);
            let blocked = cache.get("probe").is_none();
            let charged = cache.budget.used.load(Ordering::Acquire);
            drop(probe);
            // Release every worker before assertions, including on a bad outcome.
            release.wait();
            let correct = handles
                .into_iter()
                .all(|handle| handle.join().unwrap_or(false));
            (blocked, charged, correct)
        });
        assert!(blocked);
        assert!(charged > 0 && charged <= 2000);
        assert!(correct);
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
        let restored = cache.insert_bounded("restored".to_owned(), frames(100), 1000, 4);
        assert!(cache.get("restored").is_some());
        drop(restored);
        cache.remove("restored");
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn fixed_margin_survives_empty_index_and_actual_outer_arc_refund() {
        let cache = ResultCache::default();
        let first = cache.insert("a".to_owned(), frames(1));
        let output = first.select(Currentness::Current);
        drop(first);
        cache.remove("a"); // Pinned BTreeMap retains its empty root.
        let gate = Arc::new((std::sync::Barrier::new(2), std::sync::Barrier::new(2)));
        let (empty, refunded, refused, fits, joined) = std::thread::scope(|scope| {
            let worker_gate = Arc::clone(&gate);
            let worker = scope.spawn(move || {
                REFUND_GATE.with(|slot| *slot.borrow_mut() = Some(worker_gate));
                drop(output); // Charge refunds before the outer Arc can deallocate.
                REFUND_GATE.with(|slot| *slot.borrow_mut() = None);
            });
            gate.0.wait();
            let empty = cache
                .entries
                .lock()
                .is_ok_and(|entries| entries.values.is_empty());
            let refunded = cache.budget.used.load(Ordering::Acquire) == 0;
            let refused = cache.budget.reserve(MAX_BYTES).is_none();
            let reservation = cache.budget.reserve(cache.budget.limit);
            let fits = reservation.is_some()
                && cache.budget.used.load(Ordering::Acquire) + ADMISSION_MARGIN == MAX_BYTES;
            drop(reservation);
            // Release/join the real destructor before any assertion.
            gate.1.wait();
            (empty, refunded, refused, fits, worker.join().is_ok())
        });
        assert!(empty && refunded && refused && fits && joined);
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn fixed_margin_covers_pinned_index_and_all_endpoint_retirement_owners() {
        // Rust 1.99 B=6: eleven String/Arc slots, parent pointer/index/length,
        // and (for internal nodes) twelve pointer edges. Include worst-case
        // padding at each fixed metadata member, rather than assuming repr(C).
        let node_upper = 11 * (std::mem::size_of::<String>() + std::mem::size_of::<Arc<Frames>>())
            + 15 * std::mem::size_of::<usize>();
        assert!(node_upper <= MAP_NODE_BYTES);
        let fixed =
            std::mem::size_of::<ResultCache>() + std::mem::size_of::<Budget>() + ARC_HEADER_BYTES;
        let retiring = (super::super::MAX_CLIENTS + 1)
            * (std::mem::size_of::<Frames>()
                + ARC_HEADER_BYTES
                + MAX_ENTRIES * std::mem::size_of::<Arc<Frames>>());
        assert_eq!(
            ADMISSION_MARGIN,
            super::super::output_service::FIXED_BYTES + MAP_NODE_BYTES + fixed + retiring
        );
        const { assert!(ADMISSION_MARGIN < MAX_BYTES) };
        assert_eq!(
            ResultCache::default().budget.limit + ADMISSION_MARGIN,
            MAX_BYTES
        );
    }

    #[test]
    fn quota_refused_output_drops_unused_variants_with_the_uncached_family() {
        let cache = ResultCache {
            budget: Arc::new(Budget {
                used: AtomicUsize::new(0),
                limit: 2000,
            }),
            ..ResultCache::default()
        };
        let first = cache.insert_bounded("a".to_owned(), frames(100), 1000, 1);
        let held = first.select(Currentness::Current);
        drop(first);
        let historical = Arc::new(vec![8]);
        let unknown = Arc::new(vec![9]);
        let historical_weak = Arc::downgrade(&historical);
        let unknown_weak = Arc::downgrade(&unknown);
        let refused = cache.insert_bounded(
            "b".to_owned(),
            Frames::new(Arc::new(vec![7; 100]), historical, unknown),
            1000,
            1,
        );
        assert!(cache.get("b").is_none());
        assert!(refused.charge.is_none());
        let output = refused.select(Currentness::Current);
        drop(refused);
        assert!(historical_weak.upgrade().is_none());
        assert!(unknown_weak.upgrade().is_none());
        assert_eq!(&*output, &[7; 100]);
        assert!(cache.budget.used.load(Ordering::Acquire) > 0);
        drop(held);
        assert_eq!(cache.budget.used.load(Ordering::Acquire), 0);
    }
}
