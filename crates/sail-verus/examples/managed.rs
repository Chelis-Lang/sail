//! Embeddable managed-runner probe, also used as its private Cargo aliases.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    if let Some(code) = sail_verus::adapter::dispatch() {
        return code;
    }
    let get = |key: &str| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .ok_or_else(|| format!("missing {key}"))
    };
    let result = (|| {
        let config = sail_verus::managed::Config {
            workspace: get("SAIL_PROBE_WORKSPACE")?,
            cache: get("SAIL_CACHE_DIR")?,
            sail: std::env::current_exe().map_err(|e| e.to_string())?,
            cargo_verus: get("SAIL_CARGO_VERUS")?,
            cargo: get("SAIL_PROBE_CARGO")?,
            shim: get("SAIL_PROBE_SHIM")?,
            z3: get("SAIL_PROBE_Z3")?,
            socket: None,
            args: std::env::args().skip(1).collect(),
            force: false,
        };
        sail_verus::managed::run(&config, &mut std::io::stderr()).map_err(|e| e.to_string())
    })();
    match result {
        Ok(report) => {
            if let Ok(json) = serde_json::to_string_pretty(&report) {
                println!("{json}");
            }
            ExitCode::from(
                report
                    .exit_code
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or(1),
            )
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
