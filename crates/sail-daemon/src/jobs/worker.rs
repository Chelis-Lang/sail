//! Verification workers: each runs one job at a time, without holding the
//! status lock during verification.

use std::io;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::Duration;

use sail_api::jobs::{Currentness, Lifecycle, Mode};
use sail_api::project::ProjectResult;

use super::{State, get, lock_error, store};

pub(super) struct Process(pub(super) Child);
impl std::ops::Deref for Process {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for Process {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{}", self.0.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub(super) fn run(state: &State) {
    while let Some((id, key)) = next(state) {
        let _running = Running { state, key };
        let Ok(job) = get(state, &id) else { continue };
        if !job.status.state.terminal() {
            let result = execute(state, &id);
            // A run without proof reuse (SAIL3112) binds no context or
            // input revision, so it establishes no freshness record.
            if job.contracts.is_none()
                && result
                    .as_ref()
                    .is_ok_and(|(_, document, _)| document.no_reuse.is_none())
            {
                store::establish_freshness(state, &job);
            }
            if let Ok(mut jobs) = state.jobs.lock()
                && let Some(job) = jobs.get_mut(&id)
                && !job.status.state.terminal()
            {
                job.status.finished_at_ms = Some(store::now());
                match result {
                    Ok((code, document, projection)) => {
                        job.projection = Some(projection);
                        job.status.exit_code = code;
                        job.status.state = Lifecycle::Completed;
                        job.status.input_revision = Some(
                            job.contracts
                                .as_ref()
                                .map_or(document.input_revision, |s| s.source_sha256.clone()),
                        );
                        job.status.currentness = if job.contracts.is_some() {
                            Currentness::Unknown
                        } else {
                            match document.currentness {
                                sail_api::project::Currentness::Current => Currentness::Current,
                                sail_api::project::Currentness::Historical => {
                                    Currentness::Historical
                                }
                                sail_api::project::Currentness::Unknown => Currentness::Unknown,
                            }
                        };
                    }
                    Err(error) => {
                        job.status.state = Lifecycle::InfrastructureError;
                        job.status.error = Some(format!("SAIL0808: {error}"));
                    }
                }
                if let Err(error) = store::save(&state.root, job) {
                    job.status.state = Lifecycle::InfrastructureError;
                    job.status.error = Some(format!("SAIL0808: cannot persist result: {error}"));
                }
            }
        }
        state.activity.fetch_sub(1, Ordering::SeqCst);
        store::prune(state, &job.status.project_dir);
    }
}
/// Releases a job's workspace when its worker finishes with it, and wakes
/// the workers so a job queued behind it can start.
struct Running<'a> {
    state: &'a State,
    key: std::path::PathBuf,
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.state.queue.lock() {
            queue.finish(&self.key);
        }
        self.state.ready.notify_all();
    }
}
fn next(state: &State) -> Option<(String, std::path::PathBuf)> {
    let mut queue = state.queue.lock().ok()?;
    loop {
        if state.stop.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(job) = queue.take() {
            return Some(job);
        }
        queue = state.ready.wait(queue).ok()?;
    }
}
fn worker_command(
    state: &State,
    job: &store::Stored,
    directory: &std::path::Path,
) -> io::Result<Command> {
    let stdout = store::output(&directory.join("stdout"))?;
    let stderr = store::output(&directory.join("stderr"))?;
    store::output(&directory.join("events.jsonl"))?;
    store::output(&directory.join("proof-source.jsonl"))?;
    store::output(&directory.join("requests.jsonl"))?;
    let capture = serde_json::to_string(&sail_verus::managed::RequestCaptureDestination {
        path: directory.join("requests.jsonl"),
        worker_id: job.status.id.clone(),
    })
    .map_err(io::Error::other)?;
    let mut command = Command::new(&state.executable);
    match &job.submission.mode {
        Mode::Repair(selection) => {
            command
                .arg("_repair-worker")
                .arg("--_sail-request-capture-json")
                .arg(&capture)
                .arg("--selection-json")
                .arg(serde_json::to_string(selection).map_err(io::Error::other)?);
        }
        Mode::Stock | Mode::Interactive => {
            command
                .arg("verify")
                .arg("--_sail-request-capture-json")
                .arg(&capture)
                .args(["--output", "json"]);
            if job.submission.mode == Mode::Interactive {
                command.args(["--engine", "interactive"]);
            }
        }
    }
    command
        .args(&job.submission.args)
        .current_dir(&job.status.project_dir)
        .env_clear()
        .envs(&job.submission.environment)
        .env("SAIL_JOB_EVENTS", directory.join("events.jsonl"))
        .env("SAIL_PROOF_EVENTS", directory.join("proof-source.jsonl"))
        .env("SAIL_JOB_GUARD_PARENT", "1")
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0);
    if job.submission.force {
        command.env("SAIL_MANAGED_FORCE", "1");
    }
    Ok(command)
}

