# Verus issues (tracked locally)

Problems and surprises in Verus that affect sail are recorded here as
documentation. Each has a reproduction, its impact on sail, and any
workaround.

**We do not file issues in the Verus repository.** This applies to agents,
bots and automation, and is a project rule (AGENTS.md). The owner decided on
2026-10-12 that Sail stays on its own public Verus fork
([Chelis-Lang/verus](https://github.com/Chelis-Lang/verus), branch `sail`)
and fixes known Verus issues there; upstream contribution is not planned
([ADR 0039](../decisions/0039-private-verus-fork.md)).
These entries are the local record. Each fork fix is one commit with its
regression test, so the series can be rebased onto a new Verus release
([engine/verus/README.md](../../engine/verus/README.md)). Entries still keep
a description written so a person could file it upstream as is.

Sail runs the fork's Verus for every verification, default mode included
([ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)); stock Verus
is only the conformance oracle. Fork fixes therefore apply to every Sail run.
Workarounds an entry describes for stock runs apply only to the stock oracle
and to the `SAIL_CARGO_VERUS` development override.

| Id | Title | Status | Fork (`Chelis-Lang/verus`) | Reproduction |
|---|---|---|---|---|
| [VI-0001](VI-0001-nondeterministic-declaration-order.md) | Nondeterministic SMT output from `arrow_*` accessor order | open in stock Verus; fixed in the fork | fixed: `9a5b2cc8` fix: generate enum arrow accessors in source order (direct runs); `af212563` fix: declare enum arrow accessors in source order in the verifier (cargo runs; pinned since `13a1176c`) | `corpus/repros/verus-nondeterministic-declaration-order/` |
| [VI-0002](VI-0002-page-table-rlimit-on-pinned-release.md) | page-table fails one check on the pinned release | open | — | corpus entry `page-table` |
| [VI-0003](VI-0003-bit-vector-failure-reported-as-success.md) | `--output-json` reports `success: true` for a failed bit-vector query | open in stock Verus; fixed in the fork | fixed: `8a5d3655` fix: record bit-vector failures in the function failure set | `crates/sail-cli/tests/fixtures/attr/input.rs` (`arith::bv_bad`) |
| [VI-0004](VI-0004-shared-log-directory.md) | Every crate in a `cargo verus` run shares one log directory, and each clears it | open in stock Verus; fixed in the fork | fixed: `324c487c` fix: give each crate its own log directory under cargo verus | described (source references) |
| [VI-0005](VI-0005-forwarded-args-do-not-re-verify.md) | Changing forwarded Verus arguments does not re-verify a fresh crate | open | not fixed: the dep-info fix (`441d6cd9`, `b35e9226`) was dropped; it invalidated Sail repair builds | described (steps) |
| [VI-0006](VI-0006-release-without-bundled-z3.md) | The 0.2026.09.06 macOS release archive bundles no z3 | open | — | `corpus/toolchain.toml` secondary pin |
| [VI-0007](VI-0007-nondeterministic-error-order.md) | Errors from different modules are reported in a run-dependent order | open in stock Verus; fixed in the fork | fixed: `bc9cda09` fix: print bucket diagnostics in bucket order (pinned in `73dea021`; stock shown to produce the order) | `corpus/repros/verus-nondeterministic-error-order/` |
| [VI-0008](VI-0008-func-details-hash-order.md) | `--output-json` lists `func-details` in hash order | open in stock Verus; fixed in the fork | fixed: `bf59e1f0` fix: list output-json func-details sorted by name | `rust_verify_test` `output_json` tests (described) |
| [VI-0009](VI-0009-partial-success-for-skipped-functions.md) | Partial timing success includes skipped functions | open in stock Verus; fixed in the fork | fixed: `3566421a` fix: omit unselected functions from function timing | `corpus/repros/verus-partial-success/` |
| [VI-0010](VI-0010-ambiguous-friendly-function-names.md) | Distinct trait implementations share a friendly function name | open in stock Verus; fixed in the fork | fixed: `fbde5ea3` feat: add function-path to the timing function breakdown | `corpus/repros/verus-ambiguous-friendly-name/` |
| [VI-0011](VI-0011-cargo-no-vstd-launcher-mode.md) | Cargo no-vstd metadata misses launcher macro-mode selection | open | not fixed (not trivial; see entry) | `corpus/repros/verus-cargo-no-vstd/` |
| [VI-0012](VI-0012-filtered-progress-note-order.md) | Filtered bucket-start notes bypass long-running suppression and print in arrival order | open in stock Verus; fixed in the fork | fixed (order): `c9de60c6` fix: report bucket start notes in queue order | frozen HumanEval module013 source control |
| [VI-0013](VI-0013-verismo-tspec-stock-wait.md) | Stock Verismo tspec waits until the original unit deadline | open; cause unknown | — | pinned corpus project `verismo`, `source/verismo_tspec/src` |
| [VI-0014](VI-0014-exported-foreign-span-metadata-gap.md) | Exported spans can lack original source-range metadata | moot: only affected the removed prepared-lowering reuse | — | retained HumanEval diagnostic and reduced typed stock-resolver control; historical export cause unknown |
| [VI-0015](VI-0015-nondeterministic-trigger-choice.md) | Automatic trigger choice depends on hash iteration order | open in stock Verus; fixed in the fork | fixed: `404a1e30` fix: choose automatic triggers in a deterministic search order | `corpus/repros/verus-nondeterministic-trigger-choice/` |
| [VI-0016](VI-0016-cargo-verus-new-needs-git.md) | `cargo verus new` fails with a bare "No such file or directory" without git | open | — | described (steps) |
| [VI-0017](VI-0017-timing-json-order.md) | `--output-json --time` lists modules and functions in a run-dependent order | open in stock Verus; fixed in the fork | fixed: `9e32601e` fix: list output-json timing modules and functions by name (pinned in `73dea021`) | `corpus/repros/verus-timing-json-order/` |
| [VI-0018](VI-0018-source-build-cargo-verus-new.md) | A source-built `cargo verus new` knows no vstd for its own version | fixed in Sail's runtime (from `73dea021`) | not a source change: `cargo xtask build-verus-engine` writes the toolchain manifest, as the release workflow does | described (steps) |
| [VI-0019](VI-0019-crate-wide-closure-numbering.md) | Inserting an item renames closure types crate-wide | open in stock Verus; fixed on a fork branch, not yet pinned | fixed on `m11-5/stable-closure-ids`: `8d03ab54` fix: name closure types by definition path, not crate-wide index (a renaming, under [ADR 0049](../decisions/0049-stable-closure-names.md)) | `corpus/repros/verus-closure-renumbering/` |
| [VI-0021](VI-0021-proof-note-json-order.md) | JSON proof-note arrays follow hash-set order | reproduced in the pinned fork; correction pending | — | `corpus/repros/verus-proof-note-order/` |

Z3 behaviour is recorded the same way, as reproductions under
`corpus/repros/` (for example `z3-nla-rlimit-nondeterminism`).

## Adding an entry

Use the next `VI-NNNN` number. Record the Verus version, the date found, a
reproduction (prefer a minimal one under `corpus/repros/`), the impact on
sail, the workaround, and a description written as an upstream report would
be. Update this table, including the Fork column when the fork fixes it.
