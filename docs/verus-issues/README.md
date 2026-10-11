# Verus issues (tracked locally)

Problems and surprises in Verus that affect sail are recorded here as
documentation. Each has a reproduction, its impact on sail, and any
workaround.

**We do not file issues in the Verus repository.** This applies to agents,
bots and automation, and is a project rule (AGENTS.md). If a person decides
to report one upstream, each entry's description is written so it can be
filed as is.

| Id | Title | Status | Reproduction |
|---|---|---|---|
| [VI-0001](VI-0001-nondeterministic-declaration-order.md) | Nondeterministic SMT output from `arrow_*` accessor order | open | `corpus/repros/verus-nondeterministic-declaration-order/` |
| [VI-0002](VI-0002-page-table-rlimit-on-pinned-release.md) | page-table fails one check on the pinned release | open | corpus entry `page-table` |
| [VI-0003](VI-0003-bit-vector-failure-reported-as-success.md) | `--output-json` reports `success: true` for a failed bit-vector query | open | `crates/sail-cli/tests/fixtures/attr/input.rs` (`arith::bv_bad`) |
| [VI-0004](VI-0004-shared-log-directory.md) | Every crate in a `cargo verus` run shares one log directory, and each clears it | open | described (source references) |
| [VI-0005](VI-0005-forwarded-args-do-not-re-verify.md) | Changing forwarded Verus arguments does not re-verify a fresh crate | open | described (steps) |
| [VI-0006](VI-0006-release-without-bundled-z3.md) | The 0.2026.09.06 macOS release archive bundles no z3 | open | `corpus/toolchain.toml` secondary pin |
| [VI-0007](VI-0007-nondeterministic-error-order.md) | Errors from different modules are reported in a run-dependent order | open | `corpus/repros/verus-nondeterministic-error-order/` |
| [VI-0008](VI-0008-func-details-hash-order.md) | `--output-json` lists `func-details` in hash order | open | `rust_verify_test` `output_json` tests (described) |
| [VI-0009](VI-0009-partial-success-for-skipped-functions.md) | Partial timing success includes skipped functions | open | `corpus/repros/verus-partial-success/` |
| [VI-0010](VI-0010-ambiguous-friendly-function-names.md) | Distinct trait implementations share a friendly function name | open | `corpus/repros/verus-ambiguous-friendly-name/` |
| [VI-0011](VI-0011-cargo-no-vstd-launcher-mode.md) | Cargo no-vstd metadata misses launcher macro-mode selection | open | `corpus/repros/verus-cargo-no-vstd/` |
| [VI-0012](VI-0012-filtered-progress-note-order.md) | Filtered bucket-start notes bypass long-running suppression and print in arrival order | open; repeated stock identity pending | frozen HumanEval module013 source control |
| [VI-0013](VI-0013-verismo-tspec-stock-wait.md) | Stock Verismo tspec waits until the original unit deadline | open; cause unknown | pinned corpus project `verismo`, `source/verismo_tspec/src` |

| [VI-0014](VI-0014-exported-foreign-span-metadata-gap.md) | Exported spans can lack original source-range metadata | open; conservative reuse refused | retained HumanEval diagnostic and reduced typed stock-resolver control; historical export cause unknown |

Z3 behaviour is recorded the same way, as reproductions under
`corpus/repros/` (for example `z3-nla-rlimit-nondeterminism`).

## Adding an entry

Use the next `VI-NNNN` number. Record the Verus version, the date found, a
reproduction (prefer a minimal one under `corpus/repros/`), the impact on
sail, the workaround, and a description written as an upstream report would
be. Update this table.
