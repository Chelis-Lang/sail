//! Native ordinary-output controls. No fixture result is proof authority.
use super::*;
use crate::managed::{build_inputs, coherent, input};
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct CargoChild {
    child: std::process::Child,
    retired: bool,
}
impl Drop for CargoChild {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let _ = self.child.kill();
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(2) {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.retired = true;
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(_) => break,
            }
        }
        // An exceptional supervisor teardown does not certify descendant
        // closure. The failed outer test scope retains that ownership limit.
    }
}

// GNU timeout owns its Cargo process group, TERM at 60 s and KILL after 2 s.
// File-backed captures avoid pipe-reader joins and retain bounded reads. These
// local Cargo controls make no orphan or kernel-uninterruptibility guarantee.
fn command_output(
    root: &Path,
    program: &std::ffi::OsStr,
    arguments: &[std::ffi::OsString],
    environment: &[(&std::ffi::OsStr, &std::ffi::OsStr)],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    use std::io::Read as _;
    let capture = root.join("cache");
    std::fs::create_dir_all(&capture)?;
    let stdout_path = capture.join("cargo.stdout");
    let stderr_path = capture.join("cargo.stderr");
    let mut command = std::process::Command::new("timeout");
    command
        .args(["--signal=TERM", "--kill-after=2s", "60s"])
        .arg(program)
        .args(arguments)
        .current_dir(root)
        .env("RUSTC_BOOTSTRAP", "1")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    for (key, value) in environment {
        command.env(key, value);
    }
    let child = command
        .stdout(std::fs::File::create(&stdout_path)?)
        .stderr(std::fs::File::create(&stderr_path)?)
        .spawn()?;
    let mut child = CargoChild {
        child,
        retired: false,
    };
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.child.try_wait()? {
            child.retired = true;
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(65) {
            return Err(
                "Cargo supervisor exceeded its budget; descendant closure incomplete".into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let read = |path: &Path| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("Cargo control capture exceeded 1 MiB".into());
        }
        Ok(bytes)
    };
    Ok(std::process::Output {
        status,
        stdout: read(&stdout_path)?,
        stderr: read(&stderr_path)?,
    })
}

fn cargo_output(
    root: &Path,
    arguments: &[std::ffi::OsString],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    command_output(
        root,
        &std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        arguments,
        &[],
    )
}

struct Fixture {
    root: PathBuf,
    target: PathBuf,
    local: PathBuf,
    export: PathBuf,
    journal: PathBuf,
    data: Value,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_at("")
    }
    fn new_at(member: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_roles(member, false)
    }
    #[allow(clippy::too_many_lines)] // Preserve the original fixture and add one never-target package role.
    fn new_with_roles(member: &str, disabled: bool) -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sail-script-bootstrap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let package = root.join(member);
        std::fs::create_dir_all(package.join("src"))?;
        if !member.is_empty() {
            std::fs::write(
                root.join("Cargo.toml"),
                format!("[workspace]\nmembers=[{member:?}]\nresolver=\"3\"\n"),
            )?;
        }
        std::fs::write(
            package.join("Cargo.toml"),
            "[package]\nname=\"sail_bootstrap_fixture\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
        )?;
        if disabled {
            use std::io::Write as _;
            let mut manifest = std::fs::OpenOptions::new()
                .append(true)
                .open(package.join("Cargo.toml"))?;
            manifest.write_all(b"[workspace]\nexclude=[\"disabled\",\"active\"]\nresolver=\"3\"\n[dependencies]\nsail_active_fixture={path=\"active\"}\n[target.\"cfg(any())\".dependencies]\nsail_disabled_fixture={path=\"disabled\"}\n")?;
            let active = package.join("active");
            std::fs::create_dir_all(active.join("src"))?;
            std::fs::write(
                active.join("Cargo.toml"),
                b"[package]\nname=\"sail_active_fixture\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
            )?;
            std::fs::write(
                active.join("build.rs"),
                b"fn main() { println!(\"cargo:rerun-if-changed=build.rs\"); }\n",
            )?;
            std::fs::write(active.join("src/lib.rs"), b"pub const ACTIVE: u8 = 1;\n")?;
            let dependency = package.join("disabled");
            std::fs::create_dir_all(dependency.join("src"))?;
            std::fs::write(
                dependency.join("Cargo.toml"),
                b"[package]\nname=\"sail_disabled_fixture\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
            )?;
            std::fs::write(
                dependency.join("build.rs"),
                b"fn main() { panic!(\"disabled script ran\"); }\n",
            )?;
            std::fs::write(
                dependency.join("src/lib.rs"),
                b"pub const UNUSED: u8 = 0;\n",
            )?;
        }
        std::fs::write(
            package.join("build.rs"),
            "fn main() { println!(\"cargo:rerun-if-changed=build.rs\"); std::fs::write(std::path::Path::new(&std::env::var_os(\"OUT_DIR\").unwrap()).join(\"private.rs\"), \"pub const VALUE: u8 = 1;\\n\").unwrap(); }\n",
        )?;
        std::fs::write(
            package.join("src/lib.rs"),
            "include!(concat!(env!(\"OUT_DIR\"), \"/private.rs\"));\n",
        )?;
        // Establish Cargo.lock before the independent input owner is acquired.
        let arguments =
            ["metadata", "--offline", "--format-version", "1"].map(std::ffi::OsString::from);
        let metadata = cargo_output(&root, &arguments)?;
        assert!(
            metadata.status.success(),
            "metadata:{}",
            String::from_utf8_lossy(&metadata.stderr)
        );
        let mut data: Value = serde_json::from_slice(&metadata.stdout)?;
        let cache = root.join("cache");
        let target = cache.join("target");
        // Match production prepare: Cargo creates and owns the target directory,
        // including its cache tag. An eagerly created target is not Cargo-owned
        // and the original package-clean operation correctly refuses it.
        data["sail_cache_directory"] = serde_json::to_value(&cache)?;
        data["target_directory"] = serde_json::to_value(&target)?;
        data["sail_invocation_directory"] = serde_json::to_value(&root)?;
        Ok(Self {
            local: cache.join("local.json"),
            export: cache.join("family.json"),
            journal: cache.join("artifacts.jsonl"),
            root,
            target,
            data,
        })
    }
    fn compile(&self) -> TestResult {
        let mut arguments: Vec<_> = [
            "build",
            "--offline",
            "--locked",
            "--message-format=json",
            "--target-dir",
        ]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect();
        arguments.push(self.target.as_os_str().to_owned());
        let output = cargo_output(&self.root, &arguments)?;
        assert!(
            output.status.success(),
            "cargo:{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(&self.journal, output.stdout)?;
        build_inputs::update(&self.local, &self.journal, &self.data)?;
        Ok(())
    }
    fn records(&self) -> Result<Vec<BuildInput>, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(&self.local)?)?)
    }
    fn owner(&self) -> Result<Bootstrap, Box<dyn std::error::Error>> {
        Bootstrap::begin(&self.data, &[], &[])
            .ok_or_else(|| "independent script owner unavailable".into())
    }
    fn publish(&self, owner: &mut Bootstrap) {
        owner.publish(
            &self.export,
            "compiler",
            &self.target,
            &self.local,
            &self.data,
            &self.journal,
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bootstrap_actual_cargo_can_publish_ordinary_contract_without_current_proof() -> TestResult {
    let fixture = Fixture::new()?;
    assert!(!super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    let sources = fixture.target.join("sources.json");
    let (paths, covered) =
        crate::managed::source_inputs::update(&sources, &fixture.journal, &fixture.data)?;
    assert!(covered);
    let generated = paths
        .iter()
        .find(|p| {
            p.starts_with(&fixture.target) && p.file_name().is_some_and(|n| n == "private.rs")
        })
        .ok_or("generated declaration missing")?;
    let mut data = fixture.data.clone();
    data["sail_source_inputs"] = serde_json::to_value(&paths)?;
    let before = input::snapshot_metadata(&data, &[]);
    std::fs::remove_file(generated)?;
    let mut proof = coherent::Guard::begin(&before);
    std::fs::write(generated, "pub const VALUE: u8 = 1;\n")?;
    assert!(
        !proof.ready_at(coherent::Barrier::PostVerification),
        "original proof must refuse regenerated source binding"
    );
    fixture.publish(&mut owner);
    assert!(
        fixture.export.is_file(),
        "independent ordinary-script contract must escape absent-generation bootstrap"
    );
    assert!(super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    assert_eq!(
        build_inputs::stale(&fixture.local, &fixture.data).0,
        Vec::<String>::new()
    );
    assert!(
        !proof.ready(),
        "ordinary publication cannot promote the failed proof guard"
    );
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)] // Keep real member compilation and compile-cwd refusal controls together.
fn bootstrap_workspace_member_depinfo_uses_workspace_root() -> TestResult {
    let fixture = Fixture::new_at("member")?;
    assert!(!fixture.root.join("build.rs").exists());
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    let record = fixture
        .records()?
        .into_iter()
        .next()
        .ok_or("record absent")?;
    let original: Vec<Value> = std::fs::read_to_string(&fixture.journal)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_some()
    );
    // Package-parent resolution would look for member/member/build.rs. The
    // actual pinned Cargo prerequisite must be workspace-relative instead.
    let binary = original
        .iter()
        .find(|artifact| {
            artifact["reason"] == "compiler-artifact"
                && artifact["target"]["kind"] == serde_json::json!(["custom-build"])
        })
        .ok_or("binary absent")?;
    let directory = Path::new(
        binary["filenames"][0]
            .as_str()
            .ok_or("binary path absent")?,
    )
    .parent()
    .ok_or("binary directory absent")?;
    let hash = directory
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('-'))
        .ok_or("binary hash absent")?
        .1;
    let dep_info = directory.join(format!("build_script_build-{hash}.d"));
    let text = std::fs::read_to_string(dep_info)?;
    assert!(
        crate::managed::source_inputs::dependencies(&text)
            .ok_or("dep-info absent")?
            .iter()
            .any(|input| input == "member/build.rs")
    );
    fixture.publish(&mut owner);
    assert!(fixture.export.is_file());
    assert!(super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    let mut wrong = fixture.data.clone();
    wrong["workspace_root"] = serde_json::to_value(fixture.root.join("member"))?;
    assert!(
        owner
            .record(&record, &wrong, &fixture.target, &original)
            .is_none()
    );
    let mut foreign = fixture.data.clone();
    foreign["packages"][0]["source"] = serde_json::json!("registry+https://example.invalid");
    assert!(
        owner
            .record(&record, &foreign, &fixture.target, &original)
            .is_none()
    );
    let mut relative = fixture.data.clone();
    relative["workspace_root"] = serde_json::json!(".");
    assert!(
        owner
            .record(&record, &relative, &fixture.target, &original)
            .is_none()
    );
    assert!(Bootstrap::supported_arguments(&[]));
    for args in [
        vec!["-Zroot-dir=member".into()],
        vec!["-Z".into(), "root-dir=member".into()],
        vec!["--config=unstable.root-dir=member".into()],
    ] {
        assert!(!Bootstrap::supported_arguments(&args));
    }
    // The original owner must refuse a changed virtual workspace manifest.
    std::fs::write(fixture.root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")?;
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    std::fs::create_dir_all(fixture.root.join(".cargo"))?;
    std::fs::write(
        fixture.root.join(".cargo/config.toml"),
        "[unstable]\nroot-dir=\"member\"\n",
    )?;
    assert!(Bootstrap::begin(&fixture.data, &[], &[]).is_none());
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)] // Keep real Cargo baseline and each narrow lineage refusal together.
fn bootstrap_reused_binary_unguarded_depinfo_and_injected_env_are_refused() -> TestResult {
    let fixture = Fixture::new()?;
    let owner = fixture.owner()?;
    fixture.compile()?;
    let record = fixture
        .records()?
        .into_iter()
        .next()
        .ok_or("record absent")?;
    let original: Vec<Value> = std::fs::read_to_string(&fixture.journal)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_some()
    );
    let mut reused = original.clone();
    for artifact in &mut reused {
        if artifact["reason"] == "compiler-artifact"
            && artifact["target"]["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
        {
            artifact["fresh"] = true.into();
        }
    }
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &reused)
            .is_none()
    );
    let binary = original
        .iter()
        .find(|a| {
            a["reason"] == "compiler-artifact"
                && a["target"]["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
        })
        .ok_or("binary absent")?;
    let directory = Path::new(binary["filenames"][0].as_str().ok_or("filename absent")?)
        .parent()
        .ok_or("directory absent")?;
    let hash = directory
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.rsplit_once('-'))
        .ok_or("unit absent")?
        .1;
    let dep_info = directory.join(format!("build_script_build-{hash}.d"));
    let bytes = std::fs::read(&dep_info)?;
    std::fs::remove_file(&dep_info)?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::write(&dep_info, format!("{}: \n", dep_info.display()))?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::write(&dep_info, &bytes)?;
    let linked = directory.join(format!("build_script_build-{hash}"));
    std::fs::remove_file(&linked)?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::hard_link(
        Path::new(binary["filenames"][0].as_str().ok_or("filename absent")?),
        &linked,
    )?;
    let external = fixture.root.with_extension("external");
    std::fs::write(&external, "outside\n")?;
    let original_text = String::from_utf8(bytes.clone())?;
    let (first, rest) = original_text
        .split_once('\n')
        .ok_or("dep-info line absent")?;
    // Preserve the exact target and actual source prerequisite: only the
    // additional unguarded compilation input makes this contract inadmissible.
    std::fs::write(&dep_info, format!("{first} {}\n{rest}", external.display()))?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::write(&dep_info, &bytes)?;
    std::fs::write(&dep_info, "out: missing\n")?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    let mut text = String::from_utf8(bytes)?;
    text.push_str("\n# env-dep:OUT_DIR=unbound\n");
    std::fs::write(&dep_info, text)?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::write(
        &dep_info,
        format!("{original_text}\n# env-dep:RUSTC_BOOTSTRAP=2\n"),
    )?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_none()
    );
    std::fs::write(
        &dep_info,
        format!("{original_text}\n# env-dep:RUSTC_BOOTSTRAP=1\n"),
    )?;
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &original)
            .is_some()
    );
    std::fs::remove_file(&external)?;
    Ok(())
}

