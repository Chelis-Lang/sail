# Concrete-trait function spellings

Status: built and measured on native Linux; not qualified or selected. Proposed
[ADR 0056](../decisions/0056-stable-smt-function-spellings.md) preserves original
Rust/VIR identities and changes only eligible function names generated for AIR.
[VI-0030](../verus-issues/VI-0030-anonymous-impl-numbering.md) retains the cause.

| Gate | Current result |
|---|---|
| Independent production-source review | Pass for prototype03; earlier two proposals rejected |
| Matching normal verifier/vstd build | Pass on native Linux |
| Focused controls | Five pure VIR and eight compiler fixtures pass; eight standalone default-suite cases also pass |
| Matched real-project edit screen | Pass: all 27 calls, three rotated rounds, 24.693% edit-median reduction |
| Whole-project per-function/diagnostic comparison | Pending; timing checks do not replace it |
| Complete named SMT census and residual-stream classification | Pending |
| Controlled crate/root variation, full FAST/STANDARD/DEEP, source suite, determinism, macOS | Pending |
| Default runtime pin | Unchanged |

## Source and environment

Production fork source is `0a4a06cb6431d5a36e1ea4c8eb9c8da938866a24`, based on
accepted `e5365c05d0a9f8c8cb655edc035ce9a2f6dde482`. Normal candidate runtime
archive SHA-256 is
`10eff8fba0c3c8d4c517742ddf7db16284972e1d67b2236a1d18f31309b701a7`;
accepted runtime archive is
`689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928`.
Common Sail source is `83128ad0d5327f00bfea822d608ebd99dd9d8112` and Vest is
`11af01ddaef76cceded03517cd48764adb9d35b1`. The isolated native host is EC2
c7i.4xlarge, 16 vCPU, us-east-1.

## Actual matched edit screen

The retained three-actor harness compares stock, accepted Sail and candidate
Sail across cold A, an unrelated empty impl insertion B, and returned A with a
comment. All 27 calls and original checks remain. Returned A is not unchanged B.

| Metric | Accepted | Candidate |
|---|---:|---:|
| Edited wall samples (s) | 45.089, 45.108, 44.968 | 34.266, 33.955, 33.885 |
| Edited wall median (s) | 45.089 | 33.955 |
| Edited solver starts, each round | 30 | 24 |
| Edited summed-thread SMT median (s) | 46.650 | 30.423 |
| Cold wall median (s) | 89.992 | 89.595 |
| Returned-A wall median (s) | 31.105 | 30.959 |

The edit median falls 24.693%, and every candidate edit sample is faster than
every accepted sample. Summed-thread SMT adds elapsed SMT durations across threads; it is neither
measured CPU time nor whole-command elapsed wall time.
Each arm retains all 266 literal session hashes, including duplicate and empty
inputs; edit churn falls from 30 to 24 sessions. Three passively different roots
produce equal A-return input multisets within each arm. Explicit controlled
crate/root variation remains pending. Source, runtime, common command bodies,
MCP and corpus restoration guards pass. No general first-run gain is claimed.

Complete capture SHA-256:
`9f102bb20ac4fbbd000bbd7392c32423aa42031efdd092d2dc26450a8aa1c96a`
(8,296 members, 69,400,537 raw bytes, 24,261,907 compressed bytes); every member,
mode and hash was verified. Focused/timing summary SHA-256:
`03288ab9f9c72739444db044f54d35f6c761efebb7ac11164dafee21d20aa858`.

## Focused controls and limits

The five pure VIR cases exercise injectivity, bounds, complete-set selection,
duplicate fallback and raw AIR refusal. Eight compiler cases pass on accepted
and candidate runtimes, retaining complete original diagnostics and literal
inputs, plus original paths and actual native producer/consumer behavior.
Impossible-predicate and const-specialization duplicates are frontend-refusal
controls, not positive duplicate-header coverage. The standalone-suite adaptation at `d3f38611` also passes all eight cases,
with all three fixture environment variables unset. Its three negative cases use
a same-runtime aliases-disabled control; these are not independent accepted-runtime
conformance. Full original production source/runtime restoration passes. Complete
75-member capture SHA-256:
`2eb14075e8e0339a06a72b10540ed07668dbee33b1bafc406b1667e16a568d17`;
actual default summary SHA-256:
`02af5b78e0f6e38eff688e4f7c7e78708213cc452c8aeaf86266948b043cf6eb`.
The delivery implementation is [fork PR 8](https://github.com/Chelis-Lang/verus/pull/8);
it remains draft while the broader qualification proceeds.

Failed and interrupted attempts remain visible:

| Source/date | Result and cause |
|---|---|
| Proposals01/02, 2026-10-18 | Rejected by independent source review: native ghost-erasure inventory mismatch, then unsupported coherence uniqueness assumptions; never built |
| Prototype03 initial focused run, 2026-10-18 | Five compiler fixtures failed because of invalid fixture premises; matching normal runtime build succeeded; original results retained |
| Initial focused wrapper, 2026-10-18 | Missing observe-maximum control stopped the wrapper before compiler fixtures; retained with the original build capture |
| Prototype03 baseline01/candidate02 intermediate runs, 2026-10-18 | Further fixture-premise failures remain in the complete R3 capture; later repaired passes do not relabel these attempts |
| Repaired focused run, 2026-10-18 | Five pure and eight compiler controls pass; external fixture SHA-256 `04fef7326e3851de350b8ef5d04d45cfcc92bca886946b3be0fbcf4f3a5b9b5d` |

The reduced corpus example remains unrun. Timing-harness statuses and focused
controls are not complete whole-project or corpus acceptance. No fork merge,
Sail runtime re-pin or default behavior change follows from this record.
