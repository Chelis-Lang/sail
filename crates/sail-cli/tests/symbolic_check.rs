//! Exercise cross-process candidate publication through the public CLI.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Session(PathBuf);

impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Session {
    fn invoke(&self, mode: &str) -> std::io::Result<Output> {
        Command::new(env!("CARGO_BIN_EXE_sail"))
            .current_dir(&self.0)
            .env("FIXTURE_MODE", mode)
            .args([
                "runtime",
                "symbolic-check",
                "--dive",
                "dive",
                "--module",
                "M.tla",
                "--config",
                "M.cfg",
                "--session",
                "session",
                "--time-limit",
                "10",
            ])
            .output()
    }
}

#[test]
fn only_a_complete_search_publishes_and_a_receiver_proves_again()
-> Result<(), Box<dyn std::error::Error>> {
    let session =
        Session(std::env::temp_dir().join(format!("sail-symbolic-test-{}", std::process::id())));
    fs::create_dir(&session.0)?;
    let worker = session.0.join("dive");
    fs::write(&worker, include_str!("fixtures/symbolic_worker.py"))?;
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700))?;
    fs::write(session.0.join("M.tla"), "")?;
    fs::write(session.0.join("M.cfg"), "")?;
    let cold = session.invoke("cold")?;
    assert!(cold.status.success(), "{:?}", cold.stderr);
    assert!(String::from_utf8(cold.stdout)?.contains("123456789123456789123456789"));
    let predicate = session.0.join("session/sail-symbolic-predicate-v1.json");
    assert_eq!(fs::read(&predicate)?, b"cold");
    let proof = session.invoke("prove")?;
    assert!(proof.status.success());
    let proof: serde_json::Value = serde_json::from_slice(&proof.stdout)?;
    assert_eq!(proof["proof"], "inductive-safety");
    assert!(proof.get("distinct").is_none());

    for (mode, code) in [
        ("stop", 4),
        ("unbound", 2),
        ("missing", 2),
        ("counts", 2),
        ("bad-json", 2),
        ("unchecked", 2),
        ("violation", 1),
    ] {
        let output = session.invoke(mode)?;
        assert_eq!(output.status.code(), Some(code), "{mode}: {output:?}");
        assert_eq!(fs::read(&predicate)?, b"cold", "{mode}");
    }
    let refresh = session.invoke("refresh")?;
    assert!(refresh.status.success(), "{refresh:?}");
    assert_eq!(fs::read(&predicate)?, b"refresh");
    Ok(())
}
