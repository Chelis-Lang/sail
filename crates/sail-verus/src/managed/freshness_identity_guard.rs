//! Test-only candidate for avoiding repeated identity sidecar decoding.
//!
//! No parsed tree or extra Arc is retained. Eligibility is one boolean on the
//! existing validation owner, set only after its successful cold transaction.
//! Every hot operation still validates that entire uninterrupted lease owner;
//! exact raw sidecar membership is a prerequisite, not freshness authority.

use super::ReadLease;
use std::path::Path;

thread_local! {
    static ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(super) fn enabled() -> bool {
    ENABLED.with(std::cell::Cell::get)
}
pub(super) fn record_parse() {
    PARSES.with(|parses| parses.set(parses.get().saturating_add(1)));
}
pub(super) fn parses() -> usize {
    PARSES.with(std::cell::Cell::get)
}
pub(super) fn with_enabled<T>(enabled: bool, f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(self.0));
        }
    }
    let _restore = Restore(ENABLED.with(|slot| slot.replace(enabled)));
    PARSES.with(|parses| parses.set(0));
    f()
}
pub(super) fn eligible(cache: &Path, context: &str, identity: &str, lease: &ReadLease) -> bool {
    let directory = cache.join("runtime-identities");
    lease.covers_exact_path(&directory.join(format!("{context}.link")))
        && lease.covers_exact_path(&directory.join(format!("{identity}.json")))
}
