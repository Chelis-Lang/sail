//! The extension-built `sail` links the native temporal backend.
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn temporal_capability_reaches_worker_discovery() -> TestResult {
    let cache = std::env::temp_dir().join(format!("sail-temporal-linked-{}", std::process::id()));
    std::fs::create_dir_all(&cache)?;
    let output = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(["runtime", "capabilities", "--cache"])
        .arg(&cache)
        .env("SAIL_CACHE_DIR", &cache)
        .output()?;
    std::fs::remove_dir_all(&cache)?;
    assert!(output.status.success(), "{output:?}");
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let temporal = &capabilities["temporal"];
    // No worker is installed beside the test binary, so the backend reports
    // the missing installation, never that it is absent from the build.
    assert_eq!(temporal["installed"], false);
    assert_eq!(
        temporal["error"]["message"],
        "native temporal worker is not installed"
    );
    Ok(())
}
