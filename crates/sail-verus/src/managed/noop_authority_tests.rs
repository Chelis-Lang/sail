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
        Self::with_scope(name, None)
    }

    fn partial(name: &str, args: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_scope(name, Some(args))
    }

    fn with_scope(
        name: &str,
        partial_args: Option<&[String]>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
        let project_root = cache
            .join("projects")
            .join(files::digest(workspace.as_os_str().as_encoded_bytes()));
        let context = project_root.join("context");
        let family = project_root.join("compiler-artifacts/family");
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
        std::fs::write(project_root.join("workspace.lock"), [])?;
        record_readback(&cache, &context, &manifest, &source)?;
        let artifact = family.join("target/libdemo.rmeta");
        std::fs::write(&artifact, b"owned fixture artifact")?;
        let receipt = original_receipt(&context, &producer, &artifact, &manifest, partial_args)?;
        let target = retained_target(&receipt, partial_args);
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
                reused_targets: u64::from(partial_args.is_none()),
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
            args: partial_args.unwrap_or_default().to_vec(),
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

    // The fixture's independently declared scope is deliberately a single
    // ordinary library. It is not inferred from the saved project and does
    // not qualify Cargo metadata or the complete compiler-input domain.
    fn request(&self, run_id: &str) -> ResolvedRequest {
        let targets = [("demo".into(), "demo".into(), "lib".into())]
            .into_iter()
            .collect();
        super::super::request::Resolution {
            run_id,
            invocation: &self.config.workspace,
            workspace: &self.config.workspace,
            context: "context",
            revision: "actual-file-revision",
            args: &self.config.args,
            engine: None,
            targets: &targets,
        }
        .resolve()
    }

    fn replay(&self, key: &str) -> Option<Replay> {
        let workspace_lock = self.context.parent()?.join("workspace.lock");
        let location = ReplayLocation {
            cache: &self.config.cache,
            context: &self.context,
            workspace_lock: &workspace_lock,
            family: &self.family,
        };
        let _workspace = lock(location.workspace_lock, true)?;
        let _family = lock(&self.family.join("ownership.lock"), false)?;
        let current = self.request("independent-current-view");
        // The fixture supplies its actual native workspace bytes separately;
        // it does not derive these observations from the saved result/record.
        let paths = sail_api::acceptance::bind_request_paths(
            &current,
            self.config.workspace.as_os_str().as_encoded_bytes(),
            self.config.workspace.as_os_str().as_encoded_bytes(),
        )?;
        super::super::read_check::with_guarded_readback(
            &self.config.cache,
            &self.config.workspace,
            "context",
            |snapshot, readback| {
                if !snapshot.complete {
                    return None;
                }
                replay_bound(
                    &location,
                    key,
                    std::time::Instant::now(),
                    &current,
                    &paths,
                    readback,
                )
            },
        )
        .ok()?
    }
}

fn record_readback(
    cache: &Path,
    context: &Path,
    manifest: &Path,
    source: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    super::super::read_check::record(
        context,
        "actual-file-revision",
        vec![
            (manifest.to_owned(), Some(files::file_digest(manifest)?)),
            (source.to_owned(), Some(files::file_digest(source)?)),
        ],
        BTreeMap::new(),
    )?;
    let rows = vec![(
        "/bin/sh".to_owned(),
        files::stat_identity(Path::new("/bin/sh"))?,
    )];
    let sha = files::digest(&serde_json::to_vec(&rows)?);
    files::json(
        &cache.join("runtime-identities").join(format!("{sha}.json")),
        &rows,
    )?;
    files::json(&cache.join("runtime-identities/context.link"), &sha)?;
    Ok(())
}

