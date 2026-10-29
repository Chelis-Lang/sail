# Focused real-project benchmark entrypoint

**Status: native workspace gate passes; Anvil acceptance and remaining harness
qualification are pending.**
The whole-project CapybaraKV matrix is unchanged. This correction changes
benchmark invocation and report handling, without changing verification,
module selection, corpus edits, the conformance checker or its oracle.

`scripts/perf-real.sh run anvil 3` keeps `--lib -- --verify-module
vdeployment_controller`. Its `sail`, `fork-sail` and explicit `incremental`
arms now add public `--output json` before the Cargo/Verus separator. Plain
and fork-shim arms retain `cargo verus focus`. Whole-project Sail arms retain
their existing output mode. Development runtime overrides remain explicit;
no private worker selector is used.

Public partial JSON contains selected function verdicts, not a whole-crate
success claim or Verus's compiler-stage totals. The six stage fields in
`results.tsv` therefore contain `NA`; `scripts/perf-real-report.py` renders
them as unavailable. Wall time, process exit, session records and cache
counters remain measured. Existing numeric timing files keep their format.
Raw JSON, stderr and every failed sample remain retained.

The corresponding external acceptance reader pairs every actual oracle
proof document and exact function/module/mode/verdict multiset. The requested
crate must actually be partial. Any zero-function compilation documents are
counted separately, without inventing vstd proof results. Every diagnostic
is retained: the real `note: verifying module ...` must match across actors,
and the report's rendered block must match its own raw stderr, including
Cargo progress captured by the existing parser. Unknown diagnostics refuse.
The existing whole-project pairing and `check_output` remain unchanged.

## Evidence and remaining gate

- Original native attempt, 2026-10-17, source `6af7d10` on main `ee6914c7`:
  **FAIL**, all 18 Anvil Sail/fork-Sail rows exit before a solver handshake.
  Non-JSON partial verification intentionally routes to the plain verification
  refusal. Raw final archive SHA-256
  `26727bfc73b328093da236ae73c2205bb8f553843b1ca2e9596a2c648fa9963a`.
  No Anvil performance result is accepted from those rows. The independent
  whole-project CapybaraKV pass remains scoped to that project.
- Source command/reader controls: 14 pass, covering all three partial Sail
  arms, unchanged whole-project commands, unavailable stage values and the
  existing mode selection controls. Shell syntax and diff checks pass.
- A real two-module Mac fixture selects one valid proof while leaving an
  excluded failing assertion unverified. The supported public JSON route
  exits zero with one selected function and `complete: false`; the old
  non-JSON command exits one. Explicit logging uses an actual flat schema-2
  report with absent whole-crate success and the retained module-scope note.
  A repeat uses Sail source `2d264485aeb57a8426789ce33f85c0f4ee352390`,
  frozen body SHA-256
  `49a6630ed00e08f991e5f986ee037232c23984af0a8e4dc013e92492518f5148`
  and genuine fork `067` runtime; complete watched-file inventories match
  before and after every call. The result record SHA-256 is
  `29bcf1406b047dbac57e4dd17a8e76b534dd950f7a9336160e0c552c752fdd71`.
  Fifteen actual-document reader controls pass, including missing/duplicate
  functions or oracle documents, changed verdict/scope and lost/unknown
  diagnostics. Reader-control record SHA-256
  `bc4807b134ccf2afdc264bf7b033994d3a51e0e03b614e3a1d68d13e390aa984`.
  These are API correctness and reader observations, not native Anvil or
  performance qualification. An earlier check used a shared Sail pathname
  that the lead replaced before its final inventory; that check remains
  unqualified and the stable-path repeat supplies the evidence above.

The corrected native attempt used exact script source `237e0063` for its
workspace gate: 665 tests and two doctests passed on c7i.8xlarge in us-east-1.
The timing source `3038b1fa` adds only the isolated candidate fork selector;
its cc7 and candidate runtime bodies were admitted separately. The unchanged
whole-project CapybaraKV three-way matrix passed. Gate receipt SHA-256:
`ca5c6d7c47d3a80acfe520e0c93f5a5d6f2e53ee5395e34300c09fe62b3f57b8`.
Capy matrix receipt SHA-256:
`e60cd22a8f3c1bb6fb27e84213184de808c26cb27900d5f844b2623cd95219ca`.

That enclosing attempt remains **FAIL**. All 18 Anvil commands exited zero,
but the external acceptance reader rejected an actual descendant-module
progress note after expecting only the selected parent-module note. The raw
reports contain 45 module/function bucket notes and an identical 143-function
verdict multiset in every row. They also retain 144 successful Verus checks
and six near-limit instability flags; those fields are not silently replaced
with function counts or zeros.

**Reader contract: 143 functions and 144 checks are different units.**
Verus's `verification results::` count is one per successful first check of
an error-level query (`count_verified` in `rust_verify/src/verifier.rs`), not
one per function. In the 2026-10-20 Linux release matrix, every Sail anvil
report lists 143 functions, all verified, with 144 non-advisory proved
obligations: 141 functions with one query each, `spec_before_phase_n` with
only its termination check, and `objects_to_vrs_list` with two `Function-Def`
queries (the function, `exec/reconciler.rs:340`, and its loop,
`exec/reconciler.rs:357`). The report's own Verus count
(`crates[].verus.verified`) is 144, as stock prints, with 0 errors on both
sides; the 15 Sail samples of all three rounds agree. No verdict is missing.
`scripts/perf-matrix.py` therefore compares Sail's Verus count with stock's
line and fails the sample when the obligations do not account for that
count. Evidence: matrix archive SHA-256
`8aa95b2ddc3df5deba80166bb413bec3b48df66d3e291b28436f4ad55e1756ed`
(round 1 cold logs: Sail
`f35562bd9523d110eb135f33e4e6382a5e6803e78137414f4cfa28386803d692`, stock
`a666334c4c115705f0ebf985f603466132bd5bdb68b60dfbd07166ba579fe8a5`).
The Anvil stock matrix and full FAST arms are unrun. Source, input, built
binary and per-timing runtime identity checks pass; the later workload guard
is unrun. Final receipt SHA-256:
`cf727b1ac956c2ec6eec8d828be3097e01a0f000811a0909543eff94d0ec3ebe`.

Native harness qualification on 2026-10-21 (c7i.8xlarge, us-east-1,
pinned Linux CI images) passes the workspace gate, 35 pinned-solver tests,
full end-to-end suite including extra, and all four PR and full-fast
conformance arms (33 and 147 units). Standard conformance completes all
four arms: pass-through and engine have 159 Same; fill and warm have
158 Same and one stock-unstable unit, with zero Different or infrastructure
errors. Raw evidence is `experiments/2026-10-21/matrix-harness-checks.tgz`
in the S3 evidence bucket, SHA-256
`7e475df2d3efa0ae4cd10000c818e482affe6b77aed6bd45529d33fc4a813c61`.
The registry preparation and obligation-count comparison are unchanged
since the source used for those conformance runs. The final report changes
pass a fresh Linux workspace gate and 23 behavioral Python tests; the Mac
host gate passes 849 tests, with 37 skipped and Clippy clean.

The installed-release matrix and version-2 mutation measurements remain
pending. Nothing here accepts partial verification as whole-target
completeness or changes a benchmark baseline.
