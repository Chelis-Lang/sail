//! Actual private-driver transport around a real rustc metadata invocation.
//!
//! The wrapper emits an explicitly synthetic verifier summary. These tests
//! exercise observation publication, not proof soundness or replay acceptance.
#![cfg(unix)]

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;

struct Fixture {
    root: PathBuf,
    manifest: PathBuf,
    wrapper: PathBuf,
    driver: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new(name: &str, cache_failure: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let root = PathBuf::from(format!(
            "/tmp/sail-guarded-driver-{}-{name}",
            std::process::id()
        ));
        let fixture = Self {
            manifest: root.join("project/Cargo.toml"),
            wrapper: root.join("synthetic-verifier"),
            driver: root.join("sail-verus-driver"),
            root,
        };
        for relative in [
            "project/src",
            "target/deps",
            "run/invocations",
            "sidecars",
            "receipts",
        ] {
            std::fs::create_dir_all(fixture.root.join(relative))?;
        }
        std::fs::write(
            &fixture.manifest,
            "[package]\nname='demo'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        std::fs::write(
            fixture.root.join("project/src/lib.rs"),
            "pub const VALUE: u8 = 1;\n",
        )?;
        std::fs::write(
            &fixture.wrapper,
            r"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys
args = [a for a in sys.argv[1:] if a not in ('--output-json', '--time')]
result = subprocess.run([os.environ['SAIL_FIXTURE_RUSTC'], *args], capture_output=True)
sys.stdout.buffer.write(result.stdout)
sys.stderr.buffer.write(result.stderr)
if result.returncode:
    sys.exit(result.returncode)
out = pathlib.Path(args[args.index('--out-dir') + 1])
vir = out / 'libdemo-fixture.vir'
vir.write_bytes(b'synthetic proof payload: no proof claim')
print(json.dumps({'$message_type': 'artifact', 'artifact': str(vir), 'emit': 'metadata'}), file=sys.stderr)
if os.environ.get('SAIL_FIXTURE_MUTATE') == '1':
    pathlib.Path('src/lib.rs').write_text('this is a syntax error after the compiler returned\n')
print(json.dumps({'verification-results': {'verified': 0, 'errors': 0, 'success': True, 'encountered-vir-error': False}}))
",
        )?;
        std::fs::set_permissions(&fixture.wrapper, std::fs::Permissions::from_mode(0o755))?;
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_sail"), &fixture.driver)?;
        let store = fixture.root.join("store");
        if cache_failure {
            std::fs::write(
                &store,
                b"publication must fail without invalidating the proof",
            )?;
        }
        let runtime = json!({
            "dependency_build": {
                "directory": store,
                "target": fixture.root.join("target"),
                "compiler": [[fixture.wrapper, false]],
                "packages": [{
                    "manifest": fixture.manifest,
                    "key": "synthetic-fixture",
                    "sources": [fixture.root.join("project")],
                    "extern_names": []
                }]
            },
            "run_id": "fixture-run",
            "args": ["verify"],
            "z3": fixture.wrapper,
            "z3_sha256": format!("{:x}", Sha256::digest(std::fs::read(&fixture.wrapper)?)),
            "daemon": false,
            "sidecars": fixture.root.join("sidecars"),
            "invocations": fixture.root.join("run/invocations"),
            "original_wrapper": fixture.wrapper,
            "receipts": fixture.root.join("receipts"),
            "context_sha256": "fixture-context",
            "input_revision": "fixture-inputs",
            "packages": {fixture.manifest.to_str().ok_or("manifest UTF-8")?: "fixture-package"},
            "target_kinds": {fixture.manifest.to_str().ok_or("manifest UTF-8")?: {"demo": ["lib"]}},
            "engine": null,
            "attribution_logs": false
        });
        std::fs::write(
            fixture.root.join("runtime.json"),
            serde_json::to_vec(&runtime)?,
        )?;
        Ok(fixture)
    }

    fn run(&self, mutate: bool) -> Result<Value, Box<dyn std::error::Error>> {
        let rustc = Command::new("rustup").args(["which", "rustc"]).output()?;
        if !rustc.status.success() {
            return Err(String::from_utf8_lossy(&rustc.stderr).into_owned().into());
        }
        let package = format!(
            "__VERUS_DRIVER_VERIFY_demo-0.1.0-{}",
            &format!(
                "{:x}",
                Sha256::digest(self.manifest.to_string_lossy().as_bytes())
            )[..12]
        );
        let output = Command::new(&self.driver)
            .current_dir(self.root.join("project"))
            .args([
                "--crate-name",
                "demo",
                "--edition=2024",
                "--crate-type",
                "rlib",
                "--emit=dep-info,metadata",
                "-C",
                "extra-filename=-fixture",
                "--out-dir",
                self.root
                    .join("target/deps")
                    .to_str()
                    .ok_or("output UTF-8")?,
                "--error-format=json",
                "--json=artifacts",
                "src/lib.rs",
            ])
            .env("CARGO_PKG_NAME", "demo")
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_MANIFEST_DIR", self.root.join("project"))
            .env("CARGO_CRATE_NAME", "demo")
            .env(package, "1")
            .env("SAIL_MANAGED_WRAPPER", &self.wrapper)
            .env("SAIL_MANAGED_RUNTIME", self.root.join("runtime.json"))
            .env(
                "SAIL_FIXTURE_RUSTC",
                std::str::from_utf8(&rustc.stdout)?.trim(),
            )
            .env("SAIL_FIXTURE_MUTATE", if mutate { "1" } else { "0" })
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let invocations =
            std::fs::read_dir(self.root.join("run/invocations"))?.collect::<Result<Vec<_>, _>>()?;
        let files: Vec<_> = invocations
            .iter()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        assert_eq!(files.len(), 1);
        Ok(serde_json::from_slice(&std::fs::read(files[0].path())?)?)
    }

    fn entries(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let store = self.root.join("store");
        if !store.is_dir() {
            return Ok(0);
        }
        Ok(std::fs::read_dir(store)?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .filter(|entry| entry.path().join("entry.json").is_file())
            .count())
    }
}

#[test]
fn guarded_mutation_withholds_ordinary_and_dependency_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let normal = Fixture::new("normal", false)?;
    let invocation = normal.run(false)?;
    assert_eq!(invocation["success"], true);
    assert_eq!(
        normal.entries()?,
        1,
        "positive control must actually enter the guarded path"
    );
    assert!(!normal.root.join("run/inputs-changed").exists());

    let changed = Fixture::new("changed", false)?;
    let invocation = changed.run(true)?;
    assert_eq!(
        invocation["report"]["exit_code"], 0,
        "actual compiler finished before the mutation"
    );
    assert_eq!(
        invocation["success"], false,
        "a complete compiler report cannot authorize reuse after detected mutation"
    );
    assert!(changed.root.join("run/inputs-changed").is_file());
    assert_eq!(changed.entries()?, 0);

    let damaged = Fixture::new("store-io", true)?;
    let invocation = damaged.run(false)?;
    assert_eq!(
        invocation["success"], true,
        "cache publication I/O is not an input mutation"
    );
    assert!(!damaged.root.join("run/inputs-changed").exists());
    assert_eq!(damaged.entries()?, 0);
    Ok(())
}
