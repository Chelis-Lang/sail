//! Scoped maintenance cleanup for fault-injection fixtures.

use std::sync::atomic::{AtomicBool, Ordering};

/// Stop maintenance before the enclosing thread scope joins on unwind.
pub(crate) struct StopOnDrop<'a>(&'a AtomicBool);

impl<'a> StopOnDrop<'a> {
    /// Borrow the existing stop flag without changing its normal lifecycle.
    pub(crate) fn new(stop: &'a AtomicBool) -> Self {
        Self(stop)
    }
}

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::StopOnDrop;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn foreground_panic_stops_maintenance_before_scope_joins() {
        let stop = AtomicBool::new(false);
        let exited = AtomicBool::new(false);
        let started = Instant::now();
        let outcome = std::panic::catch_unwind(|| {
            std::thread::scope(|scope| {
                let _stop_on_drop = StopOnDrop::new(&stop);
                let (entered_tx, entered_rx) = std::sync::mpsc::channel();
                let maintenance_stop = &stop;
                let maintenance_exited = &exited;
                scope.spawn(move || {
                    let _ = entered_tx.send(());
                    while !maintenance_stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    maintenance_exited.store(true, Ordering::SeqCst);
                });
                assert!(entered_rx.recv_timeout(Duration::from_secs(1)).is_ok());
                eprintln!("maintenance entered; controlled foreground unwind");
                std::panic::resume_unwind(Box::new("controlled foreground panic"));
            });
        });
        assert!(outcome.is_err());
        assert!(stop.load(Ordering::SeqCst));
        assert!(exited.load(Ordering::SeqCst));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
