//! Splitting raw transcripts into commands and replies, and pairing them.
//!
//! A transcript is the exact input a solver received (`<id>.in.smt2`) and
//! the exact output it produced (`<id>.out`). [`Transcript::new`] splits
//! both and pairs each output-producing command with its reply, the way the
//! shim's tap does, but resynchronising at every `echo`: Verus ends each
//! batch with `(echo "<<DONE>>")`, so a reply line equal to the echoed text
//! closes the batch. Within a batch, replies are assigned to the
//! output-producing commands in order. A batch whose reply count does not
//! match (a solver error, or a command whose output cannot be predicted) is
//! left unpaired, and pairing resumes at the next batch.
//!
//! Nothing here normalizes bytes. Commands keep their exact text, and
//! replies are the exact items [`ResponseSplitter`] produces.

use sail_smtlib::command::{CommandKind, Output, classify};
use sail_smtlib::lex::{Lexer, TokenKind};
use sail_smtlib::split::{CommandSplitter, ItemKind, ResponseSplitter};

/// What a command is, as far as the analyses here are concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// `(set-option …)`.
    SetOption,
    /// A satisfiability check (`check-sat` and its variants).
    Check,
    /// `(get-info :all-statistics)`.
    Statistics,
    /// `(get-info :version)`.
    Version,
    /// `(echo "…")`; the reply line the solver prints for it.
    Echo(Vec<u8>),
    /// `(push …)`.
    Push,
    /// `(pop …)`.
    Pop,
    /// `assert`, `declare-*`, `define-*`.
    Stack,
    /// Any other command with exactly one reply (`get-model`,
    /// `get-info :reason-unknown`, …). These are the follow-ups.
    Query,
    /// Any other command without a reply.
    Silent,
    /// A command whose reply cannot be predicted, or that does not parse.
    Unknown,
}

/// One top-level command with its exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    /// The command's exact bytes.
    pub bytes: Vec<u8>,
    /// Byte offset of the command in the input stream.
    pub offset: u64,
    /// What the command is.
    pub class: Class,
}

impl Cmd {
    /// Whether the solver replies to this command (when it succeeds).
    #[must_use]
    pub fn output(&self) -> Output {
        match self.class {
            Class::Check | Class::Statistics | Class::Version | Class::Echo(_) | Class::Query => {
                Output::One
            }
            Class::Unknown => Output::Unknown,
            _ => Output::None,
        }
    }
}

/// The text an `(echo "…")` command makes the solver print: the string's
/// contents with `""` unescaped. `None` if the command has no string
/// argument.
#[must_use]
pub fn echo_text(command: &[u8]) -> Option<Vec<u8>> {
    let token = Lexer::new(command)
        .filter_map(Result::ok)
        .find(|t| t.kind == TokenKind::String)?;
    let quoted = token.text(command);
    let inner = quoted.get(1..quoted.len().checked_sub(1)?)?;
    let mut out = Vec::with_capacity(inner.len());
    let mut i = 0;
    while let Some(&b) = inner.get(i) {
        out.push(b);
        i += if b == b'"' && inner.get(i + 1) == Some(&b'"') {
            2
        } else {
            1
        };
    }
    Some(out)
}

/// Classifies one command's bytes.
#[must_use]
pub fn classify_command(bytes: &[u8]) -> Class {
    let Ok(command) = classify(bytes) else {
        return Class::Unknown;
    };
    match command.kind {
        CommandKind::SetOption { .. } => Class::SetOption,
        kind if kind.is_check() => Class::Check,
        CommandKind::GetInfo {
            keyword: b":all-statistics",
        } => Class::Statistics,
        CommandKind::GetInfo {
            keyword: b":version",
        } => Class::Version,
        CommandKind::Echo => echo_text(bytes).map_or(Class::Unknown, Class::Echo),
        CommandKind::Push(_) => Class::Push,
        CommandKind::Pop(_) => Class::Pop,
        CommandKind::Assert | CommandKind::Declare | CommandKind::Define => Class::Stack,
        kind => match kind.output() {
            Output::One => Class::Query,
            Output::None => Class::Silent,
            Output::Unknown => Class::Unknown,
        },
    }
}

