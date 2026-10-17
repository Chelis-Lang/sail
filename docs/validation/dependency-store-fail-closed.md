# Native-artifact store quarantine

**Status:** containment policy accepted. Native integrated gate, solver,
end-to-end, PR conformance, full FAST and STANDARD pass at their stated sources.
The revised nine-phase CI layout passes its exact-source native checks; final CI
remains required.
No release or runtime pin changes.

[ADR 0053](../decisions/0053-disable-unqualified-native-store.md) disables
v1 dependency-store reads and publication until native input identity
is qualified. This addresses the mechanism behind
[#187](https://github.com/Chelis-Lang/sail/issues/187) by making fresh targets
execute normal native compilation.

## Native assessment

On 2026-10-16, a dedicated EC2 c7i.4xlarge (16 vCPUs, us-east-1) compared actual
release binaries from baseline `3c0498c3` and containment candidate `19034f8d`,
using the original pinned runtime. Full source and runtime inventories and
binary identities were checked before and after. Both binaries were built
normally from their exact sources. Each round retained a successful donor,
fresh target, same-PATH compiler replacement, independent fresh stock failure
and stock fresh-project cost. All 48 commands and every raw output were kept.

| Round | V1 seeded fresh project | Containment fresh project | Fresh stock |
|---|---:|---:|---:|
| 1 | 1.2819 s | 32.8740 s | 34.3641 s |
| 2 | 1.2864 s | 32.5718 s | 34.8324 s |
| 3 | 1.2803 s | 32.0525 s | 34.3114 s |
| Median | 1.2819 s | 32.5718 s | 34.3641 s |

In all three rounds, replacing the private `cc` at the unchanged `PATH` made
fresh stock and the containment candidate exit 101 with the replacement's
literal diagnostic. V1 still exited 0 and reported Current. Positive store
capacity could not re-enable the candidate. Final source, binary and runtime
guards passed; owned scratch cleanup completed. The assessment itself remained
nonzero and unqualified because its unchanged default-enabled unit test was
red (654 tests passed, one failed, 35 skipped). It did not waive that gate.

Raw probe archive SHA-256
`0430cf3843d13a550b068403a898ffeccdc8e1afe66b2a95a11f6d56bf1c6ed9`;
complete assessment archive SHA-256
`9cc057c0e8ac331b8ecfb1185c800df935fa3452d85a1ef6cc3835840bd843f3`.
The original macOS reproduction remains cited by ADR 0053. This native
assessment is a bounded small-project comparison, not a whole-corpus or
full-output-equivalence qualification.

## Integration and remaining work

After seeing the safety reproduction and cost, the owner authorized repair
sequencing while requiring real performance wins. The integrated checks now
require disabled publication/reuse even with positive capacity, fresh vstd
compilation in new targets, correct changed-compiler failure, preserved
per-function verdicts, and subsequent reuse of each project's own receipts.
The old requirement and its red assessment remain visible above. No
conformance comparator or benchmark baseline is changed.

Native Linux qualification uses the original pinned fork and Z3, on separate
EC2 c7i.8xlarge hosts in us-east-1:

| Source | Actual completed scope | Result |
|---|---|---|
| `7b0908bd` | Workspace gate, solver, all seven end-to-end phases plus extra, changed native compiler, PR subset, full FAST and STANDARD | 655 unit tests and two doctests pass; 35 solver tests pass. Each PR arm has 33 Same / 311 functions; each full FAST arm has 147 Same / 3,450 functions. Zero differences, instability, infrastructure errors or skipped projects. Each STANDARD arm has 159 Same / 12,726 functions, with zero differences, instability, infrastructure errors or skipped projects. The first three STANDARD arms report one rlimit drift each; the engine arm reports zero. |
| `eae5c4d2`, based on main `3dd4e28e` | Workspace gate, solver, all seven end-to-end phases plus extra, changed native compiler, PR subset | Same gate and solver counts; all four PR arms have 33 Same / 311 functions and zero differences, instability, infrastructure errors or skipped projects. Final source, runtime and binary guards pass. |

The current-main integration changes only the failure-evidence harness and
documentation relative to `7b0908bd`; the Rust and managed acceptance source
are identical. Independently compiled `sail`, `sail-z3` and
`sail-conformance` are byte-identical, including the current-main final guard.
Full FAST and STANDARD belong to `7b0908bd`; they are not
described as current-head tier runs. The current-head queue also ran a separate
workspace gate for research source `fee8abff`, with its own target and cache;
that result does not qualify the research experiment itself.

The shared developer Mac ran `ci/run.sh host` on exact `7b0908bd`: clippy and
644 tests pass, with 35 skipped. Source inventories match before and after.
This is a host correctness check, not a clean-machine performance measurement.

Current-main final archive SHA-256
`85a913c6a411fd2f06570b83004c0085cfe7743075c196280abde6c995bd7314`;
Mac host log SHA-256
`6ef9acb4ffb029c2c20f520bf0b7a94488645b731ce6e28e877dd480af155107`.
All archive members, sizes, modes and digests were independently checked.

The original `7b0908bd` initial/final and current-main `eae5c4d2` initial/final
compiled binary maps are byte-identical for all three binaries. Its final full-tier
archive has SHA-256
`a57b8a0ec8439a5c7181110e583c77f4f8dc73b03fdce676d80d84236f767786`.

The original combined managed CI job was cancelled at its ten-minute hard limit
on `50c78190`. No assertion failure was visible, and the exact captured command
was unknown. The revised [nine-phase layout](../testing.md) retains every
assertion, gives the two new fresh-project groups independent passing donors
and keeps the complete local union. Exact `6713c491` then passed on a native
EC2 c7i.4xlarge (16 vCPUs,
us-east-1): workspace gate (655 tests, 35 skipped, plus two doctests), all
35 solver tests, the full local nine-group end-to-end union plus extra, and
separate `managed`, `managed-copies` and `managed-native` entry points. The
standalone groups took 116.3 s, 71.0 s and 67.5 s respectively; these are
correctness-job durations, not performance acceptance measurements. The full
local union took 580.0 s. The retained changed-compiler evidence and all four
PR conformance arms pass (33 Same / 311 functions each). All 576 source files,
index and untracked-input guards match; initial/final hashes of all three
compiled binaries and the final pinned runtime/MCP guards pass. This validates
the revised test layout, without relabeling the earlier full-tier runs as
`6713c491` runs. Final archive SHA-256:
`d53ed2c9b1c062821111dc43f607f5eb200b2af36d8029b666cbacaf55ad91e7`.

Final CI and independent integration review remain required before merging.
A sound fast replacement and first-run speed recovery remain open work;
containment alone does not complete them.

| Source / date | Attempt | Result and limit |
|---|---|---|
| `19034f8d`, 2026-10-16 | Initial native assessment | Default-enabled unit test red; controller refused its actual indented failure format before any cost probe. Raw gate SHA-256 `a642953da8426972bc6def7d2e369bda0cbed9db7c65081e94208c905ea2ad3c`. |
| `19034f8d`, 2026-10-16 | Corrected assessment controller | Same gate remained red; all three reproduction/cost rounds completed, with final identity guards. No merge qualification. |
| `50c78190`, 2026-10-17 | Original managed CI job | Cancelled after ten minutes; no visible assertion failure and exact in-fixture point unknown. Failure upload did not run. Literal job-log SHA-256 `58f5fabf7733015d2df54a0f1d33ac8a1e78e925e06e048fb966f903167351d4`. |
