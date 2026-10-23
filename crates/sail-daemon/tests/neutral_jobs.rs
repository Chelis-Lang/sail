//! Injected workers exercise durable neutral protocol and retention, not proof acceptance.
#![cfg(unix)]
use sail_api::contracts::{Evidence, Submission, TranslationTrust, WorkerResult};
use sail_api::jobs::{Client, Currentness, Lifecycle, Request, Response, Status};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
struct Fixture {
    root: PathBuf,
    worker: PathBuf,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        use std::os::unix::fs::PermissionsExt as _;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "sn-job-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let worker = root.join("worker");
        // No client-supplied path or environment: the test-owned executable selects its fixture.
        std::fs::write(
            &worker,
            format!(
                "#!/bin/sh\n[ \"$1\" = _contracts-worker ] || exit 2\ncat '{}'\n",
                root.join("fixture.json").display()
            ),
        )?;
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { root, worker })
    }
    fn server(&self) -> TestResult<sail_daemon::jobs::JobHandle> {
        Ok(sail_daemon::jobs::bind(
            &self.root.join("j.sock"),
            &self.root.join("jobs"),
            &self.worker,
        )?
        .spawn()?)
    }
    fn client(&self) -> Client {
        Client::new(self.root.join("j.sock"))
    }
    fn input(&self) -> TestResult<Submission> {
        let source =
            include_str!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.rs").to_owned();
        let input = Submission {
            source_sha256: sail_key::QueryKey(sail_key::sha256(source.as_bytes())).to_string(),
            source,
            crate_name: "neutral".into(),
        };
        let manifest = sail_verus::contracts::lower(&input.source, &input.crate_name)?.manifest;
        let backend = serde_json::from_value(
            serde_json::json!({"kind":"sail-project-result","schema_version":2,"run_id":"fixture","workspace":"fixture","context_sha256":"fixture","input_revision":"fixture","currentness":"current","exit_code":0,"targets":[],"execution":{"verifier_invocations":0,"reused_targets":0,"wall_ms":0},"notes":[],"error":null}),
        )?;
        let proof = sail_verus::contracts::observed_proof(&manifest, &backend);
        assert_eq!(proof.state, sail_api::contracts::ProofState::NotProved);
        let contracts = Evidence {
            version: 1,
            manifest,
            proof,
            translation_trust: TranslationTrust {
                source_policy_sha256: "fixture".into(),
                metadata_sha256: "fixture".into(),
                cargo_lock_sha256: "fixture".into(),
            },
        };
        std::fs::write(
            self.root.join("fixture.json"),
            serde_json::to_vec(&WorkerResult {
                version: 1,
                backend,
                contracts,
            })?,
        )?;
        Ok(input)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn finished(client: &Client, id: &str) -> TestResult<Status> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let Response::Status(status) = client.call(&Request::Status { id: id.into() })? else {
            return Err("status response".into());
        };
        if status.state.terminal() {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err("worker deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn read(client: &Client, id: &str) -> TestResult<sail_api::jobs::ResultDocument> {
    let Response::Result(result) = client.call(&Request::Result { id: id.into() })? else {
        return Err("result response".into());
    };
    Ok(result)
}
#[test]
fn exact_source_and_worker_evidence_survive_restart_and_baseline_outlives_jobs() -> TestResult {
    let fixture = Fixture::new()?;
    let input = fixture.input()?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    let job = client.submit_contracts(input.clone())?;
    let status = finished(&client, &job.id)?;
    assert_eq!(status.state, Lifecycle::Completed, "{status:?}");
    assert_eq!(status.currentness, Currentness::Unknown);
    assert_eq!(status.input_revision, Some(input.source_sha256));
    let result = read(&client, &job.id)?;
    assert_eq!(result.job.currentness, Currentness::Unknown);
    assert_eq!(
        result.contracts.as_ref().map(|c| c.proof.state),
        Some(sail_api::contracts::ProofState::NotProved)
    );
    let Response::Baseline(saved) = client.call(&Request::SaveBaseline {
        id: job.id.clone(),
        name: "snapshot".into(),
    })?
    else {
        return Err("baseline response".into());
    };
    assert_eq!(saved.contracts, result.contracts);
    assert!(
        client
            .call(&Request::Compare {
                before: sail_api::evidence::Reference::Job(job.id.clone()),
                after: job.id.clone()
            })
            .is_err()
    );
    server.shutdown();
    let mut restarted = fixture.server()?;
    let restored = read(&client, &job.id)?;
    assert_eq!(restored.result, result.result);
    assert_eq!(restored.contracts, result.contracts);
    restarted.shutdown();
    std::fs::remove_dir_all(fixture.root.join("jobs").join(&job.id))?;
    let mut after_pruning = fixture.server()?;
    let Response::Baseline(retained) = client.call(&Request::GetBaseline {
        name: "snapshot".into(),
    })?
    else {
        return Err("retained baseline response".into());
    };
    assert_eq!(retained, saved);
    for name in [
        "snapshot.contracts-stdout",
        "snapshot.contracts-stderr",
        "snapshot.contracts-seal",
    ] {
        let path = fixture.root.join("jobs/baselines").join(name);
        let original = std::fs::read(&path)?;
        std::fs::write(&path, b"changed")?;
        assert!(
            client
                .call(&Request::GetBaseline {
                    name: "snapshot".into()
                })
                .is_err(),
            "{name}"
        );
        std::fs::write(path, original)?;
    }
    let path = fixture.root.join("jobs/baselines/snapshot.json");
    let mut edited: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    edited
        .as_object_mut()
        .ok_or("baseline object")?
        .remove("contracts");
    std::fs::write(path, serde_json::to_vec(&edited)?)?;
    assert!(
        client
            .call(&Request::GetBaseline {
                name: "snapshot".into()
            })
            .is_err()
    );
    after_pruning.shutdown();
    Ok(())
}
#[test]
fn tampered_source_output_manifest_or_proof_never_returns_evidence() -> TestResult {
    let fixture = Fixture::new()?;
    let input = fixture.input()?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    let job = client.submit_contracts(input.clone())?;
    assert_eq!(finished(&client, &job.id)?.state, Lifecycle::Completed);
    let directory = fixture.root.join("jobs").join(&job.id);
    for name in ["stdout", "stderr", "contract-source.json"] {
        let path = directory.join(name);
        let original = std::fs::read(&path)?;
        std::fs::write(&path, b"changed")?;
        assert!(
            client
                .call(&Request::Result { id: job.id.clone() })
                .is_err(),
            "{name}"
        );
        std::fs::write(path, original)?;
    }
    server.shutdown();
    // A provider envelope may not assert Proved over an empty original report.
    let path = fixture.root.join("fixture.json");
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    value["contracts"]["proof"]["state"] = "proved".into();
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    let mut restarted = fixture.server()?;
    let invalid = client.submit_contracts(input.clone())?;
    assert_eq!(
        finished(&client, &invalid.id)?.state,
        Lifecycle::InfrastructureError
    );
    assert!(read(&client, &invalid.id)?.contracts.is_none());
    restarted.shutdown();
    fixture.input()?;
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    value["contracts"]["manifest"]["bindings"][0]["guarantee_sha256"] = "forged".into();
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    let mut restarted = fixture.server()?;
    let invalid = client.submit_contracts(input)?;
    assert_eq!(
        finished(&client, &invalid.id)?.state,
        Lifecycle::InfrastructureError
    );
    restarted.shutdown();
    Ok(())
}
#[test]
fn cancellation_cleans_descendant_held_pipes_without_blocking_the_endpoint() -> TestResult {
    let fixture = Fixture::new()?;
    let input = fixture.input()?;
    std::fs::write(&fixture.worker, "#!/bin/sh\nsleep 30 &\nexit 0\n")?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    let job = client.submit_contracts(input)?;
    let started = Instant::now();
    std::thread::sleep(Duration::from_millis(80));
    assert!(
        client
            .call(&Request::Cancel {
                id: job.id.clone(),
                superseded: false
            })
            .is_ok()
    );
    assert_eq!(finished(&client, &job.id)?.state, Lifecycle::Cancelled);
    server.shutdown();
    assert!(started.elapsed() < Duration::from_secs(3));
    Ok(())
}

// These are synthetic retained records, not solver evidence. Re-sealing lets the
// test distinguish a semantic migration refusal from an ordinary byte-drift check.
fn retain_version_one(fixture: &Fixture, source: &str, manifest: &str) -> TestResult<String> {
    let original = fixture.input()?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    let job = client.submit_contracts(original)?;
    assert_eq!(finished(&client, &job.id)?.state, Lifecycle::Completed);
    server.shutdown();
    let input = Submission {
        source: source.into(),
        source_sha256: sail_key::QueryKey(sail_key::sha256(source.as_bytes())).to_string(),
        crate_name: "neutral".into(),
    };
    let mut worker: WorkerResult =
        serde_json::from_slice(&std::fs::read(fixture.root.join("fixture.json"))?)?;
    worker.contracts.manifest = serde_json::from_str(manifest)?;
    worker.contracts.proof =
        sail_verus::contracts::observed_proof(&worker.contracts.manifest, &worker.backend);
    let stdout = serde_json::to_vec(&worker)?;
    let directory = fixture.root.join("jobs").join(&job.id);
    let record = directory.join("record.json");
    let mut stored: serde_json::Value = serde_json::from_slice(&std::fs::read(&record)?)?;
    stored["contracts"] = serde_json::to_value(&input)?;
    stored["status"]["input_revision"] = input.source_sha256.clone().into();
    stored["submission"]["input_revision"] = input.source_sha256.clone().into();
    stored["projection"]["stdout_sha256"] = sail_key::QueryKey(sail_key::sha256(&stdout))
        .to_string()
        .into();
    std::fs::write(record, serde_json::to_vec(&stored)?)?;
    std::fs::write(directory.join("stdout"), stdout)?;
    std::fs::write(
        directory.join("contract-source.json"),
        serde_json::to_vec(&input)?,
    )?;
    Ok(job.id)
}

fn retain_legacy_baseline(fixture: &Fixture, id: &str) -> TestResult {
    let job = fixture.root.join("jobs").join(id);
    let input: Submission =
        serde_json::from_slice(&std::fs::read(job.join("contract-source.json"))?)?;
    let stdout = std::fs::read(job.join("stdout"))?;
    let stderr = std::fs::read(job.join("stderr"))?;
    let original: WorkerResult = serde_json::from_slice(&stdout)?;
    let mut backend = original.backend;
    backend.currentness = sail_api::project::Currentness::Historical;
    backend.notes.push("Immutable baseline evidence applies only to its recorded input revision, not current workspace inputs.".into());
    let baseline = sail_api::evidence::Baseline {
        version: sail_api::evidence::VERSION,
        name: "legacy".into(),
        job_id: id.into(),
        result: backend,
        contracts: Some(Box::new(original.contracts)),
    };
    let bytes = serde_json::to_vec(&baseline)?;
    let digest = |data: &[u8]| sail_key::QueryKey(sail_key::sha256(data)).to_string();
    let seal = serde_json::json!({"source":input,"stdout_sha256":digest(&stdout),
        "stderr_sha256":digest(&stderr),"baseline_sha256":digest(&bytes)});
    let directory = fixture.root.join("jobs/baselines");
    std::fs::create_dir_all(&directory)?;
    for (name, bytes) in [
        ("legacy.json", bytes),
        ("legacy.contracts-stdout", stdout),
        ("legacy.contracts-stderr", stderr),
        ("legacy.contracts-seal", serde_json::to_vec(&seal)?),
    ] {
        std::fs::write(directory.join(name), bytes)?;
    }
    Ok(())
}

#[test]
fn safe_version_one_results_and_baselines_keep_the_original_bytes() -> TestResult {
    let fixture = Fixture::new()?;
    let job = retain_version_one(
        &fixture,
        include_str!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.rs"),
        include_str!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.v1-manifest.json"),
    )?;
    let raw = fixture.root.join("jobs").join(&job).join("stdout");
    let original = std::fs::read(&raw)?;
    retain_legacy_baseline(&fixture, &job)?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    assert!(matches!(
        client.call(&Request::GetBaseline {
            name: "legacy".into()
        })?,
        Response::Baseline(_)
    ));
    let result = read(&client, &job)?;
    assert_eq!(
        result.contracts.as_ref().map(|c| c.manifest.version),
        Some(1)
    );
    let Response::Baseline(saved) = client.call(&Request::SaveBaseline {
        id: job.clone(),
        name: "v1".into(),
    })?
    else {
        return Err("baseline response".into());
    };
    assert_eq!(std::fs::read(&raw)?, original);
    server.shutdown();
    std::fs::remove_dir_all(fixture.root.join("jobs").join(job))?;
    let mut server = fixture.server()?;
    let Response::Baseline(retained) = client.call(&Request::GetBaseline { name: "v1".into() })?
    else {
        return Err("retained response".into());
    };
    assert_eq!(saved, retained);
    assert_eq!(
        std::fs::read(fixture.root.join("jobs/baselines/v1.contracts-stdout"))?,
        original
    );
    server.shutdown();
    Ok(())
}

#[test]
fn affected_version_one_history_is_refused_even_with_intact_content_seals() -> TestResult {
    for (source, manifest) in [
        (
            include_str!("../../sail-verus/tests/fixtures/neutral-contracts/compound_field.rs"),
            include_str!(
                "../../sail-verus/tests/fixtures/neutral-contracts/compound_field.v1-manifest.json"
            ),
        ),
        (
            include_str!(
                "../../sail-verus/tests/fixtures/neutral-contracts/compound_field_guarded.rs"
            ),
            include_str!(
                "../../sail-verus/tests/fixtures/neutral-contracts/compound_field_guarded.v1-manifest.json"
            ),
        ),
    ] {
        let fixture = Fixture::new()?;
        let job = retain_version_one(&fixture, source, manifest)?;
        let raw = fixture.root.join("jobs").join(&job).join("stdout");
        let original = std::fs::read(&raw)?;
        retain_legacy_baseline(&fixture, &job)?;
        let mut server = fixture.server()?;
        let client = fixture.client();
        assert!(read(&client, &job).is_err());
        assert!(
            client
                .call(&Request::GetBaseline {
                    name: "legacy".into()
                })
                .is_err()
        );
        assert!(
            client
                .call(&Request::SaveBaseline {
                    id: job,
                    name: "affected".into()
                })
                .is_err()
        );
        assert_eq!(std::fs::read(raw)?, original);
        server.shutdown();
    }
    Ok(())
}

#[test]
fn a_new_worker_cannot_supply_even_an_unaffected_version_one_projection() -> TestResult {
    let fixture = Fixture::new()?;
    let input = fixture.input()?;
    let path = fixture.root.join("fixture.json");
    let mut worker: WorkerResult = serde_json::from_slice(&std::fs::read(&path)?)?;
    worker.contracts.manifest = serde_json::from_str(include_str!(
        "../../sail-verus/tests/fixtures/neutral-contracts/add_one.v1-manifest.json"
    ))?;
    std::fs::write(path, serde_json::to_vec(&worker)?)?;
    let mut server = fixture.server()?;
    let client = fixture.client();
    let job = client.submit_contracts(input)?;
    assert_eq!(
        finished(&client, &job.id)?.state,
        Lifecycle::InfrastructureError
    );
    assert!(read(&client, &job.id)?.contracts.is_none());
    server.shutdown();
    Ok(())
}
