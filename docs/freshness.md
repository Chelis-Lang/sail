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
recording the earlier overlap boundary does not extend that wait. Waiting readers
register on a per-owner publication condition variable while holding the same
mutex used to publish success or invalidation. A notification only wakes a
reader; it never authorizes evidence. Every wake rechecks the reader's entry-time
checkpoint and owner state. Spurious wakes share the original absolute wait
deadline, and a Busy reader does not invalidate an independent owner.
Publication and read completion require
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

The invocation guard covers the packages Cargo can compile for the selection:
the dependency closure (every resolved dependency kind) of the selected targets,
plus the workspace manifest, lockfile and every member manifest. A workspace
can resolve far more than that. Vest's members resolve registry crates such as
`wasm-bindgen`, which are never built for `-p vest_lib`. Watching all of them
exceeded the guard's 8,192-watch bound, so every run was Unknown. Build-script
coverage likewise does not wait for scripts outside an explicit `-p` selection's
closure, because they never run for it. After every invocation, each
`compiler-artifact` and `build-script-executed` record Cargo emitted must name a
package inside both scopes. Otherwise the guard fails with
`native-observer-scope-exceeded` and the result is Unknown. Without an exact
selection or a complete, unambiguous resolve graph, every resolved package stays
in scope, as before. The whole-workspace input revision and stamps are unchanged,
so an edit to any package still makes evidence Historical.

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

Linux namespace classification uses the existing ordered tracked-path set to
check component ancestors and the first possible descendant. Unrelated events
no longer scan every tracked input. Empty paths, non-UTF-8 components, missing
ancestors and explicit inputs beneath excluded output directories retain their
original classification. The native queue still drains to completion with the
same event/watch limits and 20 ms pre-batch budget; overflow, lost coverage and
actual changes still invalidate evidence. The [local filter validation
record](validation/m7.md) retains the initial empty-path
counterexample and exact native controls. Its isolated queue timings do not
qualify the complete service reader latency gate.

Watch acquisition also indexes canonical membership while retaining the original
watch-descriptor map as authority. A native descriptor reused for another
hardlink spelling replaces the old index membership; aliases are not accumulated
as permanently watched paths. Canonicalization, local-filesystem validation,
native masks and the 8,192-watch bound retain their original order, except that
a canonical path already in the index skips reparsing the owner's immutable
mount snapshot: it passed that same check when it was admitted. The additional
index holds at most one path/count entry per current descriptor value. See
[the acquisition controls](validation/m7.md) for the exact
native alias regression and local setup measurements; faster lookup alone does
not establish lease coverage or meet the complete product latency gates.

Inotify alone is insufficient. It cannot see writes through existing shared memory
mappings. Directory watches also miss writes through an outside hardlink unless
its inode is watched directly. See the executable reproduction in
`corpus/repros/linux-inotify-mmap/`. Read leases are rejected while a conflicting
writable file description or shared mapping exists, including a mapping that
survives closing its originating descriptor. Ordinary read-only mode bits or a
read-only bind mount do not supply that guarantee.

Linux guards also retain a proc mountinfo descriptor and check its kernel event
generation with a zero-wait poll. A bind mount and restoration invalidates the
owner even when final mountinfo, path bindings and contents match the originals.
The notification is latched permanently; a later clean poll cannot renew it.
Namespace/root identity changes and unavailable polling also refuse reuse. This
adds one descriptor and a bounded snapshot to each namespace/lease owner, with
no observation worker or grace period. See [ADR 0032](decisions/0032-kernel-mount-generation-coverage.md)
and [the isolated real fault control](validation/m7.md).

The optional lease owner is an embedded Python 3 standard-library helper launched
with isolated module loading, structured arguments and private bounded socket I/O.
Sail resolves an absolute interpreter once per establishment and binds its
loaded standard-library source/bytecode files and mapped ELF libraries. Within
one managed run, the manifest probed for the context identity also serves that
run's sysroot and coherent owners when they resolve the same interpreter; it is
never kept across runs, and daemon freshness owners probe their own. The
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
establishment. A root container may lack effective `CAP_LEASE`; ownership must
match the service's filesystem UID across the complete runtime/source/helper
inventory, not just the two executables. Use new byte-verified service-owned
copies or a dedicated container with that specific capability, preserving old
seals. See [native product setup](native-product-setup.md). Installation metadata
and ownership audits never certify Current. MacOS retains conservative content
revalidation.

Established read barriers inspect the helper's kernel-owned lease state rather
than asking a scheduled Python process for another acknowledgement. Sail binds
the initial descriptors, exact device/inode multiset and helper PID/start time,
then requires two complete ACTIVE READ inventories with live, non-stopped owner
checks. A global inventory affected by unrelated lease enumeration can fall back
to every initially bound descriptor's kernel `fdinfo`; known breaking or malformed
owner state cannot. Both sweeps share a 2 MiB byte cap and the 4,096-descriptor
limit. No failed owner is retried or revived. ASCII kernel rows use a faster
whitespace tokenizer; Unicode and vertical-tab rows retain the original parser.
The exact row classifier, malformed-owner rejection, foreign-owner behavior and
both complete sweeps are unchanged. Local classifier measurements and retained
failed reader controls are recorded in
[the parser validation record](validation/m7.md); parsing
speed alone does not establish a Current result or meet the reader latency gate.

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

