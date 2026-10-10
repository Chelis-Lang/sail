# Freshness of retained workspace proof reports

A proof report and a claim that it describes current inputs are separate facts.
The daemon retains establishing reports even when current input coverage is
unknown. `Current` requires complete declarations and coherent content validation;
`Historical` records changed inputs. `Unknown` never authorizes proof reuse.
These labels describe input freshness, not a solver verdict. A Current report can
contain a failed proof. A previously verified report with Unknown or Historical
inputs remains accessible as evidence of its establishing run, but cannot accept
a repair to today's inputs. CLI and MCP clients receive the same classification.

On Linux, the daemon keeps at most 32 process-local context projections and runs
at most two background revalidations. A context binds its cache root, workspace
spelling, runtime identity, invocation configuration and explicit environment.
Restart begins with no trusted projections. Each inspection records its overlap
boundary at entry, before waiting for the first native namespace barrier.
Overlapping readers may share a coherent snapshot only when its conservative
validation checkpoint was recorded after that reader entered and the successful
validation is published before the read returns. Queueing at the first barrier
must not turn a genuinely overlapping read into a sequential one. Sequential
reads never reuse an older checkpoint by age or delayed publication. Both caller
namespace barriers and final runtime/lease checks remain required. The separate
100 ms validation-owner wait budget starts after the first namespace barrier;
recording the earlier overlap boundary does not extend that wait. Publication and read completion require
the same runtime-generation identity and native barriers. Hash work happens outside job-service
and verifier-worker locks. Established reads retain the existing five-second RPC
deadline; uncertain readers return with Unknown and an explanatory note rather
than waiting for a verification worker. `SAIL3114` identifies unavailable or
pending current-input validation. Background establishment may make a later read
Current without rerunning a successful proof, but clients must observe that
public Current result before admitting it. The original Unknown response remains
valid evidence of what was available at that time.

The same coherent guard brackets the managed verifier invocation: leases and
native coverage begin before its establishing content snapshot and remain held
through verification, final validation and receipt publication. A preexisting writable mapping,
conflicting write, replacement or lost coverage makes the result Unknown and
prevents current receipt publication. Fresh Rust verification still runs.

A projection combines two independent mechanisms:

- Kernel read leases on every existing declared regular source, configuration,
  build and runtime file. The leases are acquired before baseline hashes and must
  still be held at each read barrier. Those exact-byte input and runtime digests
  can then be reused within that same continuously protected owner generation.
  Digests obtained before lease acquisition are never reused. A conflicting write
  or truncate releases the projection immediately; a new owner recollects and
  rehashes the inputs before becoming eligible again.
- Direct native watches for files, directories, aliases, ancestors and absent
  configuration paths. They detect replacement, newly created inputs and transient
  configuration. Reads drain the kernel queue synchronously; no callback-delivery
  delay is interpreted as absence. Mount-table changes, lost watches, overflow,
  unsupported filesystems or a bounded barrier that cannot complete invalidate it.

Inotify alone is insufficient. It cannot see writes through existing shared memory
mappings. Directory watches also miss writes through an outside hardlink unless
its inode is watched directly. See the executable reproduction in
`corpus/repros/linux-inotify-mmap/`. Read leases are rejected while a conflicting
writable file description or shared mapping exists, including a mapping that
survives closing its originating descriptor. Ordinary read-only mode bits or a
read-only bind mount do not supply that guarantee.

The optional lease owner is an embedded Python 3 standard-library helper launched
with isolated module loading, structured arguments and private bounded socket I/O.
Sail resolves an absolute interpreter once per establishment and binds its
loaded standard-library source/bytecode files and mapped ELF libraries. The
cold manifest and content hashes precede launch; the helper checks its actual
loaded closure before and after lease acquisition, and parent hashes must still
match afterward. Direct watches and read leases retain this closure. Changed
PATH resolution, replacement, new bytecode, or a broken runtime lease invalidates
the projection. These exact dependency digests also join the managed context.
The native kernel and the installed interpreter/ELF-loader bootstrap are the
trusted OS boundary: self-reported module maps cannot attest a malicious bootstrap.
No Python package or loader supplied by an untrusted party becomes trusted by
printing a manifest. It uses no third-party modules, owns at
most 4,096 file descriptors, and exits immediately on SIGIO to release leases so
an editor can write. No unsafe Rust or FFI is introduced. Missing Python, denied
lease ownership, unsupported kernel/filesystem behavior, an existing writable
mapping, helper loss or unavailable kernel evidence disables the fast path. Unprivileged
leases require matching file ownership; system-owned runtime files may prevent
establishment. MacOS retains conservative content revalidation.

Established read barriers inspect the helper's kernel-owned lease state rather
than asking a scheduled Python process for another acknowledgement. Sail binds
the initial descriptors, exact device/inode multiset and helper PID/start time,
then requires two complete ACTIVE READ inventories with live, non-stopped owner
checks. A global inventory affected by unrelated lease enumeration can fall back
to every initially bound descriptor's kernel `fdinfo`; known breaking or malformed
owner state cannot. Both sweeps share a 2 MiB byte cap and the 4,096-descriptor
limit. No failed owner is retried or revived.

