//! Order-preserving parallel map for independent, side-effect-free work such
//! as complete file digests and log parsing. Only the overlap changes: each
//! item is processed exactly as a sequential map would, and results keep the
//! input order.

use std::sync::atomic::{AtomicUsize, Ordering};

/// At most this many workers run at once. macOS serialises `open` and
/// `lstat` in the kernel: on Apple silicon, digesting vest's 11,467 input
/// files took about 230 ms with 3 or 4 workers and 323 ms with 8, and a
/// managed unchanged rerun took 0.73 s with 4 workers against 0.89 s with 8.
/// On Linux 8 is fastest.
#[cfg(target_os = "macos")]
const MAX_WORKERS: usize = 4;
#[cfg(not(target_os = "macos"))]
const MAX_WORKERS: usize = 8;

/// `items.iter().map(work).collect()`, computed by up to `MAX_WORKERS`
/// scoped workers.
/// A worker that panics leaves its items to be recomputed sequentially, so a
/// result is never missing or taken from another item.
pub(crate) fn in_order<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(MAX_WORKERS)
        .min(items.len());
    if workers <= 1 {
        return items.iter().map(&work).collect();
    }
    let next = AtomicUsize::new(0);
    let mut slots: Vec<Option<R>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            return done;
                        };
                        done.push((index, work(item)));
                    }
                })
            })
            .collect();
        let mut slots: Vec<Option<R>> = items.iter().map(|_| None).collect();
        for handle in handles {
            for (index, result) in handle.join().unwrap_or_default() {
                if let Some(slot) = slots.get_mut(index) {
                    *slot = Some(result);
                }
            }
        }
        slots
    });
    slots
        .iter_mut()
        .zip(items)
        .map(|(slot, item)| slot.take().unwrap_or_else(|| work(item)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_input_order_for_every_size() {
        for size in [0usize, 1, 2, 7, 8, 9, 100, 1000] {
            let items: Vec<usize> = (0..size).collect();
            assert_eq!(
                in_order(&items, |item| item * 3 + 1),
                items.iter().map(|item| item * 3 + 1).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn each_item_is_processed_by_the_same_function() {
        let items: Vec<String> = (0..50).map(|i| format!("item-{i}")).collect();
        let lengths = in_order(&items, String::len);
        assert_eq!(lengths, items.iter().map(String::len).collect::<Vec<_>>());
    }
}
