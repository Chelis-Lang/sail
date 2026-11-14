//! Exact session prefixes for diagnostic solver replay.
//!
//! A check must be replayed with every preceding command, including earlier
//! checks, statistics requests and stack operations. Extracting only its live
//! assertions discards solver history and can change resource-limited answers.
//! This module never executes a solver, changes a verdict or grants reuse.

use sail_smtlib::command::{CommandKind, classify};
use sail_smtlib::split::{CommandSplitter, Item, ItemKind};

use crate::PortfolioError;
use crate::transcript::{Class, classify_command};

/// Maximum original session size admitted for an in-memory diagnostic replay.
pub const MAX_SESSION_BYTES: usize = 64 * 1024 * 1024;

/// An unchanged original byte prefix ending immediately after a selected check.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Prefix<'a> {
    /// Every original byte from offset zero through the selected check.
    pub input: &'a [u8],
    /// Zero-based check index within the original process session.
    pub check: usize,
    /// Commands in the prefix, including all earlier checks.
    pub commands: usize,
}

/// Select an exact prefix through the zero-based `check` of a solver session.
///
/// Comments, whitespace, earlier checks and all original command bytes are
/// retained. Commands after the selected check are never included, even a later
/// statistics request or check. Unclassified, structurally incomplete and exit
/// commands before selection refuse. Truncated later bytes do not invalidate
/// the usable prefix. This is framing and classification, not SMT type checking.
/// No solver options, statistics requests, exit command or newline are appended.
/// Callers must separately bind the original solver and mark replay as diagnostic.
///
/// # Errors
/// `SAIL5102` if the session exceeds the bound, the check is absent, a preceding
/// command cannot be classified, or a splitter span does not match its source.
pub fn prefix(input: &[u8], check: usize) -> Result<Prefix<'_>, PortfolioError> {
    if input.len() > MAX_SESSION_BYTES {
        return Err(PortfolioError::Options(
            "profile session exceeds the 64 MiB diagnostic bound".to_owned(),
        ));
    }
    let mut splitter = CommandSplitter::new();
    let mut items = Vec::new();
    let mut selection = Selection {
        target: check,
        seen: 0,
        commands: 0,
    };
    for chunk in input.chunks(4096) {
        splitter.push(chunk, &mut items);
        for item in items.drain(..) {
            if let Some(selected) = selection.accept(input, &item)? {
                return Ok(selected);
            }
        }
    }
    if let Some(item) = splitter.finish()
        && let Some(selected) = selection.accept(input, &item)?
    {
        return Ok(selected);
    }
    Err(PortfolioError::Options(format!(
        "profile check {check} is absent from the original session"
    )))
}

struct Selection {
    target: usize,
    seen: usize,
    commands: usize,
}

impl Selection {
    fn accept<'a>(
        &mut self,
        input: &'a [u8],
        item: &Item,
    ) -> Result<Option<Prefix<'a>>, PortfolioError> {
        if item.kind != ItemKind::List || classify_command(&item.bytes) == Class::Unknown {
            return Err(PortfolioError::Options(
                "profile prefix contains an unclassified or incomplete command".to_owned(),
            ));
        }
        if matches!(classify(&item.bytes).map(|c| c.kind), Ok(CommandKind::Exit)) {
            return Err(PortfolioError::Options(
                "profile selected check follows an exit command".to_owned(),
            ));
        }
        self.commands += 1;
        if classify_command(&item.bytes) != Class::Check {
            return Ok(None);
        }
        if self.seen != self.target {
            self.seen += 1;
            return Ok(None);
        }
        let start = usize::try_from(item.offset)
            .map_err(|_| PortfolioError::Options("profile command offset overflow".to_owned()))?;
        let end = start
            .checked_add(item.bytes.len())
            .ok_or_else(|| PortfolioError::Options("profile command end overflow".to_owned()))?;
        if input.get(start..end) != Some(item.bytes.as_slice()) {
            return Err(PortfolioError::Options(
                "profile command span differs from original bytes".to_owned(),
            ));
        }
        let bytes = input.get(..end).ok_or_else(|| {
            PortfolioError::Options("profile prefix exceeds original input".to_owned())
        })?;
        Ok(Some(Prefix {
            input: bytes,
            check: self.target,
            commands: self.commands,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_checks_and_statistics_never_enter_the_prefix() -> Result<(), PortfolioError> {
        let first = b"; keep these exact bytes\r\n(set-option :rlimit 10)\n(push 1)\n(check-sat)";
        let between = b"\n(get-info :all-statistics)\n(pop 1)\n(set-option :rlimit 20)\n(check-sat-assuming (true))";
        let mut input = first.to_vec();
        input.extend_from_slice(between);
        input.extend_from_slice(b"\n(get-info :all-statistics)\n(check-sat)\n(exit)");
        let one = prefix(&input, 0)?;
        assert_eq!(one.input, first);
        assert_eq!(one.commands, 3);
        let two = prefix(&input, 1)?;
        assert_eq!(two.input, &input[..first.len() + between.len()]);
        assert_eq!(two.commands, 7);
        assert_eq!(two.check, 1);
        assert_eq!(two.input.as_ptr(), input.as_ptr());
        Ok(())
    }

    #[test]
    fn quoted_and_commented_checks_do_not_change_selection() -> Result<(), PortfolioError> {
        let input = b"(echo \"(check-sat)\")\n; (check-sat)\n(declare-const |(check-sat)| Bool)\n(check-sat)\n(check-sat)";
        let selected = prefix(input, 0)?;
        assert_eq!(selected.commands, 3);
        assert_eq!(
            selected.input,
            &input[..input.len() - b"\n(check-sat)".len()]
        );
        Ok(())
    }

    #[test]
    fn malformed_history_refuses_but_later_truncation_does_not() -> Result<(), PortfolioError> {
        for input in [
            b"junk (check-sat)".as_slice(),
            b"(unsupported) (check-sat)",
            b"(assert true",
            b"(exit) (check-sat)",
        ] {
            assert_eq!(prefix(input, 0).err().map(|e| e.code()), Some("SAIL5102"));
        }
        let good = b"(check-sat)\n(assert ";
        assert_eq!(prefix(good, 0)?.input, b"(check-sat)");
        assert!(prefix(good, 1).is_err());
        assert!(prefix(b"; only a comment", 0).is_err());
        assert!(prefix(b"(check-sat)", usize::MAX).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod boundaries {
    use super::*;

    #[test]
    fn chunk_crossing_keeps_the_original_prefix() -> Result<(), PortfolioError> {
        let mut input = b";".to_vec();
        input.extend(std::iter::repeat_n(b'x', 8193));
        input.extend_from_slice(b"\n(assert true)\r\n(check-sat)\n(unclosed");
        let selected = prefix(&input, 0)?;
        assert_eq!(selected.input, &input[..input.len() - b"\n(unclosed".len()]);
        assert_eq!(selected.commands, 2);
        assert_eq!(selected.input.as_ptr(), input.as_ptr());
        Ok(())
    }

    #[test]
    fn oversize_sessions_refuse_before_splitting() {
        let mut input = vec![b' '; MAX_SESSION_BYTES + 1];
        input[..11].copy_from_slice(b"(check-sat)");
        assert!(prefix(&input, 0).is_err());
    }
}
