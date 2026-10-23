//! Cargo target-name collisions retain distinct compiler observations.

use super::tests::{compiler_fixture, compiler_receipt};
use super::*;
use std::os::unix::fs::PermissionsExt as _;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Collision {
    root: PathBuf,
    paths: Paths,
    data: Value,
    artifacts: Vec<Value>,
    calls: [Invocation; 2],
    _owner: Ownership,
}

impl Collision {
    fn new(tag: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().canonicalize()?.join(format!(
            "sail-target-collision-{tag}-{}",
            std::process::id()
        ));
        let config = compiler_fixture(&root)?;
        let (paths, owner) = prepare(&config, &root, "selected", "compiler", "run")?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\nedition='2024'\n[package.metadata.verus]\nverify=true\n[[bin]]\nname='foo-bar'\npath='a.rs'\n[[bin]]\nname='foo_bar'\npath='b.rs'\n",
        )?;
        std::fs::write(root.join("a.rs"), "fn main() {}\n")?;
        std::fs::write(root.join("b.rs"), "fn main() {}\n")?;
        let metadata = Command::new("cargo")
            .args(["metadata", "--format-version=1", "--no-deps"])
            .current_dir(&root)
            .output()?;
        assert!(metadata.status.success());
        let mut data: Value = serde_json::from_slice(&metadata.stdout)?;
        data["sail_cache_directory"] = serde_json::to_value(&config.cache)?;
        let wrapper = root.join("observe-rustc.py");
        std::fs::write(&wrapper, b"#!/usr/bin/env python3\nimport json,os,pathlib,sys\nname=os.environ.get('CARGO_BIN_NAME')\nif name:\n    p=pathlib.Path(os.environ['SAIL_TEST_BIN_IDENTITIES'])/(name+'.json')\n    p.write_text(json.dumps({'name':name,'args':sys.argv[1:]}))\nos.execv(sys.argv[1],sys.argv[1:])\n")?;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))?;
        let output = Command::new("cargo")
            .args(["check", "--bins", "--message-format=json"])
            .env("RUSTC_WRAPPER", &wrapper)
            .env("SAIL_TEST_BIN_IDENTITIES", &root)
            .current_dir(&root)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let artifacts: Vec<Value> = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
            .filter(|value| value["reason"] == "compiler-artifact")
            .collect();
        assert_eq!(artifacts.len(), 2);
        let artifact = PathBuf::from(artifacts[0]["filenames"][0].as_str().ok_or("artifact")?);
        let first = compiler_receipt(&paths, &data, &artifact).invocation;
        let calls = ["foo-bar", "foo_bar"].map(
            |name| -> std::result::Result<_, Box<dyn std::error::Error>> {
                let observed: Value =
                    serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json")))?)?;
                assert_eq!(observed["name"], name);
                let mut call = first.clone();
                call.identity.name = "foo_bar".into();
                call.identity.crate_type = "bin".into();
                call.identity.cargo_name = observed["name"].as_str().map(str::to_owned);
                call.identity.rustc_args_sha256 = digest(&serde_json::to_vec(&observed["args"])?);
                call.report.crates[0].name = Some(name.into());
                Ok(call)
            },
        );
        let [first, second] = calls;
        let calls = [first?, second?];
        Ok(Self {
            root,
            paths,
            data,
            artifacts,
            calls,
            _owner: owner,
        })
    }

    fn record(&self, successful_only: bool) -> std::io::Result<()> {
        let invocations = self.paths.run.join("invocations");
        for entry in std::fs::read_dir(&invocations)? {
            std::fs::remove_file(entry?.path())?;
        }
        for call in &self.calls {
            std::fs::write(
                invocations.join(format!("{}.json", call.identity.key())),
                serde_json::to_vec(call)?,
            )?;
        }
        let mut stream = Vec::new();
        for artifact in &self.artifacts {
            if !successful_only || artifact["target"]["name"] == "foo-bar" {
                serde_json::to_writer(&mut stream, artifact)?;
                stream.push(b'\n');
            }
        }
        std::fs::write(self.paths.run.join("artifacts.jsonl"), stream)
    }

    fn records(&self) -> Result<(Vec<TargetResult>, BTreeSet<String>)> {
        target_records(
            &self.paths,
            &self.data,
            "selected",
            false,
            &expected_targets(&self.data, &[]),
            &[],
            None,
        )
    }
}

impl Drop for Collision {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn colliding_cargo_names_preserve_failed_and_successful_targets() -> TestResult {
    let mut fixture = Collision::new("failure")?;
    fixture.calls[1].success = false;
    fixture.calls[1].report.exit_code = Some(1);
    fixture.calls[1].report.crates[0].verus.success = Some(false);
    fixture.calls[1].report.crates[0].verus.errors = Some(1);
    fixture.record(true)?;
    let (targets, missing) = fixture.records()?;
    assert!(missing.is_empty());
    assert_eq!(targets.len(), 2);
    for target in targets {
        assert!(target.requested);
        assert_eq!(
            target.report.crates[0].name.as_deref(),
            Some(target.target.as_str())
        );
        assert_eq!(target.complete, target.target == "foo-bar");
        assert_eq!(
            target.report.exit_code,
            Some(i32::from(target.target == "foo_bar"))
        );
    }
    for (index, name) in ["foo-bar", "foo_bar"].into_iter().enumerate() {
        let id = fixture.data["packages"][0]["id"]
            .as_str()
            .ok_or("package")?;
        assert!(selected_call(
            &fixture.data,
            (id, name, "bin"),
            &fixture.calls[index]
        ));
        assert!(!selected_call(
            &fixture.data,
            (id, name, "bin"),
            &fixture.calls[1 - index]
        ));
    }
    Ok(())
}

#[test]
fn colliding_cargo_names_keep_distinct_complete_receipts() -> TestResult {
    let fixture = Collision::new("success")?;
    fixture.record(false)?;
    let (targets, missing) = fixture.records()?;
    assert!(missing.is_empty());
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| target.complete));
    let receipts: Vec<Receipt> = std::fs::read_dir(fixture.paths.run.join("pending-receipts"))?
        .map(|entry| read(&entry.ok()?.path()))
        .collect::<Option<_>>()
        .ok_or("receipts")?;
    assert_eq!(receipts.len(), 2);
    for receipt in receipts {
        assert_eq!(receipt.invocation.identity.target_name(), receipt.name);
        assert_eq!(
            receipt.invocation.identity.cargo_name.as_deref(),
            Some(receipt.name.as_str())
        );
    }
    Ok(())
}

#[test]
fn ambiguous_legacy_and_duplicate_observations_cannot_authorize_receipts() -> TestResult {
    let mut fixture = Collision::new("ambiguous")?;
    fixture.calls[0].identity.cargo_name = None;
    fixture.record(false)?;
    assert_eq!(
        fixture.records().err().ok_or("ambiguity accepted")?.code,
        "SAIL3107"
    );
    fixture.calls[0].identity.cargo_name = Some("foo-bar".into());
    fixture.calls[1] = fixture.calls[0].clone();
    fixture.calls[1].identity.rustc_args_sha256 = digest(b"different arguments");
    fixture.record(false)?;
    assert_eq!(
        fixture.records().err().ok_or("duplicate accepted")?.code,
        "SAIL3107"
    );
    Ok(())
}
