//! Diagnostics compared as a multiset of complete messages (ADR 0042).
//!
//! Stock Verus prints each module's diagnostics when that module's checks
//! finish, so messages from different modules come out in a timing-dependent
//! order (VI-0007). [`compare_stderr`] therefore splits each stderr into:
//! - **messages**: complete diagnostics that carry a source location, each
//!   compared as exact bytes, as a multiset (every message the same number of
//!   times on both sides, in any order);
//! - **progress**: complete standalone `note: verifying module ...` blocks
//!   may reorder within one uninterrupted run, with identical bytes and counts
//!   (owner decision 2026-10-21, ADR 0064);
//! - **the rest**: everything else (summary lines such as `error: aborting
//!   due to 3 previous errors`, other notes without a location, panic text, any
//!   other output), compared byte for byte and in order. Each run of
//!   consecutive messages stands in the rest as one placeholder line, so text
//!   that moves from after the messages to before them is still a difference.
//!
//! A message starts at a column-0 header line (`error`, `warning`, `note`,
//! `help` or `failure-note`, optionally with a code such as `error[E0308]`,
//! then `: `) whose next line is a location (`--> file:line:col`). It runs to
//! the first blank line, which rustc's human renderer prints after every
//! diagnostic, and includes it. Spanned child notes that rustc renders at
//! column 0 inside a diagnostic (no blank line before them) stay part of
//! their parent. A message without a blank line ends before the next column-0
//! header that has no location (a summary or a note), or at the end.
//!
//! **Module order.** ADR 0042 keeps the relative order of messages from the
//! same module. Module identity is not recoverable from a message: the
//! location names a file and a line, inline modules share a file
//! (`debug_expand.rs` has its root module and `M4` in one file), and a
//! message can point into another file (a precondition in `vstd`). Nor do
//! Verus's `--output-json` documents give function spans to map a location
//! to a module. Any rule based on files would either reject the VI-0007
//! reorderings ADR 0042 accepts or guess. The messages are therefore
//! compared as a plain multiset: a reordering within one module is not
//! reported. Every message's bytes and count, and everything outside the
//! messages, are still exact.

use std::collections::BTreeMap;

use crate::first_difference;

/// Stands in the rest of stderr for one run of consecutive messages. The NUL
/// bytes keep it from matching real output.
const MESSAGES: &str = "\0<diagnostic messages>\0\n";

/// One stderr, split.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Split<'a> {
    /// Complete messages, in the order printed.
    pub messages: Vec<&'a str>,
    /// Exact diagnostic sequences grouped by their uninterrupted runs. This
    /// preserves placement on either side of progress and other opaque output.
    pub message_runs: Vec<Vec<&'a str>>,
    /// Original opaque byte gaps before, between and after diagnostic runs.
    /// These remain typed separately from synthetic display placeholders.
    pub gaps: Vec<&'a str>,
    /// Everything else, in order, with one placeholder line per run of
    /// consecutive messages.
    pub rest: String,
}

fn is_header(line: &str) -> bool {
    let Some(tail) = ["failure-note", "warning", "error", "note", "help"]
        .iter()
        .find_map(|level| line.strip_prefix(level))
    else {
        return false;
    };
    let tail = match tail.strip_prefix('[') {
        Some(code) => match code.split_once(']') {
            Some((name, after)) if !name.is_empty() && !name.contains(char::is_whitespace) => after,
            _ => return false,
        },
        None => tail,
    };
    tail.starts_with(": ")
}

fn is_location(line: &str) -> bool {
    line.trim_start_matches(' ').starts_with("--> ")
}

fn is_blank(line: &str) -> bool {
    line.trim_end_matches(['\n', '\r']).is_empty()
}

fn starts_message(lines: &[&str], i: usize) -> bool {
    is_header(lines[i]) && lines.get(i + 1).is_some_and(|l| is_location(l))
}

