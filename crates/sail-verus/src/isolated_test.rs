//! Runs one test of this crate in a process of its own.
//!
//! A file lock belongs to the open file description, and a child process
//! shares every description open in its parent from its fork until its `exec`
//! closes them. The test harness runs tests as threads of one process, so when
//! another test spawns a process while a test holds a lock, that lock stays
//! held for a moment after the test dropped it, and a non-waiting attempt in
//! that moment is rightly refused. A test that asserts a lock is free
//! immediately after its release is therefore only deterministic in a process
//! in which nothing else spawns one.

/// Names the test a child process was started to run.
const ISOLATED: &str = "SAIL_TEST_ISOLATED";

/// Whether this process is the one that must run the body of `test`, a test
/// function of the module `module` (its `module_path!()`).
///
/// In the child process started for this test it returns true and the caller
/// runs its assertions, each on its first attempt. Anywhere else it starts
/// that child (the test binary, this one test, one thread, nothing spawned),
/// requires it to pass, and returns false: the caller has nothing left to do.
///
/// # Panics
///
/// If the child cannot be started, fails, or did not run exactly this test.
pub(crate) fn runs_here(module: &str, test: &str) -> bool {
    // The harness names a test without the crate's own name.
    let module = module.split_once("::").map_or(module, |(_, rest)| rest);
    let name = format!("{module}::{test}");
    if std::env::var_os(ISOLATED).is_some_and(|value| value.to_str() == Some(name.as_str())) {
        return true;
    }
    let output = std::env::current_exe().and_then(|binary| {
        std::process::Command::new(binary)
            .args([name.as_str(), "--exact", "--test-threads=1"])
            .env(ISOLATED, &name)
            .output()
    });
    let output = output.unwrap_or_else(|error| unreachable!("isolated {name}: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The child must have run this very test: a filter that matched nothing
    // also exits successfully.
    assert!(
        output.status.success() && stdout.contains(&format!("test {name} ... ok")),
        "isolated {name}: {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    false
}
