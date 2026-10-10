//! Local-only real product-client census. Keep the independent Python census.
//! Run with `cargo run --release -p sail-cli --example job_read_bench -- ...`.
#![forbid(unsafe_code)]

#[path = "job_read_bench/args.rs"]
mod args;
#[path = "job_read_bench/measure.rs"]
mod measure;
#[path = "job_read_bench/process.rs"]
mod process;

fn main() -> std::process::ExitCode {
    let result = args::parse(std::env::args().skip(1)).and_then(|args| measure::run(&args));
    match result {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error[SAIL0943]: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
