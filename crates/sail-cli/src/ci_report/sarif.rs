//! SARIF 2.1.0 (`--format sarif`), the channel without an annotation limit.
//!
//! Every error and warning with a location inside the repository is a
//! result. Code scanning shows a result only at a repository file, so a
//! diagnostic with no location, or one outside the repository, is a tool
//! notification of the invocation instead; nothing is dropped. No
//! `partialFingerprints` are written: GitHub's upload action computes them
//! from the checked-out sources.

use super::{Finding, Level, Place, View};
use sail_api::output::Diagnostic;
use serde_json::{Value, json};
use std::fmt::Write as _;

const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

fn level(level: Level) -> &'static str {
    level.name()
}

fn region(line: u64, column: u64) -> Value {
    json!({ "startLine": line, "startColumn": column })
}

fn physical(file: &str, line: u64, column: u64) -> Value {
    json!({
        "artifactLocation": { "uri": uri(file), "uriBaseId": "%SRCROOT%" },
        "region": region(line, column),
    })
}

/// A repository-relative path as a relative URI reference.
fn uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// The secondary (`:::`) locations of a diagnostic that lie in the repository.
fn related(view: &View<'_>, diagnostic: &Diagnostic) -> Vec<Value> {
    let mut seen_primary = false;
    diagnostic
        .spans
        .iter()
        .filter(|span| {
            let first_primary = span.primary && !seen_primary;
            seen_primary |= span.primary;
            !first_primary
        })
        .filter_map(|span| {
            let file = view.paths.repository(&span.file)?;
            Some(json!({ "physicalLocation": physical(&file, span.line, span.column) }))
        })
        .collect()
}

fn properties(finding: &Finding<'_>) -> Value {
    json!({
        "target": finding.target.target,
        "packageId": finding.target.package_id,
        "requested": finding.target.requested,
        "establishingRun": finding.target.establishing_run,
        "rendered": finding.diagnostic.rendered,
    })
}

pub(super) fn render(view: &View<'_>) -> String {
    let result = view.result;
    let mut rules: Vec<(String, Value)> = Vec::new();
    let mut results = Vec::new();
    let mut notifications = Vec::new();
    if let Some(error) = &result.error {
        notifications.push(json!({
            "level": "error",
            "descriptor": { "id": error.code },
            "message": { "text": error.message },
        }));
    }
    for finding in &view.findings {
        let Place::Repository { file, line, column } = &finding.place else {
            let mut text = finding.diagnostic.message.clone();
            if let Place::Outside { file, line, column } = &finding.place {
                let _ = write!(text, " ({file}:{line}:{column}, outside the repository)");
            }
            notifications.push(json!({
                "level": level(finding.level),
                "descriptor": { "id": finding.rule() },
                "message": { "text": text },
                "properties": properties(finding),
            }));
            continue;
        };
        let id = finding.rule();
        let index = rules
            .iter()
            .position(|(known, _)| *known == id)
            .unwrap_or_else(|| {
                let description = if id.starts_with("verus/other-") {
                    format!("A Verus or compiler {} with no code", finding.level.name())
                } else {
                    finding.diagnostic.message.clone()
                };
                rules.push((
                    id.clone(),
                    json!({
                        "id": id,
                        "shortDescription": { "text": description },
                        "defaultConfiguration": { "level": level(finding.level) },
                    }),
                ));
                rules.len() - 1
            });
        let mut entry = json!({
            "ruleId": id,
            "ruleIndex": index,
            "level": level(finding.level),
            "message": { "text": finding.diagnostic.message },
            "locations": [{ "physicalLocation": physical(file, *line, *column) }],
            "properties": properties(finding),
        });
        let related = related(view, finding.diagnostic);
        if !related.is_empty() {
            entry["relatedLocations"] = Value::Array(related);
        }
        results.push(entry);
    }
    let mut invocation = json!({
        // Whether the tool ran to completion, not whether proofs passed.
        "executionSuccessful": result.error.is_none() && result.exit_code.is_some(),
        "toolExecutionNotifications": notifications,
        "properties": {
            "runId": result.run_id,
            "inputRevision": result.input_revision,
            "contextSha256": result.context_sha256,
            "currentness": result.currentness,
            "closingCompilerLines": view.trailers,
            "noteAndHelpBlocks": view.remarks,
            "repeatedDiagnostics": view.duplicates,
        },
    });
    if let Some(code) = result.exit_code {
        invocation["exitCode"] = json!(code);
    }
    let mut driver = json!({
        "name": "sail",
        "informationUri": "https://github.com/Chelis-Lang/sail",
        "rules": rules.into_iter().map(|(_, rule)| rule).collect::<Vec<_>>(),
    });
    // The version of the sail that wrote the document; a result with no
    // target report does not record one.
    if !view.sail_version().is_empty() {
        driver["version"] = json!(view.sail_version());
    }
    let document = json!({
        "$schema": SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": driver },
            // rustc counts columns in characters.
            "columnKind": "unicodeCodePoints",
            "invocations": [invocation],
            "results": results,
        }],
    });
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_default();
    text.push('\n');
    text
}
