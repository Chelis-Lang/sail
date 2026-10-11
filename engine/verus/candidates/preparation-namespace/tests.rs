#![feature(rustc_private)]
#![forbid(unsafe_code)]
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_span;
#[path = "namespace.rs"]
mod namespace;
use namespace::Lease;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn actual_default_rustc_namespace_stays_on_compiler_thread() {
    rustc_data_structures::sync::set_dyn_thread_safe_mode(false);
    assert!(rustc_data_structures::sync::check_dyn_thread_safe().is_none());
    let map =
        rustc_span::source_map::SourceMap::new(rustc_span::source_map::FilePathMapping::empty());
    map.new_source_file(
        rustc_span::FileName::Custom("first".into()),
        "abc".to_owned(),
    );
    let (lease, request) = Lease::pair().unwrap();
    let published = Arc::new(AtomicBool::new(false));
    let signal = published.clone();
    let (diag_send, diag_receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        assert!(lease.ready());
        assert!(lease.validate([7, 8]));
        assert!(!lease.validate([9, 10]));
        diag_send.send("ordinary queued diagnostic").unwrap();
        signal.store(true, Ordering::Release);
        drop(lease);
    });
    {
        let files = map.files();
        request.serve(|words| {
            assert_eq!(files.len(), 1);
            assert!(!published.load(Ordering::Acquire));
            words == [7, 8]
        });
        assert!(published.load(Ordering::Acquire));
        // Deliberately do not dequeue/render diagnostics while files is held.
    }
    assert_eq!(diag_receive.recv().unwrap(), "ordinary queued diagnostic");
    // Actual SourceMap mutation succeeds only after service and guard disposal.
    map.new_source_file(
        rustc_span::FileName::Custom("after".into()),
        "xyz".to_owned(),
    );
    assert_eq!(map.files().len(), 2);
    worker.join().unwrap();
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn cancellation_before_ready_releases_service_and_owner() {
    let (lease, request) = Lease::pair().unwrap();
    drop(lease);
    request.serve(|_| panic!("cancelled request validated"));
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn dropped_service_refuses_ready_without_waiting() {
    let (lease, request) = Lease::pair().unwrap();
    drop(request);
    assert!(!lease.ready());
    drop(lease);
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn validator_unwind_wakes_waiting_worker() {
    let (lease, request) = Lease::pair().unwrap();
    let worker = std::thread::spawn(move || {
        assert!(lease.ready());
        assert!(!lease.validate([1, 2]));
    });
    assert!(
        std::panic::catch_unwind(|| request.serve(|_| panic!("injected validator panic"))).is_err()
    );
    worker.join().unwrap();
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn worker_unwind_disconnects_compiler_service() {
    let (lease, request) = Lease::pair().unwrap();
    let worker = std::thread::spawn(move || {
        assert!(lease.ready());
        panic!("injected worker panic");
    });
    request.serve(|_| false);
    assert!(worker.join().is_err());
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn last_peer_owns_budget_until_physical_control_disposal() {
    let (lease, request) = Lease::pair().unwrap();
    assert!(Lease::pair().is_none());
    drop(lease);
    assert!(Lease::pair().is_none());
    assert_eq!(namespace::retained_bytes(), 16384);
    drop(request);
    assert_eq!(namespace::retained_bytes(), 0);
    let pair = Lease::pair().unwrap();
    drop(pair);
    assert_eq!(namespace::retained_bytes(), 0);
}
#[test]
fn repeated_validations_use_one_fixed_slot_and_exact_words() {
    let (lease, request) = Lease::pair().unwrap();
    let worker = std::thread::spawn(move || {
        assert!(lease.ready());
        for i in 0..1000 {
            assert_eq!(lease.validate([i, u64::MAX - i]), i % 2 == 0);
        }
    });
    request.serve(|words| {
        assert_eq!(words[1], u64::MAX - words[0]);
        assert_eq!(namespace::retained_bytes(), 16384);
        words[0] % 2 == 0
    });
    worker.join().unwrap();
    assert_eq!(namespace::retained_bytes(), 0);
}
