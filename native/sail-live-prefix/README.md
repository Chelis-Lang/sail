# One-use exact-prefix research prototype

This is a disabled, solver-only prototype for #158 and ADR 0040 option A. It
is not integrated into Sail, does not cache a proof, and has no accepted
performance result. It keeps the original pinned process alive, including its
threads, while waiting for an exact suffix. No fork, preload, extra SMT
command, reset or normalization is involved.

A seed binds the executable digest, argv, complete environment digest, working
directory, exact prefix bytes and expected prefix stdout. Preparation starts a
fresh solver and replays that prefix: this is real CPU work. A slot is ready
only after its expected stdout matches exactly, prefix stdin was submitted,
and memory observation succeeded. Empty expected output proves submission,
not that trailing declarations have been processed. Callers must supply a
validated complete command/response boundary; this research tool does not
replace Sail's existing Session parser or attribution ledger.

A consumer must supply the exact prefix followed by its suffix. The original
process is claimed once, under a lock, and receives only the original suffix.
It is never popped back or reset. The result retains both whole stdout and
suffix stdout; future integration must preserve the exact association with
already served cached-prefix replies. Unexpected prefix bytes or stderr,
expired/not-ready entries, resource/identity failures and mismatches refuse
reuse. The driver closes unused preparation before starting fresh fallback.
No provisional result is reported verified.

All warming, ready and claimed processes count against a pool's slots. Each
slot reserves a conservative RSS slice of the configured total; RSS sampling
continues while active and is checked at claim. Linux PSS is recorded too.
This is a sampled safety threshold, not an OS hard memory limit. There are at
most eight slots and 16MiB per captured stream. TTL covers preparation and
idle time; a separate recorded run limit covers consumption. The driver uses
one slot. Host-wide fairness and daemon integration remain unbuilt.
The input budget remains 2MiB by default. An explicit integer budget may
increase it to at most 16MiB for real research traces; both seed and whole-input
admission enforce that budget. Fractional count/byte budgets are refused.
Native fresh reference processes use the same sampled RSS threshold.

## Executable-image boundary

The first candidate checked only the executable pathname before and after
launch. An actual ABA test replaced a private copy of Z3 with a shell stub at
Popen, ran that stub, then restored Z3 before claim: the candidate returned
`sat` where pinned Z3 returned `unsat`. That source is rejected.

Current live reuse requires Linux pidfds and `/proc/PID/exe`. Before sending
any input, it opens and hashes the actual kernel executable inode, retains
that descriptor, and revalidates the running image at claim. The pidfd pins
child lifetime and owns cancellation; ordinary path restoration cannot make a
foreign running image into the pinned one. Fresh native fallback/oracle
processes use the same image-capture routine before input. macOS live reuse
refuses until an equivalent native guard is implemented and qualified; its
fresh fallback remains ordinary direct execution, never candidate acceptance.
The bounded native Linux suite passes all 28 controls, including the actual
foreign-image substitution and refusal, genuine pinned continuations and a
two-thread idle timer worker. This does not qualify arbitrary transcripts,
shared-library/filesystem mutation, integration or performance.

## Bounded correctness and native experiment

```sh
python3 native/sail-live-prefix/test.py --z3 Z3 --out NEW_TEST_EVIDENCE
python3 native/sail-live-prefix/run.py --z3 Z3 --solver-sha256 SHA256 \
  --prefix PREFIX --prefix-stdout EXPECTED_PREFIX_OUTPUT --suffix SUFFIX \
  --preparation-window 600 --ttl 900 --run-timeout 900 \
  --oracle-timeout 900 --fallback-timeout 900 --max-bytes 1073741824 \
  --max-input-bytes 16777216 \
  --native-metrics --out NEW_SAMPLE_EVIDENCE
```

Performance metrics require native x86-64 Linux. Defaults are short correctness
limits; explicitly choose bounded limits for long workloads. Every limit and
timeout/refusal is recorded. A fallback or failed raw comparison exits 1 and
stays in the sample set. The driver retains candidate, preparation and
independent fresh stdout/stderr/status. Raw equality is never masked. The
candidate gate requires both candidate and reference exit 0; equal failing or
signal exits cannot pass. A final cleanup error forces failure and is retained
alongside any earlier error in the manifest. Cleanup is still attempted.
These driver paths have eight actual-child fixture controls:

```sh
python3 native/sail-live-prefix/test-driver.py --out NEW_DRIVER_CONTROLS
```

