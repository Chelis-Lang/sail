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

/// What the driver does after asking its sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    /// Keep going.
    Continue,
    /// The client reports a save: check the inputs now and skip the debounce.
    Verify,
    /// Cancel the run in flight and return.
    Stop,
}

/// Where a live run reports. `sail watch` writes lines ([`Stream`]); `sail
/// lsp` publishes diagnostics. The driver decides what runs and when; a sink
/// only presents it.
pub(crate) trait Sink {
    /// The job changed lifecycle: `queued`, `started`, `superseded`,
    /// `cancelled` or `finished`.
    fn job(&mut self, status: &Status, event: &str);
    /// A piece of the job's stderr, verbatim.
    fn log(&mut self, text: &str);
    /// Whether [`Sink::event`] wants the job's structured events.
    fn wants_events(&self) -> bool {
        false
    }
    /// One structured job event.
    fn event(&mut self, _job: &str, _event: serde_json::Map<String, serde_json::Value>) {}
    /// A completed job's project result.
    fn result(&mut self, _status: &Status, _result: &sail_api::project::ProjectResult) {}
    /// A completed run without proof reuse (`SAIL3112`).
    fn no_reuse(&mut self, _status: &Status, _result: &sail_api::project::ProjectResult) {}
    /// Where input-monitoring warnings go.
    fn warnings(&mut self) -> &mut dyn Write;
    /// Asked once per tick.
    fn control(&mut self) -> Control {
        Control::Continue
    }
    /// A job-service call failed. `true` makes the driver reconnect and
    /// verify again; `false` ends the run with that error.
    fn recover(&mut self, _error: &str) -> bool {
        false
    }
}

/// `sail watch`'s output: status lines and the job's stderr for people, or
/// JSON lines on `out`.
pub(crate) struct Stream<'a> {
    pub(crate) jsonl: bool,
    pub(crate) out: &'a mut dyn Write,
    pub(crate) err: &'a mut dyn Write,
}

impl Sink for Stream<'_> {
    fn job(&mut self, status: &Status, event: &str) {
        if self.jsonl {
            let _ = writeln!(
                self.out,
                "{}",
                json!({"kind":"sail-watch-event","schema_version":1,"event":event,"job":status})
            );
            let _ = self.out.flush();
        } else {
            let _ = writeln!(
                self.err,
                "sail watch: job {} {:?} (exit {:?})",
                status.id, status.state, status.exit_code
            );
        }
    }
    fn log(&mut self, text: &str) {
        let _ = write!(self.err, "{text}");
    }
    fn wants_events(&self) -> bool {
        self.jsonl
    }
    fn event(&mut self, job: &str, mut event: serde_json::Map<String, serde_json::Value>) {
        event.insert("job_id".to_owned(), json!(job));
        let _ = writeln!(self.out, "{}", serde_json::Value::Object(event));
    }
    /// The human stream already carries the job's `note[SAIL3112]` line; the
    /// JSON-lines stream gets a `no-reuse` event.
    fn no_reuse(&mut self, status: &Status, result: &sail_api::project::ProjectResult) {
        if self.jsonl {
            let _ = writeln!(
                self.out,
                "{}",
                json!({"kind":"sail-watch-event","schema_version":1,"event":"no-reuse","job_id":status.id,"no_reuse":result.no_reuse,"currentness":result.currentness,"exit_code":result.exit_code})
            );
            let _ = self.out.flush();
        }
    }
    fn warnings(&mut self) -> &mut dyn Write {
        self.err
    }
}

struct Live {
    client: Client,
    status: Option<Status>,
    pending: Option<Instant>,
    diagnostic_offset: u64,
    event_offset: u64,
    event_tail: String,
    events: bool,
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
    let (jsonl, args) = arguments(args)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    ctrlc::set_handler(move || stopping.store(true, Ordering::SeqCst))
        .map_err(|error| error.to_string())?;
    drive(&args, &stop, &mut Stream { jsonl, out, err })
}