/// Splits solver input into classified commands. A trailing incomplete
/// command, or stray bytes, become [`Class::Unknown`] commands.
#[must_use]
pub fn split_commands(input: &[u8]) -> Vec<Cmd> {
    let mut items = Vec::new();
    let mut splitter = CommandSplitter::new();
    splitter.push(input, &mut items);
    items.extend(splitter.finish());
    items
        .into_iter()
        .map(|item| {
            let class = if item.kind == ItemKind::List {
                classify_command(&item.bytes)
            } else {
                Class::Unknown
            };
            Cmd {
                bytes: item.bytes,
                offset: item.offset,
                class,
            }
        })
        .collect()
}

/// Splits solver output into replies, including any incomplete trailing
/// reply.
#[must_use]
pub fn split_replies(output: &[u8]) -> Vec<Vec<u8>> {
    let mut items = Vec::new();
    let mut splitter = ResponseSplitter::new();
    splitter.push(output, &mut items);
    items.extend(splitter.finish());
    items.into_iter().map(|i| i.bytes).collect()
}

/// A transcript split into commands, with each command's reply where it
/// could be paired.
#[derive(Debug, Clone, Default)]
pub struct Transcript {
    /// Every command, in order.
    pub commands: Vec<Cmd>,
    /// `replies[i]` is the reply to `commands[i]`, if it has one and it was
    /// paired.
    pub replies: Vec<Option<Vec<u8>>>,
    /// Batches whose replies could not be paired.
    pub unpaired_batches: usize,
    /// Solver `(error …)` replies seen anywhere in the output.
    pub errors: Vec<String>,
}

impl Transcript {
    /// Splits and pairs one transcript.
    #[must_use]
    pub fn new(input: &[u8], output: &[u8]) -> Self {
        Self::from_parts(split_commands(input), &split_replies(output))
    }

    /// Pairs already split commands and replies.
    #[must_use]
    pub fn from_parts(commands: Vec<Cmd>, replies: &[Vec<u8>]) -> Self {
        let mut paired: Vec<Option<Vec<u8>>> = vec![None; commands.len()];
        let errors = replies
            .iter()
            .filter(|r| r.starts_with(b"(error"))
            .map(|r| String::from_utf8_lossy(r).into_owned())
            .collect();
        let mut unpaired_batches = 0;
        let mut cursor = 0usize;
        let mut start = 0usize;
        while start < commands.len() {
            // The batch is everything up to and including the next echo.
            let end = commands
                .iter()
                .skip(start)
                .position(|c| matches!(c.class, Class::Echo(_)))
                .map_or(commands.len(), |p| start + p + 1);
            let batch = commands.get(start..end).unwrap_or_default();
            let (body, echo) = match batch.last().map(|c| &c.class) {
                Some(Class::Echo(text)) => {
                    (batch.get(..batch.len() - 1).unwrap_or_default(), Some(text))
                }
                _ => (batch, None),
            };
            // Replies for the body run up to the echo's reply line, or to
            // the end of the output.
            let rest = replies.get(cursor..).unwrap_or_default();
            let (body_replies, next_cursor, echo_reply) = match echo {
                Some(text) => match rest.iter().position(|r| r == text) {
                    Some(p) => (
                        rest.get(..p).unwrap_or_default(),
                        cursor + p + 1,
                        Some(text),
                    ),
                    None => (rest, replies.len(), None),
                },
                None => (rest, replies.len(), None),
            };
            let expected: Vec<usize> = (start..start + body.len())
                .filter(|&i| commands.get(i).map(Cmd::output) == Some(Output::One))
                .collect();
            let ambiguous = body.iter().any(|c| c.output() == Output::Unknown);
            let complete = echo.is_none() || echo_reply.is_some();
            if !ambiguous && complete && expected.len() == body_replies.len() {
                for (&i, reply) in expected.iter().zip(body_replies) {
                    if let Some(slot) = paired.get_mut(i) {
                        *slot = Some(reply.clone());
                    }
                }
                if let (Some(text), Some(slot)) = (echo_reply, paired.get_mut(end - 1)) {
                    *slot = Some(text.clone());
                }
            } else {
                unpaired_batches += 1;
            }
            cursor = next_cursor;
            start = end;
        }
        Self {
            commands,
            replies: paired,
            unpaired_batches,
            errors,
        }
    }

