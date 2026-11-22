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
        self.invoke_with(mode, &[])
    }

    fn invoke_with(&self, mode: &str, options: &[&str]) -> std::io::Result<Output> {
        Command::new(env!("CARGO_BIN_EXE_sail"))
            .current_dir(&self.0)
            .env("FIXTURE_MODE", mode)
            .env(
                "FIXTURE_STRATEGY",
                if options.contains(&"occupancy") {
                    "occupancy"
                } else {
                    "symbolic"
                },
            )
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
                if mode == "hung" { "1" } else { "10" },
            ])
            .args(options)
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
    let separate = session.0.join("session/sail-occupancy-predicate-v1.json");
    check_named(&session)?;
    let occupancy = ["--strategy", "occupancy", "--max-orbits", "34500000"];
    assert!(session.invoke_with("cold", &occupancy)?.status.success());
    assert_eq!(fs::read(&separate)?, b"cold");
    assert!(session.invoke_with("prove", &occupancy)?.status.success());
    for field in [
        "distinct_decimal",
        "generated_decimal",
        "occupancies",
        "trace_file",
        "predicate_format",
    ] {
        assert_eq!(
            session
                .invoke_with(&format!("extra-{field}"), &occupancy)?
                .status
                .code(),
            Some(2),
            "{field}"
        );
        assert_eq!(fs::read(&separate)?, b"cold");
    }
    assert_eq!(
        session
            .invoke_with("cross-format", &occupancy)?
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        session
            .invoke_with("cold", &["--max-orbits", "3"])?
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        session
            .invoke_with("cold", &["--strategy", "occupancy", "--max-orbits", "0"])?
            .status
            .code(),
        Some(2)
    );
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
        ("null", 2),
        ("invalid-count", 2),
        ("contradictory", 2),
        ("duplicate-var", 2),
        ("invalid-trace", 2),
        ("hung", 4),
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

fn check_named(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    let named = ["--property-proof", "Mutex"];
    let scoped = session
        .0
        .join("session/sail-named-property-predicate-v1.json");
    assert!(session.invoke_with("cold", &named)?.status.success());
    assert!(session.invoke_with("prove", &named)?.status.success());
    for mode in [
        "unbound",
        "counts",
        "extra-trace_file",
        "extra-left_on_queue",
        "extra-statistics",
        "null-obligations",
        "forged-time",
        "extra-predicate_format",
        "violation",
    ] {
        assert_eq!(
            session.invoke_with(mode, &named)?.status.code(),
            Some(2),
            "{mode}"
        );
        assert_eq!(fs::read(&scoped)?, b"cold");
    }
    let capabilities = session.0.join("libexec/dive-symbolic/capabilities");
    fs::create_dir_all(&capabilities)?;
    fs::write(capabilities.join("named-predicate-repair-v1"), "")?;
    let repaired = session.invoke_with("repair", &named)?;
    assert!(repaired.status.success(), "{:?}", repaired.stderr);
    assert_eq!(fs::read(scoped)?, b"repaired");
    Ok(())
}