/// The loop state of one live run.
struct Driver<'a> {
    root: PathBuf,
    args: &'a [String],
    live: Live,
    inputs: Inputs,
    subscriptions: BTreeMap<PathBuf, bool>,
    extra: Vec<PathBuf>,
    monitoring: monitor::Monitoring,
    /// A save was reported: the next detected edit skips the debounce.
    urgent: bool,
}

/// Verifies `args` now and after every input change until `stop` is set or
/// the sink asks to stop, reporting to `sink`.
pub(crate) fn drive(args: &[String], stop: &AtomicBool, sink: &mut dyn Sink) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    if let Some(file) = sail_verus::managed::standalone::source(args) {
        standalone_watch_roots(&root, &[root.join(file)])?;
    }
    if sail_verus::managed::standalone::source(args).is_some()
        && args.iter().skip(1).any(|arg| {
            [
                "--compile",
                "--out-dir",
                "--emit",
                "--export",
                "--log",
                "-o",
            ]
            .iter()
            .any(|prefix| arg.starts_with(prefix))
        })
    {
        return Err("standalone watch is verify-only; use sail verify for compilation and explicit output files".into());
    }

    let live = Live {
        client: crate::jobs::connect_or_start()?,
        status: None,
        pending: None,
        diagnostic_offset: 0,
        event_offset: 0,
        event_tail: String::new(),
        events: sink.wants_events(),
        observed_inputs: None,
        observed_job: None,
        diagnostics_drained: false,
        events_drained: false,
    };
    let mut inputs = Inputs::default();
    let mut subscriptions = BTreeMap::new();
    let extra = Vec::new();
    // Cargo metadata may create Cargo.lock. Finish that discovery before
    // starting native notifications, so startup's own writes cannot arrive
    // as delayed edits and cancel the first verification on macOS.
    refresh_inputs(&mut inputs, &root, args, &extra);
    let mut monitoring = monitor::Monitoring::new(sink.warnings());
    monitoring.subscribe(
        &inputs.roots,
        &inputs.excluded,
        &mut subscriptions,
        sink.warnings(),
    );
    let mut driver = Driver {
        root,
        args,
        live,
        inputs,
        subscriptions,
        extra,
        monitoring,
        urgent: false,
    };
    // Initial metadata and subscriptions can outlast SETTLE. Start the first
    // debounce window only once they are ready, just as for later edits.
    driver.live.pending = Some(Instant::now());
    while !stop.load(Ordering::SeqCst) {
        match sink.control() {
            Control::Stop => break,
            Control::Verify => {
                driver.monitoring.hint();
                driver.urgent = true;
            }
            Control::Continue => {}
        }
        if let Err(error) = driver.step(sink) {
            if !sink.recover(&error) {
                return Err(error);
            }
            if !driver.reconnect(stop, sink) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    driver.live.cancel(false)
}

impl Driver<'_> {
    fn step(&mut self, sink: &mut dyn Sink) -> Result<(), String> {
        if self.monitoring.changed(sink.warnings()) {
            self.live.cancel(true)?;
            self.live.pending = Some(Instant::now());
        }
        let due = self
            .live
            .pending
            .is_some_and(|time| self.urgent || time.elapsed() >= SETTLE);
        // A save hint applies to the edit it accompanies, not to a later one.
        self.urgent = false;
        if due {
            self.refresh_roots(sink);
            // The run reads its inputs after this point. Notifications that
            // arrive later for changes already present here are not edits.
            self.monitoring.observe();
            let status = crate::jobs::submit(&self.live.client, self.args)?;
            sink.job(&status, "queued");
            self.live.status = Some(status);
            self.live.pending = None;
            self.live.rewind();
        }
        self.live.poll(sink)?;
        if let Some(observed) = self.live.observed_inputs.take() {
            if sail_verus::managed::standalone::source(self.args).is_some() {
                standalone_watch_roots(&self.root, &observed)?;
            }
            self.extra = observed;
            self.refresh_roots(sink);
        }
        Ok(())
    }

    fn refresh_roots(&mut self, sink: &mut dyn Sink) {
        refresh_roots(
            &mut self.monitoring,
            &mut self.inputs,
            &mut self.subscriptions,
            &self.root,
            self.args,
            &self.extra,
            sink.warnings(),
        );
    }

    /// Replaces the job client after a failed call and schedules a fresh
    /// run, retrying until the service answers. A long-lived editor session
    /// must outlive a daemon restart or a transient RPC failure (the
    /// unexplained failure of issue 306 ended `sail watch`). Returns `false`
    /// when asked to stop meanwhile.
    fn reconnect(&mut self, stop: &AtomicBool, sink: &mut dyn Sink) -> bool {
        let previous = self.live.status.take();
        loop {
            match crate::jobs::connect_or_start() {
                Ok(client) => {
                    self.live.client = client;
                    break;
                }
                Err(error) => {
                    let _ = writeln!(sink.warnings(), "sail: job service unavailable: {error}");
                }
            }
            for _ in 0..10 {
                if stop.load(Ordering::SeqCst) || sink.control() == Control::Stop {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        // The run the failed call belonged to may still be alive in a
        // service that kept running. It is no longer followed, so it must
        // not keep the worker busy; a service that lost it refuses, harmlessly.
        if let Some(status) = previous.filter(|status| !status.state.terminal()) {
            let _ = self.live.client.call(&Request::Cancel {
                id: status.id,
                superseded: true,
            });
        }
        self.live.rewind();
        self.live.observed_job = None;
        self.live.pending = Some(Instant::now());
        self.urgent = true;
        true
    }
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
    if sail_verus::managed::standalone::source(args).is_some() {
        let snapshot = crate::jobs::revision(project, args);
        inputs.roots = snapshot
            .as_ref()
            .map_or_else(|| vec![project.to_owned()], |s| s.roots.clone());
        inputs.excluded = snapshot.map_or_else(
            || {
                let mut excluded = vec![project.join(".git")];
                if let Ok(cache) = sail_verus::cache_dir() {
                    excluded.push(std::fs::canonicalize(&cache).unwrap_or(cache));
                }
                excluded
            },
            |s| s.excluded,
        );
        inputs.roots.extend_from_slice(extra);
        path_aliases(&mut inputs.roots);
        path_aliases(&mut inputs.excluded);
        return;
    }

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

fn standalone_watch_roots(project: &Path, roots: &[PathBuf]) -> Result<(), String> {
    let mut resolved = roots.to_vec();
    path_aliases(&mut resolved);
    if resolved.iter().any(|path| !path.starts_with(project)) {
        return Err("SAIL3150: standalone watch requires its source and discovered inputs within the working directory; run from their common source directory or use sail verify".into());
    }
    Ok(())
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
    /// Forgets the read positions of the previous job's streams.
    fn rewind(&mut self) {
        self.diagnostic_offset = 0;
        self.event_offset = 0;
        self.event_tail.clear();
        self.diagnostics_drained = false;
        self.events_drained = false;
    }
    fn poll(&mut self, sink: &mut dyn Sink) -> Result<(), String> {
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
            sink.job(&status, event);
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
            sink.log(&chunk.text);
        }
        if self.events
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
                if let Ok(serde_json::Value::Object(event)) = serde_json::from_str(&line) {
                    sink.event(&status.id, event);
                }
            }
        }
        if !self.events {
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
            {
                sink.result(&status, &result);
                if result.no_reuse.is_some() {
                    // Verified as plain Cargo-Verus verifies it: such a run
                    // has no input record to refresh the watched roots from.
                    sink.no_reuse(&status, &result);
                } else if let Ok(environment) = crate::jobs::environment()
                    && let Ok(cache) = sail_verus::cache_dir()
                    && let Ok(snapshot) =
                        sail_verus::managed::refresh_result(&cache, &result, &environment)
                {
                    self.observed_inputs = Some(snapshot.roots);
                }
            }
        }
        self.status = Some(status);
        Ok(())
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
            events: true,
            observed_inputs: None,
            observed_job: Some("finished".to_owned()),
            diagnostics_drained: true,
            events_drained: true,
        };
        let started = Instant::now();
        for _ in 0..1000 {
            let mut sink = Stream {
                jsonl: true,
                out: &mut Vec::new(),
                err: &mut Vec::new(),
            };
            assert!(live.poll(&mut sink).is_ok());
        }
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
