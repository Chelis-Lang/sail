# Separate recovery of the interrupted c2 benchmark

After Docker recovery, the [original interrupted host checkpoint](m7-benchmark-c2a4b23-interrupted.md)
is unchanged. A separate [recovery record](m7-benchmark-c2a4b23-recovery.json) binds
the recovered native metadata and raw archive. Independent host hashing verifies
all 27 regular archive members. The 238,472-byte archive SHA is
`627bf2f643225a44d6db9b7082f38c8a2763e499211435805503b12b05a9d2fd`.

Native preparation passes in 6.241 seconds. The work controller still records
the run as `running`; results have `acceptance.complete=false`, no measured
samples and exactly three retained warmups:

| HumanEval warmup | Wall milliseconds |
|---|---:|
| M6 dependencies/solver | 128,134.163 |
| M7 dependencies/solver | 141,985.530 |
| M6 compute_gcd focused context | 67,246.325 |

No fourth M7 focus completion, original terminal exit, interruption-time closure
or final immutable seals survive. Its empty stdout cannot establish that it never
executed. Two daemon-start records and an empty partial cleanup-error list do not
certify final cleanup. The owner reports the post-restart census contains only
PID1 and census processes; ending the old VM cannot reconstruct original closure.
The acceptance container's new birth is `2026-10-12T02:24:03.212003465Z`, distinct
from its old `2026-10-11T23:02:56.820697964Z` birth.

Status remains incomplete storage interruption. These setup costs provide no
measured median, spread, ≥2× benefit or acceptance. The old 730 warmup failure,
current c2 repair pass and interrupted benchmark remain separate scopes. This
review starts no native actor, changes no original controller, and performs no
retry.
