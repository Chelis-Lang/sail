//! The pure mapping from a managed project result to
//! `textDocument/publishDiagnostics`, and the bookkeeping that keeps an
//! editor's view honest when a result stops being current.
//!
//! Verus's diagnostics are relayed, not interpreted: the header message is
//! verbatim, the position is the primary `-->` location, and the width and
//! label come from the caret line of the rendered block. Diagnostics without
//! a primary location (for example `aborting due to 1 previous error`) cannot
//! be placed in a document; [`project`] counts them instead of dropping them
//! silently.
//!
//! Positions are Verus's 1-based line and column minus one. LSP counts
//! UTF-16 code units and rustc counts characters, so a line with characters
//! outside the Basic Multilingual Plane before the span is offset by the
//! difference. Correcting that needs the file's text, which this crate does
//! not read.

use std::collections::BTreeMap;

use sail_api::output::Diagnostic;
use sail_api::project::{Currentness, ProjectResult};
use serde_json::{Value, json};

use crate::{jsonrpc, uri};

/// LSP diagnostics per document URI, in a stable order.
pub type ByUri = BTreeMap<String, Vec<Value>>;

/// The `source` every diagnostic carries.
pub const SOURCE: &str = "sail";

/// What [`project`] produced from one result.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Projection {
    /// Diagnostics with a primary location, by document.
    pub by_uri: ByUri,
    /// Errors and warnings without a primary location.
    pub unlocated: Vec<String>,
}

/// Maps every error and warning of every target to LSP diagnostics.
///
/// Relative files are resolved against the result's workspace, which is the
/// directory Cargo ran the verifier in. The same diagnostic reported by two
/// targets (a library and its tests, say) is published once.
#[must_use]
pub fn project(result: &ProjectResult) -> Projection {
    let mut projection = Projection::default();
    for target in &result.targets {
        for diagnostic in &target.report.diagnostics {
            let Some(severity) = severity(&diagnostic.level) else {
                continue;
            };
            let Some(primary) = diagnostic.spans.iter().find(|span| span.primary) else {
                if !projection.unlocated.contains(&diagnostic.message) {
                    projection.unlocated.push(diagnostic.message.clone());
                }
                continue;
            };
            let document = uri::from_path(&uri::resolve(&result.workspace, &primary.file));
            let (width, label) = underline(&diagnostic.rendered, primary.line);
            let related: Vec<Value> = diagnostic
                .spans
                .iter()
                .filter(|span| !span.primary)
                .map(|span| {
                    json!({
                        "location": {
                            "uri": uri::from_path(&uri::resolve(&result.workspace, &span.file)),
                            "range": range(span.line, span.column, 0),
                        },
                        "message": "related location",
                    })
                })
                .collect();
            let mut item = json!({
                "range": range(primary.line, primary.column, width),
                "severity": severity,
                "source": SOURCE,
                "message": message(diagnostic, label.as_deref()),
            });
            if let Some(object) = item.as_object_mut() {
                if let Some(code) = &diagnostic.code {
                    object.insert("code".to_owned(), json!(code));
                }
                if !related.is_empty() {
                    object.insert("relatedInformation".to_owned(), json!(related));
                }
            }
            let items = projection.by_uri.entry(document).or_default();
            if !items.contains(&item) {
                items.push(item);
            }
        }
    }
    projection
}

fn severity(level: &str) -> Option<u8> {
    match level {
        "error" => Some(1),
        "warning" => Some(2),
        _ => None,
    }
}

fn message(diagnostic: &Diagnostic, label: Option<&str>) -> String {
    match label {
        Some(label) if !label.is_empty() => format!("{}\n{label}", diagnostic.message),
        _ => diagnostic.message.clone(),
    }
}

fn range(line: u64, column: u64, width: u64) -> Value {
    let line = line.saturating_sub(1);
    let start = column.saturating_sub(1);
    json!({
        "start": {"line": line, "character": start},
        "end": {"line": line, "character": start + width},
    })
}

