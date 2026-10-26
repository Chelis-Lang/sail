//! The temporal-enabled `sail`: Sail's command line with the D.I.V.E.
//! backend linked into its runtime endpoint (ADR 0083).
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_cli::run_with(&sail_cli::Extensions {
        temporal: Some(std::sync::Arc::new(sail_dive::backend::Dive)),
    })
}