## Reader latency

The M7 reader gate (four clients, 200 Status and Result reads, p95 below
200 ms, every sample Current while another verification runs) is measured
with `scripts/reader-gate.sh`; see
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).

For local diagnosis only, `SAIL_FRESHNESS_TRACE=1` records static freshness
stage/reason labels and elapsed microseconds on daemon stderr. It emits no input
paths, environment values or report contents. Tracing can change scheduling and
does not qualify a performance gate. See
[the diagnostic scope and classified retained-proof profile](validation/m7.md).
That separate retained-job run refused 13 of 200 reads through bounded owner
contention; it does not establish the cause of the earlier live census's
sustained admission-loss tail. Both failures remain recorded.

## Detailed proof diagnosis scope

The [single detailed sysroot proof diagnosis](validation/m7.md)
reports a new complete Current full proof and does not reproduce the older D20
sysroot-coverage failure. Its new executable/cache identity changes the context;
no historical receipt supplies authority. The provisional focus remains
provisional. One Current diagnostic proof establishes neither sustained live
reader availability nor the original four-reader latency requirement.

## Guarded runtime identity parsing

Linux retains one eligibility boolean on the existing runtime
validation owner. It can skip the hot identity-sidecar parse only when the same
original lease binds the exact raw context `.link` and selected identity `.json`
paths. Canonical aliases or missing membership retain the original parse. The
boolean is set after the original final identity equality, complete runtime
checks and cold native barriers succeed. The initial discovery parse remains
unchanged and supplies no authority by itself.

No parsed identity tree, path inventory or additional shared owner is retained.
Every read still runs the complete original lease/helper/interpreter/kernel/mount
checks, outer namespace barriers and request-entry/generation/pointer controls.
A restored write, outside hardlink or mmap break, sidecar replacement, ancestor
redirect or helper loss must reject that owner. Index eviction alone does not
invalidate a still-held original native owner; its unchanged fresh barriers must
continue to pass. The comparison switch is test-only; ordinary unit fixtures use
the same enabled path as production. Component controls and fixed-order timing
comparisons are separate from the original complete four-reader census and cannot establish its 200 ms latency requirement.

The [sidecar component record](validation/m7.md)
retains the passing mutation controls, initial Clippy failure and small reduction in complete inspection time. The initial comparison was test-only; the enabled
path uses the same eligibility and complete barriers.

The [enabled-path checks](validation/m7.md) retain
the same mutation controls and strict validation after production selection.

The Linux implementation preserves exact unchanged private input-declaration and
runtime-link bytes rather than atomically replacing their inodes. It can retain
the existing continuous native owner for the same context; every input, namespace,
lease and helper barrier remains mandatory, and dirty or absent coverage still
recollects synchronously. The [complete component replay](validation/m7.md)
passes metadata, freshness, strict and sensitivity controls. Real source edits
still invalidate the owner. The [frozen combined gate, ordinary product and
fresh original repair fixture](validation/m7.md) pass with
unchanged checks. The small-fixture pass does not isolate the implementation's
benefit or establish real-project repair performance. The same frozen product's
[original full reader workflow](validation/m7.md) passes
both Python and Rust collectors with complete reports during actual verification.

Real source or declaration changes still invalidate the combined source/runtime
projection, even when compiler bytes remain unchanged. Independent runtime
retention across edits is a [file-only design review](validation/m7.md),
not an enabled feature. Its possible benefit depends on the exact context and
has not been isolated from complete freshness construction. Every Current result
still requires the original complete source, runtime and namespace barriers.

## First root-binding mismatch diagnosis

An isolated Linux diagnostic candidate records one fixed first-mismatch row when
the existing precise observer refuses a root binding. The completed job's
existing `SAIL3114` note can include `root-ordinal`, `path-sha256`, and old/new
`present(device,inode)`, `absent` or `error(errno=...)` states. The ordinal starts
at zero in the exact supplied root order, and SHA-256 binds the path's native
bytes without canonicalizing a different path spelling. An unavailable errno
is reported explicitly. The digest is an identifier, not a secrecy guarantee or
freshness authority.

The candidate reuses the same identity result that causes refusal. It records
no path string, environment value or error message, makes no additional stat
call, and performs no trace-file I/O at the barrier. Mount-first checking,
first-mismatch short-circuit, event draining and sticky Unknown refusal remain
unchanged. Formatting happens when the completed report is assembled. Other
failure reasons keep their existing note shape.

To identify a failing input, privately seal the actual ordered root inventory
at observer acquisition and match its exact path-byte digest and ordinal. A
terminal source list alone does not certify that historical inventory or its
captured old identities. Keep private paths out of public reports; do not treat
a matching digest as permission to ignore or rebind an input. Generated Rust
files under Cargo targets remain semantic inputs when declared by the compiler.

The [generated-source audit](validation/m7.md)
and [diagnostic source record](validation/m7.md)
retain the source chain, original refusal controls and fixed-storage accounting.
The candidate is unqualified until Linux controls and strict checks run. It
does not yet identify the changed root in the failed product-730 benchmark or
provide a performance result.
