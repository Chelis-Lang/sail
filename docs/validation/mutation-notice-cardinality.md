# Mutation explanation-note cardinality

Status: the existing per-code explanation classification is corrected. The
implementation passes complete native Linux qualification and retained-output
controls; no new diagnostic exception is introduced.
Mac qualification of the changed benchmark package and its actual retained
witness also passes after the independent product gate.
Original failed measurements remain failed under their original interpretation.

Unchanged source `039fb996`, A0e runtime, native c7i.4xlarge / us-east-1 /
Ubuntu 24.04, complete CapybaraKV catalogue: its cold round-zero original has
675 matching functions, equal successful exits, no diagnostic differences
or false verified functions, and exactly one existing `SAIL3138` plus one
existing `SAIL3146` explanation. The harness erroneously records
`more than one bundle-miss notice in one invocation`. Later cold rounds
reproduce that annotation.

`capy-notice-witness.tgz` contains its source/runtime header, original sample,
and both complete stdout/stderr pairs (six members, 85,553 bytes), SHA-256
`4d6c027da793fd4decc73530ca6ee5788f490a6df5c9457422e6d4a6d92d8a40`.

The drafted shared cardinality check allows one of each already recognized
code. Repeated copies of either, including differing text under one code,
remain failures. Existing prefix/suffix recognition and complete Verus
comparison remain unchanged. Required qualification: actual retained-output
recheck and negative controls, native gates, and separate interpretation of
any original raw samples. No new timing samples or performance acceptance
are claimed by this record.

Retained attempt: `cdea94c0`, 2026-11-03, native gate refused an unregistered
invented error-code literal in the new negative fixture. The fixture now uses
the already registered, unrelated `SAIL3102`; registration enforcement and the
unknown-note refusal remain unchanged. All other 1,324 tests passed in that
attempt; the complete corrected gate passes below.

## Native qualification

Source `29e9ef18`, pinned A0e runtime, native c7i.4xlarge / us-east-1 /
Ubuntu 24.04: Clippy, all 1,325 workspace tests and the remaining workspace
gate pass; all 51 solver controls and every local E2E phase including `extra`
pass. All four fast arms report 147 Same / 3,451 functions, and all four
standard arms report 159 Same / 13,166 functions, with zero disagreements,
instability, infrastructure failures or skipped projects. All eighteen standard
seeded mutations, cold verification and both original-source returns pass.

The actual retained 675-function case and its reordered explanation pair
recheck as Same. Nine altered-output controls are rejected: repeated copies
of either code, differing text repeated under either code, an unrelated note,
a malformed explanation, added verifier stderr or stdout, and a changed exit
that must also report false verified. No solver, product or timing execution
is replaced by this retained-output recheck.

The complete native capture contains 8,223 independently checked sealed
members, including every conformance artifact, E2E output, gate log, original
failed fixture attempt, actual witness, auditor and altered controls. Archive
SHA-256 `e5bfbcda1aaa49a61575e6efe03741a40470a222323226b571bdd15bc0690961`.
These qualify the corrected uniqueness check, not performance acceptance.

Mac qualification uses the same production benchmark source, separate physical
source/target, mac-m4.metal / us-east-1c / macOS 26.7. Package Clippy and all
68 non-solver package tests pass. The actual Mac cold capture (675 functions)
and reordered pair recheck as Same, and all nine altered-output controls are
rejected. This is qualification of the changed benchmark package; the separate
product gate covers the complete workspace. The 82-member capture is sealed
and checked after transfer, SHA-256
`c68209dca2eb354f4eaff48d059b55463c2acd1fda6c94456b268289549a804d`.
The unchanged Mac witness archive is retained inside it, SHA-256
`f1763d9f1a792730a070ccfb24e3a4890d615aa01f7de6587c202bd3aae8e2e8`.

After rebasing onto `81c41be8` and its `0.2.0-rc.1` dependency graph,
the complete native workspace gate passes again at `a90a837f`. The three
changed Rust files are byte-identical to qualified `29e9ef18`. The complete
new gate log and both successful exit markers are sealed and checked after
transfer, archive SHA-256 `b3e800ddd347559e42850f6e094e1a3d020c4be8bb3752c001bf8569178c253e`.
The earlier solver, E2E, standard corpus and Mac package results retain their
recorded source and scope; final release publication still requires owner approval.
