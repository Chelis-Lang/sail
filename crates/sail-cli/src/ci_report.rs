//! `sail report`: renders one completed `sail-project-result` document for
//! continuous integration.
//!
//! Three formats, all pure functions of the document (and of an optional
//! baseline document): a Markdown job summary, GitHub workflow-command
//! annotations and SARIF 2.1.0. Nothing here runs Verus, reads a source file
//! or contacts the daemon, and no verdict or diagnostic is reworded: Verus's
//! messages and rendered blocks are copied verbatim.

mod annotations;
mod sarif;
mod summary;

use sail_api::output::{Diagnostic, DiagnosticSpan};
use sail_api::project::{ProjectResult, TargetResult};
use std::io::{Read as _, Write};
use std::process::ExitCode;

/// `sail report --help`.
pub(crate) const USAGE: &str = "\
Render a completed `sail verify --output json` document for CI. Reads the
document only: it runs no verification and never changes a result.

USAGE:
    sail report --format FORMAT [RESULT.json | -] [--baseline FILE]
                [--repo-root DIR] [--link-base URL]
                [--max-annotations N] [--max-bytes N]

FORMATS:
    github-summary       Markdown for $GITHUB_STEP_SUMMARY: totals per requested
                         target, functions that did not verify, the baseline
                         comparison, Sail's notes and Verus's diagnostics
    github-annotations   `::error file=,line=,col=::` workflow commands, ranked;
                         at most N per level (default 10, GitHub's per-step
                         limit), with one notice stating what was left out
    sarif                SARIF 2.1.0 holding every located diagnostic

OPTIONS:
    --baseline FILE      an earlier result document; the summary then states
                         what changed, or why the two are not comparable
    --repo-root DIR      the repository checkout (in GitHub Actions,
                         $GITHUB_WORKSPACE); paths are reported relative to it.
                         A relative DIR is taken from the current directory.
                         Defaults to the document's workspace
    --link-base URL      link summary locations to URL/PATH#LLINE
    --max-annotations N  annotations per level (default 10)
    --max-bytes N        summary size limit (default 65536, at least 1024)

RESULT.json defaults to standard input. Exits 0 when the report was written,
whatever the verification result, and 2 on a usage error, an unreadable or
unsupported document, or a report that could not be written.
";

/// GitHub shows at most this many annotations of one level per step.
const DEFAULT_ANNOTATIONS: usize = 10;
/// Default summary limit. GitHub rejects a step summary over 1 MiB; 64 KiB
/// also fits a check run's text field.
const DEFAULT_SUMMARY_BYTES: usize = 64 * 1024;
/// The smallest summary limit accepted. Below it the closing line that states
/// what was left out might not fit, and the summary could be silently empty.
const MIN_SUMMARY_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    GithubSummary,
    GithubAnnotations,
    Sarif,
}

/// What the command line asked for.
#[derive(Debug)]
struct Options {
    format: Format,
    /// `None` reads standard input.
    input: Option<String>,
    baseline: Option<String>,
    repo_root: Option<String>,
    link_base: Option<String>,
    max_annotations: usize,
    max_bytes: usize,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut format = None;
    let mut options = Options {
        format: Format::GithubSummary,
        input: None,
        baseline: None,
        repo_root: None,
        link_base: None,
        max_annotations: DEFAULT_ANNOTATIONS,
        max_bytes: DEFAULT_SUMMARY_BYTES,
    };
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = || {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let number = |text: String| {
            text.parse::<usize>()
                .map_err(|_| format!("{name} needs a whole number, not `{text}`"))
        };
        match name {
            "--format" => {
                format = Some(match value()?.as_str() {
                    "github-summary" => Format::GithubSummary,
                    "github-annotations" => Format::GithubAnnotations,
                    "sarif" => Format::Sarif,
                    other => {
                        return Err(format!(
                            "unknown format `{other}` (github-summary, github-annotations or sarif)"
                        ));
                    }
                });
            }
            "--baseline" => options.baseline = Some(value()?),
            "--repo-root" => options.repo_root = Some(value()?),
            "--link-base" => options.link_base = Some(value()?),
            "--max-annotations" => options.max_annotations = number(value()?)?,
            "--max-bytes" => options.max_bytes = number(value()?)?,
            "-" => positional.push(arg.clone()),
            _ if name.starts_with('-') => return Err(format!("unknown option `{arg}`")),
            _ => positional.push(arg.clone()),
        }
    }
    options.format = format.ok_or("--format is required")?;
    if options.max_bytes < MIN_SUMMARY_BYTES {
        return Err(format!(
            "--max-bytes must be at least {MIN_SUMMARY_BYTES}, not {}",
            options.max_bytes
        ));
    }
    match positional.as_slice() {
        [] => {}
        [one] if one == "-" => {}
        [one] => options.input = Some(one.clone()),
        _ => return Err("expected at most one RESULT.json".to_owned()),
    }
    Ok(options)
}

