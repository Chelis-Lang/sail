# General Verus campaigns

| Field | Current state |
|---|---|
| Tracking | #140, #141, #664; ADRs 0111 and 0114 |
| Status | Library groundwork qualified; campaign execution and CLI/JSON/MCP remain unimplemented |
| Qualified source | `7f77d3ec973852d16d07fe100f92eb902d3571a8`; native EC2 c7i.4xlarge/us-east-2, Ubuntu 24.04, CI Rust 1.99.0 |
| Linux gates | Workspace: 1,347 pass, 53 skip; solver: 50 pass; full E2E including extra; all four PR conformance arms: 33 Same, 311 functions, zero different/unstable/infra errors |
| Acceptance | 35 direct stock-Verus model cases; 316 production-refinement cases (seven positive, 309 negative), 340 source bindings, 3,873 runtime files |
| Mac | Exact `7f77d3ec`: host clippy and 1,334 tests pass, 53 skip; book source/build/links pass, Node highlight probe explicitly skipped |
| Independent classifier review | `1ec0577a`: 133 control groups and 4,992 ordering cases pass; the subsequent rebase preserves this classifier source |
| Independent copy review | Exact `7f77d3ec`: four module tests, six independent boundary groups, and removed-guard must-fail control pass |
| Rebased gate | `b970be766254b0c32d201f22f616c1b3b1886b67`: full native Linux workspace gate passed; log SHA-256 `bf7ef0ee8f53fa6c644368937d789ab8a97c97a3bd76f46fb6eebf118d9c8a4e` |
| Remaining | Protected PR checks; source admission, owned-copy runner, durable lifecycle, coherent product surfaces, real-project and cost qualification |

The model is separate from ordinary verification output and proof tiers. It
checks every retained attempt and scope, mode-matched baseline twins,
diagnostic multiplicity, native-sat subclassing, baseline headroom and
collateral failures. Models remain unvalidated backend observations.
The classifier and eventual service have no new formal theorem: the acceptance
runners qualify the existing selected components and API assembly boundary,
with surrounding I/O, origin and service composition still explicit obligations.

Source snapshots bind relative paths, exact bytes and executable mode across
two reads. They refuse links, special files and excessive inventories; safe
no-follow/nonblocking reads are length-bounded. Both captures and patched trees
are bounded to 50,000 files and 512 MiB. Each materialization is a new private
directory. Excluding `.git` metadata and `target` directories is explicit;
external Cargo inputs and compiler-owned function/source admission remain
caller obligations. No runner or endpoint is exposed.

The main rebase adds the separately qualified #719 ledger correction. Of the
340 refinement source bindings, only the bound, unproved ledger implementation
changes; every checked production/model/checker source remains byte-identical.
The actual formal runs above remain attributed to `7f77d3ec`, not relabelled as
new executions on the rebase. The ledger correction's own qualification is
recorded in [ledger headroom](ledger-headroom.md).

Current native reports archive SHA-256:
`23c5d5ad8368c552977c2ade14e5038b3a263a57aa4151b9ec7aa74f406cd63e`.
Classifier review:
`ca3832bdc83a28edbfcf780574ad32d3d00f8b611737bd70cfe8e3def8cef099`.
Final copy-budget review:
`a218832429c43afe8be9b70ad7724a6535979d5e4fb5882fa13751b939c8c612`.
Raw logs, reports and reproductions are retained outside the repository.

## Earlier attempts

| Source/date | Result and cause |
|---|---|
| Initial model, 2026-11-11 | Gate refused the unreviewed API-root assembly binding; independent review accepted only that exact addition, with 53 capture tests and six drift controls |
| `9fbc35af`, 2026-11-11 | Earlier component gate and 316 refinements passed; historical archive `8827caaab15d7f8d81817dae612b3e10bc34f6d80f4fc40706a7e9457db28b97`, superseded by the current source run |
| `87af828d`, 2026-11-11 | Rejected: verified/sat/unknown/error contradictions and missing or contradictory scoped attempts could score despite fixture and host passes |
| `d2378835`, 2026-11-11 | Rejected: valid-looking full confirmations hid invalid scoped attribution, unsat rejection and new collateral failure |
| `97fe61ef`, 2026-11-11 | Rejected: a scoped survivor was compared again with the full baseline, causing false collateral failure and exclusion from the score |
| `9d509ff9`, 2026-11-11 | Static defect found before adoption: capture size was bounded but an oversized patch could exceed the whole-tree limit; corrected and independently qualified at `7f77d3ec` |
| Copy-review harness, 2026-11-11 | Initial wrapper lacked the private error constructor; invalid setup corrected by including the unchanged production error definitions |

These are library fixture and component results, not real-project campaign
acceptance. No corpus entry is modified. Documentation changes are this record,
the design, ADR 0114 and changelog. Consumer verification instructions are
unchanged because no campaign command is available yet.