/// Splits `stderr` into complete messages and the rest.
#[must_use]
pub fn split(stderr: &str) -> Split<'_> {
    let lines: Vec<&str> = stderr.split_inclusive('\n').collect();
    let mut out = Split::default();
    let mut offset = 0;
    let mut gap_start = 0;
    let mut i = 0;
    let mut in_run = false;
    while i < lines.len() {
        if !starts_message(&lines, i) {
            out.rest.push_str(lines[i]);
            offset += lines[i].len();
            i += 1;
            in_run = false;
            continue;
        }
        let start = offset;
        // The header and its location.
        offset += lines[i].len() + lines[i + 1].len();
        i += 2;
        while i < lines.len() {
            let line = lines[i];
            if is_header(line) && !starts_message(&lines, i) {
                break;
            }
            offset += line.len();
            i += 1;
            if is_blank(line) {
                break;
            }
        }
        let message = &stderr[start..offset];
        out.messages.push(message);
        if !in_run {
            out.rest.push_str(MESSAGES);
            out.message_runs.push(Vec::new());
            out.gaps.push(&stderr[gap_start..start]);
            in_run = true;
        }
        if let Some(run) = out.message_runs.last_mut() {
            run.push(message);
        }
        gap_start = offset;
    }
    out.gaps.push(&stderr[gap_start..]);
    out
}

/// A message's header and location, for reports.
fn describe(message: &str) -> String {
    let mut lines = message.lines();
    let header = lines.next().unwrap_or_default();
    let location = lines.next().unwrap_or_default().trim_start();
    let location = location.strip_prefix("--> ").unwrap_or(location);
    format!("{header:?} at {location}")
}

fn identity(message: &str) -> (Option<&str>, Option<&str>) {
    let mut lines = message.lines();
    (lines.next(), lines.next())
}

// Cargo compiler captures preserve rustc's JSON envelope. Admit only the exact
// standalone rendering of the already-approved progress block. Sorting keeps
// the whole original JSON line, including all formatting and escape bytes.
fn json_progress(line: &str) -> bool {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Progress {
        #[serde(rename = "$message_type")]
        kind: String,
        message: String,
        code: serde_json::Value,
        level: String,
        spans: Vec<serde_json::Value>,
        children: Vec<serde_json::Value>,
        rendered: String,
    }
    if !line.starts_with('{') || !line.ends_with('\n') {
        return false;
    }
    let Ok(progress) = serde_json::from_str::<Progress>(line) else {
        return false;
    };
    progress.kind == "diagnostic"
        && progress.level == "note"
        && progress.code.is_null()
        && progress.spans.is_empty()
        && progress.children.is_empty()
        && progress
            .message
            .strip_prefix("verifying module ")
            .is_some_and(|module| !module.trim().is_empty() && !module.contains(['\n', '\r']))
        && (progress.rendered == format!("note: {}\n\n", progress.message)
            || progress.rendered
                == format!(
                    "\u{1b}[1m\u{1b}[92mnote\u{1b}[0m\u{1b}[1m: {}\u{1b}[0m\n\n",
                    progress.message
                ))
}

fn progress_width(lines: &[&str], index: usize) -> usize {
    let Some(line) = lines.get(index) else {
        return 0;
    };
    if line
        .strip_prefix("note: verifying module ")
        .is_some_and(|module| !module.trim().is_empty())
        && lines.get(index + 1).is_some_and(|next| is_blank(next))
    {
        2
    } else {
        usize::from(json_progress(line))
    }
}

// Only complete standalone progress blocks in one uninterrupted run may
// move. Message bodies, separators, summaries and arbitrary notes stay exact.
fn ordered_progress(rest: &str) -> String {
    let lines: Vec<_> = rest.split_inclusive('\n').collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        if progress_width(&lines, i) == 0 {
            out.push_str(lines[i]);
            i += 1;
            continue;
        }
        let mut blocks = Vec::new();
        loop {
            let width = progress_width(&lines, i);
            if width == 0 {
                break;
            }
            blocks.push(lines[i..i + width].concat());
            i += width;
        }
        blocks.sort_unstable();
        for block in blocks {
            out.push_str(&block);
        }
    }
    out
}

// Compare an original message-free gap, never a synthetic placeholder stream.
// The caller retains each complete diagnostic run separately and in place.
pub(crate) fn progress_gap_matches(plain: &str, sail: &str) -> bool {
    ordered_progress(plain) == ordered_progress(sail)
}

/// Whether standalone verification-progress blocks changed order within an
/// uninterrupted run (VI-0012, owner-approved ADR 0064).
///
/// This does not compare complete diagnostic messages. Use [`compare_stderr`]
/// as well to establish that the full stderr satisfies the contract.
#[must_use]
pub fn progress_order_differs(plain: &str, sail: &str) -> bool {
    let (p, s) = (split(plain), split(sail));
    p.rest != s.rest && ordered_progress(&p.rest) == ordered_progress(&s.rest)
}

