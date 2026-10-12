// Real Cargo/compiler controls; synthetic proof receipts test storage only.

type IsolationResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct IsolationFixture {
    root: PathBuf,
    config: Config,
    wrapper: PathBuf,
    calls: PathBuf,
}

impl Drop for IsolationFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl IsolationFixture {
    fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt as _;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-repair-artifact-isolation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let config = compiler_fixture(&root)?;
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::create_dir_all(root.join("input/src"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='application'\nversion='0.1.0'\nedition='2024'\n[dependencies]\ninput={path='input'}\n[workspace]\n",
        )?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn application() -> &'static str { input::input() }\n",
        )?;
        std::fs::write(
            root.join("input/Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\nedition='2024'\n[package.metadata.verus]\nverify=true\n",
        )?;
        std::fs::write(
            root.join("input/src/lib.rs"),
            "include!(concat!(env!(\"OUT_DIR\"), \"/input.rs\"));\n",
        )?;
        std::fs::write(
            root.join("input/build.rs"),
            r#"fn main() {
    println!("cargo:rerun-if-env-changed=SAIL_TEST_SELECTOR");
    let value = std::env::var("SAIL_TEST_SELECTOR").unwrap();
    let output = std::env::var("OUT_DIR").unwrap() + "/input.rs";
    std::fs::write(output, format!("pub fn input() -> &'static str {{ {:?} }}\n", value)).unwrap();
}
"#,
        )?;
        let wrapper = root.join("wrapper.sh");
        std::fs::write(
            &wrapper,
            r#"#!/bin/sh
previous=''
for value in "$@"; do
    if [ "$previous" = '--crate-name' ]; then
        case "$value" in input|application) printf '%s\n' "$value" >> "$SAIL_TEST_CALLS";; esac
        break
    fi
    previous="$value"
done
exec "$@"
"#,
        )?;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            calls: root.join("calls"),
            root,
            config,
            wrapper,
        })
    }

    fn cargo(
        &self,
        paths: &Paths,
        selector: &str,
        clean: bool,
    ) -> std::result::Result<Vec<Value>, Box<dyn std::error::Error>> {
        use std::ffi::OsStr;
        let mut arguments: Vec<std::ffi::OsString> = if clean {
            ["clean", "--offline", "-p", "application"]
                .map(Into::into)
                .into()
        } else {
            ["build", "--offline", "--message-format=json"]
                .map(Into::into)
                .into()
        };
        arguments.extend(["--target-dir".into(), paths.target.as_os_str().to_owned()]);
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let result = super::super::build_inputs::family::bootstrap::test_command_output(
            &self.root,
            &cargo,
            &arguments,
            &[
                (OsStr::new("RUSTC_WRAPPER"), self.wrapper.as_os_str()),
                (OsStr::new("SAIL_TEST_CALLS"), self.calls.as_os_str()),
                (OsStr::new("SAIL_TEST_SELECTOR"), OsStr::new(selector)),
            ],
        )?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Ok(result
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice(line).ok())
            .collect())
    }

    fn data(&self, paths: &Paths, package_id: &str) -> Value {
        serde_json::json!({
            "workspace_root":self.root,"sail_cache_directory":self.config.cache,
            "target_directory":paths.target,
            "packages":[{"id":package_id,"name":"input","manifest_path":self.root.join("input/Cargo.toml"),
                "metadata":{"verus":{"verify":true}},"targets":[{"name":"input","kind":["lib"]}]}],
            "resolve":{"nodes":[{"id":package_id,"dependencies":[]}]}
        })
    }
}