#[test]
fn bootstrap_restored_write_and_cancel_do_not_leave_export() -> TestResult {
    let fixture = Fixture::new()?;
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    let path = fixture.root.join("build.rs");
    let file = std::fs::File::open(&path)?;
    let mtime = file.metadata()?.modified()?;
    let bytes = std::fs::read(&path)?;
    std::fs::write(&path, b"changed")?;
    std::fs::write(&path, bytes)?;
    file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    drop(owner);
    let mut owner = fixture.owner()?;
    owner.stop.store(true, Ordering::Release);
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    Ok(())
}

#[test]
fn bootstrap_alias_directory_build_dependency_and_multiple_units_refuse() -> TestResult {
    let fixture = Fixture::new()?;
    let owner = fixture.owner()?;
    fixture.compile()?;
    let record = fixture
        .records()?
        .into_iter()
        .next()
        .ok_or("record absent")?;
    let artifacts: Vec<Value> = std::fs::read_to_string(&fixture.journal)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let mut directory = record.clone();
    directory.paths = vec![fixture.root.clone()];
    assert!(
        owner
            .record(&directory, &fixture.data, &fixture.target, &artifacts)
            .is_none()
    );
    let mut metadata = fixture.data.clone();
    metadata["packages"][0]["dependencies"] =
        serde_json::json!([{"kind":"build","name":"dependency"}]);
    assert!(
        owner
            .record(&record, &metadata, &fixture.target, &artifacts)
            .is_none()
    );
    let mut duplicate = artifacts.clone();
    duplicate.extend(
        artifacts
            .iter()
            .filter(|a| a["reason"] == "build-script-executed")
            .cloned(),
    );
    assert!(
        owner
            .record(&record, &fixture.data, &fixture.target, &duplicate)
            .is_none()
    );
    let path = fixture.root.join("build.rs");
    let external_link = fixture.root.with_extension("symlink");
    let other = fixture.root.join("src/lib.rs");
    std::os::unix::fs::symlink(&other, &external_link)?;
    // A script could read a different guarded file through an unguarded alias;
    // restoring that alias to build.rs cannot make its earlier use covered.
    std::fs::remove_file(&external_link)?;
    std::os::unix::fs::symlink(&path, &external_link)?;
    assert!(owner.covered(&path).is_some());
    assert!(owner.covered(&external_link).is_none());
    std::fs::remove_file(&external_link)?;
    let alias = fixture.root.with_extension("alias");
    std::fs::hard_link(&path, &alias)?;
    let bytes = std::fs::read(&path)?;
    std::fs::write(&alias, b"changed")?;
    std::fs::write(&alias, bytes)?;
    let mut owner = owner;
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    std::fs::remove_file(alias)?;
    Ok(())
}

