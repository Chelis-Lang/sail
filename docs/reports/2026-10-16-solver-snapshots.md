# Solver snapshots: prototype findings

Status: experimental and disabled by default. No default-mode acceptance or
release selection. [ADR 0051](../decisions/0051-exact-solver-snapshots.md) describes
the candidate; the [validation record](../validation/solver-snapshots.md) lists
the completed checks and missing gates.

The prototype parks copies of the pinned Z3 process at exact input-prefix
boundaries. On an edit miss it resumes the latest available prefix instead of
re-solving that prefix. Ordinary exact cache keys and fresh-solver fallback
remain necessary. A raw-output equivalence proof is still missing.

## Exploratory native measurements

The recovered `m112b-results.tgz` archive has SHA-256
`6234d60a9ad4ef0358e09c63d2f002303145a974f5ee53fc9bdea3c619c4b021`.
Its `m112/t-anvil-shim/results.tsv` contains two rounds of the fork's Cargo-Verus
focused invocation through Sail's shim, with and without snapshots. Native
EC2 c7i.4xlarge, us-east-1; prototype source lineage through `88076b62`.
These are candidate timings, not default CLI acceptance or measurements of the
new admission bounds.

| Anvil edit | Round 1, shim | Round 1, snapshots | Round 2, shim | Round 2, snapshots |
|---|---:|---:|---:|---:|
| Body, early | 87.970 s | 35.834 s | 98.609 s | 46.906 s |
| Body, late | 87.106 s | 39.417 s | 104.083 s | 55.948 s |
| New lemma | 86.612 s | 35.996 s | 97.776 s | 39.824 s |
| Widely used spec | 114.373 s | 112.605 s | 110.403 s | 119.803 s |
| Fix after failure | 55.605 s | 55.555 s | 55.335 s | 61.785 s |

The body and lemma rows justify further investigation. Wide changes and the
failure/fix cycle have no demonstrated gain. The archive also contains earlier
Anvil `sail`/`snap` invocations that exited 1 before verification because they
used partial selectors with `verify` rather than `focus`. Those failed samples
remain in scope and cannot establish a performance comparison.

The native exactness experiments compare replies both raw and after masking
statistics. The exploratory successes after masking include `:num-allocs`
differences. They therefore do not establish raw-byte equality. The updated
report predicate counts masked replies as failures of byte-exactness. Earlier
format-comparison failures remain failures; they are not silently discarded.

## Review findings and bounded next step

The original preload's marker syntax consumed genuine NUL input. A tiny real
pinned-Z3 example, `(echo "x<NUL>y")`, exited 0 plainly and 1 with the preload;
a NUL in a comment could silently remove subsequent output. The prototype also
checked its snapshot count only when starting a solver, so a single solver
could create eight parked snapshots with a configured limit of two; four
concurrent solvers created 32.

The local hardening separates NUL data from markers, checks the preload framing
version before any input, and admits each parked process under a shared count
and conservative resident-memory budget. Corrupt or unavailable setup falls
back before forwarding input. Admission refusal only skips the snapshot.
The count/memory caps apply to parked-process admission, not to all future
memory use of ordinary active solver workers.

Next, qualify the revised prototype natively, measure its costs with the new
bounds, and preserve both raw and masked reply counts. To avoid redundant
measurement work, a future harness may memoize a fresh oracle by solver binary
digest, exact argv and input digest, explicitly recording reference reuse.
Timing an edit can select only changed transcripts after establishing the full
base cache; this is a changed-transcript experiment and cannot be labelled a
whole-project benchmark. Neither optimization permits dropping failed samples.


## Continuation state audit

Pinned Z3 source tag `z3-4.16.0` resolves to commit
`ddb49568d3520e99799e364fb22f35fc67d887b1`. Its
[`scoped_timer.cpp`](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/scoped_timer.cpp)
registers `finalize` as a pthread atfork prepare handler. Completed timed
queries leave idle timer workers; the handler joins and deletes those workers
before either process continues. On the actual pinned Mac runtime, the
minimal timeout query leaves two threads; the old snapshot marker reduces the
continuing parent to one. A global shell timeout uses an active timer worker,
which is absent from a forked child. No upstream soundness defect is alleged.
The corpus [minimal reproduction](../../corpus/repros/z3-snapshot-timer-state/README.md)
and new conservative admission checks keep those cases visible.

The source's
[`memory_manager.cpp`](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/memory_manager.cpp)
also shows why `num-allocs` is implementation state, not a logical effort
counter. Its thread-local implementation integrates allocation counts into a
global count at 100,000-byte accounting thresholds; allocation/deallocation
accounting uses the allocator's actual usable block size. Fork callbacks,
libc allocator state and parked-server control allocations therefore need
qualification, rather than an assumption that copying the address space is
sufficient. This source observation does not identify the cause of the
historical native counter differences, and no counter is reset or masked.

