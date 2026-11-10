//! `sail report` over real `sail verify --output json` documents.
//!
//! The documents under `tests/fixtures/ci-report` come from the pinned Verus
//! (see the README there). Regenerate the golden renderings with
//! `SAIL_BLESS=1 cargo test -p sail-solve --test report`.

use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const REPO_ROOT: &str = "/work/repo";
const FIXTURES: [&str; 6] = [
    "passing",
    "failing",
    "compile-error",
    "no-reuse-compile-error",
    "vir-error",
    "setup-error",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ci-report")
        .join(name)
}

fn document(name: &str) -> Value {
    let text = std::fs::read_to_string(fixture(&format!("{name}.json")))
        .unwrap_or_else(|e| unreachable!("{name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{name}: {e}"))
}

fn sail(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sail"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| unreachable!("cannot run sail: {e}"));
    if let Some(mut pipe) = child.stdin.take() {
        // A report that fails early may close its input first.
        let _ = pipe.write_all(stdin.unwrap_or_default().as_bytes());
    }
    child
        .wait_with_output()
        .unwrap_or_else(|e| unreachable!("sail: {e}"))
}

/// Renders `document` (JSON text) on standard input and returns stdout.
fn render(document: &str, args: &[&str]) -> String {
    let mut all = vec!["report"];
    all.extend_from_slice(args);
    let output = sail(&all, Some(document));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap_or_else(|e| unreachable!("{e}"))
}

fn render_fixture(name: &str, format: &str, extra: &[&str]) -> String {
    let path = fixture(&format!("{name}.json"));
    let mut args = vec![
        "report",
        "--format",
        format,
        path.to_str().unwrap_or_default(),
        "--repo-root",
        REPO_ROOT,
    ];
    args.extend_from_slice(extra);
    let output = sail(&args, None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{name} {format}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap_or_else(|e| unreachable!("{e}"))
}

fn check_golden(name: &str, text: &str) {
    let golden = fixture("golden").join(name);
    if std::env::var_os("SAIL_BLESS").is_some() {
        assert!(
            std::fs::write(&golden, text).is_ok(),
            "{}",
            golden.display()
        );
    }
    // A missing golden is a failure, never an empty expectation: one
    // golden (`passing.annotations.txt`) is legitimately empty.
    let want = match std::fs::read_to_string(&golden) {
        Ok(want) => want,
        Err(e) => unreachable!(
            "cannot read {}: {e}; rerun with SAIL_BLESS=1 to create it",
            golden.display()
        ),
    };
    assert!(
        text == want,
        "{} differs; rerun with SAIL_BLESS=1 to update:\n{text}",
        golden.display()
    );
}

/// Every golden file belongs to a fixture and every fixture has its three:
/// a deleted or stray golden fails here by name.
#[test]
fn the_golden_directory_holds_exactly_the_expected_files() {
    let mut want: Vec<String> = FIXTURES
        .iter()
        .flat_map(|name| {
            ["summary.md", "annotations.txt", "sarif"].map(|kind| format!("{name}.{kind}"))
        })
        .collect();
    want.sort();
    let mut have: Vec<String> = std::fs::read_dir(fixture("golden"))
        .unwrap_or_else(|e| unreachable!("golden: {e}"))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| unreachable!("golden: {e}"))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    have.sort();
    assert_eq!(have, want);
}

/// Every fixture in every format, byte for byte. The summary is rendered
/// against the passing run as its baseline, with source links.
#[test]
fn golden_renderings() {
    let baseline = fixture("passing.json");
    let baseline = baseline.to_str().unwrap_or_default();
    for name in FIXTURES {
        let summary = render_fixture(
            name,
            "github-summary",
            &[
                "--baseline",
                baseline,
                "--link-base",
                "https://github.com/example/demo/blob/0123abc",
            ],
        );
        check_golden(&format!("{name}.summary.md"), &summary);
        let annotations = render_fixture(name, "github-annotations", &[]);
        check_golden(&format!("{name}.annotations.txt"), &annotations);
        let sarif = render_fixture(name, "sarif", &[]);
        check_golden(&format!("{name}.sarif"), &sarif);
    }
}

