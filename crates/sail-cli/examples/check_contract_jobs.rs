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
mod compound_runtime {
    include!("../../sail-verus/tests/fixtures/neutral-contracts/compound_field.rs");
}
mod guarded_compound_runtime {
    include!("../../sail-verus/tests/fixtures/neutral-contracts/compound_field_guarded.rs");
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
fn validate_assurance(
    client: &Client,
    output: &Path,
    label: &str,
    document: &sail_api::jobs::ResultDocument,
    source: &str,
    expected: ProofState,
) -> TestResult {
    let digest = sha(source.as_bytes());
    let assurance = client.requirement_assurance(&document.job.id)?;
    let fields = sail_verus::contracts::struct_field_names(source);
    if assurance != sail_api::assurance::from_contract_result(document)?
        || assurance.currentness != sail_api::project::Currentness::Unknown
        || assurance.requirements.is_empty()
        || assurance.requirements.iter().any(|requirement| {
            requirement.evidence.iter().any(|row| {
                row.source_sha256 != digest
                    || row.outcome != expected
                    || row.tier
                        != (expected == ProofState::Proved)
                            .then_some(sail_api::assurance::EvidenceTier::SmtVerified)
                    || row.mutation != sail_api::assurance::MutationStatus::NotEstablished
            })
        })
    {
        return Err("assurance changed proof, snapshot scope or missing mutation evidence".into());
    }
    let inputs = client.assurance_inputs(&document.job.id)?;
    if inputs != sail_api::assurance::decoded_inputs(document, &fields)? {
        return Err(format!("{label}: served decoded inputs differ from the recompute").into());
    }
    validate_counterexample(label, &assurance, &inputs)?;
    std::fs::write(
        output.join(format!("{label}.assurance.json")),
        serde_json::to_vec_pretty(&assurance)?,
    )?;
    Ok(())
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
        profile: 1,
        source: source.into(),
        source_sha256: digest.clone(),
        crate_name: "neutral".into(),
        modules: Vec::new(),
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
        || evidence.manifest.version != 2
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
    } else if label == "body-edit"
        && (evidence.proof.checks.unknown == 0 || evidence.proof.unknown_reasons.is_empty())
    {
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
    validate_compound_failure(label, &backend)?;
    validate_assurance(client, output, label, &document, source, expected)?;
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
/// Failed rows decode source-named model inputs; proved rows carry none.
fn validate_counterexample(
    label: &str,
    assurance: &sail_api::assurance::View,
    inputs: &sail_api::counterexample::View,
) -> TestResult {
    use sail_api::counterexample::Value;
    for row in assurance.requirements.iter().flat_map(|r| &r.evidence) {
        let decoded = inputs
            .rows
            .iter()
            .find(|r| r.obligation_id == row.obligation_id)
            .map(|r| &r.decoded);
        let names: Vec<&str> = decoded
            .iter()
            .flat_map(|d| &d.inputs)
            .map(|input| input.name.as_str())
            .collect();
        let ok = match label {
            "body-edit" => names.contains(&"input") && decoded.is_some_and(|d| d.omitted > 0),
            "compound-field" => decoded.is_some_and(|d| {
                d.inputs
                    .iter()
                    .filter(|i| i.name == "a" || i.name == "b")
                    .all(|i| {
                        matches!(&i.value, Value::Record { constructor, fields }
                        if constructor == "Row" && fields.len() == 1 && fields[0].name == "flag")
                    })
                    && names.contains(&"x")
            }),
            _ => row.outcome != ProofState::Proved || decoded.is_none(),
        };
        if !ok {
            return Err(format!("{label}: unexpected decoded counterexample {decoded:?}").into());
        }
        println!("{label}: decoded model inputs {names:?}");
    }
    Ok(())
}
fn validate_compound_failure(
    label: &str,
    backend: &sail_api::project::ProjectResult,
) -> TestResult {
    if label == "compound-field"
        && !backend
            .targets
            .iter()
            .filter(|target| target.requested)
            .flat_map(|target| &target.report.crates)
            .any(|krate| {
                krate.verus.encountered_vir_error == Some(false)
                    && krate.verus.success == Some(false)
                    && krate.verus.errors.is_some_and(|errors| errors > 0)
            })
    {
        return Err("overflow control did not reach an actual failed property check".into());
    }
    Ok(())
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
fn compound_runtime_witness(output: &Path, source: &str) -> TestResult {
    let a = compound_runtime::Row { flag: true };
    let b = compound_runtime::Row { flag: false };
    if !compound_runtime::candidate(254, a, b) {
        return Err("compound reference output".into());
    }
    let panic = std::panic::catch_unwind(|| compound_runtime::candidate(255, a, b))
        .err()
        .ok_or("compound field lost runtime overflow")?;
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .ok_or("untyped arithmetic panic")?;
    if message != "SAIL0703: specification arithmetic overflow or invalid division" {
        return Err("unexpected compound runtime failure".into());
    }
    for x in 0..255u8 {
        if !guarded_compound_runtime::candidate(
            x,
            guarded_compound_runtime::Row { flag: true },
            guarded_compound_runtime::Row { flag: false },
        ) {
            return Err("guarded compound reference output".into());
        }
    }
    let witness = serde_json::json!({"source_sha256":sha(source.as_bytes()),
        "input":{"x":255,"a":{"flag":true},"b":{"flag":false}},
        "observed":message,"preconditions_satisfied":true,
        "assurance":"one compiled arithmetic failure; separate from SMT evidence"});
    std::fs::write(
        output.join("compound-field.runtime-witness.json"),
        serde_json::to_vec_pretty(&witness)?,
    )?;
    Ok(())
}

fn validate_pruned_baseline(client: &Client, output: &Path, label: &str) -> TestResult {
    let Response::Baseline(baseline) = client.call(&Request::GetBaseline { name: label.into() })?
    else {
        return Err("pruned baseline".into());
    };
    let saved: sail_api::evidence::Baseline = serde_json::from_slice(&std::fs::read(
        output.join(format!("{label}.baseline.json")),
    )?)?;
    if *baseline != saved {
        return Err("baseline changed after job pruning".into());
    }
    let raw = output
        .join("cache/projects/jobs/baselines")
        .join(format!("{label}.contracts-stderr"));
    let retained = std::fs::read(&raw)?;
    if retained != std::fs::read(output.join(format!("{label}.worker.stderr")))? {
        return Err("baseline did not preserve exact original worker diagnostics".into());
    }
    std::fs::write(&raw, b"changed diagnostics")?;
    if client
        .call(&Request::GetBaseline { name: label.into() })
        .is_ok()
    {
        return Err("tampered retained diagnostics were accepted".into());
    }
    std::fs::write(&raw, retained)?;
    if !matches!(
        client.call(&Request::GetBaseline { name: label.into() })?,
        Response::Baseline(_)
    ) {
        return Err("restored exact baseline did not read".into());
    }
    Ok(())
}

/// A Cargo configuration in a parent of the cache places the owned shadow
/// privately elsewhere; the unchanged admission check runs there.
fn inherited_configuration(sail: &Path, output: &Path, original: &str) -> TestResult {
    std::fs::create_dir(output.join(".cargo"))?;
    std::fs::write(output.join(".cargo/config.toml"), "[build]\n")?;
    let (service, client) = start(sail, output)?;
    let narrowed = original.replace("input < 100", "input < 99");
    if narrowed == original {
        return Err("inherited-configuration fixture did not change".into());
    }
    one(
        &client,
        output,
        "inherited-config",
        &narrowed,
        ProofState::Proved,
    )?;
    drop(service);
    Ok(())
}
/// Submit one attributed job for the project in `dir` through the installed CLI
/// and wait for it to finish.
fn submit_attributed(
    sail: &Path,
    client: &Client,
    output: &Path,
    dir: &Path,
    rows: &[(&str, bool)],
) -> TestResult<String> {
    let mut command = Command::new(sail);
    command
        .current_dir(dir)
        .env("SAIL_CACHE_DIR", output.join("cache"))
        .env("SAIL_DAEMON_SOCKET", output.join("d.sock"))
        .args(["verify", "--background", "--output", "json"]);
    for (function, _) in rows {
        command.args([
            "--requirement",
            &format!("R-{function}=attributed::{function}"),
        ]);
    }
    let submitted = command.output()?;
    if !submitted.status.success() {
        return Err(format!(
            "attributed submission failed: {}",
            String::from_utf8_lossy(&submitted.stderr)
        )
        .into());
    }
    let status: sail_api::jobs::Status = serde_json::from_slice(&submitted.stdout)?;
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        let Response::Status(current) = client.call(&Request::Status {
            id: status.id.clone(),
        })?
        else {
            return Err("wrong status response".into());
        };
        if current.state.terminal() {
            return Ok(status.id);
        }
        if Instant::now() > deadline {
            return Err("attributed job deadline".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Attributed ordinary jobs through the installed CLI and service, on one Cargo
/// project so vstd builds once. A function that proves only through
/// `assume(false)` is `Proved` like an honest one, but neither earns a tier
/// without `--no-cheating`; a failed function decodes its model inputs and
/// makes every row `NotProved`.
fn attributed_jobs(sail: &Path, client: &Client, output: &Path) -> TestResult {
    let dir = output.join("attributed");
    std::fs::create_dir_all(dir.join("src"))?;
    let pins = sail_verus::toolchain::Pins::embedded().map_err(|e| e.message)?;
    let vstd = pins.verus.vstd.ok_or("no pinned vstd")?;
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"attributed\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n[package.metadata.verus]\nverify = true\n[lib]\npath = \"src/lib.rs\"\n[dependencies]\nvstd = \"={vstd}\"\n[workspace]\n"
        ),
    )?;
    let honest = "    pub fn honest(x: u8) -> (r: u8)\n        requires x < 100\n        ensures r == x + 1\n    {\n        x + 1\n    }\n";
    let cheat = "    pub fn cheat(x: u8) -> (r: u8)\n        ensures r == x + 1\n    {\n        assume(false);\n        x\n    }\n";
    let bad = "    pub fn bad(x: u8) -> (r: u8)\n        requires x < 100\n        ensures r == x + 1\n    {\n        x\n    }\n";
    let both = format!("{honest}{cheat}");
    let failing = format!("{honest}{bad}");
    let cases: [(&str, &[(&str, bool)]); 2] = [
        (&both, &[("cheat", true), ("honest", true)]),
        (&failing, &[("bad", false), ("honest", false)]),
    ];
    for (index, (body, rows)) in cases.into_iter().enumerate() {
        std::fs::write(
            dir.join("src/lib.rs"),
            format!("use vstd::prelude::*;\nverus! {{\n{body}}}\n"),
        )?;
        let id = submit_attributed(sail, client, output, &dir, rows)?;
        let view = client.attributed_assurance(&id)?;
        std::fs::write(
            output.join(format!("attributed-{index}.json")),
            serde_json::to_vec_pretty(&view)?,
        )?;
        if view.currentness != Currentness::Current || view.requirements.len() != rows.len() {
            return Err(format!("attributed job {index}: unexpected view {view:?}").into());
        }
        for (function, proved) in rows {
            let row = view
                .requirements
                .iter()
                .find(|r| r.requirement_id == format!("R-{function}"))
                .and_then(|r| r.evidence.first())
                .ok_or("attributed row")?;
            let cheating = row
                .qualifiers
                .iter()
                .any(|q| q.contains("did not pass Verus --no-cheating"));
            let decoded = row.counterexample.as_ref().is_some_and(|d| {
                d.inputs.iter().any(|i| i.name == "x") && d.inputs.iter().any(|i| i.name == "r")
            });
            if !row.function_reported
                || (row.outcome == ProofState::Proved) != *proved
                || row.tier.is_some()
                || row.establishment.is_some()
                || !cheating
                || decoded != (*function == "bad")
            {
                return Err(format!("attributed {function}: unexpected row {row:?}").into());
            }
            println!(
                "attributed {function}: {:?}, tier {:?}, decoded inputs {decoded}",
                row.outcome, row.tier
            );
        }
    }
    Ok(())
}
const CLOSURE_ROOT: &str = "pub mod limits;\n\n#[sail_contracts::requirement(\"CLOSURE-SMALL\")]\npub fn small(value: u8) -> bool {\n    value <= 200\n}\n\n#[sail_contracts::satisfies(\"CLOSURE-SMALL\")]\n#[sail_contracts::ensures(|ret| small(ret) && ret <= value)]\npub fn clamp(value: u8) -> u8 {\n    limits::cap(value)\n}\n";
const CLOSURE_LIMITS: &str = "#[sail_contracts::satisfies(\"CLOSURE-CAP\")]\n#[sail_contracts::ensures(|ret| ret <= 200 && ret <= value)]\npub fn cap(value: u8) -> u8 {\n    if value > 200 { 200 } else { value }\n}\n";

/// One explicit profile-three job over a two-file closure: the closure
/// identity is the input revision, the manifest binds the module file, and
/// the assurance view and a restart keep the proof.
fn closure_job(sail: &Path, output: &Path) -> TestResult {
    let (service, client) = start(sail, output)?;
    let module = sail_api::contracts::ModuleSource {
        module: "limits".into(),
        source: CLOSURE_LIMITS.into(),
        source_sha256: sha(CLOSURE_LIMITS.as_bytes()),
    };
    let submission = Submission {
        profile: 3,
        source: CLOSURE_ROOT.into(),
        source_sha256: sha(CLOSURE_ROOT.as_bytes()),
        crate_name: "neutral".into(),
        modules: vec![module],
    };
    let closure = submission.closure_sha256();
    std::fs::write(output.join("closure.original.rs"), CLOSURE_ROOT)?;
    std::fs::write(output.join("closure.limits.rs"), CLOSURE_LIMITS)?;
    let job = client.submit_contracts(submission)?;
    completed(&client, &job.id)?;
    let Response::Result(document) = client.call(&Request::Result { id: job.id.clone() })? else {
        return Err("missing closure result".into());
    };
    std::fs::write(
        output.join("closure.result.json"),
        serde_json::to_vec_pretty(&document)?,
    )?;
    let evidence = document
        .contracts
        .as_deref()
        .ok_or("missing closure contract evidence")?;
    let files = evidence
        .manifest
        .bindings
        .iter()
        .map(|b| (b.function.as_str(), b.module_file.as_deref()))
        .collect::<Vec<_>>();
    if evidence.proof.state != ProofState::Proved
        || evidence.manifest.version != sail_verus::contracts::PROFILE_THREE_LOWERING_VERSION
        || evidence.manifest.original_sha256 != closure
        || evidence.manifest.modules.len() != 1
        || document.job.input_revision.as_deref() != Some(closure.as_str())
        || files
            != [
                ("neutral::clamp", None),
                ("neutral::limits::cap", Some("limits")),
            ]
    {
        return Err(format!("closure identity or proof mismatch: {files:?}").into());
    }
    let assurance = client.requirement_assurance(&job.id)?;
    if assurance != sail_api::assurance::from_contract_result(&document)?
        || assurance.requirements.len() != 2
        || assurance.requirements.iter().any(|requirement| {
            requirement.evidence.iter().any(|row| {
                row.source_sha256 != closure
                    || row.outcome != ProofState::Proved
                    || row.tier != Some(sail_api::assurance::EvidenceTier::SmtVerified)
            })
        })
    {
        return Err(format!("closure assurance mismatch: {assurance:?}").into());
    }
    drop(service);
    let (service, client) = start(sail, output)?;
    if client.requirement_assurance(&job.id)? != assurance {
        return Err("restart changed closure assurance".into());
    }
    drop(service);
    println!("closure: profile three across two files, closure identity {closure}, Proved");
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
    let original =
        include_str!("../resources/crates/sail-verus/tests/fixtures/neutral-contracts/add_one.rs");
    let changed = include_str!(
        "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/body_edit.rs"
    );
    if changed != original.replace("    input + 1\n", "    input\n") {
        return Err("body edit differs beyond intended implementation".into());
    }
    runtime_witness(&output, changed)?;
    let compound = include_str!(
        "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/compound_field.rs"
    );
    let guarded = include_str!(
        "../resources/crates/sail-verus/tests/fixtures/neutral-contracts/compound_field_guarded.rs"
    );
    compound_runtime_witness(&output, compound)?;
    let (service, client) = start(&sail, &output)?;
    let positive = one(&client, &output, "positive", original, ProofState::Proved)?;
    let negative = one(
        &client,
        &output,
        "body-edit",
        changed,
        ProofState::NotProved,
    )?;
    let unsafe_field = one(
        &client,
        &output,
        "compound-field",
        compound,
        ProofState::NotProved,
    )?;
    let guarded_field = one(
        &client,
        &output,
        "compound-field-guarded",
        guarded,
        ProofState::Proved,
    )?;
    let jobs = [positive, negative, unsafe_field, guarded_field];
    attributed_jobs(&sail, &client, &output)?;
    drop(service);
    closure_job(&sail, &output)?;
    let (service, client) = start(&sail, &output)?;
    for (label, id) in [
        "positive",
        "body-edit",
        "compound-field",
        "compound-field-guarded",
    ]
    .into_iter()
    .zip(&jobs)
    {
        let Response::Result(result) = client.call(&Request::Result { id: id.clone() })? else {
            return Err("restart result".into());
        };
        if result.contracts.is_none() || result.job.currentness != Currentness::Unknown {
            return Err("restart changed neutral evidence".into());
        }
        let assurance = client.requirement_assurance(id)?;
        let saved: sail_api::assurance::View = serde_json::from_slice(&std::fs::read(
            output.join(format!("{label}.assurance.json")),
        )?)?;
        if assurance != saved {
            return Err("restart changed requirement assurance".into());
        }
    }
    drop(service);
    for id in jobs {
        std::fs::remove_dir_all(output.join("cache/projects/jobs").join(id))?;
    }
    let (service, client) = start(&sail, &output)?;
    for label in [
        "positive",
        "body-edit",
        "compound-field",
        "compound-field-guarded",
    ] {
        validate_pruned_baseline(&client, &output, label)?;
    }
    drop(service);
    println!(
        "restart and pruning: source-bound baseline evidence preserved independently of original jobs"
    );
    inherited_configuration(&sail, &output, original)
}