fn execute(state: &State, id: &str) -> io::Result<(Option<i32>, ProjectResult, store::Projection)> {
    let job = get(state, id)?;
    if job.contracts.is_some() {
        return super::contracts::execute(state, &job);
    }
    if job.cancel.load(Ordering::SeqCst) {
        return Err(io::Error::other("cancelled"));
    }
    let directory = state.root.join(id);
    let mut command = worker_command(state, &job, &directory)?;
    let mut child = Process(command.spawn()?);
    let _parent_pipe = child.stdin.take();
    {
        let mut jobs = state.jobs.lock().map_err(lock_error)?;
        if let Some(job) = jobs.get_mut(id)
            && !job.status.state.terminal()
        {
            job.status.state = Lifecycle::Running;
            job.status.started_at_ms = Some(store::now());
            store::save(&state.root, job)?;
        }
    }
    let status = loop {
        if state.stop.load(Ordering::SeqCst) || job.cancel.load(Ordering::SeqCst) {
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--", &format!("-{}", child.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "verification cancelled",
            ));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if std::fs::metadata(directory.join("stdout")).is_ok_and(|meta| meta.len() == 0) {
        return Err(io::Error::other(no_result(&directory, status)));
    }
    let (document, projection) = read_document(&directory, &job.submission.mode)?;
    let projection = projection.with_request(captured_request(&directory, &job, &document));
    if document.exit_code != status.code() {
        return Err(io::Error::other(
            "managed report and process exit status disagree",
        ));
    }
    if document.error.is_some() {
        return Err(io::Error::other(
            "managed verifier reported an infrastructure failure",
        ));
    }
    if document.kind != "sail-project-result"
        || document.schema_version != sail_api::project::SCHEMA_VERSION
    {
        return Err(io::Error::other("unsupported or incomplete project result"));
    }
    Ok((status.code(), document, projection))
}

/// The independent journal must contain exactly one original request for this
/// output run. Missing/old/malformed evidence is an observation, never proof.
fn captured_request(
    directory: &std::path::Path,
    job: &store::Stored,
    project: &ProjectResult,
) -> Option<sail_api::project::ResolvedRequest> {
    use sail_api::project::{REQUEST_CAPTURE_VERSION, RequestCapture};
    let bytes = store::read_bytes(&directory.join("requests.jsonl")).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let invocation = job.submission.project_dir.canonicalize().ok()?;
    let invocation = invocation.to_str()?;
    let mut found = None;
    for line in text.lines() {
        let capture: RequestCapture = serde_json::from_str(line).ok()?;
        if capture.version != REQUEST_CAPTURE_VERSION || capture.worker_id != job.status.id {
            return None;
        }
        if capture.request.run_id != project.run_id {
            continue;
        }
        if found.is_some()
            || capture.request.invocation != invocation
            || !sail_api::acceptance::submitted_arguments_match(
                &job.submission.args,
                &capture.request.args,
            )
            || !store::request_matches_mode(&job.submission.mode, &capture.request)
            || !sail_api::acceptance::request_corresponds(&capture.request, project)
        {
            return None;
        }
        found = Some(capture.request);
    }
    found
}

