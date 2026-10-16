# VI-0020: External enum accessor order varies in ordinary VIR

- **Found:** 2026-10-16.
- **Observed source:** combined fork `8a373f81`, with default-off input
  observer `7812863f`. Research runtime only; Sail's selected runtime is
  unchanged. Actual native source/compiler/Z3 hashes and full captures are
  retained in the external frontend evidence.
- **Status:** confirmed literal ordinary-AST/bincode variation in unchanged
  IronKV; source reduction prepared but unexecuted. No verdict, diagnostic,
  SMT or performance regression has been demonstrated. No fix is selected.
- **Reproduction:** [trusted enum reduction](../../corpus/repros/verus-external-fn-order/README.md).

## Confirmed observation

In two fresh unchanged IronKV runs, the only ordinary-AST differences in
both `host_impl_v` and `verus_extra::clone_v` are the order of
`external_fns[96]` and `[97]`: `message_t::impl&%0::arrow_key` and
`arrow_value`. Raw JSON and bincode hashes differ; complete SMT stream
multisets and verification/proof-note JSON match. All original fields stay
intact. This defeats an exact raw frontend memo key even before an edit,
but that key is unqualified and no reuse or speed claim follows.

The library patcher was a no-op (`None []`): a copied runtime root was
passed where its API requires a full `source/` checkout. The actual retained
IronKV lock selects upstream Git
`5c8133a2eabcf9d7396ab462dd8b1ee8143a83b8`, macro version
`0.0.0-2026-09-20-0158`. The actual loaded macro image was not independently
traced. This is not evidence of a new HashMap defect in the fixed fork macro.
The prepared reduction preserves that original dependency explicitly.

External evidence under `frontend-review-2026-10-16/raw-input-probe`:

| Artifact | SHA-256 |
|---|---|
| `m8-observer-attempt2-complete.tgz` (all original source/runtime/inputs/results) | `ec08c6220ce5a4717dd89ba94b713f920878d3794b0d28166793e20f7785f0a5` |
| `ast-lossless-comparisons.json` (12 literal positional comparisons) | `cceb3623c9e4f59c521967a06e8c119d08902ed5fdf302556a62491cea2623c4` |
| `ironkv-unchanged-external-fns.json` (two original entries per unchanged sample) | `0a0a9941ce40d9b007dd061c40f656edca3cd9e7fa9ec06369ed507786ad8ee5` |
| `resolved-macro-locks.json` (original resolved library/macro source) | `d88b56f58beacc5d74960ce69337c056c895c226b60f62565106acf4576a5ab1` |

## Source audit and candidate boundary

The fork already fixes enum accessor generation and verified-impl ordering
for VI-0001. Cargo compiles the project's resolved macro dependency:
`rust_verify/src/main.rs` deliberately skips standalone library injection
when invoked through Cargo, and Sail's adapter forwards Cargo's externs.

The external-function path is separate: `external.rs` visits generated HIR
items in macro output order; `rust_to_vir.rs` appends external `Fun` values.
`ast_sort.rs`, pruning, simplification, autospec and crate merging copy or
concatenate that vector. In the audited `8a373f81` source, the only semantic
consumer is equality membership in `well_formed.rs`, selecting whether an
unsupported-call diagnostic names a locally ignored function. `Fun` contains
only its complete path; duplicates have identical membership meaning.
The VIR printer also exposes list order, so that log order changes even
for hand-written external declarations. This is a structured-log correction,
not a claim of stock byte equality for all such logs.
No SMT construction consumer was found in this source audit.

Local research candidate `00153b3d65ef5df856f060213bf8b02aae936364` on
`m11-8/external-fn-order` sorts the whole external-function vector with the
existing complete `Fun`/`Path` order in `sort_krate`, before local export.
The candidate has not been built or tested. Its source regressions cover
opposite orders, duplicate multiplicity, full crate/path identities, the
VIR log order and the ignored-function diagnostic. It must preserve every
entry, duplicate multiplicity, crate identity and all other AST fields.
Sorting by a friendly name or omitting fields from a Sail key is outside
this proposal. The default correctness contract and independent review
still apply; no input or output is normalized in Sail.

Before any adoption: execute the small reduction and both real unchanged
buckets on native hardware, retain negative controls and complete literal
AST/bincode/SMT outputs, test duplicate/full-path handling and the ignored
function diagnostic, then qualify full function verdicts/diagnostics and
determinism. A stable internal key alone is not a measured end-user gain
and cannot justify a runtime re-pin or closing frontend reuse issue #162.
