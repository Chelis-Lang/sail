#![feature(rustc_private)]
#![forbid(unsafe_code)]
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_span;
#[path = "namespace.rs"]
mod namespace;
    extern crate libc;
#[test]
fn actual_pinned_native_pthread_objects_fit_reserved_ceiling(){
    use std::mem::size_of;
    assert!(size_of::<libc::pthread_mutex_t>()<=256);
    assert!(size_of::<libc::pthread_cond_t>()<=256);
    println!("pthread-mutex-bytes={} pthread-condvar-bytes={}",size_of::<libc::pthread_mutex_t>(),size_of::<libc::pthread_cond_t>());
}