/// The error diagnostics of a document that name a location: `(file, line,
/// column, message)`, read from the JSON independently of the renderer.
fn located_errors(document: &Value) -> Vec<(String, u64, u64, String)> {
    let mut found = Vec::new();
    for target in document["targets"].as_array().into_iter().flatten() {
        for diagnostic in target["report"]["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if diagnostic["level"] != "error" {
                continue;
            }
            let spans = diagnostic["spans"].as_array().cloned().unwrap_or_default();
            let Some(span) = spans.iter().find(|span| span["primary"] == true) else {
                continue;
            };
            found.push((
                span["file"].as_str().unwrap_or_default().to_owned(),
                span["line"].as_u64().unwrap_or_default(),
                span["column"].as_u64().unwrap_or_default(),
                diagnostic["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            ));
        }
    }
    found
}

/// Every Verus error that names a location gets an annotation on that file
/// and line, and a SARIF result at the same place. The only errors without
/// one are the compiler's closing lines, which name no location.
#[test]
fn every_located_error_is_annotated_and_in_sarif() {
    let mut total = 0;
    for name in FIXTURES {
        let document = document(name);
        let errors = located_errors(&document);
        let annotations = render_fixture(name, "github-annotations", &[]);
        let sarif: Value = serde_json::from_str(&render_fixture(name, "sarif", &[]))
            .unwrap_or_else(|e| unreachable!("{name}: {e}"));
        let results = sarif["runs"][0]["results"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for (file, line, column, message) in &errors {
            // The workspace is `/work/repo/proofs`, the repository `/work/repo`.
            let line_text =
                format!("::error file=proofs/{file},line={line},col={column},title=Verus error");
            assert!(
                annotations
                    .lines()
                    .any(|l| l.starts_with(&line_text) && l.ends_with(&format!("::{message}"))),
                "{name}: no annotation for {file}:{line}:{column} {message}\n{annotations}"
            );
            assert!(
                results.iter().any(|result| {
                    let place = &result["locations"][0]["physicalLocation"];
                    place["artifactLocation"]["uri"] == format!("proofs/{file}")
                        && place["region"]["startLine"] == *line
                        && place["region"]["startColumn"] == *column
                        && result["message"]["text"] == *message
                        && result["level"] == "error"
                }),
                "{name}: no SARIF result for {file}:{line}:{column} {message}"
            );
        }
        assert_eq!(results.len(), errors.len(), "{name}");
        // Unlocated errors in these documents are closing lines only.
        for target in document["targets"].as_array().into_iter().flatten() {
            for diagnostic in target["report"]["diagnostics"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let unlocated = diagnostic["spans"].as_array().is_none_or(Vec::is_empty);
                if unlocated {
                    let message = diagnostic["message"].as_str().unwrap_or_default();
                    // Verus's closing line in a managed run, Cargo's in a
                    // run without proof reuse.
                    let closing = if name == "no-reuse-compile-error" {
                        "could not compile "
                    } else {
                        "aborting due to "
                    };
                    assert!(message.starts_with(closing), "{name}: {message}");
                }
            }
        }
        total += errors.len();
    }
    // Six verification errors, the type error with and without proof reuse,
    // and one Verus mode error.
    assert_eq!(total, 9);
}

/// A crate the Rust compiler rejected and a crate Verus rejected are told
/// apart, from Verus's own result record, and neither reads `0 verified`.
#[test]
fn compile_errors_and_verus_rejections_are_told_apart() {
    for (name, title, outcome, evidence) in [
        (
            "compile-error",
            "compilation failed before verification",
            "compilation failed",
            "did not compile",
        ),
        (
            "vir-error",
            "rejected by Verus before verification",
            "rejected by Verus",
            "rejected by Verus",
        ),
    ] {
        let record = &document(name)["targets"][0]["report"]["crates"][0]["verus"];
        assert_eq!(
            record["encountered_vir_error"],
            name == "vir-error",
            "{name}"
        );
        let summary = render_fixture(name, "github-summary", &[]);
        assert!(
            summary.starts_with(&format!("## Sail verification: {title}\n")),
            "{summary}"
        );
        assert!(
            summary.contains(&format!(
                "| alpha (lib) | {outcome} | 0 | 0 | 0 | 0 | {evidence} |"
            )),
            "{summary}"
        );
        assert!(!summary.contains("0 verified"), "{summary}");
    }
}

/// The same type error verified without proof reuse (`SAIL3112`) leaves the
/// same record and reads the same way; the baseline is refused, not compared.
#[test]
fn a_compile_error_without_proof_reuse_reads_the_same() {
    let name = "no-reuse-compile-error";
    let document = document(name);
    assert_eq!(document["no_reuse"]["code"], "SAIL3112");
    assert_eq!(
        document["targets"][0]["report"]["crates"][0]["verus"],
        self::document("compile-error")["targets"][0]["report"]["crates"][0]["verus"]
    );
    let baseline = fixture("passing.json");
    let summary = render_fixture(
        name,
        "github-summary",
        &["--baseline", baseline.to_str().unwrap_or_default()],
    );
    assert!(
        summary.starts_with("## Sail verification: compilation failed before verification\n"),
        "{summary}"
    );
    assert!(
        summary.contains(
            "| cargo verus verify | compilation failed | 0 | 0 | 0 | 0 | did not compile |"
        ),
        "{summary}"
    );
    assert!(
        summary.contains("**Not comparable:** a run without proof reuse (`SAIL3112`)"),
        "{summary}"
    );
    assert!(!summary.contains("regressed"), "{summary}");
    assert!(!summary.contains("0 verified"), "{summary}");
    assert!(
        summary.contains("- SAIL3112: no proof reuse, because LD_LIBRARY_PATH is set"),
        "{summary}"
    );
}

/// A relative `--repo-root` is taken from the current directory. Paths are
/// compared lexically, so an unresolved `.` would put every file in the
/// repository under a wrong name, and any other relative root would put
/// every file outside it.
#[test]
fn a_relative_repo_root_is_resolved_against_the_current_directory() {
    let directory = std::env::temp_dir().join(format!("sail-report-root-{}", std::process::id()));
    let proofs = directory.join("repo/proofs");
    assert!(std::fs::create_dir_all(&proofs).is_ok());
    // The same canonical spelling the child's current directory has.
    let directory = directory
        .canonicalize()
        .unwrap_or_else(|e| unreachable!("{e}"));
    let root = directory.to_str().unwrap_or_default();
    let text = std::fs::read_to_string(fixture("compile-error.json"))
        .unwrap_or_default()
        .replace("/work/repo", &format!("{root}/repo"));
    let document = directory.join("result.json");
    assert!(std::fs::write(&document, text).is_ok());
    let document = document.to_str().unwrap_or_default();
    let run = |current: &str, repo_root: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_sail"))
            .args([
                "report",
                "--format",
                "github-annotations",
                document,
                "--repo-root",
                repo_root,
            ])
            .current_dir(directory.join(current))
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| unreachable!("sail: {e}"));
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let inside = "::error file=proofs/alpha/src/lib.rs,line=10,col=27,title=Verus error[E0308]::mismatched types\n";
    let absolute = format!("{root}/repo");
    for (current, repo_root, want) in [
        ("repo", absolute.as_str(), inside),
        ("repo", ".", inside),
        ("repo", "./", inside),
        ("repo/proofs", "..", inside),
        (".", "repo", inside),
        ("repo/proofs", "../../repo", inside),
        // The workspace itself as the root.
        (
            "repo/proofs",
            ".",
            "::error file=alpha/src/lib.rs,line=10,col=27,title=Verus error[E0308]::mismatched types\n",
        ),
        // A root the file is not under: no `file=`, never a wrong one.
        (
            "repo",
            "proofs/beta",
            "::error title=Verus error[E0308]::mismatched types%0Aalpha/src/lib.rs:10:27 (outside the repository)\n",
        ),
    ] {
        let (code, stdout, stderr) = run(current, repo_root);
        assert_eq!(code, Some(0), "{current} {repo_root}: {stderr}");
        assert_eq!(stdout, want, "{current} {repo_root}");
    }
    let (code, stdout, stderr) = run("repo", "");
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with("error[SAIL0002]: sail report: --repo-root needs a directory"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

/// A diagnostic Cargo printed again for another target is reported once, and
/// every format says how many repeats were left out.
#[test]
fn repeated_diagnostics_are_reported_once_and_counted() {
    let mut document = document("failing");
    let diagnostics = document["targets"][0]["report"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    // `beta` repeats two of `alpha`'s located errors word for word.
    document["targets"][1]["report"]["diagnostics"] =
        Value::Array(vec![diagnostics[0].clone(), diagnostics[4].clone()]);
    let text = document.to_string();
    let summary = render(
        &text,
        &["--format", "github-summary", "--repo-root", REPO_ROOT],
    );
    assert!(
        summary.contains("### Verus diagnostics (6 errors, 0 warnings)\n"),
        "{summary}"
    );
    assert!(
        summary.contains("Listed once: 2 more diagnostic(s) repeat one of these word for word"),
        "{summary}"
    );
    assert_eq!(summary.matches("<details>").count(), 6, "{summary}");
    let sarif: Value = serde_json::from_str(&render(
        &text,
        &["--format", "sarif", "--repo-root", REPO_ROOT],
    ))
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        sarif["runs"][0]["results"].as_array().map(Vec::len),
        Some(6)
    );
    assert_eq!(
        sarif["runs"][0]["invocations"][0]["properties"]["repeatedDiagnostics"],
        2
    );
    // All six fit GitHub's limit: no notice. With a lower limit the notice
    // names the repeats as well.
    let annotations = render(
        &text,
        &["--format", "github-annotations", "--repo-root", REPO_ROOT],
    );
    assert_eq!(annotations.lines().count(), 6, "{annotations}");
    let limited = render(
        &text,
        &[
            "--format",
            "github-annotations",
            "--repo-root",
            REPO_ROOT,
            "--max-annotations",
            "4",
        ],
    );
    let last = limited.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("::notice title=Sail report::Annotated 4 of 6 error(s)")
            && last.ends_with(
                " 2 more diagnostic(s) repeat one of these word for word and are counted once."
            ),
        "{limited}"
    );
    // The real document has no repeat and says nothing about any.
    let plain = render_fixture("failing", "github-summary", &[]);
    assert!(!plain.contains("Listed once"), "{plain}");
}

/// A provisional function is not verified: the headline and the target row
/// count it apart, and it is listed among the functions that did not verify.
#[test]
fn provisional_functions_are_counted_apart() {
    let mut document = document("passing");
    let report = &mut document["targets"][0]["report"];
    let function = report["crates"][0]["functions"][0].clone();
    let name = function["function"].as_str().unwrap_or_default().to_owned();
    assert_eq!(function["verdict"], "verified");
    report["crates"][0]["functions"][0]["verdict"] = "provisional".into();
    let verified = report["summary"]["verified"].as_u64().unwrap_or_default();
    assert_eq!(verified, 5);
    report["summary"]["verified"] = (verified - 1).into();
    report["summary"]["provisional"] = 1.into();
    let summary = render(
        &document.to_string(),
        &["--format", "github-summary", "--repo-root", REPO_ROOT],
    );
    assert!(
        summary.contains("**5 verified, 0 failed, 0 unknown, 1 provisional (not verified)"),
        "{summary}"
    );
    assert!(
        summary.contains(
            "| Target | Result | Functions | Verified | Failed | Unknown | Provisional (not verified) | Evidence |\n|---|---|--:|--:|--:|--:|--:|---|\n| alpha (lib) | passed | 5 | 4 | 0 | 0 | 1 |"
        ),
        "{summary}"
    );
    assert!(
        summary.contains("| beta (lib) | passed | 1 | 1 | 0 | 0 | 0 |"),
        "{summary}"
    );
    assert!(
        summary.contains(&format!("| `{name}` | provisional | alpha |")),
        "{summary}"
    );
    // Without one, the table has no such column.
    let plain = render_fixture("passing", "github-summary", &[]);
    assert!(!plain.contains("rovisional"), "{plain}");
}

/// The snippet the guide and the consumer skill document, run as a GitHub
/// Actions step (`bash -e`, `bash -eo pipefail`) with this `sail report`: a
/// failing verification still writes its reports and fails the step with
/// `sail verify`'s exit code.
#[cfg(unix)]
#[test]
fn the_documented_ci_snippet_reports_a_failing_verification() {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/test-ci-report-snippet.sh");
    let output = Command::new("bash")
        .arg(&script)
        .env("SAIL_BIN", env!("CARGO_BIN_EXE_sail"))
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| unreachable!("bash: {e}"));
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("ci report snippet: 13 checks passed"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// A failing run always annotates an error, also when Sail could not start
/// Verus and there is no diagnostic at all.
#[test]
fn a_setup_failure_is_one_unlocated_error() {
    let annotations = render_fixture("setup-error", "github-annotations", &[]);
    assert_eq!(
        annotations,
        "::error title=Sail SAIL3100::error[SAIL0102]: sail-z3 must be installed next to sail\n"
    );
    let summary = render_fixture("setup-error", "github-summary", &[]);
    assert!(
        summary.starts_with("## Sail verification: did not run\n"),
        "{summary}"
    );
    assert_eq!(render_fixture("passing", "github-annotations", &[]), "");
}

/// The failing document, made large: `functions` copies of each failing
/// function and `copies` of each diagnostic, at distinct lines.
fn enlarged(functions: usize, copies: usize) -> String {
    let mut document = document("failing");
    let report = &mut document["targets"][0]["report"];
    let originals = report["crates"][0]["functions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut all = Vec::new();
    for index in 0..functions {
        for function in &originals {
            let mut function = function.clone();
            let name = format!(
                "{}_{index}",
                function["function"].as_str().unwrap_or_default()
            );
            function["function"] = Value::String(name);
            // The copies carry no solver model.
            for obligation in function["obligations"].as_array_mut().into_iter().flatten() {
                obligation["counterexample"] = Value::Null;
            }
            all.push(function);
        }
    }
    let count = all.len();
    report["crates"][0]["functions"] = Value::Array(all);
    report["summary"]["functions"] = count.into();
    report["summary"]["failed"] = count.into();
    report["crates"][0]["verus"]["errors"] = count.into();
    let originals = report["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut all = Vec::new();
    for index in 0..copies {
        for diagnostic in &originals {
            let mut diagnostic = diagnostic.clone();
            if let Some(span) = diagnostic["spans"].get_mut(0) {
                span["line"] = (1000 + index).into();
                let rendered = diagnostic["rendered"].as_str().unwrap_or_default();
                diagnostic["rendered"] = format!("{rendered}\n(copy {index})").into();
            }
            all.push(diagnostic);
        }
    }
    report["diagnostics"] = Value::Array(all);
    document.to_string()
}

/// Synthetic scale (the largest real document available here has 1877
/// functions and six errors): 1,000 failing functions and 3,000 errors, far
/// past every limit.
/// The summary stays within its limit and says what it left out, the
/// annotations stay within GitHub's per-step limit and say so, and SARIF
/// holds every error.
#[test]
fn large_results_are_bounded_with_stated_truncation() {
    let document = enlarged(200, 500);
    assert!(document.len() > 1_000_000, "{}", document.len());
    let summary = render(
        &document,
        &["--format", "github-summary", "--repo-root", REPO_ROOT],
    );
    assert!(summary.len() <= 64 * 1024, "{}", summary.len());
    assert!(summary.len() > 32 * 1024, "{}", summary.len());
    assert!(
        summary.contains("### Functions that did not verify (1000)"),
        "{summary}"
    );
    assert!(
        summary.contains("950 more function(s) are not listed"),
        "{summary}"
    );
    let last = summary.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("_Left out to keep this summary under 65536 bytes: ")
            && last.contains(" of 3000 Verus diagnostics (ranked: errors first)"),
        "{last}"
    );
    // Every block that was started is closed.
    assert_eq!(
        summary.matches("<details>").count(),
        summary.matches("</details>").count()
    );

    let small = render(
        &document,
        &[
            "--format",
            "github-summary",
            "--repo-root",
            REPO_ROOT,
            "--max-bytes",
            "2000",
        ],
    );
    assert!(small.len() <= 2000, "{}", small.len());
    assert!(
        small.starts_with("## Sail verification: failed\n"),
        "{small}"
    );
    assert!(
        small.contains("_Left out to keep this summary under 2000 bytes: "),
        "{small}"
    );

    let annotations = render(
        &document,
        &["--format", "github-annotations", "--repo-root", REPO_ROOT],
    );
    let lines: Vec<&str> = annotations.lines().collect();
    assert_eq!(lines.len(), 11, "{annotations}");
    assert!(
        lines[..10]
            .iter()
            .all(|line| line.starts_with("::error file=proofs/alpha/src/lib.rs,line="))
    );
    assert!(
        lines[10].starts_with(
            "::notice title=Sail report::Annotated 10 of 3000 error(s) and 0 of 0 warning(s)"
        ),
        "{}",
        lines[10]
    );
    let three = render(
        &document,
        &[
            "--format",
            "github-annotations",
            "--repo-root",
            REPO_ROOT,
            "--max-annotations=3",
        ],
    );
    assert_eq!(three.lines().count(), 4, "{three}");

    let sarif: Value = serde_json::from_str(&render(
        &document,
        &["--format", "sarif", "--repo-root", REPO_ROOT],
    ))
    .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(
        sarif["runs"][0]["results"].as_array().map(Vec::len),
        Some(3000)
    );
}

/// `sail_api::evidence::compare` decides comparability; the summary says
/// which of its conditions failed and reports no regression then.
#[test]
fn an_incomparable_baseline_is_explained_not_reported_as_regressions() {
    let failing = std::fs::read_to_string(fixture("failing.json")).unwrap_or_default();
    let directory = std::env::temp_dir().join(format!("sail-report-test-{}", std::process::id()));
    assert!(std::fs::create_dir_all(&directory).is_ok());
    let write = |name: &str, text: &str| {
        let path = directory.join(name);
        assert!(std::fs::write(&path, text).is_ok());
        path.to_str().unwrap_or_default().to_owned()
    };
    let passing = std::fs::read_to_string(fixture("passing.json")).unwrap_or_default();

    // The same commit checked out at another path: a path package's
    // identity includes the path, so the requested scopes differ.
    let moved = write(
        "moved.json",
        &passing.replace("/work/repo", "/elsewhere/repo"),
    );
    let summary = render(
        &failing,
        &["--format", "github-summary", "--baseline", &moved],
    );
    assert!(
        summary.contains("**Not comparable:** the requested targets, mode or arguments differ; the workspaces are at different paths (`/elsewhere/repo/proofs` against `/work/repo/proofs`)"),
        "{summary}"
    );
    assert!(!summary.contains("regressed"), "{summary}");
    assert!(
        summary.contains("| `alpha::max` | alpha | incomparable | not observed → failed |"),
        "{summary}"
    );

    // Another Sail, Verus or configuration: the context digest differs.
    let mut other: Value = serde_json::from_str(&passing).unwrap_or_default();
    other["context_sha256"] = "00".repeat(32).into();
    let other = write("other.json", &other.to_string());
    let summary = render(
        &failing,
        &["--format", "github-summary", "--baseline", &other],
    );
    assert!(
        summary.contains("**Not comparable:** the verification configuration or executable identity differs (context `000000000000` against `720206fbc41d`)"),
        "{summary}"
    );
    assert!(
        summary.contains("| `alpha::max` | alpha | incomparable | verified → failed |"),
        "{summary}"
    );

    // The same checkout and context: regressions are reported.
    let same = fixture("passing.json");
    let summary = render(
        &failing,
        &[
            "--format",
            "github-summary",
            "--baseline",
            same.to_str().unwrap_or_default(),
        ],
    );
    assert!(
        summary.contains("**5 regressed, 0 became unknown, 0 recovered**"),
        "{summary}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

/// Dependencies are summarized on one line, never listed function by
/// function, and their diagnostics rank after the requested targets'.
#[test]
fn dependencies_are_summarized_and_ranked_last() {
    let mut document = document("failing");
    // Present `alpha` as a dependency of the requested `beta`.
    document["targets"][0]["requested"] = false.into();
    let mut warning = document["targets"][0]["report"]["diagnostics"][0].clone();
    warning["level"] = "warning".into();
    warning["message"] = "unused variable: `x`".into();
    warning["rendered"] = "warning: unused variable: `x`\n --> beta/src/lib.rs:3:1".into();
    warning["spans"][0]["file"] = "/registry/dep-1.0.0/src/lib.rs".into();
    document["targets"][1]["report"]["diagnostics"] = Value::Array(vec![warning]);
    let text = document.to_string();
    let summary = render(
        &text,
        &["--format", "github-summary", "--repo-root", REPO_ROOT],
    );
    assert!(
        summary.contains("Dependencies, not listed above: alpha (5 functions; 0 of 1 reused; **1 with failed functions**)."),
        "{summary}"
    );
    assert!(summary.contains("in 1 requested target."), "{summary}");
    let annotations = render(
        &text,
        &["--format", "github-annotations", "--repo-root", REPO_ROOT],
    );
    let lines: Vec<&str> = annotations.lines().collect();
    assert_eq!(lines.len(), 7, "{annotations}");
    // The warning is outside the repository: no `file=`, the path in the text.
    assert_eq!(
        lines[6],
        "::warning title=Verus warning::unused variable: `x`%0A/registry/dep-1.0.0/src/lib.rs:12:9 (outside the repository)"
    );
    let sarif: Value = serde_json::from_str(&render(
        &text,
        &["--format", "sarif", "--repo-root", REPO_ROOT],
    ))
    .unwrap_or_else(|e| unreachable!("{e}"));
    let notifications = &sarif["runs"][0]["invocations"][0]["toolExecutionNotifications"];
    assert_eq!(
        notifications.as_array().map(Vec::len),
        Some(1),
        "{notifications}"
    );
    assert_eq!(notifications[0]["descriptor"]["id"], "verus/other-warning");
}

/// Usage and input errors exit 2 with a stable code and write no report.
#[test]
fn usage_and_input_errors() {
    let passing = fixture("passing.json");
    let passing = passing.to_str().unwrap_or_default();
    for (args, code) in [
        (vec!["report", passing], "SAIL0002"),
        (vec!["report", "--format", "html", passing], "SAIL0002"),
        (
            vec![
                "report",
                "--format",
                "github-summary",
                passing,
                "--max-bytes",
                "1023",
            ],
            "SAIL0002",
        ),
        (
            vec!["report", "--format", "sarif", "/nonexistent/result.json"],
            "SAIL0908",
        ),
        (
            vec![
                "report",
                "--format",
                "sarif",
                passing,
                "--baseline",
                "/nonexistent/b.json",
            ],
            "SAIL0908",
        ),
    ] {
        let output = sail(&args, None);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.starts_with(&format!("error[{code}]: sail report: ")),
            "{args:?}: {stderr}"
        );
    }
    let output = sail(
        &["report", "--format", "sarif"],
        Some("{\"kind\":\"other\"}"),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("error[SAIL0908]: sail report: standard input: ")
    );
    let output = sail(&["report", "--help"], None);
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("sail report --format FORMAT"));
}
