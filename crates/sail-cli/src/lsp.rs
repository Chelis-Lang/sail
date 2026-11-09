//! `sail lsp`: verification diagnostics for editors over the Language Server
//! Protocol on stdin and stdout (spike).
//!
//! The same driver as `sail watch` decides what runs; this module only
//! presents it. Every edit is verified through the durable job service, and
//! the completed project result is mapped to `textDocument/publishDiagnostics`
//! by `sail-lsp`. Diagnostics of a run that is no longer current are cleared
//! (or, with `--stale mark`, demoted) as soon as its successor is queued, so
//! an editor never shows a superseded run as current. Unlike `sail watch`,
//! a failed job-service call does not end the session: the driver reconnects
//! and verifies again.

use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use sail_api::jobs::{Lifecycle, Status};
use sail_api::project::ProjectResult;
use sail_lsp::diagnostics::{self, Publisher, Stale};
use sail_lsp::jsonrpc;
use sail_lsp::session::{Effect, Session};
use serde_json::{Value, json};

use crate::watch::{Control, Sink};

/// LSP `MessageType`.
const ERROR: u8 = 1;
const WARNING: u8 = 2;
const INFO: u8 = 3;

/// Runs the server until the client sends `exit` or closes stdin.
pub fn lsp(args: &[String], err: &mut dyn Write) -> ExitCode {
    let (policy, args) = match arguments(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0002]: sail lsp: {message}");
            return ExitCode::from(crate::EXIT_USAGE);
        }
    };
    // One thread owns stdin, so the driver never blocks on the editor.
    let (sender, incoming) = channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        // End of input or a broken frame ends the loop: dropping the sender
        // tells the driver the client is gone.
        while let Ok(Some(body)) = jsonrpc::read(&mut input) {
            if sender.send(body).is_err() {
                break;
            }
        }
    });
    let mut server = Server {
        session: Session::new(env!("CARGO_PKG_VERSION")),
        publisher: Publisher::new(policy),
        incoming,
        out: Box::new(std::io::stdout()),
        err,
        exit: None,
    };
    let Some(Effect::Initialized { root }) = server.initialize() else {
        return server.code();
    };
    // Jobs are submitted for the working directory, so adopt the folder the
    // editor opened when it names one.
    if let Some(root) = root
        && let Err(error) = std::env::set_current_dir(&root)
    {
        server.message(
            WARNING,
            &format!("sail lsp: cannot enter workspace folder {root}: {error}"),
        );
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    // An editor may signal instead of sending `exit`; cancel the run then too.
    let _ = ctrlc::set_handler(move || stopping.store(true, Ordering::SeqCst));
    if let Err(message) = crate::watch::drive(&args, &stop, &mut server) {
        // Startup failures (no job service, a refused standalone layout) are
        // not recoverable by reconnecting. Stay up so the editor shows why
        // instead of restarting the server in a loop.
        server.show(ERROR, &format!("sail lsp: {message}"));
        server.status(&json!({"event": "failed", "error": message}));
    }
    let clear = server.publisher.clear();
    server.send_all(&clear);
    // After `shutdown`, or after a failed start, wait for `exit`.
    while server.exit.is_none() && !stop.load(Ordering::SeqCst) {
        match server.incoming.recv() {
            Ok(body) => {
                server.receive(&body);
            }
            Err(_) => server.exit = Some(false),
        }
    }
    server.code()
}

fn arguments(args: &[String]) -> Result<(Stale, Vec<String>), String> {
    let mut policy = Stale::Clear;
    let mut rest = Vec::new();
    let mut iterator = args.iter();
    while let Some(argument) = iterator.next() {
        if argument == "--" {
            rest.push(argument.clone());
            rest.extend(iterator.cloned());
            break;
        }
        let value = if argument == "--stale" {
            Some(iterator.next().map(String::as_str).unwrap_or_default())
        } else {
            argument.strip_prefix("--stale=")
        };
        match value {
            Some("clear") => policy = Stale::Clear,
            Some("mark") => policy = Stale::Mark,
            Some(other) => return Err(format!("--stale takes clear or mark, not `{other}`")),
            None if argument == "--output" || argument.starts_with("--output=") => {
                return Err("--output is not supported; the server speaks LSP on stdout".to_owned());
            }
            None => rest.push(argument.clone()),
        }
    }
    Ok((policy, rest))
}

struct Server<'a> {
    session: Session,
    publisher: Publisher,
    incoming: Receiver<Vec<u8>>,
    out: Box<dyn Write>,
    err: &'a mut dyn Write,
    /// Set once the client is done: whether it shut down cleanly.
    exit: Option<bool>,
}