They execute the driver, core threads, pipes, child status and cleanup, with
explicit Linux/image/RSS guard doubles so they can run on either platform.
They do not qualify those native guards, Z3, performance or a timing exception.
The archived driver fails five controls: equal nonzero and signal exits, and
three final-cleanup manifest-retention cases. The native 28 core controls remain
a separate unchanged source/evidence scope; the memory-observer exit race and
both real-trace raw failures remain unresolved.

The fresh reference includes image-validation overhead and is labelled guarded;
this is not a plain-Verus benchmark. Preparation CPU, total solver CPU and
foreground/fallback latency are separate: background work is not saved CPU.
The driver waits until ready or the preparation window expires before claiming
the process. This measures idealized availability, not a predictor's real hit
rate or an application's editing schedule.

Before a real trace sample, run the same command with `--fresh-only
--fresh-runs 3`. This starts three independent fresh processes with the full
original input and compares every stdout/stderr byte and exit status. It
never seeds a pool; `candidate_gate_pass` stays false. Only complete, identical
native runs with exit 0 pass `fresh_control_pass`; differences, timeouts,
memory refusals and errors remain recorded failures. The trace files must be
owned immutable regular files. No archived statistics are masked to make
fresh output or prefix readiness match.

`run-real.py` runs the retained IronKV and Anvil changed-check datasets in
sequence. Its source directory contains `pairs.json`, `source-hashes.json`,
the frozen driver/core and their `response_policy.py`/`memory_lifecycle.py`
modules, and each project's `prefix.smt2`,
`prefix.stdout` and `suffix.smt2`. It validates those hashes and the full edited
input reconstructed from prefix plus suffix. It first runs three complete
fresh controls per dataset; a one-use sample is eligible only when those
strict controls pass and the archived expected prefix matches fresh output.
Every failed or ineligible dataset has a retained reason and makes the parent
exit nonzero. Source and solver identities are checked again in its final
block, including on errors.

```sh
python3 native/sail-live-prefix/run-real.py \
  --source IMMUTABLE_DATASET_DIRECTORY --out NEW_RESULTS --z3 PINNED_Z3
```

The actual native three-run controls fail for both datasets: all check replies
agree, but literal `:time` output varies between independent fresh processes.
No live-prefix timing runs follow that failure, and no statistics are removed
from the acceptance predicate. One IronKV sample also retains a memory-observer
`ESRCH` during normal process exit; it remains a failed sample. This does not
complete #158 or establish a real-workload speedup.

The native suite reproduces the executable ABA by actually running a foreign
image, restoring the path and refusing it. Earlier Mac positive fixtures
predate that defect and do not qualify the current candidate. Current Mac
checks qualify only explicit refusal and exact fresh fallback. Source/runtime
hashes and scope are recorded in the
[qualification record](../../docs/validation/solver-snapshots.md).

Before default integration, use Session/ledger eligibility to exclude
side-effect or asynchronous output modes, classify wall-time/unstable results,
preserve full provenance, validate byte partitioning, and run complete native
conformance and end-to-end performance against plain stock. A live process
preserves history; that observation does not establish raw clock or counter
equality or a new verdict-changing mode. Full corpus acceptance remains open.

## Explicit numeric-time-only experiment

The owner authorized a separate research comparison under
`--experiment-numeric-time-only`. It is not default-mode conformance, a cache
normalization, a shipped mode or permission to change another response byte.
The original `raw_equal`, `fresh_control_pass` and `candidate_gate_pass` remain
byte-exact. Separate `experiment_*` fields identify this experiment's outcome.
Both standalone drivers accept the flag; omission preserves exact comparison.
An experimental pool binds its named response policy separately in its key,
while retaining the original exact input, executable and environment bindings.

Only the numeric value token of a single bare `:time` key in a complete,
top-level, flat keyword/numeric statistics list may differ. Every other byte,
including whitespace, delimiters, keyword order, other statistics, verdicts,
models and errors, stays exact. Quoted strings/symbols, comments, nested models,
duplicate keys and unsupported numeric spellings cannot grant an exception.
Parsing has explicit byte/token/depth limits and streaming preparation never
claims an incomplete token or response. A varying-width time token uses the
actual observed prefix boundary when partitioning whole/prefix/suffix output.
No original input or raw response is rewritten. Comparison manifests retain
both hashes and every original numeric time token with its offset and length.

