//! Write-behind for `PUT`s (ADR 0015).
//!
//! A `PUT` is acknowledged once its reply is held by the in-memory tier (as
//! *pending*) and queued here. One writer thread persists queued entries in
//! order with the disk tier's usual atomic write, then settles the memory
//! slot: on-disk with the new file's stamp, or dropped if the write failed
//! or found an entry already there. The queue is bounded in bytes; a `PUT`
//! that finds it full waits briefly, then is dropped, which only costs a
//! future miss.

use std::collections::HashSet;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use sail_acceptance::query::Fifo;
use sail_cache::memory::{MemoryTier, PreparedWrite, candidate_bytes};
use sail_cache::{DiskCache, Effort};

use crate::log::Log;

/// The actual immutable source-shared candidate supplied by PUT/import callers.
pub(crate) type Job = sail_acceptance::cache_candidate::CandidateWrite<Effort>;

/// What happened to a submitted job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Submitted {
    /// Queued (and held by the memory tier as pending, if it fits there).
    Queued,
    /// A write for this key is already queued or in progress.
    Duplicate,
    /// The queue stayed full, or is closed: nothing was stored.
    Dropped,
}

struct Queue {
    jobs: Fifo<PreparedWrite>,
    /// Keys queued or being written.
    keys: HashSet<[u8; 32]>,
    /// Bytes queued or being written.
    bytes: u64,
    closed: bool,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            jobs: Fifo::empty(),
            keys: HashSet::new(),
            bytes: 0,
            closed: false,
        }
    }
}

/// The write-behind queue.
pub(crate) struct Writer {
    queue: Mutex<Queue>,
    work: Condvar,
    space: Condvar,
    capacity: u64,
}

impl Writer {
    pub fn new(capacity: u64) -> Self {
        Self {
            queue: Mutex::new(Queue::default()),
            work: Condvar::new(),
            space: Condvar::new(),
            capacity,
        }
    }

    fn lock(&self) -> Option<MutexGuard<'_, Queue>> {
        self.queue.lock().ok()
    }

    /// The queue's budget in bytes.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Writes queued or in progress, and their bytes.
    pub fn load(&self) -> (u64, u64) {
        self.lock().map_or((0, 0), |q| {
            (u64::try_from(q.keys.len()).unwrap_or(u64::MAX), q.bytes)
        })
    }

    /// Queues `job`, waiting up to `wait` for room. The memory tier takes
    /// the reply as pending in the same step, so a `GET` that follows the
    /// acknowledgement finds it.
    pub fn submit(&self, memory: &MemoryTier, job: Job, wait: Duration) -> Submitted {
        let Some(mut q) = self.lock() else {
            return Submitted::Dropped;
        };
        if q.keys.contains(&job.key()) {
            return Submitted::Duplicate;
        }
        let cost = candidate_bytes(&job);
        let Some(deadline) = Instant::now().checked_add(wait) else {
            return Submitted::Dropped;
        };
        // An empty queue takes any one job, however large.
        while !q.closed && !q.keys.is_empty() && q.bytes.saturating_add(cost) > self.capacity {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Submitted::Dropped;
            }
            q = match self.space.wait_timeout(q, left) {
                Ok((q, _)) => q,
                Err(_) => return Submitted::Dropped,
            };
            if q.keys.contains(&job.key()) {
                return Submitted::Duplicate;
            }
        }
        if q.closed {
            return Submitted::Dropped;
        }
        let Some(bytes) = q.bytes.checked_add(cost) else {
            return Submitted::Dropped;
        };
        let key = job.key();
        let prepared = memory.prepare(job);
        q.keys.insert(key);
        q.bytes = bytes;
        q.jobs.push(prepared);
        self.work.notify_one();
        Submitted::Queued
    }

    /// Stops taking jobs. The writer thread finishes the queue, then
    /// returns.
    pub fn close(&self) {
        if let Some(mut q) = self.lock() {
            q.closed = true;
        }
        self.work.notify_all();
        self.space.notify_all();
    }

    /// The writer thread: persists jobs in order until closed and empty.
    /// `delay` is a fault-injection knob that slows every write.
    pub fn run(&self, disk: &DiskCache, delay: Option<Duration>, log: &Log) {
        let mut errors = 0u64;
        loop {
            let job = {
                let Some(mut q) = self.lock() else { return };
                loop {
                    if let Some(job) = q.jobs.pop() {
                        break job;
                    }
                    if q.closed {
                        return;
                    }
                    q = match self.work.wait(q) {
                        Ok(q) => q,
                        Err(_) => return,
                    };
                }
            };
            if let Some(d) = delay {
                std::thread::sleep(d);
            }
            let key = job.key();
            let cost = job.charged_bytes();
            // This consumes the original queued record, performs the actual disk
            // call and dispatches its actual Result to its original endpoint.
            if let Err(error) = job.write(disk) {
                errors = errors.saturating_add(1);
                if errors <= 10 {
                    log.line(&format!("write failed for {key}: {error}"));
                }
            }
            if let Some(mut q) = self.lock() {
                q.keys.remove(&key.0);
                q.bytes = q.bytes.saturating_sub(cost);
            }
            self.space.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_cache::memory::Value;
    use std::sync::Arc;

    #[test]
    fn unrepresentable_wait_refuses_without_queue_or_pending_memory() {
        let writer = Writer::new(1 << 20);
        let memory = MemoryTier::new(1 << 20);
        let job = Job::new(
            [51; 32],
            Arc::new(Value {
                mode: "default".to_owned(),
                reply: b"unsat\n".to_vec(),
                effort: None,
            }),
            "check-sat".to_owned(),
            "fixture".to_owned(),
            "fixture-solver".to_owned(),
        );
        assert_eq!(
            writer.submit(&memory, job, Duration::MAX),
            Submitted::Dropped
        );
        assert_eq!(writer.load(), (0, 0));
        assert_eq!(memory.stats().entries, 0);
        assert_eq!(memory.stats().pending, 0);
    }
    #[test]
    fn queued_handle_metadata_counts_against_original_queue_budget() {
        let original_cost = sail_cache::memory::ENTRY_OVERHEAD + 6;
        let writer = Writer::new(2 * original_cost);
        let memory = MemoryTier::new(1 << 20);
        let candidate = |key| {
            Job::new(
                key,
                Arc::new(Value {
                    mode: "default".to_owned(),
                    reply: b"unsat\n".to_vec(),
                    effort: None,
                }),
                "check-sat".to_owned(),
                "fixture".to_owned(),
                "fixture-solver".to_owned(),
            )
        };
        assert_eq!(
            writer.submit(&memory, candidate([52; 32]), Duration::ZERO),
            Submitted::Queued
        );
        assert_eq!(
            writer.submit(&memory, candidate([53; 32]), Duration::ZERO),
            Submitted::Dropped
        );
        let (entries, bytes) = writer.load();
        assert_eq!(entries, 1);
        assert!(bytes > original_cost);
        assert!(bytes <= 2 * original_cost);
        assert_eq!(memory.stats().entries, 1);
        assert_eq!(memory.stats().pending, 1);
    }
}
