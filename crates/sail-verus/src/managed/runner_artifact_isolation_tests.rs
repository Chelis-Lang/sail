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
#[allow(
    clippy::too_many_lines,
    reason = "One layout transaction checks default sharing, selector isolation and ownership release together."
)]
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
    assert_eq!(
        plain.build_inputs,
        plain.input_state.with_file_name("build-inputs.json")
    );
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
    assert!(first.build_inputs.parent().is_some_and(Path::is_dir));
    assert_eq!(
        first.build_inputs.parent(),
        Some(
            first
                .input_state
                .parent()
                .ok_or("context absent")?
                .join("repair-observations-v1")
                .as_path()
        )
    );
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
    reason = "One migration transaction retains legacy evidence and tests its deletion against the original native owner."
)]
fn repair_selector_migration_does_not_watch_legacy_generated_inputs() -> IsolationResult {
    use super::super::{coherent, source_inputs};
    let fixture = IsolationFixture::new()?;
    let selected = compiler_request(Some("application::a"));
    let (paths, owner) = prepare(
        &fixture.config,
        &fixture.root,
        "selected",
        "family",
        "migration",
        Some(&selected),
    )?;
    let context = paths.input_state.parent().ok_or("context absent")?;
    let legacy_source = context.join("source-inputs.json");
    let legacy_build = context.join("build-inputs.json");
    let legacy_generated = fixture.config.cache.join("old-shared-target/out/input.rs");
    std::fs::create_dir_all(legacy_generated.parent().ok_or("legacy parent absent")?)?;
    std::fs::write(&legacy_generated, "old generated input")?;
    json(
        &legacy_source,
        &serde_json::json!({"paths":[legacy_generated],"covered":true,"by_package":{"old-input":[legacy_generated]}}),
    )?;
    std::fs::write(&legacy_build, "old ordinary observations")?;
    let old_source_bytes = std::fs::read(&legacy_source)?;
    let historical_receipt = paths.receipts.join("old-receipt.json");
    std::fs::write(&historical_receipt, "historical receipt evidence")?;

    let artifacts = fixture.cargo(&paths, "a", false)?;
    let package_id = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .and_then(|row| row["package_id"].as_str())
        .ok_or("dependency absent")?;
    let journal = paths.run.join("artifacts.jsonl");
    std::fs::write(
        &journal,
        artifacts
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
    )?;
    let source_state = paths.build_inputs.with_file_name("source-inputs.json");
    let mut data = fixture.data(&paths, package_id);
    record_observation_layout(&mut data, Some(&selected));
    let (sources, covered) = source_inputs::update(&source_state, &journal, &data)?;
    assert!(covered && !sources.contains(&legacy_generated));
    assert!(
        sources
            .iter()
            .any(|path| path.starts_with(&paths.target) && path.ends_with("input.rs"))
    );
    data["sail_source_inputs"] = serde_json::to_value(&sources)?;
    json(&paths.input_state, &data)?;
    assert_eq!(
        observation_directory(context, &data)?,
        paths.build_inputs.parent().ok_or("observations absent")?
    );
    let (roots, _) = observation_plan(
        &fixture.config.cache,
        &fixture.root,
        "selected",
        &BTreeMap::new(),
    )?;
    assert!(
        roots.contains(&source_state)
            && roots.contains(&paths.build_inputs)
            && roots.contains(&paths.input_state)
    );
    assert!(!roots.contains(&legacy_source) && !roots.contains(&legacy_build));
    assert!(
        refresh_context_validated(
            &fixture.config.cache,
            &fixture.root,
            "selected",
            &BTreeMap::new(),
            || true
        )?
        .complete
    );
    let before = snapshot_metadata(&data, &[]);
    assert!(before.complete);
    let mut guard = coherent::Guard::begin(&before);
    assert!(guard.ready_at(coherent::Barrier::Input));
    // A later shared-target cleanup must not invalidate this migrated owner.
    std::fs::remove_file(&legacy_generated)?;
    let after = snapshot_metadata(&data, &[]);
    assert!(after.complete);
    assert_eq!(before.revision, after.revision);
    assert!(guard.ready_at(coherent::Barrier::PostVerification));
    drop(guard);
    let mut old_data = data.clone();
    old_data["sail_source_inputs"] = serde_json::to_value(source_inputs::load(&legacy_source).0)?;
    assert!(!snapshot_metadata(&old_data, &[]).complete);
    assert_eq!(std::fs::read(&legacy_source)?, old_source_bytes);
    assert_eq!(
        std::fs::read_to_string(&legacy_build)?,
        "old ordinary observations"
    );
    assert_eq!(
        std::fs::read_to_string(&historical_receipt)?,
        "historical receipt evidence"
    );
    data["sail_managed_observation_layout"] = Value::from(2);
    assert!(observation_directory(context, &data).is_err());
    assert!(
        observation_plan(
            &fixture.config.cache,
            &fixture.root,
            "selected",
            &BTreeMap::new()
        )
        .is_ok()
    );
    json(&paths.input_state, &data)?;
    assert!(
        observation_plan(
            &fixture.config.cache,
            &fixture.root,
            "selected",
            &BTreeMap::new()
        )
        .is_err()
    );
    drop(owner);
    Ok(())
}

