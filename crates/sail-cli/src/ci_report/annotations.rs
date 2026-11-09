//! GitHub workflow-command annotations (`--format github-annotations`).
//!
//! GitHub keeps at most 10 error, 10 warning and 10 notice annotations per
//! step, and 50 per job, and drops the rest without saying so. This writer
//! therefore ranks the findings, writes at most `limit` of each level, and
//! states what it left out in one notice.

use super::{Level, Place, View};
use std::fmt::Write as _;

/// Escapes a workflow command's message, as GitHub's own toolkit does.
fn data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes a workflow command's property value.
fn property(text: &str) -> String {
    data(text).replace(':', "%3A").replace(',', "%2C")
}

fn command(out: &mut String, level: &str, properties: &[(&str, String)], message: &str) {
    let _ = write!(out, "::{level}");
    for (index, (name, value)) in properties.iter().enumerate() {
        let _ = write!(
            out,
            "{}{name}={}",
            if index == 0 { ' ' } else { ',' },
            property(value)
        );
    }
    let _ = writeln!(out, "::{}", data(message));
}

pub(super) fn render(view: &View<'_>, limit: usize) -> String {
    let mut out = String::new();
    let result = view.result;
    let mut errors = 0usize;
    if let Some(error) = &result.error {
        if limit > 0 {
            command(
                &mut out,
                "error",
                &[("title", format!("Sail {}", error.code))],
                &error.message,
            );
        }
        errors += 1;
    }
    let mut written = [usize::from(errors > 0 && limit > 0), 0];
    for finding in &view.findings {
        let slot = match finding.level {
            Level::Error => 0,
            Level::Warning => 1,
        };
        if written[slot] >= limit {
            continue;
        }
        written[slot] += 1;
        let title = match &finding.diagnostic.code {
            Some(code) => format!("Verus {}[{code}]", finding.level.name()),
            None => format!("Verus {}", finding.level.name()),
        };
        let message = &finding.diagnostic.message;
        match &finding.place {
            Place::Repository { file, line, column } => command(
                &mut out,
                finding.level.name(),
                &[
                    ("file", file.clone()),
                    ("line", line.to_string()),
                    ("col", column.to_string()),
                    ("title", title),
                ],
                message,
            ),
            // GitHub attaches an annotation only to a file of the repository.
            Place::Outside { file, line, column } => command(
                &mut out,
                finding.level.name(),
                &[("title", title)],
                &format!("{message}\n{file}:{line}:{column} (outside the repository)"),
            ),
            Place::Nowhere => command(&mut out, finding.level.name(), &[("title", title)], message),
        }
    }
    let total = [
        errors + view.count(Level::Error),
        view.count(Level::Warning),
    ];
    // A failing run always carries at least one error annotation.
    if total[0] == 0 && result.exit_code != Some(0) && limit > 0 {
        let exit = result.exit_code.map_or_else(
            || "no exit status".to_owned(),
            |code| format!("exit code {code}"),
        );
        command(
            &mut out,
            "error",
            &[("title", "Sail".to_owned())],
            &format!(
                "Verification did not pass ({exit}) and the result holds no error diagnostic."
            ),
        );
    }
    if written[0] < total[0] || written[1] < total[1] {
        command(
            &mut out,
            "notice",
            &[("title", "Sail report".to_owned())],
            &format!(
                "Annotated {} of {} error(s) and {} of {} warning(s), ranked: requested targets before dependencies, repository files first, then Verus's order. GitHub keeps at most 10 annotations of a level per step. The job summary and `sail report --format sarif` list every diagnostic.",
                written[0], total[0], written[1], total[1]
            ),
        );
    }
    out
}
