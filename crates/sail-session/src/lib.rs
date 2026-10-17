//! A model of an interactive solver session.
//!
//! [`Session`] follows the commands a client sends to a solver and tracks:
//! - the assertion stack: the state-affecting commands at each push level;
//! - the solver options in effect, which `push` and `pop` do not scope;
//! - where the session is in its history.
//!
//! At each satisfiability check it produces a [`Snapshot`] of exactly that
//! state. The model is conservative. Anything it cannot follow exactly (an
//! unrecognised command, a malformed command, a `pop` below the base level)
//! marks the session *inexact*, and snapshots taken afterwards say so.
//! Consumers such as the cache must then treat those snapshots as
//! unidentifiable.
//!
//! The model never alters or withholds what is sent to the solver; it only
//! observes.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use sail_smtlib::command::{CommandKind, Output, classify};

/// The exact bytes of one command.
pub type CommandBytes = Arc<[u8]>;

/// Why a session stopped being exact. The first reason is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Inexact {
    /// A command that does not parse as a list headed by a symbol.
    Malformed,
    /// A command the model does not understand.
    Unmodelled,
    /// A `pop` deeper than the number of open levels.
    PopUnderflow,
}

/// One push level: its state-affecting commands, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Level {
    /// Assertions, declarations and definitions made at this level.
    pub commands: Vec<CommandBytes>,
}

/// The complete solver state at a satisfiability check.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snapshot {
    /// The assertion stack, base level first. Level boundaries are kept,
    /// because the solver's behaviour may depend on them even when the
    /// assertions are the same.
    pub levels: Vec<Level>,
    /// Each option's most recent value, by keyword (both as exact bytes).
    pub options: BTreeMap<Vec<u8>, Vec<u8>>,
    /// `set-logic` and `set-info` commands since the last reset, in order.
    pub globals: Vec<CommandBytes>,
    /// The check command itself (`check-sat`, `check-sat-assuming …`).
    pub check: CommandBytes,
    /// Commands applied since the session started or was last reset,
    /// before this check.
    pub position: u64,
    /// Checks made since the session started or was last reset, before this
    /// one.
    pub prior_checks: u64,
    /// `None` if the model followed every command exactly; otherwise why
    /// not.
    pub inexact: Option<Inexact>,
}

impl Snapshot {
    /// Number of assertion-stack commands across all levels.
    #[must_use]
    pub fn stack_len(&self) -> usize {
        self.levels.iter().map(|l| l.commands.len()).sum()
    }

    /// Appends an unambiguous encoding of everything that can affect the
    /// solver's answer: the levels, options, globals and check command. Two
    /// snapshots encode equally exactly when those parts are equal.
    ///
    /// `position` and `prior_checks` are left out on purpose. Whether answers
    /// also depend on session history is an empirical question about the
    /// solver, recorded in ADR 0004; key derivation decides what to bind.
    pub fn encode_state(&self, out: &mut Vec<u8>) {
        fn bytes(out: &mut Vec<u8>, b: &[u8]) {
            out.extend_from_slice(&(b.len() as u64).to_le_bytes());
            out.extend_from_slice(b);
        }
        fn count(out: &mut Vec<u8>, n: usize) {
            out.extend_from_slice(&(n as u64).to_le_bytes());
        }
        out.extend_from_slice(b"sail-session/state/v1");
        count(out, self.levels.len());
        for level in &self.levels {
            count(out, level.commands.len());
            for c in &level.commands {
                bytes(out, c);
            }
        }
        count(out, self.options.len());
        for (k, v) in &self.options {
            bytes(out, k);
            bytes(out, v);
        }
        count(out, self.globals.len());
        for g in &self.globals {
            bytes(out, g);
        }
        bytes(out, &self.check);
    }
}

/// What [`Session::apply`] observed about one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The command changed or queried state but asked for no answer.
    Command,
    /// The command was a satisfiability check, taken against this state.
    Check(Box<Snapshot>),
}

