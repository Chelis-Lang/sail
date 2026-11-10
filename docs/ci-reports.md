# CI reports

`sail report` turns the document `sail verify --output json` prints into
what a CI system shows: a job summary, annotations on failing lines, or
SARIF. It only reads the document. It runs no verification, contacts no
daemon, and copies Verus's messages verbatim.

**Status.** `sail report` is implemented and tested against result documents
produced by the pinned Verus. It has not yet run in a GitHub-hosted workflow,
and no published release contains it yet: build Sail from source to use it.

## Usage

In GitHub Actions, the [Sail action](github-action.md) runs `sail verify`
and all three reports in one step. In a `run:` step of your own:

```sh
code=0
sail verify --output json > result.json || code=$?
sail report --format github-summary result.json >> "$GITHUB_STEP_SUMMARY"
sail report --format github-annotations result.json
sail report --format sarif result.json > results.sarif
exit "$code"
```

GitHub Actions runs a `run:` step with `bash -e` (`bash -eo pipefail` under
`shell: bash`), which stops the step at the first command that fails. The
`|| code=$?` keeps a failing `sail verify` from ending the step before the
reports are written; `code=$?` on a line of its own would never run. The
step then ends with `sail verify`'s exit code. If a `sail report` call itself
fails (exit 2, for example because `sail verify` printed no document), the
step stops there and fails with that code, so a failed verification never
passes either way. `scripts/test-ci-report-snippet.sh` runs this snippet in
both shells.

| Option | Meaning |
|---|---|
| `--format github-summary` | Markdown: the result, a row per requested target, the functions that did not verify with the location and solver evidence of their failed obligations, the baseline comparison, Sail's notes (for example why a proof was not reused: `SAIL3112`, `SAIL3138`, `SAIL3146`) and Verus's diagnostics verbatim |
| `--format github-annotations` | One `::error file=…,line=…,col=…::` or `::warning` workflow command per diagnostic, ranked |
| `--format sarif` | SARIF 2.1.0 with one result per located error or warning |
| `--baseline FILE` | An earlier result document to compare with (summary only) |
| `--repo-root DIR` | The repository checkout, `$GITHUB_WORKSPACE` in Actions. Paths are reported relative to it. A relative `DIR` is taken from the current directory; the path is then compared as text and no file is opened, so name the checkout as the document's `workspace` names it (not through a symbolic link). Defaults to the document's workspace |
| `--link-base URL` | Link summary locations to `URL/PATH#LLINE` |
| `--max-annotations N` | Annotations per level, default 10 |
| `--max-bytes N` | Summary size limit, default 65536, at least 1024 |

The document is the positional argument or standard input. The exit code is
0 when the report was written, whatever the verification result, and 2 for a
usage error (`SAIL0002`), for an unreadable or unsupported document and for a
report that could not be written (both `SAIL0908`). Keep `sail verify`'s own
exit code for the job, as above.

A function with a `provisional` verdict is not verified. When a requested
target has one, the headline counts them (`… 0 unknown, 2 provisional (not
verified)`) and the target table gains a `Provisional (not verified)`
column; each is also listed among the functions that did not verify.

### A crate that stopped before verification

When a crate stops before any of its functions is checked, the summary says
which of two things happened, from the record Verus wrote for the crate:

| Summary | Meaning | Verus's record |
|---|---|---|
| `compilation failed before verification` | A Rust compile error: a type error, an unresolved name, a syntax error inside or outside `verus!`, an unknown macro | `verified: 0`, `errors: 0`, `success: false`, `encountered_vir_error: false`, no function |
| `rejected by Verus before verification` | The crate compiles as Rust and Verus rejects it: a mode error, an unsupported call, a recursive `spec fn` without `decreases`, a crate that does not import `vstd` | `verified` and `errors` absent, `success: false`, `encountered_vir_error: true`, no function |

Either way the diagnostics section holds the error itself. `sail jobs result`
uses the same wording. The match is exact: a crate with no function whose
record is neither of these keeps the plain counts (`0 verified, 0 failed, 0
unknown`). The listed cases were each run with the pinned Verus, and the
compile errors also without proof reuse (`SAIL3112`); the
[fixture notes](../crates/sail-cli/tests/fixtures/ci-report/README.md) list
them.

A failure Cargo reports before any compiler runs on the package, such as a
failing build script, leaves no target and no diagnostic in the document.
The summary then says `failed` with the exit code, the annotations hold one
error saying that the result has no error diagnostic, and Cargo's message is
only in the job log ([known issues](known-issues.md)).

### What is reported, and what is left out

- **Errors and warnings** are Verus's top-level `error` and `warning`
  diagnostics. Top-level `note` and `help` blocks are counted in the summary
  and not annotated.
- **The compiler's closing lines** (`aborting due to 2 previous errors`,
  ``could not compile `demo` ``) name no location and restate the errors
  above them. They are counted, not annotated.
- **Locations** are the diagnostic's primary (`-->`) span. Verus prints
  paths relative to the Cargo workspace root; `sail report` joins them to
  the document's `workspace` and makes them relative to `--repo-root`,
  lexically, without opening any file. A location outside the repository (a
  registry dependency) cannot carry a GitHub annotation: its annotation has
  no file and gives the path in its text, and in SARIF it is a tool
  notification of the invocation.
- **Ranking**: errors before warnings; then requested targets before
  dependencies; then locations inside the repository; then Verus's order.
- **Diagnostics are not attributed to functions.** The summary's function
  table comes from each function's obligations, and the diagnostics from
  Verus's stderr. Nothing links the two by guessing from line numbers.

### Limits

GitHub's limits are the reason for the truncation rules. Sail states what it
left out; it never drops something silently. Cargo can print one diagnostic
again for another target of the same package: a diagnostic identical to an
earlier one in level and full text is reported once, and the summary says how
many repeats there were (SARIF records the number as `repeatedDiagnostics`
on the invocation, and the annotation notice names it when it is written).

| Limit | Source | What `sail report` does |
|---|---|---|
| 10 error, 10 warning and 10 notice annotations per step; 50 per job | [actions/toolkit, problem matchers](https://github.com/actions/toolkit/blob/main/docs/problem-matchers.md#limitations). GitHub's workflow-commands page states no number | Writes at most `--max-annotations` per level, then one notice: "Annotated 10 of 37 error(s) …" |
| 1 MiB of job summary per step; 20 step summaries shown per job | [Workflow commands: adding a job summary](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#adding-a-job-summary) | Keeps the summary within `--max-bytes` (64 KiB by default). The function and baseline tables list 50 rows and say how many more there are. Diagnostics are added whole while they fit, and a closing line names what did not |
| SARIF: 10 MB gzip-compressed, 25,000 results per run of which the top 5,000 by severity are shown | [SARIF support for code scanning](https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/sarif-support-for-code-scanning) | Writes every located diagnostic. No `partialFingerprints`: the upload action computes them from the sources |

Uploading SARIF needs the `security-events: write` permission and code
scanning enabled for the repository. `sail report` writes the file; the
workflow uploads it.

### Baseline comparison

`--baseline FILE` calls the same comparison as `sail compare`
([workspace verification](workspace-verification.md)) on the two documents.
Two results are comparable only when neither ran without proof reuse
(`SAIL3112`), their `context_sha256` agree, and they requested the same
targets with the same arguments. Otherwise the summary says which of these
failed, lists the functions whose reported verdicts differ as descriptive
differences, and reports no regression.

A path package's identity includes its absolute path. Two checkouts of one
commit at different paths are therefore not comparable. GitHub-hosted
runners check a repository out at the same path in every run; a
self-hosted runner must do so too for the comparison to apply.
