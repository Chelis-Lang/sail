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
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use sail_acceptance::query::Fifo;
use sail_cache::memory::{MemoryTier, SLOT_OVERHEAD, Value};
use sail_cache::{DiskCache, PutOutcome};
use sail_key::QueryKey;

use crate::log::Log;

/// One queued write.
pub(crate) struct Job {
    pub key: QueryKey,
    pub value: Arc<Value>,
    pub command: String,
    pub provenance: String,
    /// The solver identity summary (`sail_cache::shared::Solver::summary`),
    /// or empty if the session's solver is unknown.
    pub solver: String,
}

impl Job {
    fn cost(&self) -> u64 {
        u64::try_from(self.value.reply.len()).unwrap_or(u64::MAX) + SLOT_OVERHEAD
    }
}

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
    jobs: Fifo<Job>,
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
        if q.keys.contains(&job.key.0) {
            return Submitted::Duplicate;
        }
        let cost = job.cost();
        // A wait the platform clock cannot represent (for example
        // `Duration::MAX` from the public `Config::queue_wait`) drops the
        // write rather than panicking: a dropped write only costs a later
        // miss (#478).
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
            if q.keys.contains(&job.key.0) {
                return Submitted::Duplicate;
            }
        }
        if q.closed {
            return Submitted::Dropped;
        }
        memory.insert_pending(&job.key, Arc::clone(&job.value));
        q.keys.insert(job.key.0);
        q.bytes += cost;
        q.jobs.push(job);
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
    pub fn run(&self, disk: &DiskCache, memory: &MemoryTier, delay: Option<Duration>, log: &Log) {
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
            let outcome = disk.put_scoped(
                &job.key,
                &job.value.reply,
                &job.command,
                &sail_cache::StoreContext {
                    solver: &job.solver,
                    provenance: &job.provenance,
                    mode: &job.value.mode,
                },
                job.value.effort.as_ref(),
            );
            match outcome {
                Ok(PutOutcome::Written(stamp)) => memory.settle(&job.key, &job.value, Some(stamp)),
                Ok(PutOutcome::Exists) => memory.settle(&job.key, &job.value, None),
                Err(e) => {
                    memory.settle(&job.key, &job.value, None);
                    errors += 1;
                    if errors <= 10 {
                        log.line(&format!("write failed for {}: {e}", job.key));
                    }
                }
            }
            if let Some(mut q) = self.lock() {
                q.keys.remove(&job.key.0);
                q.bytes = q.bytes.saturating_sub(job.cost());
            }
            self.space.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(byte: u8) -> Job {
        Job {
            key: QueryKey([byte; 32]),
            value: Arc::new(Value {
                mode: "default".to_owned(),
                reply: b"unsat\n".to_vec(),
                effort: None,
            }),
            command: "check-sat".to_owned(),
            provenance: "pinned-solver".to_owned(),
            solver: String::new(),
        }
    }

    #[test]
    fn an_unrepresentable_wait_drops_the_write_without_panicking() {
        // #478: `Config::queue_wait` is public and unconstrained, so
        // `Duration::MAX` must drop the write, never overflow the deadline.
        let writer = Writer::new(1 << 20);
        let memory = MemoryTier::new(1 << 20);
        assert_eq!(
            writer.submit(&memory, job(1), Duration::MAX),
            Submitted::Dropped
        );
        assert_eq!(writer.load(), (0, 0));
        assert_eq!(memory.stats().pending, 0);
        assert_eq!(memory.stats().entries, 0);
        // An ordinary wait still queues.
        assert_eq!(
            writer.submit(&memory, job(2), Duration::from_secs(1)),
            Submitted::Queued
        );
        assert_eq!(writer.load().0, 1);
        assert_eq!(memory.stats().pending, 1);
    }
}
