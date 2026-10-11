# Current product repair benchmark launcher

The [host launcher](m7-current-benchmark-controller.py) binds one fresh invocation
of the [unchanged original wrapper](../../engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/ordinary/repair-benchmark/run-original.py)
to the combined product's build and owned-installation record. Preparation of
this launcher is not benchmark execution or performance evidence.

The original wrapper remains byte-identical (`477508a1…`). It owns the existing
separately recorded ordinary/focus/repair preparation, three measured rounds,
600-second invocation limits, exact verdict/diagnostic oracles, per-project
median speedup of at least 2×, spreads, and prepared-state/driver telemetry.
Neither the launcher nor its controls changes those assertions. Reader
acceptance is a separate scope. No proof receipt or compiled project target is
copied into the fresh work directory.

The launcher checks product source/archive identity, binary bytes and private
installation metadata, the original M6 binaries, the qualified c77 runtime's
13 artifacts, eight source files and source lock, and the human-eval-verus and
Vest corpus commits. It rechecks these inputs after closure. The product record
is installation evidence; all benchmark admissions still come from the
original runner's actual full proofs and oracles.

The native controller records its Linux PID, process group and birth before
executing the original wrapper. Interrupt handling signals only that same
birth-bound controller, allowing the original wrapper's existing cleanup.
There is no added whole-benchmark deadline or extended invocation deadline.
Afterward a bounded, read-only process metadata census records live candidates,
unreadable processes and truncation. It does not signal discovered processes or
claim an unlimited orphan census. Raw stdout, stderr, native records, inclusive
duration and failures are retained even when a post-run seal fails.
The host archive includes the original per-invocation streams and result records;
it excludes scratch clones and compiler/proof caches to avoid duplicating build
storage. Those original native directories remain untouched, and the exclusions
are explicit in the record.

The read-only preparation check found that the handoff container lacks the
original M6 installation, while the acceptance container retains its exact
expected M6 bytes and the expected c77 manifest. This is a deployment constraint
to resolve before execution, not a reason to alter the original wrapper.
The launcher selects the execution container named in the coherent product
record and refuses missing or changed prerequisites.

The [acceptance installer](m7-current-benchmark-install.py) resolves that
constraint by transferring only sealed product bytes and the exact tracked Git
source archive into new acceptance paths. It verifies the complete extracted
source inventory before and after using the archive-bound native installer.
The native installer creates fresh service-owned binaries with mode `0700`;
existing destinations are refused. It does not transfer a target, cache, receipt
or proof, and runs neither Cargo nor verification.

The handoff producer record remains unchanged. A separate acceptance execution
record binds its SHA, source/archive identity and exact binary bytes, records the
actual destination installation and container birth, and retains the producer's
actual build phase. The launcher prefers that bound execution record when
present and refuses a changed producer, wrong bytes or changed container birth.
The [installation controls](test-m7-current-benchmark-install.py) cover unsafe
archive paths/links, complete source mutation/extra-file refusal, truthful
immutable producer derivation, and changed installation/producer identities.

The [pure controls](test-m7-current-benchmark-controller.py) cover duplicate or
oversized metadata, revision/build/install mismatches, exact wrapper arguments,
fresh work, ordinary priority, changed-wrapper refusal and source-preserving
definition loading. They launch no Cargo,
daemon, solver or proof actor. Actual whole-loop performance remains pending.

The a9609e9 acceptance byte installation passes: its recorded transfer/install
transaction takes 18.351 seconds, excluding CLI startup and the initial producer
record read. The Docker native-installer command takes 1.287 seconds. All 1,434 tracked source
entries match their exact archive bytes, symlink targets and modes before and
after installation. A complete post-precheck source seal also passes. The
prepared-only original benchmark precheck takes 4.199 seconds through its primary
result, excluding final archival work, with matching
M6, runtime, source, corpus, product and container-birth seals. These are setup
results only. No benchmark preparation, Cargo or proof workload has run.
The [portable record](m7-current-benchmark-launcher.json) retains the producer
and execution-record identities separately; the producer record is unchanged.
See [ADR 0036](../decisions/0036-managed-compiler-artifact-families.md) for the
implemented compiler-family observations and their separate product-validation
scope, and the [roadmap](../ROADMAP.md) for M7 acceptance.