    /// The reply to command `i`, if paired.
    #[must_use]
    pub fn reply(&self, i: usize) -> Option<&[u8]> {
        self.replies.get(i).and_then(|r| r.as_deref())
    }

    /// Per-check observations, in check order.
    #[must_use]
    pub fn checks(&self) -> Vec<CheckObservation> {
        let mut out = Vec::new();
        let mut last_stats: Option<u64> = None;
        for (i, cmd) in self.commands.iter().enumerate() {
            match cmd.class {
                Class::Statistics => {
                    last_stats = self.reply(i).and_then(crate::stats::rlimit_count);
                }
                Class::Check => {
                    out.push(self.observe_check(i, last_stats));
                    // A later check with no statistics in between has no
                    // delta of its own: the span would include this one.
                    last_stats = None;
                }
                _ => {}
            }
        }
        out
    }

    fn observe_check(&self, index: usize, before: Option<u64>) -> CheckObservation {
        let mut followups = Vec::new();
        let mut after = None;
        for (j, cmd) in self.commands.iter().enumerate().skip(index + 1) {
            match cmd.class {
                Class::Check => break,
                Class::Statistics => {
                    after = Some(self.reply(j).and_then(crate::stats::rlimit_count));
                    break;
                }
                Class::Query => followups.push(Followup {
                    command: cmd.bytes.clone(),
                    reply: self.reply(j).map(<[u8]>::to_vec),
                }),
                _ => {}
            }
        }
        let rlimit_delta = match (before, after.flatten()) {
            (Some(b), Some(a)) => a.checked_sub(b),
            _ => None,
        };
        CheckObservation {
            command_index: index,
            answer: self.reply(index).map(<[u8]>::to_vec),
            rlimit_before: before,
            rlimit_delta,
            followups,
        }
    }
}

/// A follow-up command after a check (`get-model`, `get-info
/// :reason-unknown`, …) and its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followup {
    /// The command's exact bytes.
    pub command: Vec<u8>,
    /// The paired reply, if any.
    pub reply: Option<Vec<u8>>,
}

/// What one check looked like in a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckObservation {
    /// Index of the check command in [`Transcript::commands`].
    pub command_index: usize,
    /// The reply (`sat`, `unsat`, `unknown`), if paired.
    pub answer: Option<Vec<u8>>,
    /// `rlimit-count` from the last statistics reply before the check, if
    /// no other check came after that reply.
    pub rlimit_before: Option<u64>,
    /// `rlimit-count` of the first statistics reply after the check (before
    /// the next check) minus [`Self::rlimit_before`].
    pub rlimit_delta: Option<u64>,
    /// Follow-up queries between the check and the next statistics request
    /// or check, excluding `echo`, statistics and version requests.
    pub followups: Vec<Followup>,
}

