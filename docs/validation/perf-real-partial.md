# Focused real-project benchmark entrypoint

**Status: source correction and tiny public-API checks; native Anvil acceptance pending.**
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

- Original native attempt, 2026-10-17, source `6af7d10` on main `2936ead2`:
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

Native corrected Anvil measurements and full required harness qualification
remain pending. Nothing here accepts partial verification as whole-target
completeness or changes a benchmark baseline.
