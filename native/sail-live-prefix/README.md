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
fresh reference includes image-validation overhead and is labelled guarded;
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
