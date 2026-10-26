//! The runtime's native temporal backend: the isolated worker client.
use sail_runtime::temporal::{Dispatch, TemporalBackend, Worker};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Runs jobs in the installed `sail-runtime-worker` through [`crate::client::run`].
///
/// The client freezes and re-authenticates the worker bytes, owns the process
/// group, and independently binds the worker's requested-property inventory
/// to the request (parsed again in this process) before any evidence is
/// returned as [`Dispatch::Completed`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Dive;

impl TemporalBackend for Dive {
    fn run(
        &self,
        worker: &Worker,
        request: &crate::Request,
        deadline: Duration,
        cancel: &AtomicBool,
    ) -> Dispatch {
        match crate::client::run(
            &worker.executable,
            &worker.sha256,
            request,
            deadline,
            cancel,
        ) {
            crate::client::RunResult::Completed(response) => Dispatch::Completed(response),
            crate::client::RunResult::TimedOut { .. } => Dispatch::TimedOut,
            crate::client::RunResult::Cancelled { .. } => Dispatch::Cancelled,
            crate::client::RunResult::Failed(message) => Dispatch::Failed(message),
        }
    }
}
