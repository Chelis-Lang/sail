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
repository `repo`. `vir-error.json` comes from a later build of the same
commit with `sail report` added, in a fresh copy of the workspace, so its
`context_sha256` differs from the other documents'.
`no-reuse-compile-error.json` comes from the same later build at `b059ce3f`
on 2026-11-10, in another fresh copy with its own Sail cache.

| Document | Variant | What ran |
|---|---|---|
| `passing.json` | `pass` | First run: both members and vstd verified, exit 0 |
| `failing.json` | `fail` | Next run: six verification errors in five functions of `alpha`, `beta` reused, exit 101 |
| `compile-error.json` | `compile` | Next run: a Rust type error (`E0308`) in `alpha`, exit 101 |
| `vir-error.json` | `vir` | First run in a fresh copy: `alpha` compiles as Rust and Verus rejects it (a proof function calls an exec one), exit 101 |
| `no-reuse-compile-error.json` | `compile` | The same type error with `LD_LIBRARY_PATH` set, so Sail ran plain `cargo verus verify` without proof reuse (`SAIL3112`): one target named `cargo verus verify`, exit 101 |
| `setup-error.json` | none | `sail verify` without `sail-z3` beside it: `SAIL3100`, Verus never ran |

`compile-error.json` and `vir-error.json` hold the two records Verus writes
for a crate that stops before any function is checked, which `sail report`
and `sail jobs result` tell apart:

| | `verified` | `errors` | `success` | `encountered_vir_error` |
|---|---|---|---|---|
| Rust compile error | `0` | `0` | `false` | `false` |
| Rejected by Verus | `null` | `null` | `false` | `true` |

Two other programs Verus rejects produced the same record as `vir-error.json`
on the same build and are not kept: a call to `std::process::abort` (not
supported) and a recursive `spec fn` without a `decreases` clause. So did an
empty `alpha/src/lib.rs` (Verus: "The verus_builtin crate was not imported").

Other Rust compile errors were run on that build on 2026-11-10 to see whether
they leave the same record as `compile-error.json`. Every one did, exactly
(`verified: 0`, `errors: 0`, `success: false`, `encountered_vir_error:
false`, one crate entry with no function); the documents are not kept:

| Variant | Error | Run |
|---|---|---|
| `resolve` | `E0425`, an unresolved name | managed |
| `parse` | a syntax error inside `verus!` (`mismatched closing delimiter`) | managed, and without proof reuse |
| `parse-outside` | a syntax error outside `verus!` (`this file contains an unclosed delimiter`) | managed |
| `macro` | an unknown macro (`cannot find macro`) | managed |
| `compile` | `E0308` | without proof reuse (kept as `no-reuse-compile-error.json`) |

A record of any other shape keeps the plain counts; none was observed. One
failure leaves no crate record at all: with a build script in `alpha` that
exits 3, the document has exit code 101, no target and no diagnostic, so
there is no crate to describe (see `docs/known-issues.md`).

No document here holds a warning: Verus reported none for these programs,
the unused `let` in the `fail` variant included. The `::warning` annotation
and the SARIF warning path are covered only by a diagnostic that
`tests/report.rs` edits into a document
(`dependencies_are_summarized_and_ranked_last`), and so are repeated
diagnostics and provisional verdicts.

Each document is what `sail verify --output json` printed, after these
mechanical changes and nothing else:

- the checkout prefix was replaced by `/work`, and the local Sail cache
  prefix by `/work/cache`;
- targets with `requested: false` were removed. That is the vstd
  dependency, 1,871 functions and about 2 MB per document. With it,
  `failing.json` is 2,279,661 bytes; its summary, with `passing.json` as the
  baseline and source links, is then 4,574 bytes (4,508 for the reduced
  document, which has no dependency line);
- the document was written back as compact JSON with its key order kept;
- in `failing.json` only, each of the six counterexamples had its `model`
  text replaced by `(model removed from this fixture)` and its `assignments`
  cut to the first two. The models are about 26 kB each and made the document
  226,906 bytes; `sail report` reads only whether a counterexample is
  present, and the golden renderings are byte-identical with and without
  this change.

Removing vstd leaves `execution.reused_targets` (2 in `failing.json`) counting
a target the document no longer lists.

The baseline used by the golden summaries is `passing.json`: the same
workspace path, context and requested targets, so the comparison is
`compatible` and reports five regressions for `failing.json`. For
`vir-error.json` the context differs, so its golden summary shows a real
incomparable baseline. `no-reuse-compile-error.json` binds no context, so
its golden summary shows the comparison refused for a run without proof
reuse.