fn retained_target(receipt: &receipts::Receipt, partial_args: Option<&[String]>) -> TargetResult {
    let target = TargetResult {
        package_id: "demo".into(),
        target: "demo".into(),
        kind: vec!["lib".into()],
        establishing_run: "original".into(),
        reused: partial_args.is_none(),
        scope_reuse: partial_args.map(<[String]>::to_vec),
        requested: true,
        complete: partial_args.is_none(),
        report: receipt.invocation.report.clone(),
        dependency_store: None,
        dependency_build: None,
    };
    if let Some(args) = partial_args {
        assert!(sail_api::acceptance::complete_target(&target).is_none());
        assert!(sail_api::acceptance::complete_exact_scope_report(&target.report, args).is_some());
    } else {
        assert!(sail_api::acceptance::complete_target(&target).is_some());
    }
    target
}

fn original_receipt(
    context: &Path,
    producer: &Path,
    artifact: &Path,
    manifest: &Path,
    partial_args: Option<&[String]>,
) -> Result<receipts::Receipt, Box<dyn std::error::Error>> {
    let identity = receipts::TargetIdentity {
        manifest: manifest.to_owned(),
        name: "demo".into(),
        crate_type: "lib".into(),
        rustc_args_sha256: files::digest(b"exact fixture command"),
        cargo_name: None,
    };
    let mut report = zero_function_report();
    if let Some(args) = partial_args {
        report.command = args.to_vec();
        for krate in &mut report.crates {
            krate.verus.success = None;
        }
    }
    let stdout = serde_json::to_vec(&report)?;
    let selected =
        crate::adapter::attempts::fixture_history(producer, "original", &identity, &stdout, b"", 0)
            .ok_or("owned child history")?;
    let receipt = receipts::Receipt {
        version: receipts::RECEIPT_VERSION,
        partial_args: partial_args.map(<[String]>::to_vec),
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
            success: partial_args.is_none(),
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
    if let Some(args) = partial_args {
        assert!(receipt.valid_scoped_in("context", &receipts::PhaseDigests::default(), Some(args)));
        assert!(!receipt.valid("context"));
    } else {
        assert!(receipt.valid("context"));
    }
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

fn prepare_pending(
    fixture: &Fixture,
    key: &str,
) -> Result<Option<Pending>, Box<dyn std::error::Error>> {
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
    let original_request = fixture.request("warm-noop-record");
    Ok(prepare(Facts {
        key,
        cache: &fixture.config.cache,
        start: SystemTime::now(),
        workspace_lock: fixture
            .context
            .parent()
            .ok_or("project root")?
            .join("workspace.lock"),
        family: &fixture.family,
        context: &fixture.context,
        target: &fixture.family.join("target"),
        observation,
        identity: &identity,
        siblings: Vec::new(),
        result: &fixture.project,
        original_request: &original_request,
    }))
}

fn prepare_record(fixture: &Fixture, key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pending = prepare_pending(fixture, key)?.ok_or("actual replay preparation")?;
    pending.commit(b"original stdout".to_vec(), Vec::new());
    Ok(())
}

#[test]
fn exact_partial_replay_preserves_scope_and_never_becomes_a_whole_proof()
-> Result<(), Box<dyn std::error::Error>> {
    let args = vec!["--".into(), "--verify-module".into(), "m".into()];
    let mut fixture = Fixture::partial("exact-partial", &args)?;
    let key = key(&fixture.config, true).ok_or("partial replay key")?;
    prepare_record(&fixture, &key)?;
    let replayed = fixture.replay(&key).ok_or("exact partial replay")?;
    assert_eq!(replayed.result.targets, fixture.project.targets);
    assert_eq!(replayed.result.execution.reused_targets, 0);
    assert!(!replayed.result.targets[0].complete);
    assert!(!replayed.result.targets[0].reused);
    assert_eq!(replayed.result.targets[0].scope_reuse.as_ref(), Some(&args));
    assert_eq!(replayed.result.targets[0].establishing_run, "original");
    assert!(sail_api::acceptance::complete_target(&replayed.result.targets[0]).is_none());
    assert!(sail_api::acceptance::complete_selection(&replayed.result).is_none());

    // A caller with a different independent scope cannot transfer this record,
    // even if it mistakenly supplies the old key. Proper keys differ as well.
    fixture.config.args[2] = "another-module".into();
    assert_ne!(super::key(&fixture.config, true).as_ref(), Some(&key));
    assert!(fixture.replay(&key).is_none());
    fixture.config.args.clear();
    assert_ne!(super::key(&fixture.config, true).as_ref(), Some(&key));
    assert!(fixture.replay(&key).is_none());
    fixture.config.args.clone_from(&args);
    assert!(fixture.replay(&key).is_some());

    // An overall Cargo failure is not replayable even when its individual
    // retained partial receipt is otherwise valid. Actual observations settle
    // as in the positive preparation, so recent stamps cannot mask this guard.
    fixture.project.exit_code = Some(1);
    assert!(prepare_pending(&fixture, &key)?.is_none());
    fixture.project.exit_code = Some(0);
    assert!(prepare_pending(&fixture, &key)?.is_some());
    Ok(())
}

#[test]
fn exact_partial_replay_refuses_scope_flags_and_missing_original_history()
-> Result<(), Box<dyn std::error::Error>> {
    let args = vec!["--".into(), "--verify-module".into(), "m".into()];
    let fixture = Fixture::partial("partial-refusals", &args)?;
    let key = key(&fixture.config, true).ok_or("partial replay key")?;
    prepare_record(&fixture, &key)?;
    let path = record_path(&fixture.config.cache, &key);
    let original = std::fs::read(&path)?;
    for variant in [
        "original-scope",
        "retained-scope",
        "whole-complete",
        "whole-reuse",
        "failed",
        "old-version",
    ] {
        let mut record: Record = serde_json::from_slice(&original)?;
        match variant {
            "original-scope" => record.original_request.args[2] = "another-module".into(),
            "retained-scope" => {
                record.result.targets[0].scope_reuse =
                    Some(vec!["--".into(), "--verify-root".into()]);
            }
            "whole-complete" => record.result.targets[0].complete = true,
            "whole-reuse" => record.result.targets[0].reused = true,
            "failed" => record.result.exit_code = Some(1),
            _ => record.version = VERSION - 1,
        }
        files::json(&path, &record)?;
        assert!(fixture.replay(&key).is_none(), "{variant}");
        std::fs::write(&path, &original)?;
        assert!(fixture.replay(&key).is_some(), "restored after {variant}");
    }
    std::fs::remove_dir_all(&fixture.producer)?;
    assert!(!fixture.receipt.producer_history_intact());
    assert!(fixture.replay(&key).is_none());
    Ok(())
}

#[test]
fn replay_refuses_a_receipt_whose_original_selected_history_is_unavailable()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("unavailable")?;
    let key = key(&fixture.config, false).ok_or("ordinary replay key")?;
    prepare_record(&fixture, &key)?;
    let first = fixture.replay(&key).ok_or("original intact replay")?;
    assert!(first.result.targets[0].complete);
    assert!(fixture.receipt.valid("context"));
    std::fs::remove_dir_all(&fixture.producer)?;
    assert!(!fixture.receipt.valid("context"));
    assert!(!fixture.receipt.producer_history_intact());
    // This is a proof-provenance admission control, not a mathematical proof
    // generated by /bin/sh or a complete compiler observation provider.
    assert!(
        fixture.replay(&key).is_none(),
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
        assert!(fixture.replay(&key).is_none(), "{variant}");
        std::fs::write(&path, &original)?;
        assert!(fixture.replay(&key).is_some(), "positive after {variant}");
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
    assert!(fixture.replay(&key).is_none());
    std::fs::write(stdout, bytes)?;
    let restored = fixture.replay(&key).ok_or("restored original")?;
    assert_eq!(restored.stdout, b"original stdout");
    assert_eq!(restored.result.targets, fixture.project.targets);
    assert_eq!(restored.result.targets[0].establishing_run, "original");
    Ok(())
}
