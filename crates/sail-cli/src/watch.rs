//! Workspace-aware live verification, sharing the durable project job service.
//!
//! Filesystem events schedule managed Cargo verification after 300 ms of quiet.
//! Edits cancel the previous job, including all verifier descendants. Results
//! remain durable and are identified by job and input revision; a cancelled or
//! superseded run is never presented as a current proof.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sail_api::jobs::{Client, Lifecycle, Request, Response, Status};
use serde_json::json;

const SETTLE: Duration = Duration::from_millis(300);

struct Live {
    client: Client,
    status: Option<Status>,
    pending: Option<Instant>,
    diagnostic_offset: u64,
    event_offset: u64,
    event_tail: String,
    jsonl: bool,
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
        pending: Some(Instant::now()),
        diagnostic_offset: 0,
        event_offset: 0,
        event_tail: String::new(),
        jsonl,
    };
    let (mut watcher, changes) = watcher()?;
    let mut roots = Vec::new();
    refresh_roots(&mut watcher, &mut roots, &root, &args, err)?;
    while !stop.load(Ordering::SeqCst) {
        if changed(&changes, &roots) {
            live.cancel(true)?;
            live.pending = Some(Instant::now());
        }
        if live.pending.is_some_and(|time| time.elapsed() >= SETTLE) {
            refresh_roots(&mut watcher, &mut roots, &root, &args, err)?;
            let status = crate::jobs::submit(&live.client, &args)?;
            announce(&status, "queued", live.jsonl, out, err);
            live.status = Some(status);
            live.pending = None;
            live.diagnostic_offset = 0;
            live.event_offset = 0;
            live.event_tail.clear();
        }
        live.poll(out, err)?;
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
fn watcher() -> Result<(RecommendedWatcher, Receiver<notify::Result<Event>>), String> {
    let (sender, receiver) = channel();
    let watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| error.to_string())?;
    Ok((watcher, receiver))
}
fn refresh_roots(
    watcher: &mut RecommendedWatcher,
    roots: &mut Vec<PathBuf>,
    project: &Path,
    args: &[String],
    err: &mut dyn Write,
) -> Result<(), String> {
    let snapshot = crate::jobs::revision(project, args);
    let mut next = snapshot.map_or_else(
        || vec![project.to_path_buf()],
        |snapshot| {
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
    next.sort();
    next.dedup();
    for root in roots.iter().filter(|root| !next.contains(root)) {
        let _ = watcher.unwatch(root);
    }
    for root in next.iter().filter(|root| !roots.contains(root)) {
        if let Err(error) = watcher.watch(
            root,
            if root.file_name().is_some_and(|name| name == ".cargo") {
                RecursiveMode::NonRecursive
            } else {
                RecursiveMode::Recursive
            },
        ) {
            let _ = writeln!(err, "sail watch: cannot watch {}: {error}", root.display());
            return Err("filesystem monitoring is incomplete".to_owned());
        }
    }
    *roots = next;
    Ok(())
}
fn relevant(path: &Path) -> bool {
    !path.components().any(|part| {
        ["target", ".git", ".claude", ".codex"]
            .iter()
            .any(|ignored| part.as_os_str() == *ignored)
    })
}
fn changed(changes: &Receiver<notify::Result<Event>>, roots: &[PathBuf]) -> bool {
    let mut edited = false;
    while let Ok(event) = changes.try_recv() {
        edited |= event.map_or(true, |event| {
            !matches!(event.kind, EventKind::Access(_))
                && event.paths.iter().any(|path| {
                    relevant(path)
                        && roots.iter().any(|root| {
                            if root.file_name().is_some_and(|name| name == ".cargo") {
                                path == root
                                    || path.file_name().is_some_and(|name| {
                                        name == "config" || name == "config.toml"
                                    })
                            } else {
                                path.starts_with(root)
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
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
    fn poll(&mut self, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), String> {
        let Some(previous) = self.status.as_ref() else {
            return Ok(());
        };
        let id = previous.id.clone();
        let Response::Status(status) = self
            .client
            .call(&Request::Status { id: id.clone() })
            .map_err(|error| error.to_string())?
        else {
            return Err("SAIL0802: unexpected status response".to_owned());
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
            .map_err(|error| error.to_string())?
        {
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
                .map_err(|error| error.to_string())?
        {
            self.event_offset = chunk.next;
            self.event_tail.push_str(&chunk.text);
            while let Some(end) = self.event_tail.find('\n') {
                let line: String = self.event_tail.drain(..=end).collect();
                if let Ok(mut event) = serde_json::from_str::<serde_json::Value>(&line) {
                    event["job_id"] = json!(status.id);
                    let _ = writeln!(out, "{event}");
                }
            }
        }
        self.status = Some(status);
        Ok(())
    }
}