The helper never acquires or renews a lease after readiness. A final intact
inventory therefore proves continuous protection of digests collected after
acquisition. Established readers clone that captured snapshot, record its
checkpoint before validation, and perform the complete final lease and runtime
identity checks. Independent native namespace checks bracket the read. The
combined check establishes a valid snapshot within the call interval, rather
than promising an atomic filesystem state at response delivery. Scheduling time
alone supplies neither validity nor invalidity; kernel reads do not promise an
interruptible wall-clock bound. The public five-second deadline and measured
200 ms p95 goal still apply. See [ADR 0029](decisions/0029-kernel-lease-work-bounds-and-freshness.md)
for the monotonicity argument and kernel sources.

The fast path accepts only supported local filesystems. A host-shared mount such
as virtiofs, or a network filesystem whose event/lease coverage is uncertain, does
not become trusted because a test happens to pass. Container validation copies
verification sources, binaries and runtime files onto Linux-local filesystems.
Container timings under emulation are labeled as such; they are not native Linux
reference speedups.

Completed status reads use a lightweight projection sealed to the SHA-256 of the
exact stdout bytes validated by the worker. They hash those bounded bytes on each
read and perform the same freshness classification as full result reads, without
allocating the function graph. Full result reads check the same seal. Older
records without a projection retain full parsing and validation; restored file
size or timestamps never substitute for checking the output bytes.

On Linux, submission coalescing only uses established bounded inspections. Cold,
lost or incomplete coverage queues a distinct job promptly while background
validation proceeds. It never hashes the full runtime before returning a durable
job id. MacOS retains conservative full revalidation as a secondary platform;
Linux latency bounds do not describe that fallback.

## Local product-client read census

The independent Python census remains `scripts/m7-job-read-bench.py`. An additive
Rust probe uses the installed product protocol client, including its complete
response decoding and unchanged five-second I/O deadlines:

```sh
cargo build --release -p sail-cli --example job_read_bench
target/release/examples/job_read_bench \
  --socket CACHE_SOCKET --job COMPLETED_JOB --active-job RUNNING_JOB \
  --active-worker OWNED_REPAIR_WORKER_PID \
  --active-crate human_eval_verus \
  --environment 'Linux container; record architecture, emulation and load' \
  --out reader-census.json
```

It is Linux and local-only: four readers start together and make 200 status/result
reads. Every sample must preserve Current evidence and both input revisions, with
no infrastructure error; complete typed project reports and repair envelopes are
validated without dropping functions. The selected worker must belong to the
running project's directory. The same non-stopped, non-zombie Rust verifier PID,
kernel start time and ancestry must be present before and after every timed
read, with its actual `--crate-name` matching the requested proof crate.
Dependency compilation, vstd, a stopped process or a different crate do not
qualify as an independently active selected-target verification.
Both p95 latencies must stay below 200 ms. Unknown evidence, lost verifier activity,
RPC errors and slow reads fail; failed samples remain in the result artifact.
A successful Rust probe does not erase or waive a failed Python census.

Reproduce the complete preparation and both censuses with
`scripts/m7-reader-acceptance.py`, following
[the local acceptance guide](bench/m7-repair-acceptance.md#reproducible-warm-reader-preparation).
Its ordinary full verification and exact repair-function context are separate
setup steps: different contexts have different Cargo target directories, so
warming a stock target does not by itself prepare the repair target. Setup costs
and every initial Unknown result are retained. Preparation is not a speedup or
reader-latency sample, and no copied receipt supplies the baseline proof.

The local a6/b346 reader run is a latency and lifecycle diagnostic of that exact
older product/runtime pair, not final M7 acceptance or source-candidate selection.
Earlier failures and interruptions remain in `docs/validation/`, including
[the cold setup failure](validation/m7-cb07-cold-reader-failure.json),
[the prior reader failure](validation/m7-f111-reader-failure.json) and
[the interrupted diagnostic](validation/m7-a6-reader-diagnostic-interruption.json).
Cheap Running-status polls, an accepted baseline proof, kernel unit tests and a
static source review each establish their own narrower facts; none substitutes
for both complete four-reader censuses under an actual selected-target verifier.

For local diagnosis only, `SAIL_FRESHNESS_TRACE=1` records static freshness
stage/reason labels and elapsed microseconds on daemon stderr. It emits no input
paths, environment values or report contents. Tracing can change scheduling and
does not qualify a performance gate. See
[the diagnostic scope and classified retained-proof profile](validation/m7-reader-freshness-diagnosis.md).
That separate retained-job run refused 13 of 200 reads through bounded owner
contention; it does not establish the cause of the earlier live census's
sustained admission-loss tail. Both failures remain recorded.
