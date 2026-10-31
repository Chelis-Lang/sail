//! ADR 0093: two named order exceptions in the pinned stock test's debug dump.
//!
//! This parser preserves original spelling and whitespace. Only function-detail
//! entries and the two set-valued proof-note arrays may move. Unknown syntax,
//! duplicate keys/notes, excessive input, or another producer fail closed.

use std::collections::BTreeSet;

pub(super) const STOCK: &str = "7d829946231d6413a398f6e4160cdfe951261dad";
const HEADER: &str = "[rust_verify_test/tests/output_json.rs:277:5] json = ";
const LIMIT: usize = 4 * 1024 * 1024;

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    nodes: usize,
    stock: bool,
    details: bool,
}

impl Parser<'_> {
    fn take(&mut self, token: &str) -> Option<()> {
        self.input
            .get(self.pos..)?
            .starts_with(token)
            .then_some(())?;
        self.pos += token.len();
        Some(())
    }

    fn space(&mut self) {
        while self
            .input
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
    }

    fn string(&mut self) -> Option<String> {
        let start = self.pos;
        self.take("\"")?;
        loop {
            match self.input.as_bytes().get(self.pos)? {
                b'"' => {
                    self.pos += 1;
                    return serde_json::from_str(self.input.get(start..self.pos)?).ok();
                }
                b'\\' => self.pos += 2,
                _ => self.pos += 1,
            }
        }
    }

    fn value(&mut self, path: &[String]) -> Option<(String, Option<String>)> {
        self.nodes += 1;
        if path.len() > 64 || self.nodes > 100_000 {
            return None;
        }
        let start = self.pos;
        if self.input.get(self.pos..)?.starts_with("Object {") {
            let text = self.container(path, true)?;
            return Some((text, None));
        }
        if self.input.get(self.pos..)?.starts_with("Array [") {
            let text = self.container(path, false)?;
            return Some((text, None));
        }
        let mut string = None;
        if self.input.get(self.pos..)?.starts_with("String(") {
            self.take("String(")?;
            string = Some(self.string()?);
            self.take(")")?;
            if path == ["verus", "commit"] {
                self.stock = string.as_deref() == Some(STOCK);
            }
        } else if self.input.get(self.pos..)?.starts_with("Number(") {
            self.take("Number(")?;
            let number = self.pos;
            while self
                .input
                .as_bytes()
                .get(self.pos)
                .is_some_and(|c| *c != b')')
            {
                self.pos += 1;
            }
            self.input
                .get(number..self.pos)?
                .parse::<serde_json::Number>()
                .ok()?;
            self.take(")")?;
        } else if self.input.get(self.pos..)?.starts_with("Bool(true)") {
            self.take("Bool(true)")?;
        } else if self.input.get(self.pos..)?.starts_with("Bool(false)") {
            self.take("Bool(false)")?;
        } else {
            self.take("Null")?;
        }
        Some((self.input.get(start..self.pos)?.to_owned(), string))
    }

    fn container(&mut self, path: &[String], object: bool) -> Option<String> {
        let start = self.pos;
        self.take(if object { "Object {" } else { "Array [" })?;
        let head = self.input.get(start..self.pos)?;
        let close = if object { "}" } else { "]" };
        let notes = !object
            && path.len() == 3
            && path[0] == "func-details"
            && matches!(
                path[2].as_str(),
                "obligation_proof_notes" | "failed_proof_notes"
            );
        let details = object && path == ["func-details"];
        self.details |= details;
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        loop {
            let entry_start = self.pos;
            self.space();
            if self.input.get(self.pos..)?.starts_with(close) {
                self.take(close)?;
                let tail = self.input.get(entry_start..self.pos)?;
                if details || notes {
                    entries.sort_by(|a: &(String, String), b| a.0.cmp(&b.0));
                }
                return Some(format!(
                    "{head}{}{tail}",
                    entries.into_iter().map(|(_, s)| s).collect::<String>()
                ));
            }
            let mut child = path.to_vec();
            let key = if object {
                let key = self.string()?;
                if !seen.insert(key.clone()) {
                    return None;
                }
                self.space();
                self.take(":")?;
                self.space();
                child.push(key.clone());
                key
            } else {
                child.push(entries.len().to_string());
                String::new()
            };
            let value_start = self.pos;
            let (value, string) = self.value(&child)?;
            let value_end = self.pos;
            self.space();
            self.take(",")?;
            let sort_key = if notes {
                let note = string?;
                if !seen.insert(note.clone()) {
                    return None;
                }
                note
            } else {
                key
            };
            entries.push((
                sort_key,
                format!(
                    "{}{value}{}",
                    self.input.get(entry_start..value_start)?,
                    self.input.get(value_end..self.pos)?
                ),
            ));
        }
    }
}