/// `--repo-root` as an absolute path. Paths are compared lexically, so a
/// relative root would match nothing, or (for `.`) everything: it is joined
/// to `current`, the current directory. No file is opened.
fn absolute_root(root: &str, current: Option<&str>) -> Result<String, String> {
    if root.is_empty() {
        return Err("--repo-root needs a directory, not an empty value".to_owned());
    }
    if root.starts_with('/') {
        return Ok(root.to_owned());
    }
    match current {
        Some(current) if current.starts_with('/') => Ok(format!("{current}/{root}")),
        _ => Err(format!(
            "--repo-root `{root}` is relative and the current directory is not available as an absolute UTF-8 path; pass an absolute path"
        )),
    }
}

/// Reads and checks one `sail-project-result` document.
pub(crate) fn document(path: Option<&str>) -> Result<ProjectResult, String> {
    let name = path.unwrap_or("standard input");
    let text = if let Some(path) = path {
        std::fs::read_to_string(path)
    } else {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map(|_| text)
    }
    .map_err(|e| format!("cannot read {name}: {e}"))?;
    parse(&text).map_err(|e| format!("{name}: {e}"))
}

/// Parses a document, refusing anything but the supported envelope.
fn parse(text: &str) -> Result<ProjectResult, String> {
    let result: ProjectResult = serde_json::from_str(text).map_err(|e| {
        format!("not a sail-project-result document (`sail verify --output json`): {e}")
    })?;
    if result.kind != "sail-project-result" {
        return Err(format!(
            "`kind` is `{}`, not `sail-project-result`",
            result.kind
        ));
    }
    if result.schema_version != sail_api::project::SCHEMA_VERSION {
        return Err(format!(
            "unsupported sail-project-result schema version {} (this sail reads version {})",
            result.schema_version,
            sail_api::project::SCHEMA_VERSION
        ));
    }
    Ok(result)
}

/// `sail report`.
pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let options = options(args).and_then(|mut options| {
        if let Some(root) = options.repo_root.take() {
            let current = std::env::current_dir().ok();
            let current = current.as_deref().and_then(std::path::Path::to_str);
            options.repo_root = Some(absolute_root(&root, current)?);
        }
        Ok(options)
    });
    let options = match options {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0002]: sail report: {message}\n\n{USAGE}");
            return ExitCode::from(crate::EXIT_USAGE);
        }
    };
    let read = || -> Result<(ProjectResult, Option<ProjectResult>), String> {
        let result = document(options.input.as_deref())?;
        let baseline = options
            .baseline
            .as_deref()
            .map(|path| document(Some(path)))
            .transpose()?;
        Ok((result, baseline))
    };
    let (result, baseline) = match read() {
        Ok(documents) => documents,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0908]: sail report: {message}");
            return ExitCode::from(crate::EXIT_USAGE);
        }
    };
    let text = render(&result, baseline.as_ref(), &options);
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0908]: sail report: cannot write: {e}");
            ExitCode::from(crate::EXIT_USAGE)
        }
    }
}