/// Describes a worker that exited without writing a result, quoting its last
/// `error[...]` line (for example a missing engine's `SAIL3120`) so the job
/// error says what went wrong instead of reporting an unparsable result.
pub(super) fn no_result(directory: &std::path::Path, status: std::process::ExitStatus) -> String {
    let stderr = store::read_bytes(&directory.join("stderr")).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr);
    let cause = stderr
        .lines()
        .rev()
        .find(|line| line.starts_with("error["))
        .map(str::trim)
        .map(|line| line.chars().take(2048).collect::<String>());
    match cause {
        Some(line) => format!("worker exited ({status}) without a result: {line}"),
        None => format!("worker exited ({status}) without a result; see the job's retained stderr"),
    }
}

fn read_document(
    directory: &std::path::Path,
    mode: &Mode,
) -> io::Result<(ProjectResult, store::Projection)> {
    let bytes = store::read_bytes(&directory.join("stdout"))?;
    let document: ProjectResult = if matches!(mode, Mode::Repair(_)) {
        let mut repair: sail_api::repair::ResultDocument =
            serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if repair.version != sail_api::repair::VERSION {
            return Err(io::Error::other("unsupported repair result version"));
        }
        store::full_phase(&mut repair)?
    } else {
        serde_json::from_slice(&bytes).map_err(io::Error::other)?
    };
    let projection = store::Projection::validated(&bytes, &document);
    Ok((document, projection))
}

#[cfg(test)]
mod request_capture_tests {
    use super::*;
    use sail_api::project::{
        Currentness, Execution, RequestCapture, RequestTarget, ResolvedRequest, TargetResult,
    };

