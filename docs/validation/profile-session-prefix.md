# Exact profile session prefixes

Status: native Linux full gate/integration and bounded independent review pass
at `fb2660c1`. No profile CLI or worker is available from this slice.

The selector borrows original bytes through the selected zero-based check,
retaining all preceding commands. Tests cover multiple checks/statistics and
stack operations, quoted/commented check text, chunk boundaries, missing
checks, truncated input, exit-before-check refusal and the 64 MiB bound. A
pinned-Z3 solver test compares all replayed outputs for three successive
checks whose answers are sat/unsat/sat. All of these controls now pass natively.

ADR0087 classification: diagnostic preparation with no selected runtime path
or differential oracle change. Full fast CI and affected solver controls are
required; no corpus or benchmark acceptance is inferred. Formal models and
production bodies they refine are unchanged. No new dependency or platform
code is added. Mac profile acceptance remains part of the complete feature.

Documentation: ADR0138, this record, portfolio README, changelog and roadmap.
The consumer skill and user commands are unchanged until profile integration.

Retained attempt `299eda78`, 2026-11-14: the first native gate stops at
Clippy's `assert_is_empty` lint in the new solver test. The successor uses
`assert_eq!` so a failure displays the original stderr bytes; no condition is
weakened. It also rebases onto current main after #743. The selector body is
byte-identical; successor qualification now passes below.


## Actual qualification

Native source `fb2660c1ae809a29b15a9fa9abd82931d209b37a`, base main
`1da17545`, native c7i.4xlarge/us-east-2, Rust1.99.0 and verifier1.99.1,
production core image `6c19f5c8d397b61c-core`:

| Scope | Actual result |
|---|---|
| Complete workspace gate | 1,408 passed / 58 declared skips; formatting, Clippy, doctests, rustdoc, layering, resources, dependency and generated-page checks pass |
| Solver tier | 54 passed, including the new original-history replay |
| Full local E2E | Pass, including Extra, all managed cases and repair CLI/MCP |
| Four PR conformance arms | Each 33 Same / 311 functions; zero different/unstable/infra/skipped; seven resource-drift units retained |
| Fresh independent package run | All 31 portfolio tests, including ignored pinned-Z3 controls, pass |
| Fresh independent adversarial controls | 273 pass: 246 exact-prefix positives and 27 refusals; 22 actual pinned-Z3 invocations, including all 11 original Verus-toy prefixes and their original answer sequences |

Independent stress controls include exact 64 MiB and oversized input, a maximum
command, one million nesting levels and 500,000 prior commands. They require
exact borrowed bytes and independently derived endpoints. Maximum-command RSS
is 198,896 KiB; the input bound is not a total worker memory guarantee. All
26 external-harness lock packages match original dependency identities.

The lead independently rehashes all 2,013 native review originals (586,842,408
bytes) and all 2,064 complete handoff payloads. Complete review archive SHA-256:
`d26074c85680bc83114cc6ed953cfca54762357c63119d48ed38b6db955d88cc`;
native archive `c9d841c99d208d47aaf55f1002b56cc524340b48c6ea349c30b52d750777edbb`.
The reviewer approves the bounded implementation, separately from full PR
qualification; the complete native gate above is independent lead evidence.
Original `299eda78` lint failure remains retained and is not relabelled passed.

No historical near-limit slicing experiment, traced/untraced parity, complete
profile worker, full corpus, Mac or release acceptance is inferred. Final
qualification documentation preserves all compiled source from `fb2660c1`.

The full-gate archive retains 875 original payloads, including the failed
initial attempt, exact source, binaries and conformance captures. Archive SHA-256
`030ad365782bcfb2f1c5b49f89097b817d821c0192274010a0d804f84df3a7f7`.