fn render(result: &ProjectResult, baseline: Option<&ProjectResult>, options: &Options) -> String {
    let paths = Paths::new(&result.workspace, options.repo_root.as_deref());
    let view = View::of(result, &paths);
    match options.format {
        Format::GithubSummary => summary::render(
            &view,
            baseline,
            &summary::Limits {
                bytes: options.max_bytes,
                link_base: options.link_base.as_deref(),
            },
        ),
        Format::GithubAnnotations => annotations::render(&view, options.max_annotations),
        Format::Sarif => sarif::render(&view),
    }
}

/// Maps the paths Verus printed to repository-relative paths, lexically.
///
/// Cargo runs the compiler in the workspace root, so a relative path is
/// relative to the document's `workspace`. No file is opened. Both roots
/// are absolute: the document's workspace is, and [`absolute_root`] makes
/// `--repo-root` so.
#[derive(Debug)]
struct Paths {
    workspace: Vec<String>,
    root: Vec<String>,
}

/// Applies `path`'s components to `parts`: `.` is dropped and `..` removes
/// the last one. `None` when `..` climbs past the start.
fn apply(mut parts: Vec<String>, path: &str) -> Option<Vec<String>> {
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part.to_owned()),
        }
    }
    Some(parts)
}

impl Paths {
    fn new(workspace: &str, repo_root: Option<&str>) -> Self {
        let workspace = apply(Vec::new(), workspace).unwrap_or_default();
        let root = repo_root
            .and_then(|root| apply(Vec::new(), root))
            .unwrap_or_else(|| workspace.clone());
        Self { workspace, root }
    }

    /// `file` relative to the repository root, with `/` separators. `None`
    /// when it lies outside the repository (a registry dependency, say).
    fn repository(&self, file: &str) -> Option<String> {
        let base = if file.starts_with('/') {
            Vec::new()
        } else {
            self.workspace.clone()
        };
        let absolute = apply(base, file)?;
        let rest = absolute.strip_prefix(self.root.as_slice())?;
        (!rest.is_empty()).then(|| rest.join("/"))
    }
}

/// Whether a diagnostic is an error or a warning. Verus's top-level `note`
/// and `help` blocks are counted, never annotated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Error,
    Warning,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// Where a finding points.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Place {
    /// A file inside the repository, as a repository-relative path.
    Repository {
        file: String,
        line: u64,
        column: u64,
    },
    /// A location outside the repository, as Verus printed it.
    Outside {
        file: String,
        line: u64,
        column: u64,
    },
    /// The diagnostic names no location.
    Nowhere,
}

/// One error or warning diagnostic of one target.
#[derive(Debug)]
struct Finding<'a> {
    level: Level,
    target: &'a TargetResult,
    diagnostic: &'a Diagnostic,
    place: Place,
}

impl Finding<'_> {
    /// The rule this finding belongs to: the compiler's code when the
    /// diagnostic has one, a name made from a fixed message such as
    /// `postcondition not satisfied`, and a general rule for any message
    /// that carries identifiers or other variable text.
    fn rule(&self) -> String {
        if let Some(code) = &self.diagnostic.code {
            return format!("rustc/{code}");
        }
        let message = &self.diagnostic.message;
        let fixed = !message.is_empty()
            && message.split(' ').count() <= 8
            && message
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '-');
        if fixed {
            format!("verus/{}", message.replace(' ', "-"))
        } else {
            format!("verus/other-{}", self.level.name())
        }
    }
}

/// The compiler's closing lines (`aborting due to 2 previous errors`,
/// ``could not compile `demo` ``). They restate the errors above them and
/// name no location, so they are counted and not reported one by one.
fn trailer(diagnostic: &Diagnostic) -> bool {
    diagnostic.spans.is_empty()
        && (diagnostic.message.starts_with("aborting due to ")
            || diagnostic.message.starts_with("could not compile "))
}