/// Returns the same stderr except for the two explicitly allowed entry orders.
pub(super) fn canonical(stderr: &str) -> Option<String> {
    if stderr.len() > LIMIT {
        return None;
    }
    let mut out = String::new();
    let mut remaining = stderr;
    let mut count = 0;
    while let Some(at) = remaining.find(HEADER) {
        if at != 0 && !remaining.get(..at)?.ends_with('\n') {
            return None;
        }
        out.push_str(remaining.get(..at + HEADER.len())?);
        let input = remaining.get(at + HEADER.len()..)?;
        let mut parser = Parser {
            input,
            pos: 0,
            nodes: 0,
            stock: false,
            details: false,
        };
        let (value, _) = parser.value(&[])?;
        if !parser.stock || !parser.details || input.as_bytes().get(parser.pos) != Some(&b'\n') {
            return None;
        }
        out.push_str(&value);
        remaining = input.get(parser.pos..)?;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    out.push_str(remaining);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Verdict;
    use crate::rvt::{classify_stock_test, classify_tests_exact};
    use sail_verus::rvt::{Outcome, TestRecord};

    const FIRST: &str = include_str!("../../tests/fixtures/stock-debug-order/first.stderr");
    const REORDERED: &str = include_str!("../../tests/fixtures/stock-debug-order/reordered.stderr");

    fn record(stderr: &str) -> TestRecord {
        TestRecord {
            outcome: Outcome::Ok,
            exit_code: Some(0),
            stdout: "ok".to_owned(),
            stderr: stderr.to_owned(),
            wall_ms: 0.0,
        }
    }

    fn classify(a: &str, b: &str) -> crate::rvt::TestClassification {
        classify_stock_test(&[record(a)], &record(b), STOCK, "output_json")
    }

    #[test]
    fn real_stock_failures_keep_values_and_report_the_order_exception() {
        assert_ne!(FIRST, REORDERED);
        assert_eq!(canonical(FIRST), canonical(REORDERED));
        assert!(canonical(FIRST).is_some());
        let result = classify(FIRST, REORDERED);
        assert_eq!(result.verdict, Verdict::Same);
        assert_eq!(result.accepted_order_exceptions.len(), 1);
        assert!(!result.strict_differences.is_empty());
        assert_eq!(
            classify_tests_exact(&[record(FIRST)], &record(REORDERED)).verdict,
            Verdict::Different
        );
    }

    #[test]
    fn altered_notes_values_diagnostics_and_outcomes_remain_different() {
        for changed in [
            REORDERED.replace("Property 732", "Property 733"),
            REORDERED.replacen("String(\"Label 451\"),", "", 1),
            REORDERED.replacen("Bool(false)", "Bool(true)", 1),
            format!("error: a new diagnostic\n{REORDERED}"),
            REORDERED.replacen("Number(1)", "Number(2)", 1),
        ] {
            assert_ne!(classify(FIRST, &changed).verdict, Verdict::Same);
        }
        let mut failed = record(REORDERED);
        failed.outcome = Outcome::Failed;
        assert_eq!(
            classify_stock_test(&[record(FIRST)], &failed, STOCK, "output_json").verdict,
            Verdict::Different
        );
        failed.outcome = Outcome::Ok;
        failed.exit_code = Some(1);
        assert_eq!(
            classify_stock_test(&[record(FIRST)], &failed, STOCK, "output_json").verdict,
            Verdict::Different
        );
    }

    #[test]
    fn wrong_producer_helper_binary_and_syntax_fail_closed() {
        for changed in [
            REORDERED.replace(STOCK, "another-commit"),
            REORDERED.replace(":277:5]", ":278:5]"),
            REORDERED.replace("Bool(false)", "Future(false)"),
            REORDERED.replace(
                "String(\"Label 451\"),",
                "String(\"Label 451\"), String(\"Label 451\"),",
            ),
            REORDERED.replace(
                "\"verified\": Number(1),",
                "\"verified\": Number(1), \"verified\": Number(1),",
            ),
            REORDERED.trim_end_matches(['\n', '}']).to_owned(),
        ] {
            assert!(canonical(&changed).is_none());
            assert_ne!(classify(FIRST, &changed).verdict, Verdict::Same);
        }
        for (pin, binary) in [(STOCK, "other"), ("other", "output_json")] {
            let result = classify_stock_test(&[record(FIRST)], &record(REORDERED), pin, binary);
            assert_ne!(result.verdict, Verdict::Same);
            assert!(result.accepted_order_exceptions.is_empty());
        }
    }

    #[test]
    fn ordinary_array_order_whitespace_and_keys_remain_exact() {
        let a = FIRST.replacen(
            "\"profile\": String(\"release\"),",
            "\"profile\": Array [ String(\"a\"), String(\"b\"), ],",
            1,
        );
        let b = a.replacen(
            "String(\"a\"), String(\"b\")",
            "String(\"b\"), String(\"a\")",
            1,
        );
        assert!(canonical(&a).is_some());
        assert_ne!(classify(&a, &b).verdict, Verdict::Same);
        let b = REORDERED.replacen("\"profile\": String", "\"profile\":  String", 1);
        assert_ne!(classify(FIRST, &b).verdict, Verdict::Same);
        let b = REORDERED.replacen("\"profile\":", "\"another-key\":", 1);
        assert_ne!(classify(FIRST, &b).verdict, Verdict::Same);
    }

    #[test]
    fn oversized_or_deep_snapshots_are_refused() {
        assert!(canonical(&"x".repeat(LIMIT + 1)).is_none());
        let nested = format!("{}Null{}", "Array [".repeat(66), ",]".repeat(66));
        let deep = FIRST.replacen("String(\"release\")", &nested, 1);
        assert!(canonical(&deep).is_none());
    }
}
