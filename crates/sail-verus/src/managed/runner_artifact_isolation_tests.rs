// Real Cargo controls for the shared compiler-family target; synthetic proof receipts test storage only.

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
        self.cargo_selected(paths, selector, clean, &[])
    }

    fn cargo_selected(
        &self,
        paths: &Paths,
        selector: &str,
        clean: bool,
        selection: &[&str],
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
        arguments.extend(selection.iter().map(|arg| std::ffi::OsString::from(*arg)));
        arguments.extend(["--target-dir".into(), paths.target.as_os_str().to_owned()]);
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let result = std::process::Command::new(&cargo)
            .args(&arguments)
            .current_dir(&self.root)
            .env("RUSTC_BOOTSTRAP", "1")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env("RUSTC_WRAPPER", self.wrapper.as_os_str())
            .env("SAIL_TEST_CALLS", self.calls.as_os_str())
            .env("SAIL_TEST_SELECTOR", OsStr::new(selector))
            .output()?;
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
fn repair_functions_share_one_family_target_and_keep_private_proof_paths() -> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let (plain, owner) = prepare(&fixture.config, &fixture.root, "plain", "family", "run")?;
    drop(owner);
    // The engine function is not part of the family: full runs and every
    // repair function of one compiler invocation share its Cargo target.
    for context in ["whole", "a", "b"] {
        let (paths, owner) = prepare(&fixture.config, &fixture.root, context, "family", "run")?;
        assert_eq!(paths.target, plain.target);
        assert_eq!(paths.compiler_inputs, plain.compiler_inputs);
        assert_eq!(paths.cargo_alias, plain.cargo_alias);
        assert_eq!(paths.proxy_alias, plain.proxy_alias);
        assert_ne!(paths.receipts, plain.receipts);
        assert_ne!(paths.build_inputs, plain.build_inputs);
        assert_eq!(
            paths.build_inputs,
            paths.input_state.with_file_name("build-inputs.json")
        );
        drop(owner);
    }
    let mut members: Vec<String> = std::fs::read_dir(&plain.family_contexts)?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<std::result::Result<_, _>>()?;
    members.sort();
    assert_eq!(members, ["a", "b", "plain", "whole"]);
    // Another context of the same family waits for the owner of the target.
    let (_, owner) = prepare(&fixture.config, &fixture.root, "a", "family", "again")?;
    let (other, owner) = std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| prepare(&fixture.config, &fixture.root, "b", "family", "waiting"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!waiting.is_finished(), "concurrent family owner admitted");
        drop(owner);
        waiting.join()
    })
    .map_err(|_| "waiter panicked")??;
    assert_eq!(other.target, plain.target);
    drop(owner);
    let (isolated, owner) = prepare(&fixture.config, &fixture.root, "a", "different", "run")?;
    assert_ne!(isolated.target, plain.target);
    drop(owner);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One real-Cargo transaction keeps receipt establishment, sibling reuse and its refusals together."
)]
fn sibling_dependency_receipt_needs_identical_fresh_bytes_and_never_covers_a_selected_target()
-> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let (first, owner) = prepare(&fixture.config, &fixture.root, "selected", "family", "run")?;
    let artifacts = fixture.cargo(&first, "same", false)?;
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
    drop(owner);

    // A second repair function of the same family: Cargo keeps the dependency
    // fresh, and the sibling's receipt for those exact bytes is reused.
    let (second, owner) = prepare(&fixture.config, &fixture.root, "other", "family", "run")?;
    std::fs::write(&fixture.calls, [])?;
    let artifacts = fixture.cargo(&second, "same", false)?;
    assert_eq!(std::fs::read_to_string(&fixture.calls)?, "");
    let dependency = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .ok_or("reused dependency artifact absent")?;
    assert_eq!(dependency["fresh"], true);
    std::fs::write(
        second.run.join("artifacts.jsonl"),
        serde_json::to_vec(dependency)?,
    )?;
    let data = fixture.data(&second, &package_id);
    let records = |expected: &BTreeSet<(String, String, String)>| {
        target_records(&second, &data, "other", true, expected, &[], None)
    };
    let (targets, missing) = records(&BTreeSet::new())?;
    assert!(missing.is_empty());
    assert_eq!(targets.len(), 1);
    assert!(targets[0].reused && !targets[0].requested);
    assert_eq!(targets[0].establishing_run, receipt.invocation.run_id);
    // A selected target needs a receipt of its own proof context.
    let selected = BTreeSet::from([(package_id.clone(), "input".to_owned(), "lib".to_owned())]);
    for force in [false, true] {
        let (targets, missing) = target_records(&second, &data, "other", force, &selected, &[], None)?;
        assert_eq!(targets, Vec::<TargetResult>::new());
        assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
    }
    // Changed artifact bytes invalidate the sibling's receipt.
    let original = std::fs::read(&artifact)?;
    let saved = std::fs::metadata(&artifact)?.modified()?;
    std::fs::write(&artifact, "changed dependency artifact")?;
    std::fs::File::open(&artifact)?.set_times(std::fs::FileTimes::new().set_modified(saved))?;
    assert_eq!(
        records(&BTreeSet::new())?.1,
        BTreeSet::from(["input".to_owned()])
    );
    std::fs::write(&artifact, &original)?;
    assert!(records(&BTreeSet::new())?.1.is_empty());
    // A context outside the family's membership is never consulted.
    let member = second.family_contexts.join("selected");
    std::fs::remove_file(&member)?;
    assert_eq!(
        records(&BTreeSet::new())?.1,
        BTreeSet::from(["input".to_owned()])
    );
    std::fs::write(&member, [])?;
    drop(owner);

    // If the function did reach the dependency's build, Cargo's fingerprint
    // rebuilds it, and the earlier bytes' receipt no longer applies.
    let (third, owner) = prepare(&fixture.config, &fixture.root, "third", "family", "run")?;
    std::fs::write(&fixture.calls, [])?;
    let artifacts = fixture.cargo(&third, "different", false)?;
    assert!(std::fs::read_to_string(&fixture.calls)?.contains("input"));
    let dependency = artifacts
        .iter()
        .find(|row| row["reason"] == "compiler-artifact" && row["target"]["name"] == "input")
        .ok_or("rebuilt dependency artifact absent")?;
    assert_eq!(dependency["fresh"], false);
    assert!(!receipt.valid("selected"));
    std::fs::write(
        third.run.join("artifacts.jsonl"),
        serde_json::to_vec(dependency)?,
    )?;
    let (targets, missing) = target_records(
        &third,
        &fixture.data(&third, &package_id),
        "third",
        true,
        &BTreeSet::new(),
        &[],
        None,
    )?;
    assert_eq!(targets, Vec::<TargetResult>::new());
    assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
    drop(owner);
    Ok(())
}

