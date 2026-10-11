//! Bounded immutable wire frames. Freshness and the exact stdout seal are
//! validated before every lookup; these bytes never authorize Currentness.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use sail_api::jobs::Currentness;

const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;

pub(super) struct Frames {
    current: Arc<Vec<u8>>,
    historical: Arc<Vec<u8>>,
    unknown: Arc<Vec<u8>>,
    // Last field: physical buffers are disposed before their charge is released.
    charge: Option<Charge>,
}
impl Frames {
    pub fn new(current: Arc<Vec<u8>>, historical: Arc<Vec<u8>>, unknown: Arc<Vec<u8>>) -> Self {
        Self {
            current,
            historical,
            unknown,
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
            .checked_add(2 * std::mem::size_of::<usize>())?
            .checked_add(std::mem::size_of::<String>() + key_capacity)?
            .checked_add(1024)?;
        for frame in [&self.current, &self.historical, &self.unknown] {
            size = size
                .checked_add(frame.capacity())?
                .checked_add(std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>())?;
        }
        Some(size)
    }
    pub fn select(self: &Arc<Self>, currentness: Currentness) -> WireFrame {
        let bytes = Arc::clone(match currentness {
            Currentness::Current => &self.current,
            Currentness::Historical => &self.historical,
            Currentness::Unknown => &self.unknown,
        });
        WireFrame {
            bytes,
            _owner: Some(Arc::clone(self)),
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

struct Budget {
    used: AtomicUsize,
    limit: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            used: AtomicUsize::new(0),
            limit: MAX_BYTES,
        }
    }
}
impl Budget {
    fn reserve(self: &Arc<Self>, bytes: usize) -> Option<Charge> {
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
struct Charge {
    budget: Arc<Budget>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
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
}
