# Cargo target-kind binding

| Field | Current state |
|---|---|
| Tracking | #722 |
| Established source | Main 4ec9521d, name-only runtime target-kind map |
| Native reproduction | c7i.4xlarge/us-west-2, Ubuntu 24.04, CI Rust 1.99.0; real Cargo metadata has lib and bin both named same_target, while the old map retains only bin |
| Candidate | Retain each target's Cargo name, kinds and Rust crate types; select exactly one matching compiler identity or decline ambiguity |
| Focused native | Linux Rust 1.99.0: focused lib/bin/rlib/ambiguity test, all-target clippy and release Sail/shim build pass |
| Live observations | Native Linux full verification of an underscore-named package: both requested targets named capture_fixture; selected has four events bound to lib, binary_selected has four bound to bin; full/focused single-target capture also passes |
| Independent review | Exact `28f986e7`: 14 native Rust tests, corrected selector controls and removed-predicate failures, 78 checker controls pass with the exact single binding advance |
| Remaining | Current full gates and Mac host check |

This changes observation identity, not solver inputs or verification verdicts.
The private runtime document uses the new metadata shape. A legacy document
cannot satisfy its required field and therefore cannot supply the new binding.
Compiler output and query bytes are never rewritten. Ambiguous same-name,
same-crate-type metadata is unavailable rather than resolved by map order.
The original metadata reproduction is source evidence. The subsequent live\ncontrol uses the unselected campaign observation producer; it establishes the\ntarget-kind correction, not complete campaign or conformance acceptance. Raw evidence remains outside the repository.

The native report-mode adapter changes only a test fixture field name. Its
production delegates and checked source remain byte-identical. Fresh review
approves only `MODE_NATIVE_SHA256` advancing to
`f1a8fdd6e6cecdc6b1450f04664186d44360cf54b798100210dd61d04a478782`;
all native production mutations still refuse. Review SHA-256:
`4a35975146d16698a99507832a07c56ac1b20995e53e26c8937d514e59967f53`.

| Earlier source/date | Result and cause |
|---|---|
| `31bdbd36`, 2026-11-12 | Independent review rejects normalized aliases when Cargo's exact name is absent; intended dual-target live control still passes. Corrected selector considers every compatible alias and refuses ambiguity. |
| `28f986e7`, 2026-11-12 | Full gate stops at the unchanged native caller hash (78 Python controls, one failure/two errors); no full gate pass. Exact fixture-only advance independently approved above. |
| Initial dual-target wrapper, 2026-11-12 | Invalid setup retained: hyphenated package gives different Cargo lib/bin names; replaced with an underscore-named package before the actual shared-name control. |
