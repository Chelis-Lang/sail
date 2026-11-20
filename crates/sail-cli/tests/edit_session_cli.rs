//! Actual installed entry point, manifest, worker, and closed evidence envelope.
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

type Test = Result<(), Box<dyn std::error::Error>>;
struct Install(PathBuf);
impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Install {
    fn invoke(&self, input: &[u8]) -> Result<Output, Box<dyn std::error::Error>> {
        let mut cli = Command::new(self.0.join("sail"))
            .args(["runtime", "edit-session", "--file", "-"])
            .env("SAIL_CACHE_DIR", self.0.join("cache"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        cli.stdin.take().ok_or("stdin")?.write_all(input)?;
        Ok(cli.wait_with_output()?)
    }
}

#[test]
fn installed_cli_emits_bound_ordered_evidence_and_no_cache_entries() -> Test {
    let install =
        Install(std::env::temp_dir().join(format!("sail-edit-cli-{}", std::process::id())));
    std::fs::create_dir(&install.0)?;
    let worker_binary = Path::new(env!("CARGO_BIN_EXE_sail-runtime-worker"));
    for (from, to) in [
        (Path::new(env!("CARGO_BIN_EXE_sail")), "sail"),
        (worker_binary, "sail-runtime-worker"),
    ] {
        if std::fs::hard_link(from, install.0.join(to)).is_err() {
            std::fs::copy(from, install.0.join(to))?;
        }
    }
    let worker_hash = sail_dive::sha256(&std::fs::read(worker_binary)?);
    let manifest = json!({
        "version": 1, "protocol": sail_dive::VERSION,
        "platform": sail_runtime::temporal::platform(),
        "backend_revision": sail_dive::DIVE_VERSION, "sha256": worker_hash,
    });
    let manifest_path = install.0.join("sail-runtime-worker.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let example: Value = serde_json::from_slice(include_bytes!("../src/temporal_warmup.json"))?;
    let original = example["input"]["job"]["request"].clone();
    let mut edited = original.clone();
    let source = edited["model"]["files"]["Warm.tla"]
        .as_str()
        .ok_or("source")?;
    edited["model"]["files"]["Warm.tla"] = source.replace("Inv == x = 0", "Inv == x = 1").into();
    let input = json!({"version": 1, "wall_time_ms": 30000, "checks": [original, edited]});
    let output = install.invoke(&serde_json::to_vec(&input)?)?;
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(result["worker_sha256"], worker_hash);
    assert_eq!(result["backend_source"], sail_dive::edits::BACKEND_SOURCE);
    assert_eq!(result["outcome"], "completed");
    assert_eq!(result["entries"][0]["evidence"]["reused"], false);
    assert_eq!(result["entries"][1]["evidence"]["reused"], true);
    assert_eq!(
        result["entries"][1]["evidence"]["decision"]["outcome"],
        "refuted"
    );
    assert_eq!(
        result["entries"][1]["evidence"]["decision"]["validation"]["outcome"],
        "valid"
    );
    for i in 0..2 {
        let req: sail_dive::Request = serde_json::from_value(input["checks"][i].clone())?;
        assert_eq!(
            result["entries"][i]["request_sha256"],
            sail_dive::request_sha256(&req)?
        );
    }
    let mut incomplete = input.clone();
    incomplete["checks"][0]["model"]["root"] = "Missing.tla".into();
    let output = install.invoke(&serde_json::to_vec(&incomplete)?)?;
    assert_eq!(output.status.code(), Some(1));
    let answer: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(answer["entries"][0]["evidence"]["route"], "fresh");
    assert_eq!(
        answer["entries"][0]["evidence"]["response"]["outcome"],
        "rejected"
    );
    assert!(!install.0.join("cache").exists());

    let duplicate =
        serde_json::to_string(&input)?.replacen("\"checks\":", "\"checks\":[],\"checks\":", 1);
    let malformed = install.invoke(duplicate.as_bytes())?;
    assert_eq!(malformed.status.code(), Some(2));
    assert_eq!(malformed.stdout, b"");
    let mut invalid = manifest;
    invalid["sha256"] = "0".repeat(64).into();
    std::fs::write(&manifest_path, serde_json::to_vec(&invalid)?)?;
    let output = install.invoke(&serde_json::to_vec(&input)?)?;
    assert_eq!(output.status.code(), Some(1));
    let failure: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(failure["outcome"], "failed");
    assert!(failure.get("entries").is_none());
    Ok(())
}