#[test]
fn repair_selector_layout_keeps_defaults_shared_and_exports_private() -> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let a = compiler_request(Some("application::a"));
    let b = compiler_request(Some("application::b"));
    let full = compiler_request(None);
    let (plain, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "plain",
        "family",
        "run",
        None,
    )?;
    drop(owner);
    let (whole, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "whole",
        "family",
        "run",
        Some(&full),
    )?;
    assert_eq!(plain.target, whole.target);
    assert_eq!(plain.compiler_inputs, whole.compiler_inputs);
    drop(owner);
    let (first, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "a",
        "family",
        "run",
        Some(&a),
    )?;
    assert_ne!(first.target, plain.target);
    assert_ne!(first.compiler_inputs, plain.compiler_inputs);
    assert_eq!(first.proxy_alias, plain.proxy_alias);
    assert_eq!(first.cargo_alias, plain.cargo_alias);
    assert!(
        prepare(
            &fixture.config,
            &fixture.root,
            "b",
            "family",
            "run",
            Some(&b)
        )
        .is_err()
    );
    drop(owner);
    let (other, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "b",
        "family",
        "run",
        Some(&b),
    )?;
    assert_ne!(first.target, other.target);
    assert_ne!(first.compiler_inputs, other.compiler_inputs);
    assert_eq!(first.proxy_alias, other.proxy_alias);
    drop(owner);
    let (again, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "a",
        "family",
        "again",
        Some(&a),
    )?;
    assert_eq!(first.target, again.target);
    assert_eq!(first.compiler_inputs, again.compiler_inputs);
    assert_eq!(first.receipts, again.receipts);
    drop(owner);
    let mut broken = fixture.config.clone();
    broken.sail = fixture.root.join("absent-executable");
    assert!(prepare(&broken, &fixture.root, "a", "family", "error", Some(&a)).is_err());
    let (_, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "a",
        "family",
        "after-error",
        Some(&a),
    )?;
    drop(owner);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One A/B/A transaction keeps the real compiler generation and receipt sensitivity together."
)]
fn repair_selector_real_cargo_preserves_dependency_receipt_and_one_driver() -> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let selected = compiler_request(Some("application::a"));
    let other = compiler_request(Some("application::b"));
    let (first, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "selected",
        "family",
        "run",
        Some(&selected),
    )?;
    let artifacts = fixture.cargo(&first, "a", false)?;
    let dependency = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .ok_or("dependency artifact absent")?;
    let artifact = PathBuf::from(
        dependency["filenames"][0]
            .as_str()
            .ok_or("dependency filename absent")?,
    );
    let package_id = dependency["package_id"]
        .as_str()
        .ok_or("dependency package ID absent")?
        .to_owned();
    let data = fixture.data(&first, &package_id);
    let mut receipt = compiler_receipt(&first, &data, &artifact);
    receipt.package_id.clone_from(&package_id);
    receipt.input_revision = target_revision(&data, &package_id);
    let receipt = receipt.seal();
    let path = receipt_path(&first, &package_id, "input", &["lib".to_owned()]);
    json(
        &first
            .run
            .join("pending-receipts")
            .join(path.file_name().ok_or("receipt name absent")?),
        &receipt,
    )?;
    super::super::receipts::publish(&first.run, &first.receipts, "selected")?;
    assert!(receipt.valid("selected"));
    let original = file_digest(&artifact)?;
    let original_bytes = std::fs::read(&artifact)?;
    drop(owner);

    let (second, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "other",
        "family",
        "run",
        Some(&other),
    )?;
    let artifacts = fixture.cargo(&second, "b", false)?;
    let dependency = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .ok_or("other dependency artifact absent")?;
    let different = PathBuf::from(
        dependency["filenames"][0]
            .as_str()
            .ok_or("other dependency filename absent")?,
    );
    assert_ne!(file_digest(&different)?, original);
    // Removing only layout isolation must fail this unchanged byte/receipt check.
    assert_eq!(file_digest(&artifact)?, original);
    assert!(receipt.valid("selected"));
    drop(owner);

    let (again, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "selected",
        "family",
        "again",
        Some(&selected),
    )?;
    fixture.cargo(&again, "a", true)?;
    std::fs::write(&fixture.calls, [])?;
    let artifacts = fixture.cargo(&again, "a", false)?;
    assert_eq!(std::fs::read_to_string(&fixture.calls)?, "application\n");
    let dependency = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .ok_or("reused dependency artifact absent")?;
    assert_eq!(dependency["fresh"], true);
    assert!(receipt.valid("selected"));
    std::fs::write(
        again.run.join("artifacts.jsonl"),
        serde_json::to_vec(dependency)?,
    )?;
    let (targets, missing) = target_records(
        &again,
        &fixture.data(&again, &package_id),
        "selected",
        true,
        &BTreeSet::new(),
    )?;
    assert!(missing.is_empty());
    assert_eq!(targets.len(), 1);
    assert!(targets[0].reused && !targets[0].requested);

    let saved = std::fs::metadata(&artifact)?.modified()?;
    std::fs::write(&artifact, "changed dependency artifact")?;
    std::fs::File::open(&artifact)?.set_times(std::fs::FileTimes::new().set_modified(saved))?;
    assert!(!receipt.valid("selected"));
    let (targets, missing) = target_records(
        &again,
        &fixture.data(&again, &package_id),
        "selected",
        true,
        &BTreeSet::new(),
    )?;
    assert_eq!(targets, Vec::<TargetResult>::new());
    assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
    assert!(!receipt.valid("other"));
    std::fs::write(&artifact, original_bytes)?;
    assert!(receipt.valid("selected"));
    let source = fixture.root.join("input/src/lib.rs");
    let saved = std::fs::metadata(&source)?.modified()?;
    std::fs::write(&source, "pub fn input() -> &'static str { \"changed\" }\n")?;
    std::fs::File::open(&source)?.set_times(std::fs::FileTimes::new().set_modified(saved))?;
    let (targets, missing) = target_records(
        &again,
        &fixture.data(&again, &package_id),
        "selected",
        true,
        &BTreeSet::new(),
    )?;
    assert_eq!(targets, Vec::<TargetResult>::new());
    assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
    std::fs::remove_file(&receipt.committed)?;
    assert!(!receipt.valid("selected"));
    drop(owner);
    Ok(())
}
