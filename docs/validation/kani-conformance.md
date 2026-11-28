# Kani conformance

Status: fast tier passes over three rounds and the fault arm passes, on one
native host. The Sail side is the current per-harness CLI. Standard tier not
run yet.

Harness and rules: [ADR 0181](../decisions/0181-kani-parity-and-outcomes.md)
(parity, named variance, verdicts) and
[ADR 0183](../decisions/0183-kani-conformance-harness.md) (harness, fault arm,
corpus tiers). Corpus: `corpus/kani-corpus.toml`.

Environment: EC2 c7i.8xlarge (32 vCPU), us-east-2, Ubuntu kernel
`7.0.0-1018-aws`, native (no emulation), lane `kani-conf`, 2026-11-28.
Source: `feat/kani-conformance` on `main` at `1127d615`. Original Kani 0.68.0,
CBMC 6.11.0, the pinned nightly payload, Sail's staged bundle.

## Fast tier, three rounds

`ci/run.sh conformance kani fast` with `SAIL_KANI_JOBS=12`: each harness
runs direct, Sail, direct in every round.

| Project | Harnesses | Verdicts (3 rounds) | Sail claims | Checks compared |
|---|---|---|---|---|
| arrayvec-pilot | 9 | `same` 27 | bounded-success 18, counterexample 6, bound-insufficient 3 | 5,451 |
| lading_throttle | 17 | `same` 51 | bounded-success 51 | 12,075 |
| x86_64 | 0 | blocked: does not compile without `--all-features` (`SAIL6321`) | none | 0 |

Sail claims equal direct claims on all 78 harness rounds. There were no
differences between direct runs, or between Sail and direct, outside the
named fields. No name differed only against Sail (`unwitnessed: []`). These
named fields varied between the two direct runs: every timing and path name
in all 78 harness rounds, Cargo's `Finished` line in 76, export
`duration_ms` in 77, and Cargo `Compiling` order in 44. The vtable-size
description did not occur in this corpus.

`lading_throttle`'s CI argument `--solver cadical` is not passed; it is
Kani's default. Its `default_unwind` is 12, the smallest bound at which
`max_requests_per_interval_bounded` (a loop over 10 elements, no
`#[kani::unwind]`) is not bound-insufficient.

## Fault arm

`sail-conformance kani fault`, in-tree fixture `kani-fault` (`small`: four
assertions; `hog`: S6's 1000-step loop), memory cap 1536M.

| Fault | Direct Kani | Decoder alone | Sail | Pass |
|---|---|---|---|---|
| none (real tools) | exit 0, `VERIFICATION:- SUCCESSFUL` | admits bounded-success | bounded-success, as direct | yes |
| last result cut, exit 6 | exit 0, `SUCCESSFUL` | **admits bounded-success** | no claim: tool authentication | yes |
| results, then `SIGKILL` | exit 0, `SUCCESSFUL` | **admits bounded-success** | no claim: tool authentication | yes |
| `SIGKILL` before results | exit 1, `CBMC failed`, `FAILED` | refuses (`SAIL6301`) | no claim: tool authentication | yes |
| hang | killed at the deadline, no status | no driver exit | no claim: tool authentication | yes |
| cgroup memory cap | exit 1, `CBMC failed`, `FAILED` | refuses (`SAIL6301`) | no claim: refused (`SAIL6304`/`SAIL6301`) | yes |
| Sail deadline 1 s (`hog`) | not run | not run | no claim: timeout (`SAIL6304`) | yes |

No row left an entry in Sail's state directory other than attempt captures.
The kernel log shows the cgroup OOM killer killing `cbmc` (about 1.5 GiB
resident) in both memory-cap runs. Direct Kani prints success for both of
S6's false-success faults. The decoder alone admits them too, so today only
tool authentication keeps them out of Sail. Integrity supervision at the CBMC
boundary (ADR 0182) has to close them.

## Benchmark arms (structure smoke)

`sail-bench kani`, `-j 8`, same host, shared tenancy. These numbers show the
arms run. They are not a performance claim.

| Arm | Project | Direct `-j 8` median | Sail |
|---|---|---|---|
| cold | arrayvec-pilot | 5.31 s (3 samples) | 82.70 s: per-harness CLI, in sequence |
| warm | arrayvec-pilot | 4.69 s (3) | not built (ADR 0189) |
| edit | arrayvec-pilot | 5.37 s (3) | not built (ADR 0188) |
| ci-rerun | arrayvec-pilot | 5.57 s (3) | not built (ADR 0190) |
| history-replay | lading_throttle, 5 commits | 9.88 s (5, all 17/17 complete) | not built (ADR 0188) |

## Limits

- x86_64 is blocked until the CLI can pass Cargo features.
- Both arms pass `--unwind`, because the CLI requires it. Harnesses with
  their own `#[kani::unwind]` keep it.
- The standard tier and secondary Kani pins have not been run.
- The `ci/local.sh` container path was not run. There, the memory-cap case
  is skipped and says so.
- The direct oracle uses Sail's staged copy of the original driver, so the
  check locations' paths match. The bundle's bytes are authenticated against
  the pinned release.

Evidence: `2026-11-28-kani-conformance.tar.gz`, sha256
`82f68042848b8f1ae47a51139dc9229a4ea9d45853e33852566791696a4c9134`,
4,132,040 bytes (reports, per-harness records, fault rows, bench JSON).

## Attempts

- 2026-11-28, smoke 1, arrayvec-pilot, 1 round: `same` 9/9.
- 2026-11-28, smoke 2, lading_throttle: 13 `different` and 4 `unstable`,
  caused by the order of Cargo `Compiling` lines between direct runs. Fixed
  by naming that order as variance. 1 bound-insufficient at unwind 8, so the
  unwind was raised to 12.
- 2026-11-28, fault arm 1 and 2: the Sail deadline row ran the fast harness
  and finished inside 1 s with bounded-success (the row failed). Fixed by
  using `hog` for that row.
- 2026-11-28, first history replay: three older commits had no summary
  because their dependencies were not fetched. Fixed by fetching each
  commit's lock and counting such samples as failed.
