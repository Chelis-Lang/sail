//! The temporal-enabled `sail`: Sail's command line with the D.I.V.E.
//! backend linked into its runtime endpoint (ADRs 0083 and 0102). Archives
//! install it as `bin/sail`.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    sail_cli::run_with(&sail_cli::Extensions {
        temporal: Some(std::sync::Arc::new(sail_dive::backend::Dive)),
    })
}
