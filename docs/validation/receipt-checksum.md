# Borrowed receipt checksum candidate

| Field | Status |
|---|---|
| Issue | #156, remaining managed overhead |
| Source | Candidate prepared on main `555b8467` |
| Qualification | Source and compatibility controls prepared; build, tests, conformance and measurements pending |
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

This is an internal implementation candidate, with no command, configuration,
output or supported-workflow change. The existing
[managed-overhead record](managed-overhead.md) retains its original source and
measurement scope.
