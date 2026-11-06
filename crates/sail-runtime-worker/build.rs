//! Compile the native protocol fault fixture, never installed as a product binary.
#![forbid(unsafe_code)]
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=tests/fixtures/control.rs");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?)
        .join("sail-native-worker-control");
    let compiler = std::env::var_os("RUSTC").ok_or("missing RUSTC")?;
    let target = std::env::var_os("TARGET").ok_or("missing TARGET")?;
    let built = std::process::Command::new(compiler)
        .args([
            "--edition=2024",
            "--crate-name=sail_native_worker_control",
            "--target",
        ])
        .arg(target)
        .arg("tests/fixtures/control.rs")
        .arg("-o")
        .arg(&output)
        .output()?;
    if !built.status.success() {
        return Err(format!(
            "native worker fixture did not compile: {}",
            String::from_utf8_lossy(&built.stderr)
        )
        .into());
    }
    println!(
        "cargo:rustc-env=SAIL_RUNTIME_TEST_WORKER={}",
        output.display()
    );
    Ok(())
}
