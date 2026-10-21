//! Installed service acceptance: original source goes only through the negotiated API.
use sail_api::contracts::{ProofState, Submission, WorkerResult};
use sail_api::jobs::{Client, Currentness, Lifecycle, Request, Response};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod original_runtime {
    include!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.rs");
}
mod changed_runtime {
    include!("../../sail-verus/tests/fixtures/neutral-contracts/body_edit.rs");
}
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn sha(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
}
struct Service {
    child: Child,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
    }
}
fn start(sail: &Path, output: &Path) -> TestResult<(Service, Client)> {
    let cache = output.join("cache");
    let socket = output.join("d.sock");
    let child = Command::new(sail)
        .args(["serve", "--socket"])
        .arg(&socket)
        .arg("--cache")
        .arg(&cache)
        .env("SAIL_CACHE_DIR", &cache)
        .env("SAIL_DAEMON_SOCKET", &socket)
        .stdin(Stdio::null())
        .stdout(
            std::fs::File::options()
                .append(true)
                .create(true)
                .open(output.join("daemon.stdout"))?,
        )
        .stderr(
            std::fs::File::options()
                .append(true)
                .create(true)
                .open(output.join("daemon.stderr"))?,
        )
        .spawn()?;
    let mut service = Service { child };
    let client = Client::new(sail_api::jobs::socket_path(&socket));
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            client.call(&Request::Capabilities),
            Ok(Response::Capabilities {
                neutral_contracts_version: Some(1),
                ..
            })
        ) {
            return Ok((service, client));
        }
        if service.child.try_wait()?.is_some() || Instant::now() > deadline {
            return Err(
                "installed Sail service did not advertise neutral source capability".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn completed(client: &Client, id: &str) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(sail_api::contracts::WALL_TIME_SECS + 20);
    loop {
        let Response::Status(status) = client.call(&Request::Status { id: id.into() })? else {
            return Err("wrong status response".into());
        };
        if status.state.terminal() {
            if status.state != Lifecycle::Completed {
                return Err(format!("neutral job failed: {status:?}").into());
            }
            if status.currentness != Currentness::Unknown {
                return Err("caller source falsely reported current".into());
            }
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("service acceptance deadline".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
fn one(
    client: &Client,
    output: &Path,
    label: &str,
    source: &str,
    expected: ProofState,
) -> TestResult<String> {
    let path = output.join(format!("{label}.original.rs"));
    std::fs::write(&path, source)?;
    let digest = sha(source.as_bytes());
    let job = client.submit_contracts(Submission {
        source: source.into(),
        source_sha256: digest.clone(),
        crate_name: "neutral".into(),
    })?;
    completed(client, &job.id)?;
    let Response::Result(document) = client.call(&Request::Result { id: job.id.clone() })? else {
        return Err("missing service result".into());
    };
    let evidence = document
        .contracts
        .as_deref()
        .ok_or("missing typed contract evidence")?;
    if evidence.proof.state != expected
        || evidence.manifest.original_sha256 != digest
        || document.job.currentness != Currentness::Unknown
        || document.job.input_revision.as_deref() != Some(digest.as_str())
    {
        return Err("source or conservative proof identity mismatch".into());
    }
    if expected == ProofState::Proved {
        if evidence.proof.checks.unsat == 0
            || evidence.proof.checks.unknown != 0
            || evidence.proof.proved_functions != ["neutral::candidate"]
        {
            return Err("positive has incomplete attributed proof".into());
        }
    } else if evidence.proof.checks.unknown == 0 || evidence.proof.unknown_reasons.is_empty() {
        return Err("body edit did not retain the actual underlying unknown".into());
    }
    let backend: sail_api::project::ProjectResult = serde_json::from_value(
        document
            .result
            .clone()
            .ok_or("missing original backend result")?,
    )?;
    if backend.currentness != sail_api::project::Currentness::Current
        || backend.no_reuse.is_some()
        || backend.error.is_some()
    {
        return Err("owned snapshot has no qualified backend evidence".into());
    }
    std::fs::write(
        output.join(format!("{label}.result.json")),
        serde_json::to_vec_pretty(&document)?,
    )?;
    // Observe exact worker bytes directly from our owned test service's store,
    // independently of the public typed projection. These paths are never submitted.
    let owned = output.join("cache/projects/jobs").join(&job.id);
    let stdout = std::fs::read(owned.join("stdout"))?;
    let stderr = std::fs::read(owned.join("stderr"))?;
    let worker: WorkerResult = serde_json::from_slice(&stdout)?;
    if worker.backend != backend || &worker.contracts != evidence {
        return Err("public result differs from exact original worker envelope".into());
    }
    std::fs::write(output.join(format!("{label}.worker.stdout")), &stdout)?;
    std::fs::write(output.join(format!("{label}.worker.stderr")), &stderr)?;
    let Response::Baseline(baseline) = client.call(&Request::SaveBaseline {
        id: job.id.clone(),
        name: label.into(),
    })?
    else {
        return Err("baseline save failed".into());
    };
    if baseline.contracts.as_deref() != Some(evidence)
        || baseline.result.currentness != sail_api::project::Currentness::Historical
    {
        return Err("baseline changed or upgraded contract evidence".into());
    }
    std::fs::write(
        output.join(format!("{label}.baseline.json")),
        serde_json::to_vec_pretty(&baseline)?,
    )?;
    if std::fs::read(&path)? != source.as_bytes() {
        return Err("original source changed".into());
    }
    let hashes = serde_json::json!({"source_sha256":digest,"worker_stdout_sha256":sha(&stdout),"worker_stderr_sha256":sha(&stderr),"proof_state":expected});
    std::fs::write(
        output.join(format!("{label}.hashes.json")),
        serde_json::to_vec_pretty(&hashes)?,
    )?;
    println!(
        "{label}: installed negotiated service, exact source and worker bytes, {expected:?}, live currentness unknown"
    );
    Ok(job.id)
}
fn runtime_witness(output: &Path, changed: &str) -> TestResult {
    let reference = original_runtime::candidate(1);
    if reference != 2 {
        return Err("reference output".into());
    }
    eprintln!(
        "Reexecuting the deliberate body edit: the caught ADD-ONE postcondition violation is expected."
    );
    let failure = std::panic::catch_unwind(|| changed_runtime::candidate(1))
        .err()
        .ok_or("missing runtime violation")?;
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .ok_or("untyped panic")?;
    if message != "SAIL0702: postcondition failed for requirement ADD-ONE" {
        return Err("not the expected bound postcondition violation".into());
    }
    let input = serde_json::json!({"input":1,"type":"u8"});
    let observed = serde_json::json!({"kind":"postcondition-violation", "message":message});
    let witness = serde_json::json!({"kind":"runtime-sample-violation","source_sha256":sha(changed.as_bytes()),"input":input,"input_sha256":sha(&serde_json::to_vec(&input)?),"observed":observed,"observed_sha256":sha(&serde_json::to_vec(&observed)?),"diagnostic":message,"diagnostic_sha256":sha(message.as_bytes()),"reference_output":reference,"preconditions_satisfied":true,"assurance":"one compiled runtime sample; no SMT refutation or mutation-gate verdict"});
    std::fs::write(
        output.join("body-edit.runtime-witness.json"),
        serde_json::to_vec_pretty(&witness)?,
    )?;
    Ok(())
}
fn main() -> TestResult {
    let mut args = std::env::args_os().skip(1);
    let sail = PathBuf::from(
        args.next()
            .ok_or("usage: check_contract_jobs INSTALLED_SAIL FRESH_EVIDENCE_DIR")?,
    );
    let output = PathBuf::from(args.next().ok_or("fresh evidence directory required")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    std::fs::create_dir(&output)?;
    let output = std::fs::canonicalize(output)?;
    let original = include_str!("../../sail-verus/tests/fixtures/neutral-contracts/add_one.rs");
    let changed = include_str!("../../sail-verus/tests/fixtures/neutral-contracts/body_edit.rs");
    if changed != original.replace("    input + 1\n", "    input\n") {
        return Err("body edit differs beyond intended implementation".into());
    }
    runtime_witness(&output, changed)?;
    let (service, client) = start(&sail, &output)?;
    let positive = one(&client, &output, "positive", original, ProofState::Proved)?;
    let negative = one(
        &client,
        &output,
        "body-edit",
        changed,
        ProofState::NotProved,
    )?;
    drop(service);
    let (service, client) = start(&sail, &output)?;
    for id in [&positive, &negative] {
        let Response::Result(result) = client.call(&Request::Result { id: id.clone() })? else {
            return Err("restart result".into());
        };
        if result.contracts.is_none() || result.job.currentness != Currentness::Unknown {
            return Err("restart changed neutral evidence".into());
        }
    }
    drop(service);
    for id in [positive, negative] {
        std::fs::remove_dir_all(output.join("cache/projects/jobs").join(id))?;
    }
    let (service, client) = start(&sail, &output)?;
    for label in ["positive", "body-edit"] {
        let Response::Baseline(baseline) =
            client.call(&Request::GetBaseline { name: label.into() })?
        else {
            return Err("pruned baseline".into());
        };
        let saved: sail_api::evidence::Baseline = serde_json::from_slice(&std::fs::read(
            output.join(format!("{label}.baseline.json")),
        )?)?;
        if *baseline != saved {
            return Err("baseline changed after job pruning".into());
        }
    }
    drop(service);
    println!(
        "restart and pruning: source-bound baseline evidence preserved independently of original jobs"
    );
    Ok(())
}