/// The modelled state of one solver session.
#[derive(Debug, Clone)]
pub struct Session {
    levels: Vec<Level>,
    options: BTreeMap<Vec<u8>, Vec<u8>>,
    globals: Vec<CommandBytes>,
    position: u64,
    checks: u64,
    inexact: Option<Inexact>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A fresh session: one empty base level, no options set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            levels: vec![Level::default()],
            options: BTreeMap::new(),
            globals: Vec::new(),
            position: 0,
            checks: 0,
            inexact: None,
        }
    }

    /// Number of open push levels above the base level.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.levels.len() - 1
    }

    /// Why the model stopped being exact, if it did.
    #[must_use]
    pub fn inexact(&self) -> Option<Inexact> {
        self.inexact
    }

    /// The current value of option `keyword` (for example `b":rlimit"`).
    #[must_use]
    pub fn option(&self, keyword: &[u8]) -> Option<&[u8]> {
        self.options.get(keyword).map(Vec::as_slice)
    }

    /// Whether the solver will answer `command` with a response. Errors,
    /// which the solver may report for any command, are not predicted.
    #[must_use]
    pub fn expects_output(&self, command: &[u8]) -> Output {
        if self.option(b":print-success") == Some(b"true") {
            return Output::One;
        }
        classify(command).map_or(Output::Unknown, |c| c.kind.output())
    }

    fn mark(&mut self, why: Inexact) {
        self.inexact.get_or_insert(why);
    }

    fn top(&mut self) -> &mut Level {
        if self.levels.is_empty() {
            self.levels.push(Level::default());
        }
        let last = self.levels.len() - 1;
        &mut self.levels[last]
    }

    /// Applies one complete top-level command (as split by
    /// [`sail_smtlib::split::CommandSplitter`]).
    pub fn apply(&mut self, command: &[u8]) -> Event {
        self.advance_inner(command, true)
    }

    /// Applies a command without copying the state into a check snapshot.
    ///
    /// All session state and counters advance exactly as for [`Self::apply`].
    /// Consumers that identify queries from the complete literal input prefix
    /// can use this when they need options and output classification but never
    /// inspect the returned snapshot. A later [`Self::apply`] still observes
    /// the complete state, including commands advanced this way.
    pub fn advance(&mut self, command: &[u8]) {
        self.advance_inner(command, false);
    }

    fn advance_inner(&mut self, command: &[u8], capture: bool) -> Event {
        let event = self.apply_inner(command, capture);
        // A reset starts the count again at zero; it is not part of the new
        // history.
        if !matches!(classify(command).map(|c| c.kind), Ok(CommandKind::Reset)) {
            self.position += 1;
        }
        event
    }

    fn apply_inner(&mut self, command: &[u8], capture: bool) -> Event {
        let Ok(parsed) = classify(command) else {
            self.mark(Inexact::Malformed);
            return Event::Command;
        };
        match parsed.kind {
            CommandKind::Push(n) => {
                for _ in 0..n {
                    self.levels.push(Level::default());
                }
            }
            CommandKind::Pop(n) => {
                let n = usize::try_from(n).unwrap_or(usize::MAX);
                if n > self.depth() {
                    // The solver rejects this pop; we cannot know its state.
                    self.mark(Inexact::PopUnderflow);
                } else {
                    self.levels.truncate(self.levels.len() - n);
                }
            }
            CommandKind::Assert | CommandKind::Declare | CommandKind::Define => {
                self.top().commands.push(command.into());
            }
            CommandKind::SetOption { keyword, value } => {
                self.options.insert(keyword.to_vec(), value.to_vec());
            }
            CommandKind::SetInfo | CommandKind::SetLogic => self.globals.push(command.into()),
            kind if kind.is_check() => {
                let prior_checks = self.checks;
                self.checks += 1;
                if !capture {
                    return Event::Command;
                }
                let snapshot = Snapshot {
                    levels: self.levels.clone(),
                    options: self.options.clone(),
                    globals: self.globals.clone(),
                    check: command.into(),
                    position: self.position,
                    prior_checks,
                    inexact: self.inexact,
                };
                return Event::Check(Box::new(snapshot));
            }
            CommandKind::GetInfo { .. }
            | CommandKind::Get
            | CommandKind::Echo
            | CommandKind::Exit => {}
            CommandKind::Reset => {
                // Back to a fresh process, except that the session never
                // becomes exact again once it was not.
                let inexact = self.inexact;
                *self = Self::new();
                self.inexact = inexact;
            }
            CommandKind::ResetAssertions => {
                self.levels = vec![Level::default()];
            }
            _ => {
                // Unknown commands may change solver state. Record them on
                // the stack (so they are part of the encoded state) and stop
                // claiming exactness.
                self.top().commands.push(command.into());
                self.mark(Inexact::Unmodelled);
            }
        }
        Event::Command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(commands: &[&str]) -> (Session, Vec<Snapshot>) {
        let mut session = Session::new();
        let mut checks = Vec::new();
        for c in commands {
            if let Event::Check(s) = session.apply(c.as_bytes()) {
                checks.push(*s);
            }
        }
        (session, checks)
    }

    fn texts(level: &Level) -> Vec<String> {
        level
            .commands
            .iter()
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect()
    }

    #[test]
    fn tracks_levels_options_and_checks() {
        let (session, checks) = run(&[
            "(set-option :auto_config false)",
            "(declare-const x Int)",
            "(push)",
            "(assert (> x 0))",
            "(set-option :rlimit 30000000)",
            "(check-sat)",
            "(set-option :rlimit 0)",
            "(pop)",
            "(push 2)",
            "(assert (< x 0))",
            "(check-sat)",
        ]);
        assert_eq!(checks.len(), 2);
        let first = &checks[0];
        assert_eq!(first.levels.len(), 2);
        assert_eq!(texts(&first.levels[0]), ["(declare-const x Int)"]);
        assert_eq!(texts(&first.levels[1]), ["(assert (> x 0))"]);
        assert_eq!(
            first.options.get(&b":rlimit"[..]).map(Vec::as_slice),
            Some(&b"30000000"[..])
        );
        assert_eq!((first.position, first.prior_checks), (5, 0));
        let second = &checks[1];
        assert_eq!(second.levels.len(), 3);
        assert_eq!(texts(&second.levels[2]), ["(assert (< x 0))"]);
        assert_eq!(
            second.options.get(&b":rlimit"[..]).map(Vec::as_slice),
            Some(&b"0"[..])
        );
        assert_eq!(session.depth(), 2);
        assert!(checks.iter().all(|c| c.inexact.is_none()));
    }

    #[test]
    fn pop_underflow_and_unknown_commands_make_the_session_inexact() {
        let (session, checks) = run(&["(pop)", "(check-sat)"]);
        assert_eq!(session.inexact(), Some(Inexact::PopUnderflow));
        assert_eq!(checks[0].inexact, Some(Inexact::PopUnderflow));
        let (session, _) = run(&["(apply simplify)"]);
        assert_eq!(session.inexact(), Some(Inexact::Unmodelled));
        let (session, _) = run(&["garbage"]);
        assert_eq!(session.inexact(), Some(Inexact::Malformed));
    }

    #[test]
    fn reset_clears_state_but_not_inexactness() {
        let (session, checks) = run(&[
            "(pop)",
            "(assert a)",
            "(set-option :x 1)",
            "(reset)",
            "(check-sat)",
        ]);
        assert_eq!(checks[0].stack_len(), 0);
        assert!(checks[0].options.is_empty());
        assert_eq!(checks[0].position, 0);
        assert_eq!(session.inexact(), Some(Inexact::PopUnderflow));
    }

    #[test]
    fn print_success_changes_output_expectations() {
        let mut session = Session::new();
        assert_eq!(session.expects_output(b"(assert true)"), Output::None);
        session.apply(b"(set-option :print-success true)");
        assert_eq!(session.expects_output(b"(assert true)"), Output::One);
    }

    #[test]
    fn advancing_without_snapshots_preserves_every_subsequent_observation() {
        let commands = [
            "(set-logic ALL)",
            "(set-info :source |literal input|)",
            "(declare-const x Int)",
            "(define-fun f ((y Int)) Int (+ x y))",
            "(assert (> x 0))",
            "(push 2)",
            "(set-option :rlimit 1234)",
            "(check-sat)",
            "(check-sat-assuming ((> x 1)))",
            "(check-sat-using smt)",
            "(get-info :all-statistics)",
            "(set-option :print-success true)",
            "(pop)",
            "(set-option :print-success false)",
            "(reset-assertions)",
            "(check-sat)",
            "(reset)",
            "(check-sat)",
            "(pop)",
            "(reset)",
            "(assert true)",
            "(unknown-command)",
            "malformed",
            "(check-sat)",
            "(exit)",
        ];
        let mut capturing = Session::new();
        let mut advancing = Session::new();
        for command in commands {
            let command = command.as_bytes();
            assert_eq!(
                capturing.expects_output(command),
                advancing.expects_output(command)
            );
            capturing.apply(command);
            advancing.advance(command);
            assert_eq!(capturing.depth(), advancing.depth());
            assert_eq!(capturing.inexact(), advancing.inexact());
            assert_eq!(capturing.option(b":rlimit"), advancing.option(b":rlimit"));
            assert_eq!(
                capturing.clone().apply(b"(check-sat)"),
                advancing.clone().apply(b"(check-sat)"),
                "all stack bytes, options, globals and counters after {command:?}"
            );
        }
    }
}
