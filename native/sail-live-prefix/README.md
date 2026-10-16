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
the frozen `run.py`/`live_prefix.py`, and each project's `prefix.smt2`,
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
no continuation. See the [actual scope and costs](../../docs/validation/solver-snapshots.md#numeric-time-only-experiment-partial-native-evidence-pair-blocked).

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


## Prospective fresh-process RSS lifetime correction

The native Anvil refusal above remains failed. A source-only correction now
retains a separate `memory_observations` record for every failed fresh RSS read.
It distinguishes only a proved owned-child terminal teardown from an observer
failure; it does not invent a final RSS sample or suppress the original error.
The same held pidfd inode and original PID/starttime must remain bound, with
at least one earlier successful RSS/PSS sample. Only `ESRCH`/`ENOENT` can be
considered, and either the owned pidfd is already terminal or the actual task
has `PF_EXITING`. A bounded 100 ms wait must then establish exit 0 and a terminal
pidfd. Complete stdout/stderr draining, existing output budgets, image guards
and the strict response comparison still apply afterward. Missing proc state,
eventual exit alone, foreign/closed descriptors, live-process errors, other
errno values and incomplete I/O refuse. The warming/ready/claim monitor is
unchanged and continues to refuse its observation failures.

```sh
python3 native/sail-live-prefix/test-memory-identity.py
python3 native/sail-live-prefix/test-memory-lifecycle.py --out NEW_LOCAL_CONTROLS
# Mandatory genuine Linux controls before another trace evaluation:
python3 native/sail-live-prefix/test-memory-lifecycle-native.py --out NEW_NATIVE_CONTROLS
```

The local controls use actual owned children for waits/status and explicitly
double Linux kernel observations. They do not qualify the native boundary.
The native fixture has a 90-second whole-control bound and bounded attempts:
a child creates 8,192 4 KiB mappings, waits for a pipe, then calls `os._exit`.
It must establish genuine mm teardown both after terminal status and before
waitability, with complete empty remaining streams and exit 0; an unobserved
window is an explicit failed control. Kernel identity, actual task flags,
all attempts and the original read errors are retained. Linux v6.8 source
explains the ordering; it is not asserted to be the actual host kernel. No
corrected trace result or acceptance follows before native qualification.

Every native lifecycle-control attempt retains bounded raw stdout/stderr,
including consumed readiness bytes, startup failures, negative controls and
actual post-cleanup status. Each stream is capped at 64 KiB; overflow/read
failure/EOF timeout is explicit and fails that control. Actual owned-child
startup/post-readiness diagnostic fixtures exercise this capture locally,
with the Linux image guard and worker explicitly doubled.
