//! Workspace-aware live verification, sharing the durable project job service.
//!
//! Filesystem events schedule managed Cargo verification after 300 ms of quiet.
//! Edits cancel the previous job, including all verifier descendants. Results
//! remain durable and are identified by job and input revision; a cancelled or
//! superseded run is never presented as a current proof.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use notify::{Event, EventKind};
use sail_api::jobs::{Client, Lifecycle, Request, Response, Status};
use serde_json::json;

mod monitor;

const SETTLE: Duration = Duration::from_millis(300);

#[derive(Default)]
struct Inputs {
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
}

struct Live {
    client: Client,
    status: Option<Status>,
    pending: Option<Instant>,
    diagnostic_offset: u64,
    event_offset: u64,
    event_tail: String,
    jsonl: bool,
    observed_inputs: Option<Vec<PathBuf>>,
    observed_job: Option<String>,
    diagnostics_drained: bool,
    events_drained: bool,
}

/// Watches the Cargo workspace and local path dependencies until interrupted.
pub fn watch(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    match run(args, out, err) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0201]: sail watch: {message}");
            ExitCode::from(2)
        }
    }
}
fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let (jsonl, args) = arguments(args)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    ctrlc::set_handler(move || stopping.store(true, Ordering::SeqCst))
        .map_err(|error| error.to_string())?;
    let mut live = Live {
        client: crate::jobs::connect_or_start()?,
        status: None,
        pending: None,
        diagnostic_offset: 0,
        event_offset: 0,
        event_tail: String::new(),
        jsonl,
        observed_inputs: None,
        observed_job: None,
        diagnostics_drained: false,
        events_drained: false,
    };
    let mut inputs = Inputs::default();
    let mut subscriptions = BTreeMap::new();
    let mut extra = Vec::new();
    // Cargo metadata may create Cargo.lock. Finish that discovery before
    // starting native notifications, so startup's own writes cannot arrive
    // as delayed edits and cancel the first verification on macOS.
    refresh_inputs(&mut inputs, &root, &args, &extra);
    let mut monitoring = monitor::Monitoring::new(err);
    monitoring.subscribe(&inputs.roots, &inputs.excluded, &mut subscriptions, err);
    // Initial metadata and subscriptions can outlast SETTLE. Start the first
    // debounce window only once they are ready, just as for later edits.
    live.pending = Some(Instant::now());
    while !stop.load(Ordering::SeqCst) {
        if monitoring.changed(err) {
            live.cancel(true)?;
            live.pending = Some(Instant::now());
        }
        if live.pending.is_some_and(|time| time.elapsed() >= SETTLE) {
            refresh_roots(
                &mut monitoring,
                &mut inputs,
                &mut subscriptions,
                &root,
                &args,
                &extra,
                err,
            );
            // The run reads its inputs after this point. Notifications that
            // arrive later for changes already present here are not edits.
            monitoring.observe();
            let status = crate::jobs::submit(&live.client, &args)?;
            announce(&status, "queued", live.jsonl, out, err);
            live.status = Some(status);
            live.pending = None;
            live.diagnostic_offset = 0;
            live.event_offset = 0;
            live.event_tail.clear();
            live.diagnostics_drained = false;
            live.events_drained = false;
        }
        live.poll(out, err)?;
        if let Some(observed) = live.observed_inputs.take() {
            extra = observed;
            refresh_roots(
                &mut monitoring,
                &mut inputs,
                &mut subscriptions,
                &root,
                &args,
                &extra,
                err,
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    live.cancel(false)
}
fn arguments(args: &[String]) -> Result<(bool, Vec<String>), String> {
    let mut jsonl = false;
    let mut rest = Vec::new();
    let mut iterator = args.iter();
    while let Some(argument) = iterator.next() {
        if argument == "--" {
            rest.push(argument.clone());
            rest.extend(iterator.cloned());
            break;
        }
        let value = if argument == "--output" {
            Some(iterator.next().map(String::as_str).unwrap_or_default())
        } else {
            argument.strip_prefix("--output=")
        };
        if let Some(value) = value {
            if value != "jsonl" {
                return Err("watch supports --output jsonl".to_owned());
            }
            jsonl = true;
        } else {
            rest.push(argument.clone());
        }
    }
    Ok((jsonl, rest))
}
fn refresh_roots(
    monitoring: &mut monitor::Monitoring,
    inputs: &mut Inputs,
    subscriptions: &mut BTreeMap<PathBuf, bool>,
    project: &Path,
    args: &[String],
    extra: &[PathBuf],
    err: &mut dyn Write,
) {
    refresh_inputs(inputs, project, args, extra);
    monitoring.subscribe(&inputs.roots, &inputs.excluded, subscriptions, err);
}

fn refresh_inputs(inputs: &mut Inputs, project: &Path, args: &[String], extra: &[PathBuf]) {
    let snapshot = crate::jobs::revision(project, args);
    let mut next = snapshot.map_or_else(
        || vec![project.to_path_buf()],
        |snapshot| {
            inputs.excluded = snapshot.excluded;
            let mut roots = snapshot.roots;
            roots.push(snapshot.workspace);
            roots
        },
    );
    // Cargo configuration inherited from ancestors is also a watched input.
    for ancestor in project.ancestors() {
        let cargo = ancestor.join(".cargo");
        if cargo.is_dir() {
            next.push(cargo);
        }
    }
    if let Some(home) = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
    {
        for name in ["config", "config.toml"] {
            if home.join(name).is_file() {
                next.push(home.clone());
                break;
            }
        }
    }
    next.extend_from_slice(extra);
    // An explicit `--target-dir` (verified without proof reuse, SAIL3112) is
    // Cargo's output, not an input: watching it would restart every run.
    if let Some(target) = target_dir(args) {
        inputs.excluded.push(project.join(target));
    }
    path_aliases(&mut next);
    path_aliases(&mut inputs.excluded);
    inputs.roots = next;
}
// Native notifications can use physical paths while Cargo preserves a supplied
// symlink alias (for example /tmp vs /private/tmp). Preserve both spellings,
// including absent paths resolved through their nearest existing ancestor.
fn path_aliases(paths: &mut Vec<PathBuf>) {
    let aliases: Vec<_> = paths
        .iter()
        .filter_map(|path| {
            path.ancestors().find_map(|ancestor| {
                let canonical = ancestor.canonicalize().ok()?;
                Some(canonical.join(path.strip_prefix(ancestor).ok()?))
            })
        })
        .collect();
    paths.extend(aliases);
    paths.sort();
    paths.dedup();
}

fn desired_subscriptions(roots: &[PathBuf]) -> BTreeMap<PathBuf, bool> {
    let mut subscriptions = BTreeMap::new();
    for root in roots {
        let recursive = root.is_dir() && root.file_name().is_none_or(|name| name != ".cargo");
        let path = if root.is_dir() {
            Some(root.as_path())
        } else {
            root.ancestors().skip(1).find(|parent| parent.is_dir())
        };
        if let Some(path) = path {
            subscriptions
                .entry(path.to_path_buf())
                .and_modify(|mode| *mode |= recursive)
                .or_insert(recursive);
        }
    }
    subscriptions
}

/// Cargo's `--target-dir` among `args` (before any `--`).
fn target_dir(args: &[String]) -> Option<PathBuf> {
    let mut iterator = args.iter().take_while(|arg| arg.as_str() != "--");
    let mut found = None;
    while let Some(arg) = iterator.next() {
        if arg == "--target-dir" {
            found = iterator.next().map(PathBuf::from);
        } else if let Some(value) = arg.strip_prefix("--target-dir=") {
            found = Some(PathBuf::from(value));
        }
    }
    found
}

fn relevant(path: &Path) -> bool {
    !path.components().any(|part| {
        [".git", ".claude", ".codex"]
            .iter()
            .any(|ignored| part.as_os_str() == *ignored)
    })
}
fn changed(
    changes: &Receiver<notify::Result<Event>>,
    roots: &[PathBuf],
    excluded: &[PathBuf],
) -> bool {
    let mut edited = false;
    while let Ok(event) = changes.try_recv() {
        edited |= event.is_ok_and(|event| {
            !matches!(event.kind, EventKind::Access(_))
                && event.paths.iter().any(|path| {
                    relevant(path)
                        && (!excluded.iter().any(|output| path.starts_with(output))
                            || roots.iter().any(|root| {
                                path.starts_with(root)
                                    && excluded.iter().any(|output| root.starts_with(output))
                            }))
                        && roots.iter().any(|root| {
                            if root.file_name().is_some_and(|name| name == ".cargo") {
                                path == root
                                    || path.file_name().is_some_and(|name| {
                                        name == "config" || name == "config.toml"
                                    })
                            } else if root.is_dir() {
                                path.starts_with(root)
                            } else {
                                path == root || root.starts_with(path)
                            }
                        })
                })
        });
    }
    edited
}
fn announce(status: &Status, event: &str, jsonl: bool, out: &mut dyn Write, err: &mut dyn Write) {
    if jsonl {
        let _ = writeln!(
            out,
            "{}",
            json!({"kind":"sail-watch-event","schema_version":1,"event":event,"job":status})
        );
        let _ = out.flush();
    } else {
        let _ = writeln!(
            err,
            "sail watch: job {} {:?} (exit {:?})",
            status.id, status.state, status.exit_code
        );
    }
}
impl Live {
    fn cancel(&mut self, superseded: bool) -> Result<(), String> {
        if let Some(status) = &self.status
            && !status.state.terminal()
        {
            self.client
                .call(&Request::Cancel {
                    id: status.id.clone(),
                    superseded,
                })
                .map_err(|error| format!("cannot cancel job: {error}"))?;
        }
        Ok(())
    }
    fn poll(&mut self, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), String> {
        let Some(previous) = self.status.as_ref() else {
            return Ok(());
        };
        if previous.state.terminal() && self.diagnostics_drained && self.events_drained {
            return Ok(());
        }
        let id = previous.id.clone();
        let status = if previous.state.terminal() {
            previous.clone()
        } else {
            let Response::Status(status) = self
                .client
                .call(&Request::Status { id: id.clone() })
                .map_err(|error| format!("cannot read job status: {error}"))?
            else {
                return Err("SAIL0802: unexpected status response".to_owned());
            };
            status
        };
        if previous.state != status.state {
            let event = match status.state {
                Lifecycle::Running => "started",
                Lifecycle::Superseded => "superseded",
                Lifecycle::Cancelled => "cancelled",
                _ => "finished",
            };
            announce(&status, event, self.jsonl, out, err);
        }
        if let Response::Chunk(chunk) = self
            .client
            .call(&Request::Logs {
                id: id.clone(),
                offset: self.diagnostic_offset,
            })
            .map_err(|error| format!("cannot read job diagnostics: {error}"))?
        {
            self.diagnostics_drained = chunk.complete && chunk.next == self.diagnostic_offset;
            self.diagnostic_offset = chunk.next;
            let _ = write!(err, "{}", chunk.text);
        }
        if self.jsonl
            && let Response::Chunk(chunk) = self
                .client
                .call(&Request::Events {
                    id,
                    offset: self.event_offset,
                })
                .map_err(|error| format!("cannot read job events: {error}"))?
        {
            self.events_drained = chunk.complete && chunk.next == self.event_offset;
            self.event_offset = chunk.next;
            self.event_tail.push_str(&chunk.text);
            while let Some(end) = self.event_tail.find('\n') {
                let line: String = self.event_tail.drain(..=end).collect();
                if let Ok(serde_json::Value::Object(mut event)) = serde_json::from_str(&line) {
                    event.insert("job_id".to_owned(), json!(status.id));
                    let _ = writeln!(out, "{}", serde_json::Value::Object(event));
                }
            }
        }
        if !self.jsonl {
            self.events_drained = true;
        }
        if status.state == Lifecycle::Completed && self.observed_job.as_deref() != Some(&status.id)
        {
            self.observed_job = Some(status.id.clone());
            if let Response::Result(document) = self
                .client
                .call(&Request::Result {
                    id: status.id.clone(),
                })
                .map_err(|error| format!("cannot read job result: {error}"))?
                && let Some(result) = document.result
                && let Ok(result) =
                    serde_json::from_value::<sail_api::project::ProjectResult>(result)
                && !self.no_reuse(&status, &result, out)
                && let Ok(environment) = crate::jobs::environment()
                && let Ok(cache) = sail_verus::cache_dir()
                && let Ok(snapshot) =
                    sail_verus::managed::refresh_result(&cache, &result, &environment)
            {
                self.observed_inputs = Some(snapshot.roots);
            }
        }
        self.status = Some(status);
        Ok(())
    }
    /// Announces a completed run without proof reuse (`SAIL3112`, verified as
    /// plain Cargo-Verus verifies it) and returns whether it was one. The
    /// human stream already carries the job's `note[SAIL3112]` line; the
    /// JSON-lines stream gets a `no-reuse` event. Such a run has no input
    /// record to refresh the watched roots from.
    fn no_reuse(
        &self,
        status: &Status,
        result: &sail_api::project::ProjectResult,
        out: &mut dyn Write,
    ) -> bool {
        let Some(no_reuse) = &result.no_reuse else {
            return false;
        };
        if self.jsonl {
            let _ = writeln!(
                out,
                "{}",
                json!({"kind":"sail-watch-event","schema_version":1,"event":"no-reuse","job_id":status.id,"no_reuse":no_reuse,"currentness":result.currentness,"exit_code":result.exit_code})
            );
            let _ = out.flush();
        }
        true
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let _ = self.cancel(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn physical_output_events_respect_aliases_and_exact_declared_input_overrides()
    -> std::io::Result<()> {
        let root = std::env::temp_dir().join(format!("sail-watch-alias-{}", std::process::id()));
        let physical = root.join("physical");
        let alias = root.join("alias");
        std::fs::create_dir_all(physical.join("src/target"))?;
        std::os::unix::fs::symlink(&physical, &alias)?;
        let declared = alias.join("target/declared.rs");
        let mut roots = vec![alias.clone(), declared];
        let mut excluded = vec![alias.join("target")];
        // Resolve absent output and declared-file paths before Cargo creates them.
        path_aliases(&mut roots);
        path_aliases(&mut excluded);
        let physical = physical.canonicalize()?;
        std::fs::create_dir_all(physical.join("target"))?;
        let (sender, receiver) = std::sync::mpsc::channel();
        for (path, expected) in [
            (physical.join("target/generated.rs"), false),
            (physical.join("target/declared.rs"), true),
            (physical.join("src/target/mod.rs"), true),
        ] {
            sender
                .send(Ok(Event::new(EventKind::Create(
                    notify::event::CreateKind::File,
                ))
                .add_path(path)))
                .map_err(std::io::Error::other)?;
            assert_eq!(changed(&receiver, &roots, &excluded), expected);
        }
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn terminal_drained_job_never_contacts_the_service_again() {
        let mut live = Live {
            client: Client::new(PathBuf::from("/missing-sail-watch-test.sock")),
            status: Some(Status {
                version: 1,
                id: "finished".to_owned(),
                project_dir: PathBuf::from("/workspace"),
                args: vec![],
                state: Lifecycle::Completed,
                submitted_at_ms: 0,
                started_at_ms: Some(0),
                finished_at_ms: Some(1),
                exit_code: Some(0),
                input_revision: Some("earlier-revision".to_owned()),
                currentness: sail_api::jobs::Currentness::Current,
                error: None,
            }),
            pending: None,
            diagnostic_offset: 0,
            event_offset: 0,
            event_tail: String::new(),
            jsonl: true,
            observed_inputs: None,
            observed_job: Some("finished".to_owned()),
            diagnostics_drained: true,
            events_drained: true,
        };
        let started = Instant::now();
        for _ in 0..1000 {
            assert!(live.poll(&mut Vec::new(), &mut Vec::new()).is_ok());
        }
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