fn primary(diagnostic: &Diagnostic) -> Option<&DiagnosticSpan> {
    diagnostic
        .spans
        .iter()
        .find(|span| span.primary)
        .or_else(|| diagnostic.spans.first())
}

/// A document prepared for rendering: its findings in ranked order.
#[derive(Debug)]
struct View<'a> {
    result: &'a ProjectResult,
    paths: &'a Paths,
    /// Errors before warnings; within a level, requested targets before
    /// dependencies, then locations inside the repository, then the order
    /// Verus printed them.
    findings: Vec<Finding<'a>>,
    /// Closing compiler lines left out of `findings`.
    trailers: usize,
    /// Top-level `note` and `help` diagnostics left out of `findings`.
    remarks: usize,
    /// Errors and warnings left out of `findings` because an identical one
    /// (same level, same rendered text) is already there.
    duplicates: usize,
}

impl<'a> View<'a> {
    fn of(result: &'a ProjectResult, paths: &'a Paths) -> Self {
        let mut view = Self {
            result,
            paths,
            findings: Vec::new(),
            trailers: 0,
            remarks: 0,
            duplicates: 0,
        };
        let mut seen = std::collections::BTreeSet::new();
        for target in &result.targets {
            for diagnostic in &target.report.diagnostics {
                let level = match diagnostic.level.as_str() {
                    "error" => Level::Error,
                    "warning" => Level::Warning,
                    _ => {
                        view.remarks += 1;
                        continue;
                    }
                };
                if trailer(diagnostic) {
                    view.trailers += 1;
                    continue;
                }
                // Cargo can print one diagnostic for several targets of a package.
                if !seen.insert((level, &diagnostic.rendered)) {
                    view.duplicates += 1;
                    continue;
                }
                let place = match primary(diagnostic) {
                    None => Place::Nowhere,
                    Some(span) => match paths.repository(&span.file) {
                        Some(file) => Place::Repository {
                            file,
                            line: span.line,
                            column: span.column,
                        },
                        None => Place::Outside {
                            file: span.file.clone(),
                            line: span.line,
                            column: span.column,
                        },
                    },
                };
                view.findings.push(Finding {
                    level,
                    target,
                    diagnostic,
                    place,
                });
            }
        }
        let requested = view.any_requested();
        // Stable: equal keys keep the order Verus printed them.
        view.findings.sort_by_key(|finding| {
            (
                finding.level,
                requested && !finding.target.requested,
                !matches!(finding.place, Place::Repository { .. }),
            )
        });
        view
    }

    fn any_requested(&self) -> bool {
        self.result.targets.iter().any(|target| target.requested)
    }

