//! A small LRU of immutable encoded Result frames. Freshness and the exact
//! stdout seal are validated before every lookup; these bytes never authorize
//! Currentness.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sail_api::jobs::Currentness;

/// Bound on the indexed frame bytes. Frames already selected for an
/// in-flight response stay alive until written, even after eviction.
const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;

/// One completed job's encoded responses, one per possible Currentness.
pub(super) struct Frames {
    current: Arc<Vec<u8>>,
    historical: Arc<Vec<u8>>,
    unknown: Arc<Vec<u8>>,
}
impl Frames {
    pub fn new(current: Arc<Vec<u8>>, historical: Arc<Vec<u8>>, unknown: Arc<Vec<u8>>) -> Self {
        Self {
            current,
            historical,
            unknown,
        }
    }
    fn size(&self) -> usize {
        self.current.len() + self.historical.len() + self.unknown.len()
    }
    pub fn select(&self, currentness: Currentness) -> Arc<Vec<u8>> {
        Arc::clone(match currentness {
            Currentness::Current => &self.current,
            Currentness::Historical => &self.historical,
            Currentness::Unknown => &self.unknown,
        })
    }
}

#[derive(Default)]
struct Entries {
    /// Key to (last use, frames).
    values: BTreeMap<String, (u64, Arc<Frames>)>,
    bytes: usize,
    clock: u64,
}
impl Entries {
    fn remove(&mut self, key: &str) -> Option<Arc<Frames>> {
        let (_, old) = self.values.remove(key)?;
        self.bytes -= old.size();
        Some(old)
    }
    fn evict_oldest(&mut self) -> Option<Arc<Frames>> {
        let key = self
            .values
            .iter()
            .min_by_key(|(_, (used, _))| *used)
            .map(|(key, _)| key.clone())?;
        self.remove(&key)
    }
}

#[derive(Default)]
pub(super) struct ResultCache {
    entries: Mutex<Entries>,
}
impl ResultCache {
    pub fn get(&self, key: &str) -> Option<Arc<Frames>> {
        let mut entries = self.entries.lock().ok()?;
        entries.clock += 1;
        let clock = entries.clock;
        let (used, frames) = entries.values.get_mut(key)?;
        *used = clock;
        Some(Arc::clone(frames))
    }
    pub fn remove(&self, key: &str) {
        let old = self
            .entries
            .lock()
            .ok()
            .and_then(|mut entries| entries.remove(key));
        drop(old); // Never dispose large buffers under the index lock.
    }
    pub fn insert(&self, key: String, frames: Frames) -> Arc<Frames> {
        self.insert_bounded(key, frames, MAX_BYTES, MAX_ENTRIES)
    }
    fn insert_bounded(&self, key: String, frames: Frames, cap: usize, count: usize) -> Arc<Frames> {
        let frames = Arc::new(frames);
        if frames.size() > cap || count == 0 {
            return frames;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return frames;
        };
        let mut retired: Vec<_> = entries.remove(&key).into_iter().collect();
        while entries.bytes + frames.size() > cap || entries.values.len() >= count {
            let Some(old) = entries.evict_oldest() else {
                break;
            };
            retired.push(old);
        }
        entries.clock += 1;
        let clock = entries.clock;
        entries.bytes += frames.size();
        entries.values.insert(key, (clock, Arc::clone(&frames)));
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
    fn bytes(cache: &ResultCache) -> Option<usize> {
        cache.entries.lock().ok().map(|entries| entries.bytes)
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
        assert_eq!(bytes(&cache), Some(15));
    }

    #[test]
    fn eviction_removes_the_least_recently_used_entry() {
        let cache = ResultCache::default();
        cache.insert_bounded("a".to_owned(), frames(1), 100, 2);
        cache.insert_bounded("b".to_owned(), frames(1), 100, 2);
        assert!(cache.get("a").is_some());
        cache.insert_bounded("c".to_owned(), frames(1), 100, 2);
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn evicted_frames_stay_valid_for_an_in_flight_response() {
        let cache = ResultCache::default();
        let first = cache.insert_bounded("a".to_owned(), frames(100), 300, 1);
        let output = first.select(Currentness::Current);
        drop(first);
        cache.insert_bounded("b".to_owned(), frames(100), 300, 1);
        assert!(cache.get("a").is_none());
        assert_eq!(&*output, &[1; 100]);
        cache.remove("b");
        assert_eq!(bytes(&cache), Some(0));
    }
}