#[test]
fn bootstrap_helper_loss_and_ineligible_generation_keep_original_cleaning() -> TestResult {
    let fixture = Fixture::new()?;
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    owner.lease.stop_owner_for_test()?;
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    drop(owner);
    super::super::publish(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data,
    );
    assert!(fixture.export.exists());
    let record = fixture
        .records()?
        .into_iter()
        .next()
        .ok_or("record absent")?;
    let output = record.output.ok_or("output absent")?.0;
    std::fs::write(output, "cargo:rerun-if-changed=changed.rs\n")?;
    assert!(!super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    assert_eq!(
        build_inputs::stale(&fixture.local, &fixture.data).0,
        vec!["sail_bootstrap_fixture"]
    );
    assert!(!fixture.export.exists());
    Ok(())
}

#[test]
fn bootstrap_empty_generation_and_scriptless_workload_do_not_block_recovery() -> TestResult {
    assert!(!Bootstrap::eligible_workload(
        &serde_json::json!({"packages":[]})
    ));
    assert!(!Bootstrap::eligible_workload(
        &serde_json::json!({"packages":[{
            "dependencies":[],"targets":[{"kind":["lib"]}]
        }]})
    ));
    let fixture = Fixture::new()?;
    assert!(Bootstrap::eligible_workload(&fixture.data));
    super::super::publish_records(&fixture.export, "compiler", Vec::new());
    assert!(!super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    fixture.publish(&mut owner);
    assert!(super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    assert!(super::super::allowed_environment("RUSTC_BOOTSTRAP"));
    assert_eq!(
        effective_value("RUSTC_BOOTSTRAP", Some(&owner.environment)).as_deref(),
        Some("1")
    );
    Ok(())
}

#[test]
fn bootstrap_runtime_identity_and_metadata_are_jointly_guarded() -> TestResult {
    let fixture = Fixture::new()?;
    let runtime = fixture.root.join("compiler-tool");
    std::fs::write(&runtime, "runtime bytes")?;
    let records = vec![(
        runtime.to_string_lossy().into_owned(),
        crate::managed::files::digest(b"runtime bytes"),
    )];
    let encoded = super::super::encode(&records).ok_or("runtime encoding absent")?;
    let sha = crate::managed::files::digest(&encoded);
    let cache = fixture.root.join("cache");
    let directory = cache.join("runtime-identities");
    std::fs::create_dir_all(&directory)?;
    let identity = directory.join(format!("{sha}.json"));
    let link = directory.join("context.link");
    std::fs::write(&identity, encoded)?;
    std::fs::write(&link, serde_json::to_vec(&sha)?)?;
    let mut owner = Bootstrap::begin(
        &fixture.data,
        &[],
        &[runtime.clone(), link.clone(), identity.clone()],
    )
    .ok_or("runtime owner absent")?;
    assert!(owner.runtime_matches(&cache, "context"));
    assert!(!owner.runtime_matches(&cache, "another-context"));
    fixture.compile()?;
    let file = std::fs::File::open(&runtime)?;
    let mtime = file.metadata()?.modified()?;
    std::fs::write(&runtime, "changed runtime")?;
    std::fs::write(&runtime, "runtime bytes")?;
    file.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    drop(owner);
    let mut owner = Bootstrap::begin(
        &fixture.data,
        &[],
        &[runtime, link.clone(), identity.clone()],
    )
    .ok_or("replacement runtime owner absent")?;
    std::fs::write(&identity, b"[]")?;
    assert!(!owner.runtime_matches(&cache, "context"));
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    Ok(())
}

#[test]
fn bootstrap_actual_cargo_injected_environment_is_not_parent_authority() -> TestResult {
    let fixture = Fixture::new()?;
    let build = fixture.root.join("build.rs");
    let text = std::fs::read_to_string(&build)?;
    std::fs::write(
        &build,
        text.replace(
            "fn main() {",
            "fn main() { println!(\"cargo:rerun-if-env-changed=LD_LIBRARY_PATH\");",
        ),
    )?;
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    let record = fixture
        .records()?
        .into_iter()
        .next()
        .ok_or("record absent")?;
    assert!(record.env.contains_key("LD_LIBRARY_PATH"));
    assert!(record.covered);
    assert!(super::super::output_matches(&record, &fixture.target));
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    super::super::publish(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data,
    );
    // Original publication can emit an empty envelope. Its existence is not
    // reuse authority: begin must refuse it and retain forced script cleaning.
    let emitted: Value = serde_json::from_slice(&std::fs::read(&fixture.export)?)?;
    assert_eq!(emitted["records"], serde_json::json!([]));
    assert!(!super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    assert_eq!(
        build_inputs::stale(&fixture.local, &fixture.data).0,
        vec!["sail_bootstrap_fixture"]
    );
    for key in [
        "RUSTDOC",
        "RUSTC_LINKER",
        "RUSTFLAGS",
        "LD_LIBRARY_PATH",
        "DEP_METADATA",
    ] {
        assert!(!super::super::allowed_environment(key));
    }
    Ok(())
}

#[test]
fn bootstrap_non_utf8_environment_is_not_absent_authority() -> TestResult {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStringExt as _;
    let marker = "SAIL_TEST_BOOTSTRAP_NON_UTF8_CHILD";
    if let Some(mode) = std::env::var_os(marker) {
        assert!(environment().is_none());
        let path = std::env::var_os("SAIL_TEST_BOOTSTRAP_DATA").ok_or("child metadata absent")?;
        let data: Value = serde_json::from_slice(
            &super::super::bytes(Path::new(&path)).ok_or("child metadata unreadable")?,
        )?;
        assert!(Bootstrap::eligible_workload(&data));
        assert!(Bootstrap::begin(&data, &[], &[]).is_none());
        if mode.to_str() == Some("value") {
            assert!(matches!(
                std::env::var("LANG"),
                Err(std::env::VarError::NotUnicode(_))
            ));
            assert!(!super::super::allowed_environment("LANG"));
        }
        return Ok(());
    }
    assert!(environment().is_some());
    assert!(super::super::allowed_environment("LANG"));
    let fixture = Fixture::new()?;
    let metadata = fixture.root.join("cache/utf8-test-metadata.json");
    std::fs::write(&metadata, serde_json::to_vec(&fixture.data)?)?;
    let arguments = [
        OsString::from(
            "managed::build_inputs::family::bootstrap::tests::bootstrap_non_utf8_environment_is_not_absent_authority",
        ),
        OsString::from("--exact"),
        OsString::from("--test-threads=1"),
    ];
    let invalid = OsString::from_vec(vec![0xff]);
    let invalid_key = OsString::from_vec(b"SAIL_TEST_INVALID_\xff".to_vec());
    for (mode, key, value) in [
        ("value", OsStr::new("LANG"), invalid.as_os_str()),
        ("key", invalid_key.as_os_str(), OsStr::new("value")),
    ] {
        let output = command_output(
            &fixture.root,
            std::env::current_exe()?.as_os_str(),
            &arguments,
            &[
                (OsStr::new(marker), OsStr::new(mode)),
                (OsStr::new("SAIL_TEST_BOOTSTRAP_DATA"), metadata.as_os_str()),
                (key, value),
            ],
        )?;
        assert!(
            output.status.success(),
            "non-UTF-8 child:{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = std::str::from_utf8(&output.stdout)?;
        assert!(stdout.contains("1 passed; 0 failed"));
    }
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)] // Keep actual first/reused/rebuilt journal and next-generation coverage in one transaction.
fn bootstrap_disabled_dependency_and_retry_keep_fresh_compilation_roles() -> TestResult {
    let fixture = Fixture::new_with_roles("", true)?;
    let packages = fixture.data["packages"]
        .as_array()
        .ok_or("packages absent")?;
    let disabled = packages
        .iter()
        .find(|package| package["name"] == "sail_disabled_fixture")
        .ok_or("disabled package absent")?["id"]
        .as_str()
        .ok_or("disabled id absent")?;
    let members = fixture.data["workspace_members"]
        .as_array()
        .ok_or("members absent")?;
    for package in packages.iter().filter(|package| {
        package["name"] == "sail_active_fixture" || package["name"] == "sail_disabled_fixture"
    }) {
        assert!(
            !members.contains(&package["id"]),
            "dependency must not be a workspace member"
        );
    }
    assert!(build_inputs::never_scheduled(&fixture.data, disabled));
    assert!(!fixture.target.exists(), "Cargo must create its target");
    let mut owner = fixture.owner()?;
    fixture.compile()?;
    assert!(
        fixture.target.join("CACHEDIR.TAG").is_file(),
        "Cargo must establish target ownership before package clean"
    );
    let roles = || -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        Ok(std::fs::read_to_string(&fixture.journal)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?)
    };
    let script_binaries = |records: Vec<Value>| -> Vec<Value> {
        records
            .into_iter()
            .filter(|row| {
                row["reason"] == "compiler-artifact"
                    && row["target"]["kind"] == serde_json::json!(["custom-build"])
            })
            .collect()
    };
    let first = roles()?;
    assert!(first.iter().all(|row| row["package_id"] != disabled));
    let first = script_binaries(first);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|row| row["fresh"] == false));
    // The original retry truncates the journal. Without the added clean, its
    // only remaining binary role is reused and must continue to be refused.
    fixture.compile()?;
    let reused = script_binaries(roles()?);
    assert_eq!(reused.len(), 2);
    assert!(reused.iter().all(|row| row["fresh"] == true));
    fixture.publish(&mut owner);
    assert!(!fixture.export.exists());
    let selected = packages
        .iter()
        .find(|package| package["name"] == "sail_bootstrap_fixture")
        .ok_or("selected package absent")?["id"]
        .as_str()
        .ok_or("selected id absent")?
        .to_owned();
    let retry = Bootstrap::retry_packages(&fixture.data, vec![selected.clone()]);
    assert_eq!(retry.len(), 2);
    assert_eq!(retry[0], selected);
    assert!(retry.iter().any(|id| id.contains("sail_active_fixture")));
    assert!(!retry.iter().any(|id| id == disabled));
    let mut arguments: Vec<_> = ["clean", "--offline", "--locked", "--target-dir"]
        .map(std::ffi::OsString::from)
        .into_iter()
        .collect();
    arguments.push(fixture.target.as_os_str().to_owned());
    for id in &retry {
        arguments.push("--package".into());
        arguments.push(id.into());
    }
    let cleaned = cargo_output(&fixture.root, &arguments)?;
    assert!(
        cleaned.status.success(),
        "clean:{}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    fixture.compile()?;
    let rebuilt = script_binaries(roles()?);
    assert_eq!(rebuilt.len(), 2);
    assert!(rebuilt.iter().all(|row| row["fresh"] == false));
    fixture.publish(&mut owner);
    assert!(fixture.export.is_file());
    assert!(super::super::begin(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data
    )?);
    let (clean, _, covered) = build_inputs::stale(&fixture.local, &fixture.data);
    assert_eq!(clean, Vec::<String>::new());
    assert!(
        covered,
        "second declaration coverage must include no unexecuted script"
    );
    assert_eq!(fixture.records()?.len(), 2);
    let source_state = fixture.target.join("sources.json");
    let (paths, source_covered) =
        crate::managed::source_inputs::update(&source_state, &fixture.journal, &fixture.data)?;
    assert!(source_covered);
    let mut next = fixture.data.clone();
    next["sail_source_inputs"] = serde_json::to_value(paths)?;
    let before = input::snapshot_metadata(&next, &[]);
    assert!(before.complete);
    let mut proof = coherent::Guard::begin(&before);
    fixture.compile()?;
    assert!(proof.ready_at(coherent::Barrier::PostVerification));
    // This is the original input barrier, not a new Verus verdict or receipt.
    // Explicit package selection conservatively restores the original refusal.
    let mut explicitly_selected = fixture.data.clone();
    explicitly_selected["sail_selected_package_ids"] = serde_json::json!([disabled]);
    let (clean, _, covered) = build_inputs::stale(&fixture.local, &explicitly_selected);
    assert_eq!(clean, vec!["sail_disabled_fixture"]);
    assert!(!covered);
    Ok(())
}

#[test]
fn bootstrap_retry_clean_preserves_missing_and_refuses_unsupported_script_roles() {
    let data = serde_json::json!({"packages":[
        {"id":"supported", "dependencies":[], "targets":[{"kind":["custom-build"]}]},
        {"id":"build-dependency", "dependencies":[{"kind":"build"}],
            "targets":[{"kind":["custom-build"]}]},
        {"id":"multiple", "dependencies":[],
            "targets":[{"kind":["custom-build"]},{"kind":["custom-build"]}]},
        {"id":"library", "dependencies":[], "targets":[{"kind":["lib"]}]}]});
    assert_eq!(
        Bootstrap::retry_packages(&data, vec!["missing".into()]),
        vec!["missing", "supported"]
    );
    assert_eq!(
        Bootstrap::retry_packages(&data, vec!["supported".into()]),
        vec!["supported"]
    );
    assert_eq!(
        Bootstrap::retry_packages(&serde_json::json!({}), vec!["missing".into()]),
        vec!["missing"]
    );
}

#[test]
fn bootstrap_observation_preserves_actual_publication_and_first_role_refusal() -> TestResult {
    let fixture = Fixture::new()?;
    let path = fixture.root.join("cache/bootstrap-diagnostic.jsonl");
    let diagnostics = crate::managed::diagnostics::Diagnostics::for_test(&path);
    let mut owner = Bootstrap::begin_traced(&fixture.data, &[], &[], Some(&diagnostics))
        .ok_or("observed owner unavailable")?;
    fixture.compile()?;
    fixture.publish(&mut owner);
    assert!(fixture.export.is_file());
    std::fs::remove_file(&fixture.export)?;
    owner.publish_traced(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &fixture.data,
        &fixture.journal,
        Some(&diagnostics),
    );
    assert!(
        fixture.export.is_file(),
        "observation cannot refuse an original healthy contract"
    );
    std::fs::remove_file(&fixture.export)?;
    let mut unsupported = fixture.data.clone();
    unsupported["packages"][0]["dependencies"] = serde_json::json!([{"kind":"build"}]);
    owner.publish_traced(
        &fixture.export,
        "compiler",
        &fixture.target,
        &fixture.local,
        &unsupported,
        &fixture.journal,
        Some(&diagnostics),
    );
    assert!(
        !fixture.export.exists(),
        "observation cannot admit an original refused role"
    );
    let rows: Vec<Value> = std::fs::read_to_string(&path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["stage"], "owner-complete");
    assert_eq!(rows[0]["outcome"], "ready");
    assert_eq!(rows[1]["stage"], "publish-emission");
    assert_eq!(rows[1]["outcome"], "attempted");
    assert_eq!(rows[2]["stage"], "record-package");
    assert_eq!(rows[2]["outcome"], "refused");
    for row in rows {
        assert_eq!(row["kind"], "managed-script-bootstrap");
        assert_eq!(row.as_object().map(serde_json::Map::len), Some(4));
    }
    Ok(())
}