```sh
python3 native/sail-live-prefix/test-response-policy.py
python3 native/sail-live-prefix/test-time-driver.py --out NEW_TIME_CONTROLS
python3 native/sail-live-prefix/run-real.py \
  --source IMMUTABLE_DATASET_DIRECTORY --out NEW_RESULTS --z3 PINNED_Z3 \
  --experiment-numeric-time-only
```

An experimental fresh gate still needs all three complete fresh runs, actual
native image binding, stable executable identity, equal stderr/status, exit 0
and no error. Live-process or unclassified `ESRCH` and cleanup failures refuse. Only an eligible
fresh dataset with a matching historical prefix may attempt one live sample;
any skipped/refused project remains an explicit overall failure. Preparation,
foreground latency, total/background CPU, RSS/PSS and all raw failures remain
visible. This selected-pair experiment cannot establish a real application hit
rate, default-mode equivalence, shipped acceptance or a performance win by
itself. The first native pair remains failed: IronKV has one successful guarded
continuation, while Anvil records a fresh-run RSS-observer `ESRCH` and attempts
no continuation. See the [actual scope and costs](../../docs/validation/solver-snapshots.md#named-numeric-time-only-research-experiment).

The readiness regression drives an actual child through exact-prefix ready,
extra incomplete output and a claim attempt; the claim must refuse before
that extra response completes. The same child then completes the unexpected
response and is refused. Both exact and experimental policies keep this
boundary. `test-readiness.py` runs these real pipe/thread controls with native
image/RSS checks explicitly doubled.

`run-real.py --project ironkv|anvil` explicitly selects one dataset for a
durable per-project checkpoint. Omission runs both. The aggregate native
record must identify both datasets, every fresh/live refusal and any deferred
scope; selection cannot silently turn a partial run into a completed pair.

A child manifest never overrides its actual process outcome: fresh eligibility
also requires recorded exit0 and no process error. The parent refuses a true
manifest followed by actual exit7, or exit0 with a retained timeout observation,
and preserves that failure before skipping live continuation.


## Fresh-process RSS lifetime boundary

The original native Anvil refusal remains failed. A later native preflight at
`8b7c9922` passes its terminal-teardown case and seven negatives, but cannot
observe the required pre-waitable exit window in eight bounded attempts. The
mandatory gate fails and neither dataset runs. All actual errors/streams and
that failed gate remain preserved in the
[validation record](../../docs/validation/solver-snapshots.md).

The current source narrows admission further: a failed fresh RSS read can be
classified as terminal teardown only when the owned pidfd is **already
terminal before any wait**. A nonterminal pidfd always refuses, including
exit-in-progress flags and eventual exit 0. There is no `PF_EXITING` admission.
The original RSS error stays verbatim in `memory_observations`; it is not a
synthetic final RSS sample. At least one earlier valid RSS/PSS sample and the
original owned PID/starttime/held pidfd binding are required. Only
`ESRCH`/`ENOENT` can be considered. The subsequent bounded 100 ms status wait
must establish exit 0, then complete stdout/stderr draining, existing output
budgets, image guards and strict response comparison must still pass.
Missing proc state alone, foreign/closed descriptors, nonterminal errors,
other errno values, incomplete I/O and unsuccessful status refuse. The live
warming/ready/claim monitor is unchanged and remains strict.

The actual driver captures the pidfd once and retains it continuously until
after classification and complete I/O; cancellation signals it and never
closes/reassigns it. This lifetime plus live PID binding is essential. Inode
stamps are supplemental: no portable inode-uniqueness theorem for replaced,
reaped foreign descriptors follows from doubled fixtures or older kernels.

```sh
python3 native/sail-live-prefix/test-memory-identity.py
python3 native/sail-live-prefix/test-memory-lifecycle.py --out NEW_LOCAL_CONTROLS
# Mandatory genuine Linux controls before another trace evaluation:
python3 native/sail-live-prefix/test-memory-lifecycle-native.py --out NEW_NATIVE_CONTROLS
```

Local controls use actual owned children for waits/status and explicitly
double kernel observations. They do not qualify Linux. The native fixture has
a 90-second whole-control bound and bounded attempts: a child creates 8,192
4 KiB mappings, waits for a pipe, then calls `os._exit`. Its two positive cases
must establish genuine failed smaps reads with an already-terminal owned
pidfd: after explicit reaping, and while still unreaped before any explicit
status wait. Both require complete empty remaining streams and exit 0; no
injected positive is substituted. Kernel identity, every attempt and original
read error stay retained. A missing positive fails before either trace runs.
No pre-waitable exit window or kernel flag semantics are claimed.

Every native lifecycle-control attempt retains bounded raw stdout/stderr,
including consumed readiness bytes, startup failures, negative controls and
actual post-cleanup status. Each stream is capped at 64 KiB; overflow/read
failure/EOF timeout or cleanup error is explicit and fails that control.
Actual-owned-child startup, post-readiness and injected cleanup-report failure
fixtures exercise capture locally, with Linux image/platform/worker doubled.
The terminal-only source passes its nine genuine native controls, then fails
the real workload pair: IronKV falls back on its unchanged live RSS monitor,
and Anvil refuses a nonterminal fresh RSS observation. The earlier single
IronKV success remains separate evidence. See the validation record for all
raw hashes and preparation, foreground, total CPU and fallback costs. No
continuation implementation is selected; no more lifecycle retries are planned.


## Named kernel accounting owner

`--experiment-kernel-accounting-owner` is a separate research resource policy,
requiring `--experiment-numeric-time-only` and `--native-metrics`. Both drivers
accept it; default behavior and the old sampled-memory classifier remain as
recorded above. Its pool key additionally binds
`owned-wait4-lifetime-peak-v1`. The fixed native twelve-control gate and both retained traces pass this named
policy at `10b96ed1`. Default exact-byte gates still fail on numeric time
variation. No old failure is reclassified, and no application gain, default
integration or selection is accepted.

The deferred owner is assigned before executable capture or worker startup.
It retains the raw `Popen` and releases exactly one `wait4` reaper only after
acquiring the continuously held pidfd and actual executable descriptor, while
the child is still unreaped. The existing image check still runs at claim.
No other poll, wait or communicate path reaps the underlying `Popen`.
Cancellation uses the held pidfd; an unreleased child is killed and reaped by
its already assigned owner. Missing accounting, a competing waiter, startup,
I/O, descriptor or cleanup errors refuse and retain the available raw facts.
The owner never converts missing usage into zero or eventual success.

Admission requires complete original input delivery, bounded complete stdout
and stderr, actual exit zero, and a strictly positive kernel lifetime peak at
most the same configured byte limit (1GiB in the retained workload plan).
Linux `ru_maxrss` is converted from KiB; macOS local controls use bytes. This
post-completion peak gate is **not an OS hard live memory cap**. No `/proc`
RSS sample, terminal-race exception or `PF_EXITING` rule is used in this named
path. The direct pinned Z3/shared-library/process-tree assumptions remain;
this does not establish a portable arbitrary-child resource authority.

Complete per-child user and system CPU are retained for preparation plus
serving, discarded preparation, any fallback, and the independent fresh
reference. Preparation and serving CPU are combined because no exact
claim-time split is observed. An absent fallback is explicitly zero work;
missing accounting remains unknown and refuses. All preparation and fallback
costs remain part of the candidate total.

```sh
python3 -B native/sail-live-prefix/test-owned-wait4.py
python3 -B native/sail-live-prefix/test-kernel-owner.py
python3 -B native/sail-live-prefix/test-kernel-real-driver.py
# Native Linux only; all twelve controls must pass before either real dataset:
python3 -B native/sail-live-prefix/test-kernel-owner-native.py \
  --z3 PINNED_Z3 --out NEW_NATIVE_CONTROLS
python3 -B native/sail-live-prefix/run-real.py \
  --source IMMUTABLE_DATASET_DIRECTORY --out NEW_RESULTS --z3 PINNED_Z3 \
  --experiment-numeric-time-only --experiment-kernel-accounting-owner
```

The immutable dataset must also bind `owned_wait4.py` and
`kernel_accounting.py`. The same fixed IronKV and Anvil inputs, three fresh
controls, one eligible continuation, exact executable/input identity, strict
non-time response bytes and all original errors remain mandatory. Native
control or fresh-control failure blocks the real continuation. The local controls alone do not qualify a native result; the separate actual
native scope and costs are recorded below.


A failed sole `wait4` never skips cancellation because `poll()` also refuses.
Error cleanup signals only the continuously held original pidfd, then uses
bounded `waitid(P_PIDFD, WEXITED | WNOHANG)` after the original reaper stops.
This may retain actual termination status, but supplies no replacement CPU or
peak record: the sample stays refused. An already-reaped `ECHILD` also needs
an independently terminal held pidfd and cannot invent exit zero. No bare-PID
signal/wait or automatic `Popen` destructor waiter is used in this error path.
The mandatory native controls include both an actual descriptor-based reap
and an already-reaped error. Actual old/new caller controls retain the initial
refusal and the corrected cleanup scope separately.

The error-only terminal check retains its actual poll event mask. It requires
`POLLIN` for the same held descriptor, permits accompanying `POLLHUP`, and
refuses `POLLERR`, `POLLNVAL`, other events or ambiguous event sets. Native
closed-descriptor refusal is mandatory alongside the already-reaped positive.
The API boundary is the documented [Python waitid interface](https://docs.python.org/3.12/library/os.html#os.waitid)
and [Linux process-descriptor wait](https://man7.org/linux/man-pages/man2/waitpid.2.html).

The first kernel-owner native run passed ten of eleven mandatory controls and
failed the remaining fixture: an unknown SMT command returned `unsupported`
and exit zero, contrary to that fixture's nonzero assumption. Both real datasets
remained unrun. The [minimal solver reproduction](../../corpus/repros/z3-unsupported-command-exit/README.md)
records that distinction. The corrected controls retain it as a separate exact
exit-zero observation and use a tested parse error for actual nonzero status,
complete accounting, retained diagnostic and resource refusal. A successful
resource-completion gate alone is not verification. The corrected twelve-case native gate passes before both fixed real traces.
Each dataset passes three fresh controls and one eligible continuation under
the named response/resource policies, with the same prepared/serving PID and
no fallback. IronKV preparation is 2.242s and foreground 0.294s versus guarded
fresh 2.419s; total solver CPU is 2.282s versus 2.394s. Anvil preparation is
53.867s and foreground 6.212s versus fresh 59.610s; total CPU is 59.848s
versus 59.577s, so it saves no CPU. Preparation is real work, and the driver
waits for readiness: these selected traces do not measure an actual editing
schedule, predictor hit rate or application speedup. Complete preparation plus
foreground wall is not improved. The enclosing native container is capped at
28GiB; the 1GiB per-child kernel peak is a final admission check. All earlier
failed attempts stay failed. See the [native evidence](../../docs/validation/solver-snapshots.md#named-kernel-owner-resource-experiment).

### Source-only edit workflow components

The separate streaming prototype fixes donor session ordinal0, future session
ordinal0 and the first complete donor check batch before examining an edit's
solver input. Its [contract](edit-workflow-contract.json) freezes readiness at
edit invocation start: later readiness cannot turn a miss into a hit. Literal
command framing handles the pinned Verus writer's complete DONE batches,
including early version replies that the compiler needs before producing its
first check input. It never rewrites queries or responses.

`StreamingClaim` reuses the named owner's actual prepared replies and child,
with complete final accounting and the same positive kernel peak bound. A
divergence before any reply requests a fresh attempt. After a reply, divergence
or infrastructure failure aborts the whole compiler attempt; there is no child
swap or replacement-stream splice. A whole fresh retry must retain the failed
attempt and both costs. A duplicate claimant cannot cancel the first claimant.

```sh
python3 -B native/sail-live-prefix/test-edit-stream.py
python3 -B native/sail-live-prefix/test-edit-claim.py
python3 -B native/sail-live-prefix/test-edit-wire.py
python3 -B native/sail-live-prefix/test-edit-workflow.py
```

These local controls exercise literal all-split framing, a real owned caller
waiting for its version reply, and interactive owned children with actual
`wait4` accounting. Linux image/pidfd APIs are explicitly doubled on the Mac.
The bounded single-connection IPC component retains raw SMT reads before
parsing, including a partial frame. Its actual owned caller/grandchild control
demonstrates compiler exit0 after ignoring interposer exit1/backend exit7;
the separate outer attempt gate correctly fails. A separately executed whole
fresh retry has its own completed child/I/O/accounting record. A compiler's
exit0 alone never qualifies a result. The direct fresh streaming path also
uses the same owner and complete final resource admission; it waits for fresh
startup only, without waiting for speculative readiness.

The disabled `edit-workflow.py` entry point now owns a private Unix listener,
an executable `VERUS_Z3_PATH` interposer, a donor compiler invocation, background
preparation, a fixed-delay edited invocation and at most one whole fresh retry.
Session ordinals are assigned at socket accept before inspecting solver input.
Actual caller argv, cwd and the full environment bind each child; a context
miss runs fresh. The wrapper is recorded separately from the genuine solver.
The private start/finish records, Linux peer credentials and broker completion
must agree, independently of compiler exit. A late child failure still refuses
an attempt when its caller returns zero. Nonselected sessions run fresh and
every created solver, including cancelled preparation, retains actual kernel
accounting and raw streams. Each stream/input is bounded at 16MiB, at most 32
registered sessions and sixteen active sessions are supported, and the workflow
raw budget is 1GiB. Refusals remain failures.

Readiness uses nonblocking pool/entry locks at the single edit-start snapshot.
A preparation still starting or holding either lock cannot become a later hit.
Original and edited source bytes, raw compiler streams, literal session streams,
actual child status/CPU/peak, failed attempts and whole retries remain separate.
Compiler `wait4` can include waited descendants: it must not be added to the
separately reported solver/preparation CPU as though disjoint.
An independent fresh edited control always runs after the edited attempt (and
any required whole fresh retry). Compiler stdout/stderr stay literal; cancelled
background preparation is completed before the independent fresh control.

Seven local workflow controls use real processes, wait4, sockets and the
executable wrapper. They cover ready reuse, unready preparation, context miss,
post-reply divergence, ignored late exit7, missing wrapper startup and unsupported
wrapper arguments. Linux image/pidfd/peer APIs are explicitly doubled on macOS.
They provide no actual Verus, Z3, native or performance qualification.

The start/finish/connection join is **not an exhaustive spawn census**. An
unrecorded wrapper failure before its first record remains a pending boundary;
an actual Verus early-failure control and an independently observed process
trace are still needed. Therefore the executable always exits nonzero with
`application_acceptance=false`, even if its separately recorded registered
protocol `driver_exit` is zero. No result is reported verified or selected.
These source-only components do not change ordinary Sail or its default oracle.

An explicit `--strace /usr/bin/strace` records the owned compiler tree using
`-ff -q -ttt -xx -s 65536 -e trace=%process`. The narrow parser joins each
source-bound wrapper exec and actual terminal status to its broker/start/finish
PID, refusing missing, duplicate, partial or failed executions and late nonzero
exits. Raw trace files and compiler stderr remain intact. Parser fixtures do not
qualify actual strace coverage; the application acceptance gate remains blocked.
The prospective native driver permits at most 270 seconds per compiler attempt,
inside the unchanged 300-second serving guard. No launched lease is extended.

The explicit edit-only `--experiment-rustup-loader-target` binds the actual
Cargo target directory when Verus inherits Rustup's `LD_LIBRARY_PATH`. It never
removes or rewrites the caller environment. Other loader variables still refuse.
Only ordinary Rustup toolchain library directories, that target's debug/deps
paths and the pinned runtime directory are admitted. Before a reply, actual
file-backed mappings of the held owned solver are read through `/proc`, joined
to regular held file descriptors and hashed. Later replies must retain those
mappings and bytes; final held-file hashes are checked before descriptor release.
The default prototype still refuses loader variables. This named context relies
on the owned native image/toolchain remaining unchanged; it is not a general
immutable-memory or shared-library mutation theorem. All context/hash work is
part of the research cost, outside the solver child's own CPU accounting.

The first actual Tiny pilot on `e3a55f0f` correctly refused a repeated Cargo-fresh
no-op with no wrapper executions. A prospective root-package-only clean forced
real frontend work, but the next pilot refused inherited `LD_LIBRARY_PATH` before
any reply. Both failed attempts leave application and project-edit acceptance
blocked; neither ran the early/late fault controls or IronKV/Anvil edits. Those
pilots also selected the engine frontend rather than the intended normal
accepted runtime, so they do not qualify the accepted fork's application path.
The next caller must select the normal frontend explicitly and include any
forced root-only clean in its measured work and literal streams.

The normal-runtime `bf92e5eb` pilot subsequently passed Tiny and its actual
early/late fault controls. IronKV warmed up normally, then its donor refused
the application's former depth-128 parser cap before any edited attempt;
Anvil did not run. Retained exact complete traces reach depths 154 and 90 for
IronKV and Anvil respectively. Application framing now has an explicit
1,024-level counter cap, still with 16 MiB input/reply and 100,000-command
budgets. The parser is iterative; it never recursively traverses the query.
The next level, incomplete inputs and all byte/resource/identity violations
still refuse. The failed pilot remains failed; project edit qualification
remains pending.
