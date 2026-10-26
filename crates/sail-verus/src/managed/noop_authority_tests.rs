//! Real native archive/replay fixtures, not a trusted Verus process or complete
//! compiler-input domain. Every supplied stamp names a real observed path.

use super::*;
use crate::managed::{files, receipts};
use sail_api::project::{Currentness, Execution, TargetResult};

struct Fixture {
    root: PathBuf,
    config: Config,
    context: PathBuf,
    family: PathBuf,
    producer: PathBuf,
    receipt: receipts::Receipt,
    project: ProjectResult,
    observed: Vec<PathBuf>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-noop-producer-history-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let root = root.canonicalize()?;
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("src"))?;
        let manifest = workspace.join("Cargo.toml");
        let source = workspace.join("src/lib.rs");
        std::fs::write(
            &manifest,
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        std::fs::write(&source, "pub fn ordinary_fixture() {}\n")?;
        let cache = root.join("cache");
        let context = cache.join("projects/workspace/context");
        let family = cache.join("projects/workspace/compiler-artifacts/family");
        let producer = context.join("runs/original");
        for directory in [
            context.join("receipts"),
            family.join("contexts"),
            family.join("target"),
            cache.join("runtime-identities"),
        ] {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(family.join("contexts/context"), [])?;
        std::fs::write(family.join("ownership.lock"), [])?;
        std::fs::write(cache.join("projects/workspace/workspace.lock"), [])?;
        let artifact = family.join("target/libdemo.rmeta");
        std::fs::write(&artifact, b"owned fixture artifact")?;
        let receipt = original_receipt(&context, &producer, &artifact, &manifest)?;
        let target = TargetResult {
            package_id: "demo".into(),
            target: "demo".into(),
            kind: vec!["lib".into()],
            establishing_run: "original".into(),
            reused: true,
            scope_reuse: None,
            requested: true,
            complete: true,
            report: receipt.invocation.report.clone(),
            dependency_store: None,
            dependency_build: None,
        };
        assert!(sail_api::acceptance::complete_target(&target).is_some());
        let project = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: sail_api::project::SCHEMA_VERSION,
            run_id: "warm-noop-record".into(),
            workspace: workspace.to_string_lossy().into_owned(),
            context_sha256: "context".into(),
            input_revision: "actual-file-revision".into(),
            currentness: Currentness::Current,
            exit_code: Some(0),
            targets: vec![target],
            execution: Execution {
                verifier_invocations: 0,
                reused_targets: 1,
                wall_ms: 0.0,
            },
            notes: Vec::new(),
            error: None,
            engine: None,
            no_reuse: None,
        };
        let config = Config {
            workspace: workspace.clone(),
            cache,
            sail: "/bin/sh".into(),
            cargo_verus: "/bin/sh".into(),
            cargo: "/bin/sh".into(),
            cargo_toolchain: super::super::CargoToolchain::Caller,
            shim: "/bin/sh".into(),
            z3: "/bin/sh".into(),
            socket: None,
            args: Vec::new(),
            force: false,
            attribution_logs: false,
            request_capture: None,
        };
        Ok(Self {
            root,
            config,
            context,
            family,
            producer,
            receipt,
            project,
            observed: vec![manifest, source, workspace.join("src")],
        })
    }
}

fn original_receipt(
    context: &Path,
    producer: &Path,
    artifact: &Path,
    manifest: &Path,
) -> Result<receipts::Receipt, Box<dyn std::error::Error>> {
    let identity = receipts::TargetIdentity {
        manifest: manifest.to_owned(),
        name: "demo".into(),
        crate_type: "lib".into(),
        rustc_args_sha256: files::digest(b"exact fixture command"),
        cargo_name: None,
    };
    let report = zero_function_report();
    let stdout = serde_json::to_vec(&report)?;
    let selected =
        crate::adapter::attempts::fixture_history(producer, "original", &identity, &stdout, b"", 0)
            .ok_or("owned child history")?;
    let receipt = receipts::Receipt {
        version: receipts::RECEIPT_VERSION,
        partial_args: None,
        producer_archive: None,
        checksum: String::new(),
        package_id: "demo".into(),
        name: "demo".into(),
        kind: vec!["lib".into()],
        context: "context".into(),
        input_revision: "actual-file-revision".into(),
        invocation: receipts::Invocation {
            retained_from: None,
            retained_history: None,
            selected_attempt: Some(selected),
            identity,
            run_id: "original".into(),
            report: report.clone(),
            success: true,
            dependencies: BTreeMap::new(),
            dependency_build: None,
        },
        artifact_hashes: BTreeMap::from([(
            artifact.to_owned(),
            files::digest(b"owned fixture artifact"),
        )]),
        committed: producer.join("committed.json"),
        declared_environment: BTreeMap::new(),
    }
    .seal();
    files::json(
        &receipt.committed,
        &serde_json::json!({"context":"context", "run_id":"original", "receipt_checksums":[receipt.checksum]}),
    )?;
    files::json(
        &receipts::target_path(&context.join("receipts"), "demo", "demo", &["lib".into()]),
        &receipt,
    )?;
    assert!(receipt.valid("context"));
    Ok(receipt)
}

