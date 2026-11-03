//! Cargo-installed entry point for Sail's small solver shim.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_shim::run()
}
