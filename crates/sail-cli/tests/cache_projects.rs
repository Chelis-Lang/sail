//! `sail cache projects list|prune` and the managed total in `sail cache stats`,
//! against a synthetic managed cache.
#![cfg(unix)]
use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sail(cache: &Path, args: &[&str]) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(args)
        .env("SAIL_CACHE_DIR", cache)
        .env("SAIL_DAEMON_SOCKET", cache.join("absent.sock"))
        .output()
    {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run sail: {e}"),
    }
}

fn json(output: &Output) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_default()
}

/// One managed workspace with a target, a receipt and a run record naming it.
fn workspace(cache: &Path, path: &Path, bytes: usize) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(path)?;
    let key = format!("{:x}", Sha256::digest(path.as_os_str().as_encoded_bytes()));
    let directory = cache.join("projects").join(key);
    let family = directory.join("compiler-artifacts/family");
    std::fs::create_dir_all(family.join("target/debug"))?;
    std::fs::write(family.join("target/debug/dep.rlib"), vec![0u8; bytes])?;
    std::fs::write(family.join("ownership.lock"), [])?;
    std::fs::create_dir_all(directory.join("context/receipts"))?;
    std::fs::write(directory.join("context/receipts/target.json"), "{}")?;
    let run = directory.join("context/runs/run");
    std::fs::create_dir_all(&run)?;
    std::fs::write(
        run.join("result.json"),
        serde_json::json!({ "workspace": path }).to_string(),
    )?;
    Ok(directory)
}

#[test]
fn lists_and_prunes_managed_workspaces_skipping_busy_ones() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::env::temp_dir().join(format!("sail-cache-projects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fixture = Fixture(root.clone());
    let cache = fixture.0.join("cache");
    let kept = workspace(&cache, &root.join("kept"), 4096)?;
    let gone = workspace(&cache, &root.join("gone"), 8192)?;
    let busy = workspace(&cache, &root.join("busy"), 1024)?;
    std::fs::remove_dir_all(root.join("gone"))?;
    std::fs::remove_dir_all(root.join("busy"))?;

    for args in [
        &["cache", "projects", "--help"][..],
        &["cache", "projects", "list", "--help"],
        &["cache", "projects", "prune", "-h"],
    ] {
        let help = sail(&cache, args);
        assert_eq!(help.status.code(), Some(0), "{args:?}");
        assert!(String::from_utf8_lossy(&help.stdout).contains("--older-than DAYS"));
    }
    let usage = sail(&cache, &["cache", "projects", "prune"]);
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("SAIL0002"));

    let listed = json(&sail(&cache, &["cache", "projects", "list", "--json"]));
    let projects = listed["projects"].as_array().ok_or("no projects")?;
    assert_eq!(projects.len(), 3, "{listed}");
    // Largest first, with the recorded path and whether it still exists.
    assert_eq!(
        projects[0]["workspace"],
        root.join("gone").display().to_string()
    );
    assert_eq!(projects[0]["exists"], false);
    assert!(projects.iter().any(|p| p["exists"] == true));
    let text = sail(&cache, &["cache", "projects", "list"]);
    assert!(String::from_utf8_lossy(&text.stdout).contains("(missing)"));

    // `cache stats` reports the managed total alongside the reply entries.
    let stats = json(&sail(&cache, &["cache", "stats", "--json"]));
    assert_eq!(stats["managed_projects"]["workspaces"], 3, "{stats}");
    assert!(stats["managed_projects"]["bytes"].as_u64() >= Some(4096 + 8192 + 1024));

    let dry = json(&sail(
        &cache,
        &[
            "cache",
            "projects",
            "prune",
            "--missing",
            "--dry-run",
            "--json",
        ],
    ));
    assert_eq!(dry["removed"].as_array().map(Vec::len), Some(2), "{dry}");
    assert!(gone.is_dir() && busy.is_dir());

    // A run holds `busy`: it is skipped and reported, the idle one removed.
    let lock = std::fs::File::open(busy.join("compiler-artifacts/family/ownership.lock"))?;
    lock.lock()?;
    let pruned = sail(&cache, &["cache", "projects", "prune", "--missing"]);
    assert_eq!(pruned.status.code(), Some(0));
    let text = String::from_utf8_lossy(&pruned.stdout);
    assert!(text.contains("skipped: in use"), "{text}");
    assert!(text.contains("pruned 1 workspace"), "{text}");
    assert!(!gone.exists() && busy.join("context/receipts/target.json").is_file());
    drop(lock);
    let pruned = json(&sail(
        &cache,
        &["cache", "projects", "prune", "--missing", "--json"],
    ));
    assert_eq!(
        pruned["removed"].as_array().map(Vec::len),
        Some(1),
        "{pruned}"
    );
    assert!(!busy.exists() && kept.is_dir());

    // Nothing was used within a day, so nothing is that old yet.
    let recent = json(&sail(
        &cache,
        &["cache", "projects", "prune", "--older-than", "1", "--json"],
    ));
    assert_eq!(recent["removed"].as_array().map(Vec::len), Some(0));
    let all = json(&sail(
        &cache,
        &["cache", "projects", "prune", "--all", "--json"],
    ));
    assert_eq!(all["removed"].as_array().map(Vec::len), Some(1));
    assert!(!kept.exists());
    let listed = json(&sail(&cache, &["cache", "projects", "list", "--json"]));
    assert_eq!(listed["projects"].as_array().map(Vec::len), Some(0));
    Ok(())
}