fn zero_function_report() -> sail_api::output::Report {
    let mut report = crate::report::setup_failure(&[], "SAIL3100", "native fixture");
    report.error = None;
    report.exit_code = Some(0);
    report.crates.push(sail_api::output::CrateReport {
        name: Some("demo".into()),
        functions: Vec::new(),
        verus: sail_api::output::VerusResults {
            verified: Some(0),
            errors: Some(0),
            success: Some(true),
            encountered_vir_error: Some(false),
        },
    });
    report
}

fn prepare_record(fixture: &Fixture, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Real wall-clock settling, not a fabricated old stamp or future start.
    std::thread::sleep(SETTLED + Duration::from_millis(50));
    let observation = fixture
        .observed
        .iter()
        .map(|path| Some((path.clone(), lstat(path).ok()?)))
        .collect::<Option<Vec<_>>>()
        .ok_or("actual source observations")?;
    assert_eq!(observation.len(), fixture.observed.len());
    assert_eq!(observation.len(), 3);
    let identity = vec![PathBuf::from("/bin/sh")];
    let pending = prepare(Facts {
        key,
        cache: &fixture.config.cache,
        start: SystemTime::now(),
        workspace_lock: fixture
            .config
            .cache
            .join("projects/workspace/workspace.lock"),
        family: &fixture.family,
        context: &fixture.context,
        target: &fixture.family.join("target"),
        observation,
        identity: &identity,
        siblings: Vec::new(),
        result: &fixture.project,
    })
    .ok_or("actual replay preparation")?;
    pending.commit(b"original stdout".to_vec(), Vec::new());
    Ok(())
}

#[test]
fn replay_refuses_a_receipt_whose_original_selected_history_is_unavailable()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("unavailable")?;
    let key = key(&fixture.config, false).ok_or("ordinary replay key")?;
    prepare_record(&fixture, &key)?;
    let first =
        replay(&fixture.config, &key, std::time::Instant::now()).ok_or("original intact replay")?;
    assert!(first.result.targets[0].complete);
    assert!(fixture.receipt.valid("context"));
    std::fs::remove_dir_all(&fixture.producer)?;
    assert!(!fixture.receipt.valid("context"));
    assert!(!fixture.receipt.producer_history_intact());
    // This is a proof-provenance admission control, not a mathematical proof
    // generated by /bin/sh or a complete compiler observation provider.
    assert!(
        replay(&fixture.config, &key, std::time::Instant::now()).is_none(),
        "a cached Current result cannot replace missing original selected history"
    );
    Ok(())
}

#[test]
fn replay_refuses_changed_history_or_mismatched_cached_producer_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("mismatched")?;
    let key = key(&fixture.config, false).ok_or("ordinary key")?;
    prepare_record(&fixture, &key)?;
    let path = record_path(&fixture.config.cache, &key);
    let original = std::fs::read(&path)?;
    for variant in [
        "omitted",
        "establishing",
        "report",
        "context",
        "old-version",
    ] {
        let mut record: Record = serde_json::from_slice(&original)?;
        match variant {
            "omitted" => record.origins.clear(),
            "establishing" => record.result.targets[0].establishing_run = "another-producer".into(),
            "report" => record.result.targets[0]
                .report
                .command
                .push("--no-verify".into()),
            "context" => record.result.context_sha256 = "another-context".into(),
            _ => record.version = VERSION - 1,
        }
        files::json(&path, &record)?;
        assert!(
            replay(&fixture.config, &key, std::time::Instant::now()).is_none(),
            "{variant}"
        );
        std::fs::write(&path, &original)?;
        assert!(
            replay(&fixture.config, &key, std::time::Instant::now()).is_some(),
            "positive after {variant}"
        );
    }
    let selected = fixture
        .receipt
        .invocation
        .selected_directory(&fixture.producer)
        .ok_or("selected original")?;
    let stdout = selected.join("verus.stdout");
    let bytes = std::fs::read(&stdout)?;
    std::fs::write(&stdout, b"changed original output")?;
    assert!(!fixture.receipt.producer_history_intact());
    assert!(replay(&fixture.config, &key, std::time::Instant::now()).is_none());
    std::fs::write(stdout, bytes)?;
    let restored =
        replay(&fixture.config, &key, std::time::Instant::now()).ok_or("restored original")?;
    assert_eq!(restored.stdout, b"original stdout");
    assert_eq!(restored.result.targets, fixture.project.targets);
    assert_eq!(restored.result.targets[0].establishing_run, "original");
    Ok(())
}