    fn fixture(
        root: &std::path::Path,
    ) -> Result<(store::Stored, ProjectResult, RequestCapture), Box<dyn std::error::Error>> {
        let job = store::Stored::new(sail_api::jobs::Submission {
            mode: Mode::Stock,
            project_dir: root.to_path_buf(),
            args: Vec::new(),
            force: false,
            environment: std::collections::BTreeMap::new(),
            input_revision: None,
        });
        let mut report: sail_api::output::Report = serde_json::from_str(include_str!(
            "../../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))?;
        report.exit_code = Some(0);
        report.error = None;
        for krate in &mut report.crates {
            krate
                .functions
                .retain(|f| f.verdict == sail_api::output::VerdictName::Verified);
            krate.verus.errors = Some(0);
            krate.verus.success = Some(true);
            krate.verus.encountered_vir_error = Some(false);
        }
        report.summary = sail_api::output::Summary::of(&report.crates);
        let project = ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: sail_api::project::SCHEMA_VERSION,
            run_id: "run".into(),
            workspace: "/workspace".into(),
            context_sha256: "context".into(),
            input_revision: "revision".into(),
            currentness: Currentness::Current,
            exit_code: Some(0),
            targets: vec![TargetResult {
                package_id: "package".into(),
                target: "app".into(),
                kind: vec!["lib".into()],
                establishing_run: "run".into(),
                requested: true,
                complete: true,
                reused: false,
                scope_reuse: None,
                report,
                dependency_store: None,
                dependency_build: None,
            }],
            execution: Execution::default(),
            notes: Vec::new(),
            error: None,
            engine: None,
            no_reuse: None,
        };
        let capture = RequestCapture {
            version: sail_api::project::REQUEST_CAPTURE_VERSION,
            worker_id: job.status.id.clone(),
            request: ResolvedRequest {
                run_id: "run".into(),
                invocation: root.canonicalize()?.to_string_lossy().into_owned(),
                workspace: "/workspace".into(),
                context_sha256: "context".into(),
                input_revision: "revision".into(),
                args: Vec::new(),
                engine: None,
                function: None,
                repair_package: String::new(),
                repair_target: None,
                repair_kind: String::new(),
                targets: vec![RequestTarget {
                    package_id: "package".into(),
                    target: "app".into(),
                    kind: "lib".into(),
                }],
            },
        };
        Ok((job, project, capture))
    }

    #[test]
    fn independent_request_journal_refuses_swaps_omissions_and_truncation()
    -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-request-capture-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let (job, project, capture) = fixture(&root)?;
        let file = root.join("requests.jsonl");
        assert!(captured_request(&root, &job, &project).is_none());
        std::fs::write(&file, serde_json::to_vec(&capture)?)?;
        let request = captured_request(&root, &job, &project).ok_or("valid journal refused")?;
        assert!(sail_api::acceptance::complete_request(&request, &project).is_some());
        assert_ne!(project.targets[0].report.crates[0].functions.len(), 0);
        for change in [
            "worker", "run", "context", "revision", "scope", "omission", "args",
        ] {
            let mut wrong = capture.clone();
            match change {
                "worker" => wrong.worker_id.push('x'),
                "run" => wrong.request.run_id.push('x'),
                "context" => wrong.request.context_sha256.push('x'),
                "revision" => wrong.request.input_revision.push('x'),
                "scope" => wrong.request.targets[0].target.push('x'),
                "omission" => wrong.request.targets.clear(),
                "args" => wrong.request.args = vec!["--".into(), "--no-verify".into()],
                _ => unreachable!(),
            }
            std::fs::write(&file, serde_json::to_vec(&wrong)?)?;
            assert!(
                captured_request(&root, &job, &project).is_none(),
                "{change}"
            );
        }
        let line = serde_json::to_string(&capture)?;
        for malformed in [
            format!("{line}\n{line}\n"),
            format!("{line}\n{{"),
            String::new(),
        ] {
            std::fs::write(&file, malformed)?;
            assert!(captured_request(&root, &job, &project).is_none());
        }
        let mut prior = capture.clone();
        prior.request.run_id = "earlier-failed-engine-run".into();
        std::fs::write(
            &file,
            format!("{}\n{line}\n", serde_json::to_string(&prior)?),
        )?;
        assert!(captured_request(&root, &job, &project).is_some());
        let mut refused = project.clone();
        store::withhold_unbound_authority(&mut refused, None);
        assert_eq!(refused.currentness, Currentness::Current);
        assert_eq!(refused.targets[0].report, project.targets[0].report);
        assert!(!refused.targets[0].complete);
        assert!(
            refused
                .notes
                .iter()
                .any(|note| note.starts_with("SAIL3115:"))
        );
        for flag in ["--no-verify", "--verify-function=input::Counter::get"] {
            let mut partial_request = capture.request.clone();
            partial_request.args = vec!["--".into(), flag.into()];
            let mut observation = project.clone();
            observation.targets[0].report.command = partial_request.args.clone();
            let notes = observation.notes.clone();
            store::withhold_unbound_authority(&mut observation, Some(&partial_request));
            assert!(!observation.targets[0].complete);
            assert_eq!(
                observation.notes, notes,
                "known partial/compile-only scope is not a request observation failure"
            );
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn retained_repair_mode_cannot_swap_the_captured_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-repair-request-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let (_, project, capture) = fixture(&root)?;
        let mut request = capture.request;
        request.engine = Some(sail_api::project::EngineIdentity {
            name: "interactive".into(),
            manifest_sha256: "manifest".into(),
        });
        request.function = Some("app::lemma".into());
        request.repair_package = "app@1.0.0".into();
        request.repair_target = Some("app-bin".into());
        request.repair_kind = "bin".into();
        let selection = sail_api::repair::Selection {
            package: "app@1.0.0".into(),
            target: Some("app-bin".into()),
            kind: "bin".into(),
            function: "app::lemma".into(),
        };
        // The previous projection only used this batch predicate; it cannot
        // distinguish any original repair function or target selection.
        let batch = sail_api::repair::BatchResult {
            focus: None,
            full: Some(project.clone()),
            manifest_sha256: "manifest".into(),
            compiler_invocations: 1,
            batched: false,
            selection_error: None,
            engine_fault: None,
        };
        assert!(batch.accepted_with(&project));
        assert!(store::request_matches_mode(
            &Mode::Repair(selection.clone()),
            &request
        ));
        assert!(!store::request_matches_mode(&Mode::Stock, &request));
        assert!(!store::request_matches_mode(&Mode::Interactive, &request));
        for field in ["package", "target", "kind", "function"] {
            let mut swapped = selection.clone();
            match field {
                "package" => swapped.package.push('x'),
                "target" => swapped.target = None,
                "kind" => swapped.kind = "lib".into(),
                "function" => swapped.function.push('x'),
                _ => unreachable!(),
            }
            assert!(
                !store::request_matches_mode(&Mode::Repair(swapped), &request),
                "{field}"
            );
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
