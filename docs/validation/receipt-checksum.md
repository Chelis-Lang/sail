# Borrowed receipt checksum candidate

| Field | Status |
|---|---|
| Issue | #156, remaining managed overhead |
| Source | Original candidate `9d0a59d1`; production code ported as `0bc5724a` on optional-plan candidate `e130f632`, based on main `3892881a` |
| Qualification | Mac focused compatibility/corruption/publication controls and all-target Clippy pass on `c66049a0`; full gates, conformance and measurements pending |
| Selection | Unselected; useful matched real-project measurements are required before acceptance |
| Scope | Receipt checksum serialization only; no dependency, schema, key or verdict-policy change |

The candidate borrows every persisted receipt field and serializes it in the
existing order, representing `checksum` as the empty string. A bounded buffer
feeds SHA-256 instead of cloning the embedded report and allocating the entire
serialized receipt. Exhaustive receipt destructuring requires any future field
addition to make an explicit checksum decision. Serialization failure retains
the original empty-bytes digest behavior.

The compatibility oracle independently uses the original algorithm: clone the
receipt, empty its checksum, serialize it with the derived `Serialize`
implementation, then hash those bytes. Controls include the existing rich
report golden, nested portfolio and engine records, escaped and non-ASCII
strings, maps, declared environment absence and raw values, and a non-UTF path
serialization failure. Existing corruption, commit, context, environment and
artifact checks are retained. Full receipt parsing and content reads remain
unchanged; this candidate carries no file digest across observations.

No speed gain is claimed. Native qualification must use a current baseline
containing the already accepted managed optimizations, then compare the same
Demo and Vest unchanged/body-edit cases in alternating rounds on one quiet
host, with identical pinned fork/Z3 and independent warm targets/caches. Keep
every sample, exact outputs, Currentness and invocation counts. Diagnostic
runs are separate from default-output timings. The workspace gate, solver,
all end-to-end phases plus `extra`, PR conformance, full fast and standard
conformance remain required. Source and raw evidence hashes belong here only
once those checks complete.

The current stack keeps the optional-plan optimization's production source
unchanged. Its measured unchanged-run gain does not qualify this additional
checksum change: matched measurements must compare against `e130f632` as
well as plain stock. Both candidates remain unselected until their gates pass.

## Focused Mac checks

On 2026-10-18, source `c66049a0` passed all five receipt compatibility,
corruption and publication controls, followed by all-target Clippy with warnings
denied, using Rust 1.99.0 on native macOS arm64. The complete bounded run took
76.71 s. It used one compiler job, retained the original serialization oracle,
and left source, protected runtime and evidence bodies unchanged. This is a
focused correctness result, with no performance claim or full-gate acceptance.
The exact command, actual unit-binary hash and complete logs are retained in
external `receipt-checksum-current-2026-10-18/mac-focused-02/`; its result
SHA-256 is `97f063c44a8f9aa655308a400b085d74adb3624106f08d451a35da110e328805`.

## Indexed interrupted attempts

- `0bc5724a`, 2026-10-18: the first Mac focused compile was stopped at its
  operating disk floor after 22.9 s. The five receipt controls and Clippy are
  unrun. Source and 3,290 protected files remain unchanged, and the compiler
  is reaped. The final free-space guard also fails; no product result is
  inferred. External `receipt-checksum-current-2026-10-18/mac-focused-01/result.json`
  has SHA-256 `c0e37c92496ff299820699d2f926b877e5afedd68c8565535df4417e3c807496`.

This is an internal implementation candidate, with no command, configuration,
output or supported-workflow change. The existing
[managed-overhead record](managed-overhead.md) retains its original source and
measurement scope.