/// The caret run under source line `line` in a rendered block: its width and
/// the label after it. `(0, None)` when the block has no such line.
fn underline(rendered: &str, line: u64) -> (u64, Option<String>) {
    let mut under = false;
    for text in rendered.lines() {
        let Some((gutter, rest)) = text.split_once('|') else {
            under = false;
            continue;
        };
        let gutter = gutter.trim();
        if !gutter.is_empty() {
            under = gutter.parse() == Ok(line);
            continue;
        }
        if under && let Some(start) = rest.find('^') {
            let carets = rest[start..].chars().take_while(|c| *c == '^').count();
            let label = rest[start + carets..].trim();
            return (
                u64::try_from(carets).unwrap_or(0),
                Some(label.to_owned()).filter(|label| !label.is_empty()),
            );
        }
    }
    (0, None)
}

/// What happens to published diagnostics once their run is no longer the
/// current one (a newer run is queued, or the run was superseded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stale {
    /// Remove them until a current result arrives.
    #[default]
    Clear,
    /// Keep them, demoted to information and labelled stale.
    Mark,
}

/// Tracks what the editor currently shows, so every document that had
/// diagnostics is explicitly emptied when they no longer hold.
#[derive(Debug, Default)]
pub struct Publisher {
    policy: Stale,
    shown: ByUri,
    stale: bool,
}

impl Publisher {
    /// A publisher with nothing shown.
    #[must_use]
    pub fn new(policy: Stale) -> Self {
        Self {
            policy,
            shown: ByUri::new(),
            stale: false,
        }
    }

    /// The notifications for a completed result. Only a result the service
    /// classifies as current is published as current. A historical result,
    /// or one whose currentness is unknown, still completed and may hold
    /// real failures, so under either policy it is published demoted and
    /// labelled instead of being dropped: an empty document would read as
    /// "nothing failed".
    pub fn result(&mut self, result: &ProjectResult) -> Vec<Value> {
        let next = project(result).by_uri;
        let label = match result.currentness {
            Currentness::Current => return self.current(next),
            Currentness::Historical => HISTORICAL,
            Currentness::Unknown => UNKNOWN,
        };
        self.stale = true;
        self.replace(next, Some(label))
    }

    /// Publishes `next` as the current diagnostics and empties every document
    /// that had diagnostics before and has none now.
    pub fn current(&mut self, next: ByUri) -> Vec<Value> {
        self.stale = false;
        self.replace(next, None)
    }

    /// The run behind the shown diagnostics is no longer current.
    pub fn stale(&mut self) -> Vec<Value> {
        match self.policy {
            Stale::Clear => self.clear(),
            Stale::Mark if self.stale => Vec::new(),
            Stale::Mark => {
                self.stale = true;
                self.shown
                    .iter()
                    .map(|(document, items)| publish(document, &marked(items, PENDING)))
                    .collect()
            }
        }
    }

    /// Empties every document that has diagnostics.
    pub fn clear(&mut self) -> Vec<Value> {
        self.stale = false;
        std::mem::take(&mut self.shown)
            .keys()
            .map(|document| publish(document, &[]))
            .collect()
    }

    /// Whether any document currently has diagnostics.
    #[must_use]
    pub fn showing(&self) -> bool {
        !self.shown.is_empty()
    }

    fn replace(&mut self, next: ByUri, label: Option<&str>) -> Vec<Value> {
        let mut out: Vec<Value> = self
            .shown
            .keys()
            .filter(|document| !next.contains_key(*document))
            .map(|document| publish(document, &[]))
            .collect();
        out.extend(next.iter().map(|(document, items)| match label {
            Some(label) => publish(document, &marked(items, label)),
            None => publish(document, items),
        }));
        self.shown = next;
        out
    }
}

fn publish(document: &str, items: &[Value]) -> Value {
    jsonrpc::notification(
        "textDocument/publishDiagnostics",
        &json!({"uri": document, "diagnostics": items}),
    )
}

/// The label on diagnostics whose successor run is queued.
pub const PENDING: &str = "stale: verification pending";
/// The label on a completed result whose inputs changed since it ran.
pub const HISTORICAL: &str = "not current: inputs changed since this run";
/// The label on a completed result whose currentness could not be established.
pub const UNKNOWN: &str = "currentness unknown: verify again for a current result";

