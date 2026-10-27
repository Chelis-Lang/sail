//! Shallow classification of top-level SMT-LIB commands.
//!
//! [`classify`] reads only as many tokens as it needs: the command name and,
//! for a few commands, their small arguments. It never walks the body of an
//! `assert`, so it is cheap on the very large assertions Verus sends.

use crate::Error;

pub use sail_acceptance::query_command::{Command, CommandKind, Output, expects_output};

/// Classifies one top-level command, given exactly its bytes (as produced by
/// [`crate::split::CommandSplitter`]).
///
/// # Errors
///
/// [`crate::ErrorCode::InvalidToken`] if the text does not start with `(` and a
/// symbol, or a lexical error in the tokens it reads.
pub fn classify(text: &[u8]) -> Result<Command<'_>, Error> {
    sail_acceptance::query_command::classify(text).map_err(crate::lex::native_failure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    fn kind(src: &str) -> CommandKind<'_> {
        match classify(src.as_bytes()) {
            Ok(c) => c.kind,
            Err(e) => unreachable!("{src}: {e}"),
        }
    }

    #[test]
    fn shared_borrowed_head_preserves_shallow_and_error_behavior() {
        assert_eq!(kind("(assert"), CommandKind::Assert);
        assert_eq!(kind(";c\r\n(check-sat)"), CommandKind::CheckSat);
        for (raw, code, offset) in [
            (b"".as_slice(), ErrorCode::Empty, 0),
            (b"(".as_slice(), ErrorCode::UnclosedList, 0),
            (b"(42)".as_slice(), ErrorCode::InvalidToken, 1),
            (b";c\r\n(push x)".as_slice(), ErrorCode::InvalidToken, 10),
        ] {
            assert_eq!(classify(raw), Err(Error::new(code, offset)));
        }
    }

    #[test]
    fn shared_classifier_preserves_borrowed_values_and_shallow_failures() {
        assert!(sail_acceptance::query_command::classifier_constructive_control());
        assert!(sail_acceptance::query_command::value_constructive_control());
        assert!(sail_acceptance::query_command::expectation_constructive_control());
        for (raw, value) in [
            (
                b"(set-option :x (a (b)) )".as_slice(),
                b"(a (b))".as_slice(),
            ),
            (
                b"(set-option :x \"a\"\"b\")".as_slice(),
                b"\"a\"\"b\"".as_slice(),
            ),
            (b"(set-option :x |a)b|)".as_slice(), b"|a)b|".as_slice()),
            (
                b"(set-option :x ;c\r\n(a)) trailing".as_slice(),
                b"(a)".as_slice(),
            ),
            (b"(set-option :x 1".as_slice(), b"1".as_slice()),
        ] {
            let command = classify(raw).unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(
                command.kind,
                CommandKind::SetOption {
                    keyword: b":x",
                    value
                }
            );
            let CommandKind::SetOption { value: actual, .. } = command.kind else {
                unreachable!()
            };
            assert!(std::ptr::eq(
                actual.as_ptr(),
                raw[value_start(raw)..].as_ptr()
            ));
        }
        assert_eq!(
            kind("(push 18446744073709551615)"),
            CommandKind::Push(u64::MAX)
        );
        for raw in [
            b"(push 18446744073709551616)".as_slice(),
            b"(push +1)".as_slice(),
        ] {
            assert_eq!(classify(raw), Err(Error::new(ErrorCode::InvalidToken, 6)));
        }
        assert_eq!(
            kind("(get-info :x"),
            CommandKind::GetInfo { keyword: b":x" }
        );
        assert_eq!(expects_output(b"malformed", Some(b"true")), Output::One);
        assert_eq!(
            expects_output(b"malformed", Some(b"false")),
            Output::Unknown
        );
    }

    fn value_start(raw: &[u8]) -> usize {
        let mut lexer = crate::lex::Lexer::new(raw);
        for _ in 0..3 {
            assert!(lexer.next().is_some());
        }
        match lexer.next() {
            Some(Ok(t)) => t.start,
            other => unreachable!("{other:?}"),
        }
    }

    #[test]
    fn classifies_verus_commands() {
        assert_eq!(kind("(push)"), CommandKind::Push(1));
        assert_eq!(kind("(pop 3)"), CommandKind::Pop(3));
        assert_eq!(kind("(assert (=> a b))"), CommandKind::Assert);
        assert_eq!(kind("(declare-fun succ (Fuel) Fuel)"), CommandKind::Declare);
        assert_eq!(
            kind("(set-option :rlimit 30000000)"),
            CommandKind::SetOption {
                keyword: b":rlimit",
                value: b"30000000"
            }
        );
        assert_eq!(
            kind("(set-option :x (a (b)) )"),
            CommandKind::SetOption {
                keyword: b":x",
                value: b"(a (b))"
            }
        );
        assert_eq!(
            kind("(get-info :all-statistics)"),
            CommandKind::GetInfo {
                keyword: b":all-statistics"
            }
        );
        assert_eq!(kind("(get-model)"), CommandKind::Get);
        assert_eq!(kind("(echo \"<<DONE>>\")"), CommandKind::Echo);
        assert_eq!(kind("(check-sat)"), CommandKind::CheckSat);
        assert_eq!(kind("(eval x)"), CommandKind::Other);
    }

    #[test]
    fn output_expectations() {
        assert_eq!(kind("(check-sat)").output(), Output::One);
        assert_eq!(kind("(check-sat-using smt)").output(), Output::One);
        assert_eq!(kind("(check-sat-assuming (a))").output(), Output::One);
        assert_eq!(kind("(assert true)").output(), Output::None);
        assert_eq!(kind("(simplify x)").output(), Output::Unknown);
    }

    #[test]
    fn rejects_malformed_commands() {
        for src in [
            "x",
            "()",
            "(push x)",
            "(set-option rlimit 1)",
            "(set-option :rlimit)",
            "(get-info)",
            "(pop",
        ] {
            assert!(classify(src.as_bytes()).is_err(), "{src}");
        }
    }
}
