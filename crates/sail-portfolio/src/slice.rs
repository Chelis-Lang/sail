//! Spike prototype: slicing one check out of a captured session.
//!
//! The slice is the session as the solver's assertion stack saw it at that
//! check, without the earlier checks: every command of a scope still open
//! at the check, every `set-option` (options are global, not scoped), then
//! the check with its follow-ups up to the statistics request after it.
//! Popped scopes and earlier checks, queries, echoes and statistics are
//! dropped, so the solver reaches the check with the same assertions but
//! not the same history. That is why a sliced replay is a measurement to be
//! shown beside the recorded answer, never a substitute for it.

use sail_smtlib::command::{CommandKind, classify};

use crate::transcript::{Class, Cmd};

const STATISTICS: &[u8] = b"(get-info :all-statistics)";

/// One check sliced out of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    /// The input to run.
    pub input: Vec<u8>,
    /// Commands kept from before the check.
    pub kept: usize,
    /// Commands before the check that were dropped.
    pub dropped: usize,
}

/// Slices the check at `commands[check]`. `None` if that command is not a
/// check, or if anything before it cannot be classified (the stack can then
/// not be followed).
#[must_use]
pub fn slice_check(commands: &[Cmd], check: usize) -> Option<Slice> {
    if commands.get(check).map(|c| &c.class) != Some(&Class::Check) {
        return None;
    }
    // (command index, scope depth it belongs to); options carry depth 0.
    let mut live: Vec<(usize, u64)> = Vec::new();
    let mut depth = 0u64;
    for (i, c) in commands.iter().enumerate().take(check) {
        match c.class {
            Class::SetOption => live.push((i, 0)),
            Class::Stack | Class::Silent => live.push((i, depth)),
            Class::Push => {
                let Ok(CommandKind::Push(n)) = classify(&c.bytes).map(|k| k.kind) else {
                    return None;
                };
                // The scope's own `push` belongs to the scope it opens.
                depth = depth.checked_add(n)?;
                live.push((i, depth));
            }
            Class::Pop => {
                let Ok(CommandKind::Pop(n)) = classify(&c.bytes).map(|k| k.kind) else {
                    return None;
                };
                depth = depth.checked_sub(n)?;
                live.retain(|&(_, d)| d <= depth);
            }
            Class::Check | Class::Statistics | Class::Version | Class::Echo(_) | Class::Query => {}
            Class::Unknown => return None,
        }
    }
    let mut input = Vec::new();
    let mut emit = |bytes: &[u8]| {
        input.extend_from_slice(bytes);
        input.push(b'\n');
    };
    for &(i, _) in &live {
        emit(&commands.get(i)?.bytes);
    }
    emit(STATISTICS);
    emit(&commands.get(check)?.bytes);
    let mut closed = false;
    for c in commands.iter().skip(check + 1) {
        match c.class {
            Class::Check | Class::Pop | Class::Push | Class::Unknown => break,
            Class::Statistics => {
                emit(&c.bytes);
                closed = true;
                break;
            }
            _ => emit(&c.bytes),
        }
    }
    if !closed {
        emit(STATISTICS);
    }
    Some(Slice {
        input,
        kept: live.len(),
        dropped: check - live.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::split_commands;

    #[test]
    fn keeps_open_scopes_and_all_options() {
        let session = b"(set-option :auto_config false)\n(declare-fun f (Int) Int)\n(push)\n\
            (assert (> (f 1) 0))\n(set-option :rlimit 5)\n(check-sat)\n(set-option :rlimit 0)\n\
            (get-info :all-statistics)\n(pop)\n(push)\n(assert (< (f 2) 0))\n(push 2)\n(pop 2)\n\
            (set-option :rlimit 7)\n(check-sat)\n(get-info :reason-unknown)\n\
            (set-option :rlimit 0)\n(get-info :all-statistics)\n(pop)\n";
        let commands = split_commands(session);
        let checks: Vec<usize> = commands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.class == Class::Check)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(checks.len(), 2);
        let second = slice_check(&commands, checks[1]);
        let Some(second) = second else {
            unreachable!("the second check slices")
        };
        assert_eq!(
            String::from_utf8_lossy(&second.input),
            "(set-option :auto_config false)\n(declare-fun f (Int) Int)\n\
             (set-option :rlimit 5)\n(set-option :rlimit 0)\n(push)\n(assert (< (f 2) 0))\n\
             (set-option :rlimit 7)\n(get-info :all-statistics)\n(check-sat)\n\
             (get-info :reason-unknown)\n(set-option :rlimit 0)\n(get-info :all-statistics)\n"
        );
        assert_eq!(slice_check(&commands, 0), None);
        let first = slice_check(&commands, checks[0]).map(|s| s.kept);
        assert_eq!(first, Some(5));
    }
}
