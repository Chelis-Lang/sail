//! Proof explanations and immutable baselines, independent of job pruning.

use std::io::{self, Read as _, Write};
use std::os::unix::fs::OpenOptionsExt as _;

use sail_api::evidence::{Baseline, Comparison, Explanation, Reference, VERSION};
use sail_api::project::ProjectResult;
use sail_api::proof::{Cursor, Observation, Target};

use super::{State, get, store};

/// Private on-disk provenance; the public baseline contract remains unchanged.
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredBaseline<T> {
    #[serde(flatten)]
    baseline: T,
    #[serde(default)]
    proof_authority_revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request: Option<sail_api::project::ResolvedRequest>,
}

// One native retrieval observation, not a deserializable process/proof token.
// Keep the request paired with the exact project whose flags it admitted.
struct CompletedObservation {
    result: ProjectResult,
    request: Option<sail_api::project::ResolvedRequest>,
}

fn completed(state: &State, id: &str) -> io::Result<ProjectResult> {
    completed_with_request(state, id).map(|observation| observation.result)
}

fn completed_with_request(state: &State, id: &str) -> io::Result<CompletedObservation> {
    let job = get(state, id)?;
    if job.contracts.is_some() {
        return Err(io::Error::other(
            "SAIL3142: use typed neutral contract result; generic comparison/explanation is unsupported",
        ));
    }
    if job.status.state != sail_api::jobs::Lifecycle::Completed {
        return Err(io::Error::other(
            "SAIL0904: complete job evidence is not yet available",
        ));
    }
    let (document, request) = store::result_with_request(state, &job)?;
    let project = serde_json::from_value(
        document
            .result
            .ok_or_else(|| io::Error::other("SAIL0904: job has no project evidence"))?,
    )
    .map_err(io::Error::other)?;
    Ok(CompletedObservation {
        result: project,
        request: request.cloned(),
    })
}

pub(super) fn explain(
    state: &State,
    id: &str,
    target: &Target,
    function: &str,
    obligation: Option<&str>,
) -> io::Result<Explanation> {
    let result = completed(state, id)?;
    let mut explanation = sail_api::evidence::explain(&result, target, function, obligation)
        .map_err(io::Error::other)?;
    let job = get(state, id)?;
    let mut cursor = Cursor::default();
    loop {
        let page = state.proofs.page(
            &state.root.join(id),
            id,
            &cursor,
            64,
            job.status.state.terminal(),
        )?;
        for event in &page.events {
            if event.input.phase == sail_api::proof::Phase::Full
                && event.input.target == *target
                && event.input.function.as_deref() == Some(function)
                && event.input.run_id == explanation.establishing_run
                && event.input.input_revision == result.input_revision
                && let Observation::Diagnostic(diagnostic) = &event.input.observation
            {
                explanation.attributed_diagnostics.push(diagnostic.clone());
            }
        }
        if let Some(gap) = page.gap {
            explanation.notes.push(gap);
        }
        if page.next.after == cursor.after {
            break;
        }
        cursor = page.next;
    }
    // Explicitly linked diagnostics are useful, but absent links do not prove
    // that every diagnostic was attributed. Keep remaining raw run diagnostics.
    Ok(explanation)
}

fn valid_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 80
        || name == "."
        || name == ".."
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(io::Error::other(
            "SAIL0905: baseline name must use 1-80 ASCII letters, digits, dots, underscores or hyphens",
        ));
    }
    Ok(())
}

pub(super) fn save_baseline(state: &State, id: &str, name: &str) -> io::Result<Baseline> {
    valid_name(name)?;
    if get(state, id)?.contracts.is_some() {
        return super::contracts::save_baseline(state, id, name);
    }
    let observation = completed_with_request(state, id)?;
    publish_baseline(state, id, name, observation)
}

// Both fields come from one completed_with_request observation. A later path
// change does not replace that historical original request during publication.
fn publish_baseline(
    state: &State,
    id: &str,
    name: &str,
    observation: CompletedObservation,
) -> io::Result<Baseline> {
    valid_name(name)?;
    let CompletedObservation {
        mut result,
        request,
    } = observation;
    result.currentness = sail_api::project::Currentness::Historical;
    result.notes.push("Immutable baseline evidence applies only to its recorded input revision, not current workspace inputs.".into());
    let baseline = Baseline {
        contracts: None,
        version: VERSION,
        name: name.into(),
        job_id: id.into(),
        result,
    };
    let directory = state.root.join("baselines");
    store::private_dir(&directory)?;
    let target = directory.join(format!("{name}.json"));
    let temporary = directory.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        store::now()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        let stored = StoredBaseline {
            baseline: &baseline,
            proof_authority_revision: store::PROOF_AUTHORITY_REVISION,
            request,
        };
        file.write_all(&serde_json::to_vec(&stored).map_err(io::Error::other)?)?;
        file.sync_all()?;
        // Atomic create without overwriting an existing immutable named baseline.
        std::fs::hard_link(&temporary, &target)
    })();
    let _ = std::fs::remove_file(temporary);
    result.map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::other("SAIL0905: baseline already exists and is immutable")
        } else {
            error
        }
    })?;
    Ok(baseline)
}

