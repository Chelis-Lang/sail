# VI-0021: JSON proof-note arrays follow hash-set order

- **Status:** fixed in the selected `cc7bbce4` fork runtime (PR #221). Not filed upstream.
- **Fix:** producer commit `5793253cb7a0dec204c1cb3515a9c12cda370911`, retained in `cc7bbce4`; no Sail-side normalization.
- **Affected reproduction runtime:** fork `73dea02112d730e7ab52f58158012d440fb89512`, upstream base
  `426d8b01e7ffb910a36c15e1f870ff985bb61eef`, Z3 4.16.0; macOS arm64,
  Rust 1.98.1, source test-suite build.
- **Found:** 2026-10-16 while qualifying the exact fork arm of `deep-rvt` (#124).
- **Reproduction:** [minimal program](../../corpus/repros/verus-proof-note-order/repro.rs).
- **Impact:** identical verifier invocations produce different orders in the
  JSON `obligation_proof_notes` and `failed_proof_notes` arrays. The observed
  verdicts, note sets and rendered diagnostics agree. This is a serialization
  determinism defect; no solver-input or speed difference is claimed.
- **Workaround:** none. The exact fork comparison remains strict.

On the affected runtime, run the fork's `verus --output-json --multiple-errors 2` on the
reproduction repeatedly. All eight fresh invocations exited 1 and reported
zero verified functions and one error. Both arrays appeared in both orders:
`["alpha", "zebra"]` and `["zebra", "alpha"]`.
The eight-row raw order summary has SHA-256
`b73c7db6562b35a293ccd1c76508c57a04af7648c9da2959969155879d5932be`.

`FuncDetails` in `rust_verify/src/verifier.rs` derives serialization over
`HashSet<String>` fields. The sets correctly preserve membership; their
iteration order is not stable. The correction sorts only these serialized
arrays lexically. The original producer uses sets and never retained source order; lexical string order is
therefore the explicit stable output order, rather than invented source order.
The serializer copies every existing set entry without additional deduplication;
internal membership/merging, note text, source identities, verdicts and diagnostic
bytes are unchanged. It does not reinterpret caller arrays as sets. Existing
raw-array order checks run before the original membership assertions.

The selected source-suite run covered 244 tests (3 ignored): 241 Same,
3 Different, 0 Unstable, 0 infrastructure errors. The three differences were
`output_json::test_json_proof_note_on_requires`,
`output_json::test_json_proof_note_on_ensures` and
`output_json::test_json_proof_note_on_assume_and_req_with_no_cheating`.
The archive of reports, all confirming outputs and logs has SHA-256
`475aa777b9e0ebd98f0ab1b3498d453d3803091aeef54c1d8f2927d43e103444`.
This sample does not qualify the full source suite or a release runtime.

## Selected-runtime qualification

The complete genuine `cc7bbce4` source suite passed on native Linux with
Sail harness source `d422adf0`: 4,709 Same (58 ignored), all 4,651 nonignored
plain tests passed, zero Different, Unstable or infrastructure errors. All nine
`output_json` tests passed on both arms, including the three originally failing
proof-note cases. The exact source-suite harness implementation remains unchanged
on main `2936ead2`. This is qualified source evidence, not a claim that the
whole suite was rerun at that composed main commit.

The full-suite archive SHA-256 is
`4e5698ce37b3d4c3b61aa9c91ad052635c1f152c55b9f115f039cd04dc7519dd`;
its report SHA-256 is
`36978d5069b7ce82ef68a7372b7c7141e8ac74183b3bae0691a446178399e6cf`.
The [deep-tier record](../validation/deep-tier.md#fork-source-suite-gate-124)
retains the failed original sample and the stock-control failure. Broader normal
runtime, corpus and determinism qualification is in the
[selected-runtime record](../validation/closure-full-identity.md).