impl CheckObservation {
    /// The `:reason-unknown` reply among the follow-ups, if any.
    #[must_use]
    pub fn reason_unknown(&self) -> Option<&[u8]> {
        self.followups
            .iter()
            .find(|f| f.command.windows(15).any(|w| w == b":reason-unknown"))
            .and_then(|f| f.reply.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &[u8] = b"(set-option :auto_config false)\n\
(declare-const x Int)\n\
(push)\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(assert (> x 0))\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(get-info :version)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 100)\n(check-sat)\n(echo \"<<DONE>>\")\n\
(set-option :rlimit 0)\n(get-model)\n(echo \"<<DONE>>\")\n\
(assert (not l))\n(get-info :all-statistics)\n(echo \"<<DONE>>\")\n\
(pop)\n";

    const OUTPUT: &[u8] = b"(:memory 1.0\n :rlimit-count 10)\n<<DONE>>\n\
(:memory 1.0\n :rlimit-count 12)\n<<DONE>>\n\
(:version \"4.16.0\")\n<<DONE>>\n\
sat\n<<DONE>>\n\
(\n  (define-fun x () Int\n    1)\n)\n<<DONE>>\n\
(:memory 2.0\n :rlimit-count 40)\n<<DONE>>\n";

    #[test]
    fn classifies_and_unescapes_echo() {
        assert_eq!(echo_text(b"(echo \"a\"\"b\")"), Some(b"a\"b".to_vec()));
        assert_eq!(echo_text(b"(echo)"), None);
        assert_eq!(classify_command(b"(get-model)"), Class::Query);
        assert_eq!(
            classify_command(b"(get-info :reason-unknown)"),
            Class::Query
        );
        assert_eq!(classify_command(b"(get-info :version)"), Class::Version);
        assert_eq!(classify_command(b"(eval x)"), Class::Unknown);
        assert_eq!(classify_command(b"(push)"), Class::Push);
        assert_eq!(classify_command(b"(exit)"), Class::Silent);
    }

    #[test]
    fn pairs_verus_batches_and_observes_checks() {
        let t = Transcript::new(INPUT, OUTPUT);
        assert_eq!(t.unpaired_batches, 0);
        assert_eq!(t.errors, Vec::<String>::new());
        let checks = t.checks();
        assert_eq!(checks.len(), 1);
        let c = &checks[0];
        assert_eq!(c.answer.as_deref(), Some(&b"sat"[..]));
        assert_eq!(c.rlimit_before, Some(12));
        assert_eq!(c.rlimit_delta, Some(28));
        assert_eq!(c.followups.len(), 1);
        assert_eq!(c.followups[0].command, b"(get-model)");
        assert_eq!(
            c.followups[0].reply.as_deref(),
            Some(&b"(\n  (define-fun x () Int\n    1)\n)"[..])
        );
    }

    #[test]
    fn an_error_unpairs_only_its_batch() {
        let output = b"(:memory 1.0\n :rlimit-count 10)\n<<DONE>>\n\
(error \"line 7 column 1: boom\")\n(:memory 1.0\n :rlimit-count 12)\n<<DONE>>\n\
(:version \"4.16.0\")\n<<DONE>>\n\
unsat\n<<DONE>>\n";
        let t = Transcript::new(INPUT, output);
        assert_eq!(t.unpaired_batches, 3, "error batch, plus two missing");
        assert_eq!(t.errors.len(), 1);
        let checks = t.checks();
        assert_eq!(checks[0].answer.as_deref(), Some(&b"unsat"[..]));
        // The statistics before the check were in the unpaired batch.
        assert_eq!(checks[0].rlimit_before, None);
        assert_eq!(checks[0].rlimit_delta, None);
    }

    #[test]
    fn pairs_without_echo_markers() {
        let input = b"(get-info :all-statistics)(check-sat)(get-info :reason-unknown)(get-info :all-statistics)";
        let output =
            b"(:rlimit-count 5)\nunknown\n(:reason-unknown \"canceled\")\n(:rlimit-count 105)\n";
        let t = Transcript::new(input, output);
        let checks = t.checks();
        assert_eq!(checks[0].answer.as_deref(), Some(&b"unknown"[..]));
        assert_eq!(checks[0].rlimit_delta, Some(100));
        assert_eq!(
            checks[0].reason_unknown(),
            Some(&b"(:reason-unknown \"canceled\")"[..])
        );
        // A missing reply leaves the whole (single) batch unpaired.
        let t = Transcript::new(input, b"unknown\n");
        assert_eq!(t.unpaired_batches, 1);
        assert_eq!(t.checks()[0].answer, None);
    }

    #[test]
    fn verus_toy_fixture_pairs_completely() {
        let input = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.in.smt2");
        let output = include_bytes!("../../sail-smtlib/tests/fixtures/verus-toy.out.txt");
        let t = Transcript::new(input, output);
        assert_eq!(t.unpaired_batches, 0);
        let checks = t.checks();
        assert_eq!(checks.len(), 11);
        let answers: Vec<_> = checks
            .iter()
            .map(|c| String::from_utf8_lossy(c.answer.as_deref().unwrap_or_default()).into_owned())
            .collect();
        assert_eq!(answers.iter().filter(|a| *a == "unknown").count(), 3);
        assert_eq!(answers.iter().filter(|a| *a == "unsat").count(), 8);
        // Two re-checks follow an earlier check with no statistics request
        // in between, so they have no delta of their own.
        let deltas: Vec<Option<u64>> = checks.iter().map(|c| c.rlimit_delta).collect();
        assert_eq!(
            deltas.iter().filter(|d| d.is_none()).count(),
            2,
            "{deltas:?}"
        );
        assert_eq!(deltas[0], Some(782));
    }
}
