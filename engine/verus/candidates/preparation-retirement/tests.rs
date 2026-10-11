#![feature(rustc_private)]
#![forbid(unsafe_code)]
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_span;
extern crate libc;
#[path = "namespace.rs"]
mod namespace;

#[test]
fn actual_pinned_native_objects_keep_the_original_fixed_ceiling() {
    assert!(std::mem::size_of::<libc::pthread_mutex_t>() <= 256);
    assert!(std::mem::size_of::<libc::pthread_cond_t>() <= 256);
}
