//! A minimal work queue over scoped std threads.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// Applies `f` to every item using up to `jobs` threads, and returns the
/// results in item order. `progress` is called (from worker threads) after
/// each item with the number finished so far.
pub fn map<T, R, F, P>(items: &[T], jobs: usize, f: F, progress: P) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
    P: Fn(usize) + Sync,
{
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, items.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = f(item);
                    if let Ok(mut slots) = results.lock()
                        && let Some(slot) = slots.get_mut(i)
                    {
                        *slot = Some(r);
                    }
                    progress(done.fetch_add(1, Ordering::Relaxed) + 1);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::map;

    #[test]
    fn preserves_order() {
        let items: Vec<u64> = (0..100).collect();
        let out = map(&items, 4, |x| x * 2, |_| {});
        assert_eq!(out, items.iter().map(|x| x * 2).collect::<Vec<_>>());
        let empty: Vec<u64> = Vec::new();
        assert_eq!(map(&empty, 4, |x| *x, |_| {}), Vec::<u64>::new());
    }
}
