//! Actual installed-source mutation acceptance; no injected verifier results.
use sail_api::{
    contracts::{ProofState, Submission},
    jobs::{Client, Lifecycle, Request, Response},
    mutation::{self, Outcome, State, View},
};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let _ = self.0.wait();
    }
}
fn start(sail: &Path, output: &Path) -> TestResult<(Service, Client)> {
    let socket = output.join("d.sock");
    let cache = output.join("cache");
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
                .create(true)
                .append(true)
                .open(output.join("daemon.stdout"))?,
        )
        .stderr(
            std::fs::File::options()
                .create(true)
                .append(true)
                .open(output.join("daemon.stderr"))?,
        )
        .spawn()?;
    let mut service = Service(child);
    let client = Client::new(sail_api::jobs::socket_path(&socket));
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            client.call(&Request::Capabilities),
            Ok(Response::Capabilities {
                mutation_version: Some(mutation::VERSION),
                ..
            })
        ) {
            return Ok((service, client));
        }
        if service.0.try_wait()?.is_some() || Instant::now() > deadline {
            return Err("installed mutation endpoint unavailable".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn baseline(client: &Client, source: &str) -> TestResult<String> {
    let job = client.submit_contracts(Submission {
        profile: 1,
        source: source.into(),
        source_sha256: mutation::digest(source.as_bytes()),
        crate_name: "neutral".into(),
    })?;
    let deadline = Instant::now() + Duration::from_secs(320);
    loop {
        let Response::Result(result) = client.call(&Request::Result { id: job.id.clone() })? else {
            return Err("missing actual source result".into());
        };
        if result.job.state.terminal() {
            if result.job.state != Lifecycle::Completed
                || result
                    .contracts
                    .as_ref()
                    .is_none_or(|c| c.proof.state != ProofState::Proved)
            {
                return Err(format!("actual baseline not proved: {result:?}").into());
            }
            return Ok(job.id);
        }
        if Instant::now() > deadline {
            return Err("baseline acceptance deadline".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn campaign(client: &Client, baseline: &str, wall_time_ms: u64) -> TestResult<View> {
    let submitted = client.submit_mutation(mutation::Submission {
        baseline_job_id: baseline.into(),
        max_mutants: 8,
        wall_time_ms,
    })?;
    let deadline = Instant::now() + Duration::from_millis(wall_time_ms + 20_000);
    loop {
        let view = client.mutation(&submitted.id)?;
        if view.state != State::Running {
            return Ok(view);
        }
        if Instant::now() > deadline {
            return Err("campaign acceptance deadline".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn save(output: &Path, name: &str, view: &View) -> TestResult {
    std::fs::write(output.join(name), serde_json::to_vec_pretty(view)?)?;
    Ok(())
}
fn observe_unknown(view: &View) -> TestResult {
    if view.state != State::Completed || view.passed || view.observations.is_empty() {
        return Err("observation mode requires a completed nonpassing strong campaign".into());
    }
    for observation in &view.observations {
        let original = observation
            .original
            .as_ref()
            .ok_or("original inconclusive worker evidence")?;
        let result = mutation::decode_original(original)?;
        if observation.outcome != Outcome::Inconclusive
            || result.contracts.proof.state != ProofState::NotProved
            || result.contracts.proof.checks.unknown == 0
            || result.contracts.proof.checks.sat != 0
            || !result
                .contracts
                .proof
                .unknown_reasons
                .iter()
                .any(|r| r.contains("incomplete quantifiers"))
        {
            return Err("observation mode did not retain the actual expected solver unknown without a false kill".into());
        }
    }
    Ok(())
}
fn qualification(
    output: &Path,
    observe: bool,
    native_kill_qualified: bool,
    controls_completed: bool,
) -> TestResult {
    std::fs::write(
        output.join("qualification.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version":1,
            "mode":if observe {"observe-inconclusive"} else {"strict"},
            "native_kill_qualified":native_kill_qualified,
            "lifecycle_and_evidence_controls_completed":controls_completed,
            "scope":"bounded source API, survivor, deadline, cancellation, restart, pruning and tamper controls",
            "qualifiers":["unknown remains inconclusive and never establishes a passing campaign","negative-path observation does not establish native kills or full mutation milestone completion"]
        }))?,
    )?;
    Ok(())
}
fn walk(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
#[allow(
    clippy::too_many_lines,
    reason = "One actual acceptance sequence preserves original service lifetime and retained artifacts across restart and tamper controls."
)]
fn main() -> TestResult {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let observe = args.len() == 3 && args[2] == "--observe-inconclusive";
    if args.len() != 2 && !observe {
        return Err(
            "usage: check_mutation_jobs SAIL FRESH_EVIDENCE_DIRECTORY [--observe-inconclusive]"
                .into(),
        );
    }
    let sail = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    std::fs::create_dir(&output)?;
    let (service, client) = start(&sail, &output)?;
    let strong_source = "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|ret| ret == value)] pub fn candidate(value:bool)->bool { if value {true} else {false} }";
    let strong_id = baseline(&client, strong_source)?;
    let strong = campaign(&client, &strong_id, 900_000)?;
    save(&output, "strong.json", &strong)?;
    let native_kill_qualified = strong.passed
        && !strong.observations.is_empty()
        && strong
            .observations
            .iter()
            .all(|o| o.outcome == Outcome::Killed);
    qualification(&output, observe, native_kill_qualified, false)?;
    if observe {
        observe_unknown(&strong)?;
    } else if !native_kill_qualified {
        return Err(
            "actual strong campaign did not establish conclusive kills; original evidence retained"
                .into(),
        );
    }
    let assurance = client.requirement_assurance(&strong_id)?;
    if assurance.requirements.iter().flat_map(|r| &r.evidence).any(|e| !matches!(&e.mutation,sail_api::assurance::MutationStatus::Bounded {campaign_id,passed,..} if campaign_id==&strong.id && *passed==strong.passed)) { return Err("authenticated mutation was not attached to matching assurance".into()); }
    let weak_source = "#[sail_contracts::satisfies(\"WEAK\")] #[sail_contracts::requires(value < 100)] #[sail_contracts::ensures(|ret| ret <= value + 2)] pub fn candidate(value:u8)->u8 {value}";
    let weak_id = baseline(&client, weak_source)?;
    let weak = campaign(&client, &weak_id, 900_000)?;
    save(&output, "weak.json", &weak)?;
    if weak.passed
        || !weak
            .observations
            .iter()
            .any(|o| o.outcome == Outcome::Survived)
    {
        return Err("actual weak campaign did not retain survived evidence".into());
    }
    let timeout = campaign(&client, &strong_id, 1)?;
    save(&output, "timeout.json", &timeout)?;
    if timeout.passed || timeout.state != State::TimedOut {
        return Err("aggregate deadline was not explicit".into());
    }
    let cancelled = client.submit_mutation(mutation::Submission {
        baseline_job_id: strong_id.clone(),
        max_mutants: 8,
        wall_time_ms: 900_000,
    })?;
    client.cancel_mutation(&cancelled.id)?;
    let cancel_deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let view = client.mutation(&cancelled.id)?;
        if view.state != State::Running {
            save(&output, "cancelled.json", &view)?;
            if view.state != State::Cancelled || view.passed {
                return Err("explicit cancellation did not remain separate from proof".into());
            }
            break;
        }
        if Instant::now() > cancel_deadline {
            return Err("cancellation acknowledgement deadline".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(service);
    for id in std::iter::once(&strong_id).chain(strong.observations.iter().map(|o| &o.job_id)) {
        std::fs::remove_dir_all(output.join("cache/projects/jobs").join(id))?;
    }
    let (service, client) = start(&sail, &output)?;
    if client.mutation(&strong.id)? != strong {
        return Err("restarted original campaign changed".into());
    }
    // Campaign baselines retain every original source/stdout/stderr independently.
    let mut artifacts = Vec::new();
    walk(&output.join("cache"), &mut artifacts)?;
    let expected = format!(
        "{}.contracts-stderr",
        strong.observations[0]
            .evidence_name
            .as_ref()
            .ok_or("retained mutant")?
    );
    let artifact = artifacts
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(expected.as_str()))
        .ok_or("exact original stderr artifact")?;
    let bytes = std::fs::read(artifact)?;
    std::fs::write(artifact, b"tampered")?;
    if client.mutation(&strong.id).is_ok() {
        return Err("changed original stderr was accepted".into());
    }
    std::fs::write(artifact, bytes)?;
    if client.mutation(&strong.id)? != strong {
        return Err("restored original evidence changed".into());
    }
    drop(service);
    qualification(&output, observe, native_kill_qualified, true)?;
    println!(
        "native_kill_qualified={native_kill_qualified}; actual bounded source API, survivor, deadline, cancellation, restart, pruning and tamper controls passed"
    );
    Ok(())
}