The new raw probe retained a 125,724-byte IronKV transcript at its first push
(offset 105,643), whose prefix emitted no stdout/stderr. It ran two independent
fresh processes, one marked process and three resumes on local macOS. One old
prototype resume differed in three printed `:time` values, 0.00 versus 0.01;
a second run with admission checks also observed the same clock variation
between two fresh processes. Every exit was zero and stderr empty. Both whole
raw comparisons fail. In this sample the other bytes, including `num-allocs`,
match; this does not qualify other transcripts, later snapshots or native
Linux. It also does not establish that snapshotting caused the clock changes.

Z3's
[`stopwatch.h`](https://github.com/Z3Prover/z3/blob/ddb49568d3520e99799e364fb22f35fc67d887b1/src/util/stopwatch.h)
uses a steady clock. Literal equality of independent clock readings is not a
useful state-copy theorem. The snapshot acceptance predicate nevertheless
remains unchanged and failing; ADR 0011's stored timing behavior does not
license a new normalization or statistical-counter exemption. Semantic/verdict
agreement must be qualified independently on the exact runtime and complete
corpus, with resource limits and all diagnostics preserved.

The complete inherited native run is now archived externally as
`m112a-results.tgz`, SHA-256
`bcccdcc149d31cb6bef770e5b760897c085d2ebbf7e917d83552c385ffcef573`.
Its source, traces and masked replay summaries are retained, along with the
actual binary/preload archive SHA-256
`68294281141178e832e17a15a8323e613fb16c068ca90e6572afaa1456b0c309`
and Linux Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
Historical replay headlines discard stderr and judge masked stdout; they are
exploratory measurements, not complete raw-output acceptance.


## One-use live-prefix implementation

`native/sail-live-prefix` implements an alternative research path: prepare a
fresh pinned process on the exact prefix, keep its original threads/state,
then consume it once on an exact match. A first set of tiny Mac fixtures
matched raw Boolean, NUL, model and timeout continuations and observed the
same serving PID with two timer threads. Independent executable-image review
then rejected that source: swapping a private executable at Popen and restoring
its pathname before claim returned `sat` where fresh pinned Z3 returned `unsat`.
Those positives therefore do not qualify a usable candidate.

The current correction holds the actual Linux `/proc/PID/exe` image and pidfd,
verifies it before sending prefix input and again at claim, and refuses Mac
live reuse. All 28 bounded native Linux controls pass, including actual foreign
execution refusal, exact continuations, a two-thread timer worker and resource
and claim controls. Mac refusal and exact fresh fallback have been checked. This is not
Sail/daemon integration, arbitrary-SMT eligibility or real-project acceptance.

The driver has explicit recorded preparation, TTL, serving, fallback and oracle
limits, each bounded by one hour with short correctness defaults. It saves raw
streams/status and keeps refused/timeout/fallback samples as failures. Native
metrics distinguish background CPU, total solver CPU, foreground fallback cost
and a guarded fresh reference. No Mac performance measurements or saved-CPU
claim are made. The complete command/response boundary must currently come
from trusted capture metadata; Session/ledger eligibility and full protocol
binding remain implementation gates before default use.

## Real full-trace fresh controls

The native baseline now uses complete, actually changed IronKV and Anvil
body-edit transcripts from the preserved archive. IronKV's exact common prefix
contains 23 solved checks before one remaining check; Anvil's contains seven
before two remaining checks. The full edited inputs are 2.46MB and 6.29MB.
The selected command boundary uses existing trace responses; no SMT command
is added and no output is normalized.

Three independent complete runs per dataset use the same actual pinned Linux
Z3, with finite 300 s limits, a 16MiB input ceiling and 1GiB sampled RSS budget.
Every process exits 0 with empty stderr and the same check replies: 24 `unsat`
for IronKV, nine for Anvil. All six actual executable images are verified.
Their complete stdout is nevertheless different between fresh runs. Every
changed line in these recorded comparisons is a `:time` field. The other
bytes, including `num-allocs`, match in this particular fresh scope. That does
not explain or erase the old fork/resume allocation differences.

IronKV's guarded fresh runs take 2.190, 2.178 and 2.177 s; Anvil takes 55.068,
55.910 and 55.853 s on the native c7i.8xlarge. These are baseline observations,
not live-prefix gains. One IronKV sample also retains an `ESRCH` memory-read
error during normal process exit; it remains a failed sample. The literal raw
gate fails for both datasets, so the controller records two explicit skips
and runs no one-use timing sample. It exits 1 and keeps every raw result.

The disabled prototype therefore remains unaccepted. This evidence separates
agreement of check replies from literal clock-byte equality; it does not
change the charter, normalize output or relax the gate. The
[qualification record](../validation/solver-snapshots.md#actual-native-full-trace-fresh-controls)
records exact source/runtime identities, limits, all failures and checksums.
