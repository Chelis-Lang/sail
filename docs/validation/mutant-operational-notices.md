# Mutation benchmark operational-notice classification

| Field | State |
|---|---|
| Tracking | #728; #595 export recovery remains separate |
| Classification | Benchmark diagnostic tooling; no verifier, solver, key, reuse or default-output change |
| Status | Native Mac, fresh bounded review and Linux merge gates pass; protected PR checks remain |
| Affected observation | Capybara deep mutation qualification, first arithmetic mutant: 675 function comparisons, matching exit/outcome, no false verification; only an explained Sail retention notice differs |

The mutation runner requests `--explain` and already retains known Sail
explanations separately from Verus stderr. The producer's newer missing-export
notice used a different exact shape and was left in the diagnostic stream.
The original failure archive remains retained outside git at SHA-256
`fb1abf9de9ae688867689f52fdfc46f49ff51867ccf9c4dfd542685238c07062`.
This is not a new default-mode verifier diagnostic or an accepted runtime
qualification result. The original strict failures remain failed.

The separator recognizes this existing exact prefix/suffix and a bounded
Cargo target list as its own category. It preserves raw logs, stores the
complete original notice in `sail_operational_notes`, and keeps one notice
per category. The existing bundle and retention categories keep their own
limits. Changed text, codes, multiline values, invalid target lists and
repeated notices refuse; all actual Verus diagnostics stay comparable.
No generic `SAIL3146` filter is added.

No corpus source, benchmark expected outcome or score rule changes. There is
no user CLI/MCP or consumer skill change. Actual real-project replays and
remaining gates will be recorded here with their original scope.

The first native Mac host gate at `2b6bc0ba` refused the negative test's
invented error code as absent from the registry. The test now substitutes
the existing bundle code and still requires rejection; no production
recognition rule changed.

At `bf9d140c`, native EC2 mac-m4.metal/us-east-1c, macOS 27.0,
Rust 1.99.0: all 13 focused workspace tests, all-target benchmark clippy,
the complete host gate (1,364 passed, 53 skipped), book source checks,
mdBook build and local book links pass. Node was absent, so the optional
highlighting probe did not execute.

Fresh independent native Mac review replays the original Linux capture through
the actual comparison and judgment functions: 675 function comparisons,
exits 101/101, the original failing outcome, two separately retained notices,
zero differences, no accepted divergence and no false verification. Remaining
Verus stderr is byte-identical. All 27 independent negative cases and the
13 focused tests pass; changed diagnostics, outcomes, notice shapes and
duplicates still refuse. This is frozen-output replay, not a fresh Capybara
verification or a completed deep mutation suite. Review SHA-256:
`07e7fbabdc2ffc1a65328436c2261bedf7e8bc96a4b4c8fc35d429da79bf0a73`.

Reviewer setup failures stay retained outside git: the first Mac detached
launch did not execute; the bin-target filter selected zero tests; a relative
producer path used the wrong working directory. Corrected library execution
ran every intended control. None of those attempts counts as a pass.

At `8f6cc865`, native Linux c7i.4xlarge/us-east-2, Ubuntu 24.04,
CI Rust 1.99.0: full workspace gate 1,377 passed/53 skipped, solver 50 passed,
complete E2E including extra, and all four PR conformance arms pass. Each arm
compares 33 units and 311 functions, with seven rlimit-drift units and zero
different, unstable, skipped-project or infrastructure results. The final
implementation preserves the exact reviewed benchmark source; final evidence
updates change documentation only. No longer corpus or new performance result
is claimed by these merge gates.
