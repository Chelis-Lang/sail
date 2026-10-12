//! Proof explanations and immutable baselines, independent of job pruning.

use std::io::{self, Read as _, Write};
use std::os::unix::fs::OpenOptionsExt as _;

use sail_api::evidence::{Baseline, Comparison, Explanation, Reference, VERSION};
use sail_api::project::ProjectResult;
use sail_api::proof::{Cursor, Observation, Target};

use super::{State, get, store};

fn completed(state: &State, id: &str) -> io::Result<ProjectResult> {
    let job = get(state, id)?;
    if job.status.state != sail_api::jobs::Lifecycle::Completed {
        return Err(io::Error::other(
            "SAIL0904: complete job evidence is not yet available",
        ));
    }
    let document = store::result(state, &job)?;
    serde_json::from_value(
        document
            .result
            .ok_or_else(|| io::Error::other("SAIL0904: job has no project evidence"))?,
    )
    .map_err(io::Error::other)
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
    let mut result = completed(state, id)?;
    result.currentness = sail_api::project::Currentness::Historical;
    result.notes.push("Immutable baseline evidence applies only to its recorded input revision, not current workspace inputs.".into());
    let baseline = Baseline {
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
        file.write_all(&serde_json::to_vec(&baseline).map_err(io::Error::other)?)?;
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
    let baseline: Baseline = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::other(format!("SAIL0905: invalid baseline: {error}")))?;
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
    Ok(baseline)
}

pub(super) fn compare(state: &State, before: &Reference, after: &str) -> io::Result<Comparison> {
    let before = match before {
        Reference::Job(id) => completed(state, id)?,
        Reference::Baseline(name) => baseline(state, name)?.result,
    };
    let after = completed(state, after)?;
    Ok(sail_api::evidence::compare(&before, &after))
}