/// The real Cargo transition that made the first repair rebuild vstd: the
/// same package changes from default selection to explicit `--lib`.
#[test]
fn adding_lib_selection_keeps_real_dependency_artifacts() -> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let request = compiler_request(None);
    let identity = vec![("compiler".to_owned(), "bytes".to_owned())];
    let selected = compiler_request(Some("application"));
    let first_args = vec!["--package".to_owned(), "application".to_owned()];
    let mut next_args = first_args.clone();
    next_args.push("--lib".to_owned());
    let (first_context, first_family) = context_hashes(&first_args, Some(&request), &identity, &[])?;
    let (next_context, next_family) = context_hashes(&next_args, Some(&selected), &identity, &[])?;
    assert_ne!(first_context, next_context);
    assert_eq!(first_family, next_family);
    let (first, owner) = prepare(&fixture.config, &fixture.root, &first_context, &first_family, "first")?;
    fixture.cargo_selected(&first, "same", false, &["--package", "application"])?;
    assert!(std::fs::read_to_string(&fixture.calls)?.contains("input"));
    drop(owner);
    let (next, owner) = prepare(&fixture.config, &fixture.root, &next_context, &next_family, "next")?;
    assert_eq!(first.target, next.target);
    assert_ne!(first.receipts, next.receipts);
    std::fs::write(&fixture.calls, [])?;
    let artifacts = fixture.cargo_selected(&next, "same", false, &["--package", "application", "--lib"])?;
    let dependency = artifacts.iter().find(|row| row["target"]["name"] == "input")
        .ok_or("dependency artifact absent")?;
    assert_eq!(dependency["fresh"], true);
    assert_eq!(std::fs::read_to_string(&fixture.calls)?, "");
    // Sharing a directory cannot conceal changed dependency source bytes.
    let dependency_source = fixture.root.join("input/src/lib.rs");
    let original = std::fs::read_to_string(&dependency_source)?;
    std::fs::write(&dependency_source, format!("{original}\n// dependency changed\n"))?;
    let changed = fixture.cargo_selected(
        &next, "same", false, &["--package", "application", "--lib"],
    )?;
    let dependency = changed.iter().find(|row| row["target"]["name"] == "input")
        .ok_or("changed dependency artifact absent")?;
    assert_eq!(dependency["fresh"], false);
    assert!(std::fs::read_to_string(&fixture.calls)?.contains("input"));
    drop(owner);
    Ok(())
}

#[test]
fn selection_sharing_keeps_real_feature_profile_and_config_builds_separate() -> IsolationResult {
    let fixture = IsolationFixture::new()?;
    let manifest = fixture.root.join("input/Cargo.toml");
    let original = std::fs::read_to_string(&manifest)?;
    std::fs::write(&manifest, format!("{original}[features]\nextra=[]\n"))?;
    let identity = vec![("compiler".to_owned(), "bytes".to_owned())];
    let request = compiler_request(None);
    let mut families = BTreeSet::new();
    for extra in [
        &[][..],
        &["--features", "input/extra"],
        &["--release"],
        &["--config", "build.rustflags=['--cfg', 'sail_family_control']"],
    ] {
        let mut args = vec!["--package", "application", "--lib"];
        args.extend(extra);
        let arguments: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let (context, family) = context_hashes(&arguments, Some(&request), &identity, &[])?;
        assert!(families.insert(family.clone()), "compiler input was merged: {extra:?}");
        let (paths, owner) = prepare(&fixture.config, &fixture.root, &context, &family, "run")?;
        std::fs::write(&fixture.calls, [])?;
        let artifacts = fixture.cargo_selected(&paths, "same", false, &args)?;
        let dependency = artifacts.iter().find(|row| row["target"]["name"] == "input")
            .ok_or("dependency artifact absent")?;
        assert_eq!(dependency["fresh"], false);
        assert!(std::fs::read_to_string(&fixture.calls)?.contains("input"));
        drop(owner);
    }
    Ok(())
}
