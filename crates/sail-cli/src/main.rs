//! The `sail` binary: [`sail_cli::run_with`] with no extensions linked.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_cli::run_with(&sail_cli::Extensions::default())
}