/// Copies that are not current: information severity and a bracketed label.
/// Pending ones also carry LSP's `Unnecessary` tag so editors fade them.
fn marked(items: &[Value], label: &str) -> Vec<Value> {
    items
        .iter()
        .cloned()
        .map(|mut item| {
            if let Some(object) = item.as_object_mut() {
                let message = object
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let message = format!("[{label}] {message}");
                object.insert("message".to_owned(), json!(message));
                object.insert("severity".to_owned(), json!(3));
                if label == PENDING {
                    object.insert("tags".to_owned(), json!([1]));
                }
            }
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real failing result from the pinned Verus (see the fixture's note).
    const FAILING: &str = include_str!("../fixtures/failing.json");
    const PASSING: &str = include_str!("../fixtures/passing.json");

    fn load(text: &str, workspace: &str) -> ProjectResult {
        serde_json::from_str(&text.replace("@WORKSPACE@", workspace))
            .unwrap_or_else(|error| unreachable!("fixture: {error}"))
    }

    fn published(notifications: &[Value]) -> Vec<(String, usize)> {
        notifications
            .iter()
            .map(|n| {
                assert_eq!(n["method"], "textDocument/publishDiagnostics");
                (
                    n["params"]["uri"].as_str().unwrap_or_default().to_owned(),
                    n["params"]["diagnostics"].as_array().map_or(0, Vec::len),
                )
            })
            .collect()
    }

    #[test]
    fn a_failed_postcondition_lands_on_its_line_with_its_width_and_label() {
        let projection = project(&load(FAILING, "/w s"));
        assert_eq!(
            projection.unlocated,
            vec!["aborting due to 1 previous error".to_owned()]
        );
        assert_eq!(projection.by_uri.len(), 1);
        let items = &projection.by_uri["file:///w%20s/src/lib.rs"];
        assert_eq!(
            items,
            &vec![json!({
                // `ensures r == 2 * x,` is line 7, columns 13 to 22 (1-based).
                "range": {
                    "start": {"line": 6, "character": 12},
                    "end": {"line": 6, "character": 22},
                },
                "severity": 1,
                "source": "sail",
                "message": "postcondition not satisfied\nfailed this postcondition",
            })]
        );
    }

    #[test]
    fn a_passing_result_has_nothing_to_publish() {
        assert_eq!(project(&load(PASSING, "/w")), Projection::default());
    }

    #[test]
    fn two_targets_reporting_one_diagnostic_publish_it_once() {
        let mut result = load(FAILING, "/w");
        let again = result.targets.clone();
        result.targets.extend(again);
        let projection = project(&result);
        assert_eq!(projection.by_uri["file:///w/src/lib.rs"].len(), 1);
    }

    #[test]
    fn codes_warnings_absolute_files_and_secondary_locations() {
        let mut result = load(FAILING, "/w");
        let Some(target) = result
            .targets
            .iter_mut()
            .find(|t| !t.report.diagnostics.is_empty())
        else {
            unreachable!("no diagnostics in the fixture")
        };
        target.report.diagnostics = vec![
            Diagnostic {
                level: "warning".to_owned(),
                code: Some("E0308".to_owned()),
                message: "m".to_owned(),
                spans: vec![
                    sail_api::output::DiagnosticSpan {
                        file: "/abs/a.rs".to_owned(),
                        line: 3,
                        column: 5,
                        primary: true,
                    },
                    sail_api::output::DiagnosticSpan {
                        file: "src/b.rs".to_owned(),
                        line: 9,
                        column: 1,
                        primary: false,
                    },
                ],
                rendered: "warning[E0308]: m\n --> /abs/a.rs:3:5\n  |\n3 |     x\n  |     ^\n  |\n ::: src/b.rs:9:1".to_owned(),
            },
            Diagnostic {
                level: "note".to_owned(),
                code: None,
                message: "ignored".to_owned(),
                spans: vec![],
                rendered: String::new(),
            },
        ];
        let projection = project(&result);
        assert_eq!(projection.unlocated, Vec::<String>::new());
        assert_eq!(
            projection.by_uri["file:///abs/a.rs"],
            vec![json!({
                "range": {
                    "start": {"line": 2, "character": 4},
                    "end": {"line": 2, "character": 5},
                },
                "severity": 2,
                "source": "sail",
                "message": "m",
                "code": "E0308",
                "relatedInformation": [{
                    "location": {
                        "uri": "file:///w/src/b.rs",
                        "range": {
                            "start": {"line": 8, "character": 0},
                            "end": {"line": 8, "character": 0},
                        },
                    },
                    "message": "related location",
                }],
            })]
        );
    }

    #[test]
    fn underline_reads_only_the_caret_line_under_the_primary_line() {
        let rendered = "error: e\n --> a.rs:9:5\n  |\n7 |     requires x,\n  |              - note\n8 | {\n9 |     f(x)\n  |     ^^^^ here it is\n";
        assert_eq!(underline(rendered, 9), (4, Some("here it is".to_owned())));
        assert_eq!(underline(rendered, 7), (0, None));
        assert_eq!(underline("error: e", 1), (0, None));
    }

    #[test]
    fn clearing_policy_empties_documents_on_stale_and_after_the_fix() {
        let mut publisher = Publisher::new(Stale::Clear);
        let failing = publisher.result(&load(FAILING, "/w"));
        assert_eq!(
            published(&failing),
            vec![("file:///w/src/lib.rs".to_owned(), 1)]
        );
        assert!(publisher.showing());
        // A newer run is queued: the squiggle goes away, once.
        assert_eq!(
            published(&publisher.stale()),
            vec![("file:///w/src/lib.rs".to_owned(), 0)]
        );
        assert_eq!(publisher.stale(), Vec::<Value>::new());
        // Failing again, then fixed without an intervening stale event.
        let _ = publisher.result(&load(FAILING, "/w"));
        assert_eq!(
            published(&publisher.result(&load(PASSING, "/w"))),
            vec![("file:///w/src/lib.rs".to_owned(), 0)]
        );
        assert!(!publisher.showing());
    }

    #[test]
    fn marking_policy_demotes_then_replaces_or_clears() {
        let mut publisher = Publisher::new(Stale::Mark);
        let _ = publisher.result(&load(FAILING, "/w"));
        let stale = publisher.stale();
        assert_eq!(stale.len(), 1);
        let item = &stale[0]["params"]["diagnostics"][0];
        assert_eq!(item["severity"], 3);
        assert_eq!(item["tags"], json!([1]));
        assert!(
            item["message"]
                .as_str()
                .is_some_and(|m| m.starts_with("[stale: verification pending] postcondition"))
        );
        // Marked once per run.
        assert_eq!(publisher.stale(), Vec::<Value>::new());
        // The stale marking is not stored: a current result shows errors again.
        let current = publisher.result(&load(FAILING, "/w"));
        assert_eq!(current[0]["params"]["diagnostics"][0]["severity"], 1);
        let _ = publisher.stale();
        assert_eq!(
            published(&publisher.result(&load(PASSING, "/w"))),
            vec![("file:///w/src/lib.rs".to_owned(), 0)]
        );
    }

    #[test]
    fn a_result_that_is_not_current_is_published_labelled_never_as_current() {
        for (currentness, label) in [
            (Currentness::Historical, HISTORICAL),
            (Currentness::Unknown, UNKNOWN),
        ] {
            let mut result = load(FAILING, "/w");
            result.currentness = currentness;
            for policy in [Stale::Clear, Stale::Mark] {
                let mut publisher = Publisher::new(policy);
                let out = publisher.result(&result);
                let item = &out[0]["params"]["diagnostics"][0];
                assert_eq!(item["severity"], 3);
                assert_eq!(
                    item["message"],
                    format!("[{label}] postcondition not satisfied\nfailed this postcondition")
                );
                assert_eq!(item.get("tags"), None);
                assert_eq!(item["range"]["start"]["line"], 6);
                let queued = publisher.stale();
                if policy == Stale::Clear {
                    assert_eq!(
                        published(&queued),
                        vec![("file:///w/src/lib.rs".to_owned(), 0)]
                    );
                } else {
                    // Already labelled: a later queued run does not republish it.
                    assert_eq!(queued, Vec::<Value>::new());
                }
            }
            // A passing result of the same standing has nothing to label.
            let mut passing = load(PASSING, "/w");
            passing.currentness = currentness;
            assert_eq!(
                Publisher::new(Stale::Clear).result(&passing),
                Vec::<Value>::new()
            );
        }
    }
}
