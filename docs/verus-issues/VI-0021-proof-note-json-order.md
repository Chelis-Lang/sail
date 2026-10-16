# VI-0021: JSON proof-note arrays follow hash-set order

- **Status:** reproduced in Sail's pinned fork; correction pending. Not filed upstream.
- **Runtime:** fork `73dea02112d730e7ab52f58158012d440fb89512`, upstream base
  `426d8b01e7ffb910a36c15e1f870ff985bb61eef`, Z3 4.16.0; macOS arm64,
  Rust 1.98.1, source test-suite build.
- **Found:** 2026-10-16 while qualifying the exact fork arm of `deep-rvt` (#124).
- **Reproduction:** [minimal program](../../corpus/repros/verus-proof-note-order/repro.rs).
- **Impact:** identical verifier invocations produce different orders in the
  JSON `obligation_proof_notes` and `failed_proof_notes` arrays. The observed
  verdicts, note sets and rendered diagnostics agree. This is a serialization
  determinism defect; no solver-input or speed difference is claimed.
- **Workaround:** none. The exact fork comparison remains strict.

Run the pinned fork's `verus --output-json --multiple-errors 2` on the
reproduction repeatedly. All eight fresh invocations exited 1 and reported
zero verified functions and one error. Both arrays appeared in both orders:
`["alpha", "zebra"]` and `["zebra", "alpha"]`.
The eight-row raw order summary has SHA-256
`b73c7db6562b35a293ccd1c76508c57a04af7648c9da2959969155879d5932be`.

`FuncDetails` in `rust_verify/src/verifier.rs` derives serialization over
`HashSet<String>` fields. The sets correctly preserve membership; their
iteration order is not stable. A correction should sort only these serialized
arrays and preserve internal membership/merging, verdicts, diagnostic bytes and
all test assertions.

The selected source-suite run covered 244 tests (3 ignored): 241 Same,
3 Different, 0 Unstable, 0 infrastructure errors. The three differences were
`output_json::test_json_proof_note_on_requires`,
`output_json::test_json_proof_note_on_ensures` and
`output_json::test_json_proof_note_on_assume_and_req_with_no_cheating`.
The archive of reports, all confirming outputs and logs has SHA-256
`475aa777b9e0ebd98f0ab1b3498d453d3803091aeef54c1d8f2927d43e103444`.
This sample does not qualify the full source suite or a release runtime.