/// Every diagnostics difference between a plain and a sail stderr under ADR
/// 0042, one line each; empty if they match.
#[must_use]
pub fn compare_stderr(plain: &str, sail: &str) -> Vec<String> {
    if plain == sail {
        return Vec::new();
    }
    let (p, s) = (split(plain), split(sail));
    let mut d = Vec::new();
    if ordered_progress(&p.rest) != ordered_progress(&s.rest) {
        d.push(format!(
            "diagnostics (stderr) outside messages differ at {} (messages collapsed to one line per run)",
            first_difference(&p.rest, &s.rest)
        ));
    }
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for m in &p.messages {
        counts.entry(m).or_default().0 += 1;
    }
    for m in &s.messages {
        counts.entry(m).or_default().1 += 1;
    }
    // Messages short on one side, in the order they were first printed.
    let mut missing: Vec<&str> = Vec::new();
    let mut extra: Vec<&str> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for m in p.messages.iter().chain(&s.messages) {
        if !seen.insert(*m) {
            continue;
        }
        let (np, ns) = counts.get(m).copied().unwrap_or_default();
        missing.extend(std::iter::repeat_n(*m, np.saturating_sub(ns)));
        extra.extend(std::iter::repeat_n(*m, ns.saturating_sub(np)));
    }
    // A count difference is reported once per message.
    let mut reported = std::collections::BTreeSet::new();
    for m in missing {
        let (np, ns) = counts.get(m).copied().unwrap_or_default();
        if ns == 0
            && let Some(pos) = extra
                .iter()
                .position(|e| counts.get(e).is_some_and(|c| c.0 == 0) && identity(e) == identity(m))
        {
            let e = extra.remove(pos);
            d.push(format!(
                "diagnostic message {} altered in sail: {}",
                describe(m),
                first_difference(m, e)
            ));
        } else if ns == 0 {
            d.push(format!(
                "diagnostic message {} missing from sail",
                describe(m)
            ));
        } else if reported.insert(m) {
            d.push(format!(
                "diagnostic message {} printed {np} times by plain, {ns} by sail",
                describe(m)
            ));
        }
    }
    for e in extra {
        let (np, ns) = counts.get(e).copied().unwrap_or_default();
        if np == 0 {
            d.push(format!("diagnostic message {} only in sail", describe(e)));
        } else if reported.insert(e) {
            d.push(format!(
                "diagnostic message {} printed {np} times by plain, {ns} by sail",
                describe(e)
            ));
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "error: assertion failed\n  --> src/a.rs:17:12\n   |\n17 |     assert(x);\n   |            ^ assertion failed\n\n";
    const B: &str = "error: assertion failed\n   --> src/a.rs:251:16\n    |\n251 |         assert(y);\n    |                ^ assertion failed\n\n";
    const NOTE: &str = "note: function body check: not all errors may have been reported\n  --> src/a.rs:9:7\n   |\n9 | proof fn f() {\n   |       ^\n\n";
    const SUMMARY: &str = "error: aborting due to 2 previous errors\n\n";

    fn cat(parts: &[&str]) -> String {
        parts.concat()
    }

    const PROGRESS_A: &str = "note: verifying module a\n\n";
    const PROGRESS_B: &str = "note: verifying module b, function b::f\n\n";

    #[test]
    fn progress_reordering_keeps_exact_text_counts_and_boundaries() {
        let plain = cat(&[PROGRESS_A, PROGRESS_B, A, SUMMARY]);
        let reordered = cat(&[PROGRESS_B, PROGRESS_A, A, SUMMARY]);
        assert_eq!(compare_stderr(&plain, &reordered), Vec::<String>::new());
        assert!(progress_order_differs(&plain, &reordered));
        for changed in [
            cat(&[PROGRESS_A, A, SUMMARY]),
            cat(&[PROGRESS_A, PROGRESS_A, A, SUMMARY]),
            cat(&[PROGRESS_B, PROGRESS_A, PROGRESS_A, A, SUMMARY]),
            reordered.replace("b::f", "b::g"),
            reordered.replace("assert(x)", "assert(y)"),
            reordered.replace(SUMMARY, "error: aborting due to 1 previous error\n\n"),
        ] {
            assert_ne!(
                compare_stderr(&plain, &changed),
                Vec::<String>::new(),
                "{changed}"
            );
        }
        assert_ne!(
            compare_stderr(
                &cat(&[PROGRESS_A, SUMMARY, PROGRESS_B]),
                &cat(&[PROGRESS_B, SUMMARY, PROGRESS_A])
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn progress_like_text_inside_diagnostics_and_unterminated_notes_stay_exact() {
        let spanned = "note: verifying module a\n --> a.rs:1:1\n  |\n\n";
        assert_ne!(
            compare_stderr(spanned, &spanned.replace("module a", "module b")),
            Vec::<String>::new()
        );
        assert_ne!(
            compare_stderr(
                "note: verifying module a\nnote: verifying module b\n",
                "note: verifying module b\nnote: verifying module a\n"
            ),
            Vec::<String>::new()
        );
        let other = "note: other information\n\n";
        assert_ne!(
            compare_stderr(
                &cat(&[PROGRESS_A, other, PROGRESS_B]),
                &cat(&[PROGRESS_B, other, PROGRESS_A])
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn splits_messages_from_summary_and_other_text() {
        let s = cat(&["warning: progress\n", A, NOTE, B, SUMMARY]);
        let split = split(&s);
        assert_eq!(split.messages, vec![A, NOTE, B]);
        assert_eq!(
            split.rest,
            format!("warning: progress\n{MESSAGES}{SUMMARY}")
        );
    }

    #[test]
    fn headers_with_codes_and_spanned_children_stay_whole() {
        let m = "warning[E0001]: unused\n --> a.rs:1:1\n  |\nnote: the lint level is defined here\n --> a.rs:2:2\n  |\n\n";
        let split = split(m);
        assert_eq!(split.messages, vec![m]);
        assert!(!is_header("error [x]: no"));
        assert!(!is_header("errors: no"));
        assert!(is_header("error[E0308]: mismatched types"));
    }

    #[test]
    fn a_message_without_a_blank_line_ends_at_the_summary() {
        let s = "error: e\n --> a.rs:1:1\n  |\nerror: aborting due to 1 previous error\n";
        let split = split(s);
        assert_eq!(split.messages, vec!["error: e\n --> a.rs:1:1\n  |\n"]);
        assert_eq!(
            split.rest,
            format!("{MESSAGES}error: aborting due to 1 previous error\n")
        );
    }

    #[test]
    fn messages_reordered_across_modules_match() {
        let plain = cat(&[B, A, NOTE, SUMMARY]);
        let sail = cat(&[A, NOTE, B, SUMMARY]);
        assert_eq!(compare_stderr(&plain, &sail), Vec::<String>::new());
    }

    #[test]
    fn a_missing_message_is_reported() {
        let d = compare_stderr(&cat(&[B, A, SUMMARY]), &cat(&[A, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("missing from sail"), "{d:?}");
        assert!(d[0].contains("src/a.rs:251:16"), "{d:?}");
    }

    #[test]
    fn an_extra_message_is_reported() {
        let d = compare_stderr(&cat(&[A, SUMMARY]), &cat(&[A, B, SUMMARY]));
        assert_eq!(
            d,
            vec![format!("diagnostic message {} only in sail", describe(B))]
        );
    }

    #[test]
    fn a_duplicated_message_is_reported() {
        let d = compare_stderr(&cat(&[A, B, SUMMARY]), &cat(&[A, B, A, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(
            d[0].contains("printed 1 times by plain, 2 by sail"),
            "{d:?}"
        );
        let d = compare_stderr(&cat(&[A, A, SUMMARY]), &cat(&[A, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(
            d[0].contains("printed 2 times by plain, 1 by sail"),
            "{d:?}"
        );
        // A count difference is one line, whatever its size.
        let d = compare_stderr(&cat(&[A, A, A, SUMMARY]), &cat(&[A, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("printed 3 times by plain, 1 by sail"));
        let d = compare_stderr(&cat(&[A, SUMMARY]), &cat(&[A, A, A, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
    }

    #[test]
    fn a_one_byte_change_inside_a_message_is_reported() {
        let altered = A.replace("assertion failed\n\n", "assertion failes\n\n");
        let d = compare_stderr(&cat(&[A, B, SUMMARY]), &cat(&[B, &altered, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("altered in sail"), "{d:?}");
        assert!(d[0].contains("line 5"), "{d:?}");
        // A change in the header or location is a missing plus an extra message.
        let moved = A.replace("17:12", "17:13");
        let d = compare_stderr(&cat(&[A, SUMMARY]), &cat(&[&moved, SUMMARY]));
        assert_eq!(d.len(), 2, "{d:?}");
    }

    #[test]
    fn a_reordered_summary_is_reported() {
        let warnings = "warning: 1 warning emitted\n\n";
        let d = compare_stderr(&cat(&[A, SUMMARY, warnings]), &cat(&[A, warnings, SUMMARY]));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("outside messages"), "{d:?}");
        // The summary moved in front of the messages.
        let d = compare_stderr(&cat(&[A, B, SUMMARY]), &cat(&[SUMMARY, A, B]));
        assert_eq!(d.len(), 1, "{d:?}");
        // Other text moved between two messages.
        let d = compare_stderr(&cat(&[A, B, SUMMARY]), &cat(&[A, SUMMARY, B]));
        assert_eq!(d.len(), 1, "{d:?}");
    }

    fn json_note(module: &str) -> String {
        serde_json::json!({
            "$message_type": "diagnostic", "message": format!("verifying module {module}"),
            "code": null, "level": "note", "spans": [], "children": [],
            "rendered": format!("\u{1b}[1m\u{1b}[92mnote\u{1b}[0m\u{1b}[1m: verifying module {module}\u{1b}[0m\n\n")
        }).to_string() + "\n"
    }

    #[test]
    fn native_anvil_cargo_progress_uses_the_existing_exact_block_exception() {
        let plain = include_str!("../tests/fixtures/json-progress-order/plain.stderr");
        let sail = include_str!("../tests/fixtures/json-progress-order/sail.stderr");
        assert_ne!(plain, sail);
        assert_eq!(plain.lines().count(), 45);
        assert_eq!(sail.lines().count(), 45);
        assert_eq!(compare_stderr(plain, sail), Vec::<String>::new());
        assert!(progress_order_differs(plain, sail));
        let changed = sail.replacen("model::install", "model::changed", 1);
        assert_ne!(changed, sail);
        assert_ne!(compare_stderr(plain, &changed), Vec::<String>::new());
    }

    #[test]
    fn cargo_json_progress_keeps_original_bytes_counts_and_run_boundaries() {
        let a = json_note("vdeployment_controller::model::install");
        let b = json_note("vdeployment_controller::exec::reconciler");
        let before = format!("{a}{b}");
        let after = format!("{b}{a}");
        assert!(progress_order_differs(&before, &after));
        assert_eq!(compare_stderr(&before, &after), Vec::<String>::new());
        for changed in [
            a.clone(),
            format!("{a}{b}{b}"),
            format!("{a}summary\n{b}"),
            after.replace("model::install", "model::changed"),
        ] {
            assert_ne!(compare_stderr(&before, &changed), Vec::<String>::new());
        }
        assert_ne!(
            compare_stderr(&format!("{a}summary\n{b}"), &format!("{b}summary\n{a}")),
            Vec::<String>::new()
        );
        let reformatted = a.replace("\"level\":", "\"level\": ");
        assert_ne!(a, reformatted);
        assert_ne!(
            compare_stderr(&before, &format!("{b}{reformatted}")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn json_progress_refuses_partial_ambiguous_or_nonstandalone_envelopes() {
        let original = json_note("m");
        for mutation in 0..10 {
            let mut value: serde_json::Value =
                serde_json::from_str(&original).unwrap_or_else(|error| unreachable!("{error}"));
            match mutation {
                0 => value["level"] = serde_json::json!("error"),
                1 => value["spans"] = serde_json::json!([{}]),
                2 => value["children"] = serde_json::json!([{"message": "another note"}]),
                3 => value["rendered"] = serde_json::json!("note: verifying module m\n"),
                4 => value["rendered"] = serde_json::json!("error: changed\n\n"),
                5 => value["$message_type"] = serde_json::json!("unknown"),
                6 => value["unexpected"] = serde_json::json!(true),
                7 => {
                    value
                        .as_object_mut()
                        .unwrap_or_else(|| unreachable!())
                        .remove("code");
                }
                8 => value["code"] = serde_json::json!("E0000"),
                _ => value["message"] = serde_json::json!("verifying module m\nsecond note"),
            }
            let changed = value.to_string() + "\n";
            assert!(!json_progress(&changed), "mutation {mutation}");
            assert!(
                !compare_stderr(
                    &format!("{original}{changed}"),
                    &format!("{changed}{original}")
                )
                .is_empty(),
                "mutation {mutation}"
            );
        }
        assert!(!json_progress(original.trim_end()));
        let duplicate = original.replacen("\"level\":", "\"level\":\"error\",\"level\":", 1);
        assert!(!json_progress(&duplicate));
    }

    #[test]
    fn text_without_messages_is_compared_byte_for_byte() {
        assert_eq!(compare_stderr("x\ny\n", "x\ny\n"), Vec::<String>::new());
        let d = compare_stderr("x\ny\n", "y\nx\n");
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("line 1"), "{d:?}");
    }

    /// Two real stderrs of `debug_expand.rs` (stock Verus 0.2026.10.04.426d8b0,
    /// native Linux x86-64): plain Verus printed `M4`'s error first, and a
    /// cache-warm `sail` run printed the root module's first
    /// (`corpus/repros/sail-cache-warm-error-order/`).
    const M4_FIRST: &str = include_str!("../tests/fixtures/debug-expand-order/m4-first.stderr");
    const ROOT_FIRST: &str = include_str!("../tests/fixtures/debug-expand-order/root-first.stderr");
    const M4_ERROR: &str = "debug_expand.rs:251:16";

    #[test]
    fn native_benchmark_warning_and_error_order_uses_the_same_contract() {
        let stock = include_str!("../tests/fixtures/benchmark-mixed-diagnostic-order/stock.stderr");
        let sail = include_str!("../tests/fixtures/benchmark-mixed-diagnostic-order/sail.stderr");
        assert_ne!(stock, sail);
        assert_eq!(split(stock).messages.len(), 4);
        assert_eq!(compare_stderr(stock, sail), Vec::<String>::new());
        let changed_body = sail.replacen("assert(1 + 1 == 3)", "assert(1 + 1 == 2)", 1);
        assert_ne!(changed_body, sail);
        assert_ne!(compare_stderr(stock, &changed_body), Vec::<String>::new());
    }

    #[test]
    fn the_debug_expand_module_orders_are_the_same_diagnostics() {
        assert_ne!(M4_FIRST, ROOT_FIRST);
        let (a, b) = (split(M4_FIRST), split(ROOT_FIRST));
        assert_eq!(a.messages.len(), 18, "16 errors and 2 notes");
        assert_eq!(a.rest, b.rest);
        assert_eq!(
            a.rest,
            format!("{MESSAGES}error: aborting due to 16 previous errors\n\n")
        );
        assert!(a.messages[0].contains(M4_ERROR));
        assert!(b.messages[17].contains(M4_ERROR));
        assert_eq!(compare_stderr(M4_FIRST, ROOT_FIRST), Vec::<String>::new());
        assert_eq!(compare_stderr(ROOT_FIRST, M4_FIRST), Vec::<String>::new());
    }

    #[test]
    fn the_debug_expand_stderr_still_catches_real_differences() {
        let m4 = split(M4_FIRST).messages[0];
        // Missing: the M4 error dropped from the root-first variant.
        let dropped = ROOT_FIRST.replacen(m4, "", 1);
        let d = compare_stderr(M4_FIRST, &dropped);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("missing from sail") && d[0].contains(M4_ERROR));
        // Duplicated.
        let doubled = ROOT_FIRST.replacen(m4, &format!("{m4}{m4}"), 1);
        let d = compare_stderr(M4_FIRST, &doubled);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("printed 1 times by plain, 2 by sail"));
        // One byte changed inside the M4 error.
        let altered = ROOT_FIRST.replacen(m4, &m4.replace("M3::", "M3:;"), 1);
        let d = compare_stderr(M4_FIRST, &altered);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("altered in sail") && d[0].contains("line 4"));
        // The summary moved in front of the messages.
        let summary = "error: aborting due to 16 previous errors\n\n";
        let moved = format!("{summary}{}", ROOT_FIRST.replacen(summary, "", 1));
        let d = compare_stderr(M4_FIRST, &moved);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("outside messages"));
    }
}