    /// The targets the invocation asked for; every target when the document
    /// marks none.
    fn requested(&self) -> impl Iterator<Item = &'a TargetResult> {
        let any = self.any_requested();
        self.result
            .targets
            .iter()
            .filter(move |target| target.requested || !any)
    }

    /// The other targets: dependencies verified or reused along the way.
    fn dependencies(&self) -> impl Iterator<Item = &'a TargetResult> {
        let any = self.any_requested();
        self.result
            .targets
            .iter()
            .filter(move |target| any && !target.requested)
    }

    fn count(&self, level: Level) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }

    /// The version of the `sail` that wrote the document.
    fn sail_version(&self) -> &'a str {
        self.result
            .targets
            .first()
            .map_or("", |target| target.report.sail_version.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|&arg| arg.to_owned()).collect()
    }

    #[test]
    fn paths_are_relative_to_the_repository_root() {
        let nested = Paths::new("/work/repo/proofs", Some("/work/repo/"));
        assert_eq!(
            nested.repository("alpha/src/lib.rs").as_deref(),
            Some("proofs/alpha/src/lib.rs")
        );
        assert_eq!(
            nested.repository("./alpha/../beta/src/lib.rs").as_deref(),
            Some("proofs/beta/src/lib.rs")
        );
        assert_eq!(
            nested.repository("/work/repo/build.rs").as_deref(),
            Some("build.rs")
        );
        assert_eq!(
            nested.repository("../shared/lib.rs").as_deref(),
            Some("shared/lib.rs")
        );
        // Outside the repository: a registry dependency, or a climb past it.
        assert_eq!(nested.repository("/home/u/.cargo/registry/x/lib.rs"), None);
        assert_eq!(nested.repository("../../elsewhere/lib.rs"), None);
        assert_eq!(nested.repository("/work/repository/lib.rs"), None);
        // Without a repository root, the workspace is the root.
        let plain = Paths::new("/work/repo", None);
        assert_eq!(
            plain.repository("src/lib.rs").as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(plain.repository("/work/other/src/lib.rs"), None);
    }

    #[test]
    fn a_relative_repository_root_is_taken_from_the_current_directory() {
        let current = Some("/work/repo/proofs");
        for (root, file) in [
            ("/work/repo", Some("proofs/alpha/src/lib.rs")),
            (".", Some("alpha/src/lib.rs")),
            ("./", Some("alpha/src/lib.rs")),
            ("..", Some("proofs/alpha/src/lib.rs")),
            ("../", Some("proofs/alpha/src/lib.rs")),
            ("alpha", Some("src/lib.rs")),
            ("alpha/..", Some("alpha/src/lib.rs")),
            // Not where the file is: outside, never a wrong path.
            ("beta", None),
            ("../../elsewhere", None),
        ] {
            let root = absolute_root(root, current).unwrap_or_else(|e| unreachable!("{e}"));
            assert!(root.starts_with('/'), "{root}");
            let paths = Paths::new("/work/repo/proofs", Some(&root));
            assert_eq!(
                paths.repository("alpha/src/lib.rs").as_deref(),
                file,
                "{root}"
            );
            // A registry dependency is outside whatever the root is.
            assert_eq!(paths.repository("/home/u/.cargo/registry/x/lib.rs"), None);
        }
        // An absolute root does not need the current directory.
        assert_eq!(
            absolute_root("/work/repo", None).as_deref(),
            Ok("/work/repo")
        );
        for (root, current) in [("", current), (".", None), ("repo", Some("relative"))] {
            assert!(
                absolute_root(root, current).is_err(),
                "{root:?} {current:?}"
            );
        }
    }

    #[test]
    fn options_need_a_format_and_at_most_one_document() {
        let parsed = options(&args(&[
            "result.json",
            "--format=sarif",
            "--baseline",
            "main.json",
            "--max-annotations",
            "3",
        ]))
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(parsed.format, Format::Sarif);
        assert_eq!(parsed.input.as_deref(), Some("result.json"));
        assert_eq!(parsed.baseline.as_deref(), Some("main.json"));
        assert_eq!(parsed.max_annotations, 3);
        assert_eq!(
            options(&args(&["--format", "github-summary", "--max-bytes=1024"]))
                .map(|o| o.max_bytes)
                .ok(),
            Some(1024)
        );
        assert_eq!(
            options(&args(&["--format", "github-summary", "-"]))
                .map(|o| o.input)
                .ok(),
            Some(None)
        );
        for bad in [
            &["result.json"][..],
            &["--format", "html"],
            &["--format"],
            &["--format", "sarif", "a.json", "b.json"],
            &["--format", "sarif", "--max-bytes", "many"],
            &["--format", "github-summary", "--max-bytes", "1023"],
            &["--format", "sarif", "--colour"],
        ] {
            assert!(options(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn other_documents_are_refused() {
        assert!(parse("{}").is_err());
        assert!(parse("not json").is_err());
        let report = include_str!("../tests/fixtures/attr/report.golden.json");
        assert!(parse(report).is_err(), "a bare report is not an envelope");
    }
}