pub(super) fn baselines(state: &State) -> io::Result<Vec<String>> {
    let directory = state.root.join("baselines");
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut names = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Some(name) = path.file_stem().and_then(|name| name.to_str())
            && valid_name(name).is_ok()
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

pub(super) fn baseline(state: &State, name: &str) -> io::Result<Baseline> {
    valid_name(name)?;
    let file = std::fs::File::open(state.root.join("baselines").join(format!("{name}.json")))
        .map_err(|error| io::Error::other(format!("SAIL0905: cannot read baseline: {error}")))?;
    let mut bytes = Vec::new();
    file.take((sail_api::jobs::MAX_FRAME + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io::Error::other(format!("SAIL0905: cannot read baseline: {error}")))?;
    if bytes.len() > sail_api::jobs::MAX_FRAME {
        return Err(io::Error::other(
            "SAIL0814: baseline exceeds RPC limit; retained file remains available",
        ));
    }
    let stored: StoredBaseline<Baseline> = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::other(format!("SAIL0905: invalid baseline: {error}")))?;
    let mut baseline = stored.baseline;
    if baseline.version != VERSION
        || baseline.name != name
        || baseline.job_id.is_empty()
        || baseline.result.kind != "sail-project-result"
        || baseline.result.schema_version != sail_api::project::SCHEMA_VERSION
    {
        return Err(io::Error::other("SAIL0905: baseline identity is corrupt"));
    }
    if baseline.result.currentness != sail_api::project::Currentness::Historical {
        return Err(io::Error::other(
            "SAIL0905: baseline must retain historical evidence",
        ));
    }
    if baseline.contracts.is_some()
        || state
            .root
            .join("baselines")
            .join(format!("{name}.contracts-stderr"))
            .exists()
        || state
            .root
            .join("baselines")
            .join(format!("{name}.contracts-seal"))
            .exists()
        || state
            .root
            .join("baselines")
            .join(format!("{name}.contracts-stdout"))
            .exists()
    {
        super::contracts::validate_baseline(state, &baseline, &bytes)?;
    } else {
        store::withhold_legacy_authority(&mut baseline.result, stored.proof_authority_revision);
        store::withhold_unbound_authority(&mut baseline.result, stored.request.as_ref());
    }
    Ok(baseline)
}

pub(super) fn compare(state: &State, before: &Reference, after: &str) -> io::Result<Comparison> {
    let before = match before {
        Reference::Job(id) => completed(state, id)?,
        Reference::Baseline(name) => {
            let saved = baseline(state, name)?;
            if saved.contracts.is_some() {
                return Err(io::Error::other(
                    "SAIL3142: generic comparison is unsupported for neutral evidence",
                ));
            }
            saved.result
        }
    };
    let after = completed(state, after)?;
    Ok(sail_api::evidence::compare(&before, &after))
}

#[cfg(test)]
mod baseline_request_tests {
    use super::*;
    use std::collections::BTreeMap;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn zero_function_report() -> TestResult<sail_api::output::Report> {
        let mut report: sail_api::output::Report = serde_json::from_str(include_str!(
            "../../../sail-cli/tests/fixtures/attr/report.golden.json"
        ))?;
        report.exit_code = Some(0);
        for krate in &mut report.crates {
            krate.functions.clear();
            krate.verus.verified = Some(0);
            krate.verus.errors = Some(0);
            krate.verus.success = Some(true);
            krate.verus.encountered_vir_error = Some(false);
        }
        report.summary = sail_api::output::Summary::of(&report.crates);
        Ok(report)
    }

    fn project(invocation: &std::path::Path) -> TestResult<ProjectResult> {
        Ok(ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: sail_api::project::SCHEMA_VERSION,
            run_id: "recorded-run".into(),
            workspace: invocation.to_string_lossy().into_owned(),
            context_sha256: "fixture-context".into(),
            input_revision: "recorded-revision".into(),
            currentness: sail_api::project::Currentness::Current,
            exit_code: Some(0),
            targets: vec![sail_api::project::TargetResult {
                package_id: "package".into(),
                target: "target".into(),
                kind: vec!["lib".into()],
                establishing_run: "original-proof-run".into(),
                reused: true,
                scope_reuse: None,
                requested: true,
                complete: true,
                report: zero_function_report()?,
                dependency_store: None,
                dependency_build: None,
            }],
            execution: sail_api::project::Execution {
                verifier_invocations: 0,
                reused_targets: 1,
                wall_ms: 0.0,
            },
            notes: Vec::new(),
            error: None,
            engine: None,
            no_reuse: None,
        })
    }

    fn completed_job(
        state: &State,
        invocation: &std::path::Path,
    ) -> TestResult<(String, ProjectResult)> {
        let project = project(invocation)?;
        let request = sail_api::project::ResolvedRequest {
            run_id: project.run_id.clone(),
            invocation: invocation.canonicalize()?.to_string_lossy().into_owned(),
            workspace: project.workspace.clone(),
            context_sha256: project.context_sha256.clone(),
            input_revision: project.input_revision.clone(),
            args: Vec::new(),
            engine: None,
            function: None,
            repair_package: String::new(),
            repair_target: None,
            repair_kind: String::new(),
            targets: vec![sail_api::project::RequestTarget {
                package_id: "package".into(),
                target: "target".into(),
                kind: "lib".into(),
            }],
        };
        assert!(sail_api::acceptance::complete_request(&request, &project).is_some());
        let mut job = store::Stored::new(sail_api::jobs::Submission {
            mode: sail_api::jobs::Mode::Stock,
            project_dir: invocation.to_owned(),
            args: Vec::new(),
            force: false,
            environment: BTreeMap::new(),
            input_revision: None,
        });
        job.status.state = sail_api::jobs::Lifecycle::Completed;
        let bytes = serde_json::to_vec(&project)?;
        job.projection =
            Some(store::Projection::validated(&bytes, &project).with_request(Some(request)));
        let directory = state.root.join(&job.status.id);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("stdout"), bytes)?;
        let id = job.status.id.clone();
        state
            .jobs
            .lock()
            .map_err(|_| io::Error::other("fixture jobs lock poisoned"))?
            .insert(id.clone(), job);
        Ok((id, project))
    }

    /// Exercises the actual retrieval/publication boundary. Reports are
    /// declared metadata fixtures, not a trusted verifier or physical readback.
    #[test]
    fn baseline_publication_preserves_its_observed_request_in_both_directions() -> TestResult {
        let root = std::env::temp_dir().join(format!(
            "sail-baseline-request-observation-{}",
            std::process::id()
        ));
        let invocation = root.join("invocation");
        let displaced = root.join("displaced");
        std::fs::create_dir_all(&invocation)?;
        let server = super::super::bind(
            &root.join("socket"),
            &root.join("jobs"),
            std::path::Path::new("/bin/true"),
        )?;
        let state = &server.state;
        let (id, original) = completed_job(state, &invocation)?;
        let admitted = completed_with_request(state, &id)?;
        assert!(admitted.result.targets[0].complete);
        assert!(admitted.request.is_some());
        let original_request = admitted.request.clone();

        // Move between actual result retrieval and immutable publication.
        std::fs::rename(&invocation, &displaced)?;
        assert!(store::retained_request(&get(state, &id)?).is_none());
        let saved = publish_baseline(state, &id, "admitted-before-move", admitted)?;
        assert!(saved.result.targets[0].complete);
        assert_eq!(
            saved.result.currentness,
            sail_api::project::Currentness::Historical
        );
        assert_eq!(saved, baseline(state, "admitted-before-move")?);
        let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(
            state.root.join("baselines/admitted-before-move.json"),
        )?)?;
        assert_eq!(stored["request"], serde_json::to_value(&original_request)?);
        assert_eq!(saved.result.targets[0].report, original.targets[0].report);
        assert_eq!(
            saved.result.targets[0].establishing_run,
            original.targets[0].establishing_run
        );

        let refused = completed_with_request(state, &id)?;
        assert!(!refused.result.targets[0].complete);
        assert!(refused.request.is_none());
        std::fs::rename(&displaced, &invocation)?;
        assert!(store::retained_request(&get(state, &id)?).is_some());
        let saved = publish_baseline(state, &id, "refused-before-restore", refused)?;
        assert!(!saved.result.targets[0].complete);
        assert_eq!(saved, baseline(state, "refused-before-restore")?);
        let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(
            state.root.join("baselines/refused-before-restore.json"),
        )?)?;
        assert!(stored["request"].is_null());
        assert_eq!(saved.result.targets[0].report, original.targets[0].report);
        assert_eq!(
            saved.result.targets[0].establishing_run,
            original.targets[0].establishing_run
        );
        drop(server);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
