# `sail report` fixtures

Real `sail verify --output json` documents, and the renderings `sail report`
must produce for them (`golden/`). `tests/report.rs` checks every rendering
byte for byte; `SAIL_BLESS=1 cargo test -p sail-solve --test report`
regenerates them.

## Where the documents come from

Sail 0.2.0 at `b0c6b3a5` with its pinned Verus (fork runtime `a0e11cff`,
Verus `0.2026.10.18.7d82994`), Linux x86-64, 2026-11-09. `make-workspace.sh
DIR VARIANT` writes the two-member workspace (`alpha`, `beta`) that was
verified at `repo/proofs`, so the workspace is a subdirectory of the
repository `repo`.

| Document | Variant | What ran |
|---|---|---|
| `passing.json` | `pass` | First run: both members and vstd verified, exit 0 |
| `failing.json` | `fail` | Next run: six verification errors in five functions of `alpha`, `beta` reused, exit 101 |
| `compile-error.json` | `compile` | Next run: a Rust type error (`E0308`) in `alpha`, exit 101 |
| `setup-error.json` | none | `sail verify` without `sail-z3` beside it: `SAIL3100`, Verus never ran |

Each document is what `sail verify --output json` printed, after three
mechanical changes and nothing else:

- the checkout prefix was replaced by `/work`, and the local Sail cache
  prefix by `/work/cache`;
- targets with `requested: false` were removed. That is the vstd
  dependency, 1,871 functions and about 2 MB per document. With it,
  `failing.json` is 2,279,661 bytes; its summary, with `passing.json` as the
  baseline and source links, is then 4,574 bytes (4,508 for the reduced
  document, which has no dependency line);
- the document was written back as compact JSON with its key order kept.

Removing vstd leaves `execution.reused_targets` (2 in `failing.json`) counting
a target the document no longer lists.

The baseline used by the golden summaries is `passing.json`: the same
workspace path, context and requested targets, so the comparison is
`compatible` and reports five regressions for `failing.json`.