impl Server<'_> {
    fn code(&self) -> ExitCode {
        if self.exit == Some(true) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        }
    }

    /// Blocks until `initialize` is answered and returns that effect, or
    /// `None` when the client left first.
    fn initialize(&mut self) -> Option<Effect> {
        loop {
            let Ok(body) = self.incoming.recv() else {
                self.exit = Some(false);
                return None;
            };
            match self.receive(&body) {
                effect @ Effect::Initialized { .. } => return Some(effect),
                Effect::Exit { .. } => return None,
                _ => {}
            }
        }
    }

    fn receive(&mut self, body: &[u8]) -> Effect {
        let (replies, effect) = self.session.handle(body);
        self.send_all(&replies);
        if let Effect::Exit { clean } = effect {
            self.exit = Some(clean);
        }
        effect
    }

    fn send_all(&mut self, messages: &[Value]) {
        for message in messages {
            if jsonrpc::write(&mut self.out, message).is_err() {
                // The editor closed its end; nothing more can be reported.
                self.exit.get_or_insert(false);
                return;
            }
        }
    }

    fn message(&mut self, kind: u8, text: &str) {
        let message =
            jsonrpc::notification("window/logMessage", &json!({"type": kind, "message": text}));
        self.send_all(&[message]);
    }

    fn show(&mut self, kind: u8, text: &str) {
        let message = jsonrpc::notification(
            "window/showMessage",
            &json!({"type": kind, "message": text}),
        );
        self.send_all(&[message]);
    }

    /// A `$/sail/status` notification. Clients may ignore `$/` notifications;
    /// those that listen get the job lifecycle the squiggles alone cannot
    /// show (running, current or historical, infrastructure failure).
    fn status(&mut self, params: &Value) {
        let message = jsonrpc::notification("$/sail/status", params);
        self.send_all(&[message]);
    }
}

impl Sink for Server<'_> {
    fn job(&mut self, status: &Status, event: &str) {
        // A newer run is queued, or this one will never complete: whatever
        // is shown belongs to an older input state.
        if matches!(event, "queued" | "superseded" | "cancelled") {
            let stale = self.publisher.stale();
            self.send_all(&stale);
        }
        if status.state == Lifecycle::InfrastructureError {
            let error = status.error.as_deref().unwrap_or("infrastructure error");
            self.show(ERROR, &format!("sail: verification did not run: {error}"));
        }
        self.message(
            INFO,
            &format!(
                "sail: job {} {event} ({:?}, exit {:?})",
                status.id, status.state, status.exit_code
            ),
        );
        self.status(&json!({
            "event": event,
            "job": status.id,
            "state": status.state,
            "exit_code": status.exit_code,
            "currentness": status.currentness,
            "error": status.error,
        }));
    }

    fn log(&mut self, text: &str) {
        // Editors keep a language server's stderr in its log.
        let _ = write!(self.err, "{text}");
    }

    fn result(&mut self, status: &Status, result: &ProjectResult) {
        let projection = diagnostics::project(result);
        let located: usize = projection.by_uri.values().map(Vec::len).sum();
        let notifications = self.publisher.result(result);
        self.send_all(&notifications);
        let mut summary = sail_api::output::Summary::default();
        for target in result.targets.iter().filter(|target| target.requested) {
            let counts = target.report.summary;
            summary.functions += counts.functions;
            summary.verified += counts.verified;
            summary.failed += counts.failed;
            summary.unknown += counts.unknown;
            summary.provisional += counts.provisional;
            summary.unstable += counts.unstable;
        }
        self.status(&json!({
            "event": "result",
            "job": status.id,
            "exit_code": result.exit_code,
            "currentness": result.currentness,
            "published": self.publisher.showing(),
            "diagnostics": located,
            "unlocated": projection.unlocated,
            "summary": summary,
        }));
    }

    fn warnings(&mut self) -> &mut dyn Write {
        self.err
    }

    fn control(&mut self) -> Control {
        let mut control = Control::Continue;
        loop {
            match self.incoming.try_recv() {
                Ok(body) => match self.receive(&body) {
                    Effect::Saved { .. } => control = Control::Verify,
                    Effect::Exit { .. } => return Control::Stop,
                    _ => {}
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.exit.get_or_insert(false);
                    return Control::Stop;
                }
            }
        }
        if self.exit.is_some() || !self.session.running() {
            return Control::Stop;
        }
        control
    }

    fn recover(&mut self, error: &str) -> bool {
        let stale = self.publisher.stale();
        self.send_all(&stale);
        self.message(
            WARNING,
            &format!("sail: job service call failed ({error}); reconnecting and verifying again"),
        );
        self.status(&json!({"event": "reconnecting", "error": error}));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_policy_and_refused_output() {
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|a| (*a).to_owned()).collect() };
        assert_eq!(
            arguments(&args(&["-p", "x", "--stale", "mark", "--", "--stale"])),
            Ok((Stale::Mark, args(&["-p", "x", "--", "--stale"])))
        );
        assert_eq!(
            arguments(&args(&["--stale=clear"])),
            Ok((Stale::Clear, vec![]))
        );
        assert!(arguments(&args(&["--stale", "keep"])).is_err());
        assert!(arguments(&args(&["--stale"])).is_err());
        assert!(arguments(&args(&["--output", "jsonl"])).is_err());
    }
}
