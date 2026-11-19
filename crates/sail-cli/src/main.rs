//! The `sail` binary: [`sail_cli::run_with`] with the native temporal backend
//! (D.I.V.E., through the isolated `sail-runtime-worker`) linked (ADR 0172).
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_cli::run_with(&sail_cli::Extensions {
        temporal: Some(std::sync::Arc::new(sail_dive::backend::Dive)),
    })
}
