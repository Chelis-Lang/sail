//! Bounded immutable wire frames. Freshness and the exact stdout seal are
//! validated before every lookup; these bytes never authorize Currentness.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sail_api::jobs::Currentness;

const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;

pub(super) struct Frames {
    pub current: Arc<Vec<u8>>,
    pub historical: Arc<Vec<u8>>,
    pub unknown: Arc<Vec<u8>>,
}
impl Frames {
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
    values: BTreeMap<String, Arc<Frames>>,
    bytes: usize,
}
#[derive(Default)]
pub(super) struct ResultCache(Mutex<Entries>);
impl ResultCache {
    pub fn get(&self, key: &str) -> Option<Arc<Frames>> {
        self.0.lock().ok()?.values.get(key).cloned()
    }
    pub fn remove(&self, key: &str) {
        if let Ok(mut entries) = self.0.lock()
            && let Some(old) = entries.values.remove(key)
        {
            entries.bytes -= old.size();
        }
    }
    pub fn insert(&self, key: String, frames: Frames) -> Arc<Frames> {
        self.insert_bounded(key, frames, MAX_BYTES, MAX_ENTRIES)
    }
    fn insert_bounded(&self, key: String, frames: Frames, cap: usize, count: usize) -> Arc<Frames> {
        let frames = Arc::new(frames);
        let Ok(mut entries) = self.0.lock() else {
            return frames;
        };
        if frames.size() > cap || count == 0 {
            return frames;
        }
        if let Some(old) = entries.values.remove(&key) {
            entries.bytes -= old.size();
        }
        while entries.bytes + frames.size() > cap || entries.values.len() >= count {
            let Some(first) = entries.values.keys().next().cloned() else {
                break;
            };
            if let Some(old) = entries.values.remove(&first) {
                entries.bytes -= old.size();
            }
        }
        entries.bytes += frames.size();
        entries.values.insert(key, Arc::clone(&frames));
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frames(n: usize) -> Frames {
        Frames {
            current: Arc::new(vec![1; n]),
            historical: Arc::new(vec![2; n]),
            unknown: Arc::new(vec![3; n]),
        }
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
        assert_eq!(cache.0.lock().ok().map(|entries| entries.bytes), Some(15));
    }
}