#[test]
fn repair_observation_layout_preserves_default_and_rejects_unknown_schema() -> IsolationResult {
    let context = Path::new("context");
    for engine in [None, Some(compiler_request(None))] {
        let mut data = serde_json::json!({});
        record_observation_layout(&mut data, engine.as_ref());
        assert!(data.get("sail_managed_observation_layout").is_none());
        assert_eq!(observation_directory(context, &data)?, context);
    }
    // The same selector is recorded in both original metadata acquisition paths.
    let engine = compiler_request(Some("application::a"));
    let mut initial = serde_json::json!({});
    let mut final_data = serde_json::json!({});
    record_observation_layout(&mut initial, Some(&engine));
    record_observation_layout(&mut final_data, Some(&engine));
    assert_eq!(initial, final_data);
    assert_eq!(
        observation_directory(context, &final_data)?,
        context.join("repair-observations-v1")
    );
    for version in [
        serde_json::json!(null),
        serde_json::json!("1"),
        serde_json::json!(0),
        serde_json::json!(2),
    ] {
        final_data["sail_managed_observation_layout"] = version;
        assert!(observation_directory(context, &final_data).is_err());
    }
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
    // The actual Cargo artifact can include both rlib and rmeta. Match the
    // production receipt's complete observed set, not the single-file helper.
    let mut dependency_files: Vec<PathBuf> = dependency["filenames"]
        .as_array()
        .ok_or("dependency filenames absent")?
        .iter()
        .map(|path| {
            path.as_str()
                .map(PathBuf::from)
                .ok_or("invalid dependency filename")
        })
        .collect::<std::result::Result<_, _>>()?;
    dependency_files.extend(
        dependency_files
            .clone()
            .into_iter()
            .map(|path| path.with_extension("vir"))
            .filter(|path| path.is_file()),
    );
    receipt.artifact_hashes = artifact_hashes(&dependency_files)?;
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
    assert_eq!(
        receipt.input_revision,
        target_revision(&fixture.data(&again, &package_id), &package_id)
    );
    let mut observed_files: BTreeSet<PathBuf> = dependency["filenames"]
        .as_array()
        .ok_or("reused filenames absent")?
        .iter()
        .map(|path| {
            path.as_str()
                .map(PathBuf::from)
                .ok_or("invalid reused filename")
        })
        .collect::<std::result::Result<_, _>>()?;
    observed_files.extend(
        observed_files
            .clone()
            .into_iter()
            .map(|path| path.with_extension("vir"))
            .filter(|path| path.is_file()),
    );
    assert_eq!(
        receipt
            .artifact_hashes
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        observed_files
    );
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
