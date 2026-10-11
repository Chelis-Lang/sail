# Loaded-import candidate build and qualification handoff

This is a runnable sequence for a **reviewed, unselected** candidate. The corrected
full patch is `506679543779f2856899006d7d70420ce85d47cb74c9873a064f97ab82c44984`.
Its [new ordinary build and source-check record](loaded-import-test-layout-build.json)
retains fresh vstd 2,060/0, 38 VIR and 43 driver library controls, and both strict
all-target Clippy passes. The original fourteen-stage source qualification failed at revision 0 of the
previous-revision FOCUS stage; no source or product qualification is inferred.

The [earlier e6f build record](loaded-import-ordinary-build.json) retains its
mandatory driver lint failure separately. [The authority review](loaded-import-authority-review.md)
and [causal diagnostic](span-provenance-causal-diagnostic.json) retain prior
failures. The selected runtime remains unchanged.

## Freeze before compilation

Retain the complete patch and its SHA-256, the narrow delta and base patch hash,
all changed source-file hashes, pinned commit/toolchain and the unchanged source
Cargo lock. Preserve c05 and f833 source/runtime seals and raw failures. Freeze
the private Sail builder snapshot with only its embedded `interactive.patch`
changed to the reviewed complete patch. The selected patch remains untouched.

Use the already retained fourteen-script qualification archive, SHA-256
`74113be77d3e0832033d6a42243f4666df3e679dc495c88ac09804c18afbdbad`.
Its cross-edit script is
`80b49c69b84599b606f200157c67cdc1f92e73054905f3a22b11ce839789fc2a`.
The separately reviewed scheduling-only pipeline copy is
`f7ddcf603e2ff7c52a2a107ce97139ee2ccc43c75f558b61d14642762d92cc62`:
source tests run at ordinary priority, without changed assertions or limits.
Do not substitute an older worktree's pipeline: that version lacks the private
span migration and previous-revision FOCUS stages.

## Runnable sequence

Paths below are supplied owned paths. `BUILD` is the new isolated source variant,
`PATCH` its reviewed complete patch, `HE` the private pinned source checkout,
`OLD_ENGINE` the sealed 2421 executable, and `OUT` a new evidence directory.
`BOOTSTRAP_TARGET` is the existing private compiler target, never the common
product target. Verify every script and input hash before executing it.

1. Run the actual pinned-source VIR and RustVerify library tests and strict
   all-target Clippy against the exact combined source. Preserve complete
   stdout/stderr, process identity, priority and elapsed time on every outcome.
   Include the Loaded mapping's authority, state, capacity, drop and new-domain
   controls. Compilation uses two jobs; bounded runtime controls run at ordinary
   priority. No source test is inferred from helper-only results.
2. From the frozen private Sail snapshot, run the ordinary builder:

   ```sh
   CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR="$BOOTSTRAP_TARGET" \
     cargo xtask build-verus-engine --keep-intermediates
   ```

   A compiler-cache seed may contain only independently verified inactive
   intermediates copied without hardlinks. Never seed runtime files, receipts
   or seals. The changed VIR/driver crates must rebuild; the ordinary builder
   must freshly verify vstd and produce its own complete manifest.
3. Freeze the new manifest and all thirteen direct runtime artifacts. Require
   exact pinned commit `426d8b01e7ffb910a36c15e1f870ff985bb61eef`, Rust `1.98.1`,
   the reviewed patch feature, source-lock feature, exact solver bytes and no
   unmanifested direct runtime files. Record fresh vstd 2,060/0 from the actual
   build output. A build pass is not source or product qualification.
4. Invoke the frozen original pipeline:

   ```sh
   python3 qualification-scripts/engine/verus/candidates/qualify-same-input.py \
     --build "$BUILD" --stock "$STOCK" --z3 "$Z3" --he "$HE" \
     --out "$OUT" --execution linux-x86_64-container-emulated-sharedload \
     --patch "$PATCH" --private-span-boundary --old-engine "$OLD_ENGINE"
   ```

   Do not supply prior hit controls or proof outputs from another runtime.
   Require private graph/span audits, old-domain migration, real same-input
   positive and order/body/effect negatives, source fixtures, checkpoint,
   cancellation, key delta, genuine previous-revision FOCUS, full same-input HE,
   and actual VIR/AIR/driver tests. The task023 edit leaves task013 selected;
   exact stock stderr and every HIT oracle remain mandatory. Original compiler
   invocations retain 600-second limits. Failure stops the original pipeline.
5. Supplement the retained raw pipeline with the mapping-state and ownership
   checks below, then validate the same thirteen runtime hashes again. Preserve
   any failure independently. Even complete source gates leave product,
   HE/Vest performance and operational qualification pending.

## Selected-state observation and physical owner checks

The existing f833 trace is not a resolver-state event. The new candidate needs
a bounded observation derived from its admitted immutable mapping, identifying
the actual original words `[2208441662476345389, 316229852143522]`, initial and
pre-focus state, refusal reason when unavailable, and checked original/current
bounds. Current typed hash/stable identity/name/lengths come from this authority,
not an unrelated matching SourceMap file. Borrow fields under the existing
observer admission; do not load source, retain compiler context, expand a Value,
or allocate a second owner to emit evidence. Missing/truncated observations
remain missing evidence. Their final event schema must be frozen with the patch.

Retain existing producer streams and public terminal output in every original
stage. Supplementary parsing must not continue a failed original assertion or
substitute observation for a genuine restored FOCUS HIT. Selected-state evidence
may explain a refusal, but cannot relabel it as acceptance.

The mapping seed and validated snapshot must be one moved family, admitted
before allocation. The unchanged 8 MiB family includes both-phase overlap,
records, original authority, name growth, Arc/header/control metadata and any
memo. Physical destruction precedes release. Record storage arithmetic is
checked before reservation and actual capacity checked afterward. The family
must appear in retained-byte telemetry whenever physically owned, including
the constructor phase before Sources publication; no duplicate snapshot is
hidden from telemetry.

The existing conservative preparation phase maximum is 492 MiB. A wholly
contained 8 MiB family would bring that prospective maximum to 500 MiB, below
the unchanged 512 MiB total. This arithmetic is conditional on the reviewed
implementation containing **all** new ownership inside that family. New owners
outside it require another explicit bound; the remaining 12 MiB margin is not an
unaccounted allowance. Telemetry is evidence for its stated retained owners,
not a substitute for transient allocation/drop proof or an RSS claim. Preserve
all original decode, graph scratch, reporter, observer and focus limits.

Negative controls must reject changed original words/range, relevant source
content, typed hash kind, stable ID/name, normalized/unnormalized length,
namespace, ambiguous/missing owners, unsupported raw types and state transitions.
Unrelated coordinate relocation alone is only a candidate positive; its actual
previous-revision restoration and strict fresh-versus-cached/stock controls
still have to pass.

## Preserved e6f execution

The ordinary Linux build completed in 727.842 seconds with two build jobs and
ordinary process priority. All 38 VIR and 43 driver library tests passed with
independent cases scheduled serially. The initial strict-check attempt stopped
because the pinned Linux toolchain lacked Clippy; that setup failure is retained.
The exact Rust 1.98.1 component was installed, and only strict checks resumed.
VIR all-target strict Clippy passed. Driver all-target strict Clippy failed
`items_after_test_module`: the test-only `drop_probe` module precedes
`memo_test_authority`. The exact failure and candidate remain retained. The
correction must relocate the unchanged test module, receive a new patch identity
and ordinary build, and pass strict checks before qualification. The original
fourteen-stage pipeline has not started. No restored
HIT, full same-input result or product qualification is inferred from this build.

For space-aware checks after all ordinary-builder descendants have exited, the
owned `target-verus` can be reused. Cargo may add the exact empty `.cargo-lock`
and `.cargo-artifact-lock` files directly under its runtime directory. Retain
those files' hashes and metadata, prove no process has an open descriptor, then
move only those empty control files to evidence. Any other direct artifact or
nonempty control file stops qualification. Validate every original manifest
artifact and the exact thirteen-file inventory before and afterward; never
reseal or move runtime artifacts.

The post-check runtime audit retained two zero-byte Cargo control files, with
no open descriptor holders. Evidence and the runtime are on different native
mounts, so a rename returned `EXDEV`. The completed retention copied the exact
empty bytes, checked their hash and unchanged source device/inode/size/ctime,
then removed only those control files. All thirteen manifest artifact hashes
and the exact direct-file inventory matched afterward, without resealing.

## Test-layout correction

The corrected full patch is
`506679543779f2856899006d7d70420ce85d47cb74c9873a064f97ab82c44984`,
with the exact from-e6f delta
`47451f9c2f1e32e1042778a3ef77b2328aa1b7b8c2c72c93e4fd40134c43e89b`.
Only `sail_owned_imports.rs` changes: the identical test-only `drop_probe` module
moves below the unchanged helper. Its new source hash is
`10fbd5f5e9647e5ebb13a6297d198a45037731a7f65c7e1bb6f35c4af271b410`;
the other five source hashes remain identical. No authority, runtime logic,
budget, assertion or lint exception changes.

The corrected feature-bound private variant `b46efd051424968f` completed its
ordinary build in 1,055.064 seconds at ordinary priority with two jobs. Its verified inactive compiler seed copied 3,522 files
(1,090,456,706 bytes) in 15.008 seconds, excluding 103 changed-crate paths, every
runtime/proof/seal output and hardlinks. The e6f failed candidate remains frozen
and unselected. Fresh vstd 2,060/0, the exact thirteen-artifact inventory and
all six source hashes plus the lock were independently validated. New actual
38 VIR and 43 driver tests passed; strict all-target Clippy passed for both.
Only the reviewed empty Cargo control files were retained after checking no
open holders; runtime hashes stayed unchanged without resealing. The original
fourteen-stage pipeline then started with unchanged assertions and 600-second
compiler bounds. Its strict failure is retained below.

## Closed compiler-intermediate cleanup

After the corrected ordinary builder and its descendants closed, inactive c05,
f833 and e6f compiler intermediates were removed. The
[cleanup record](loaded-import-compiler-cleanup.json) retains their manifest
identities, per-file inventory hashes and before/after checks. No active bound
process or open descriptor held those directories. Every variant's original
thirteen runtime hashes and exact direct-file inventory matched before and
after cleanup; no runtime was resealed. Source, locks, prepared entries, probes
and all raw qualification failures remain retained. The new b46 source-control
target stays protected.

The removed files occupied 3,275,419,648 allocated bytes and 3,249,564,865 logical
bytes. Host APFS available space increased from 25,049,512 to 27,749,312 KiB
during the operation. That backing observation includes concurrent activity;
the container's much larger apparent free capacity is not host backing space.

## Corrected candidate qualification failure

The [b46 qualification record](loaded-import-qualification-failure.json) retains
the original pipeline's failure after 236.291 seconds. Eight stages passed,
including source fixtures, graph checkpoint and cancellation. Revision 0 of
the previous-revision FOCUS stage failed its exact stock stderr comparison:
both arms emitted the same three NOTE lines, but the two function-start lines
arrived in different order. The original assertion, flags, thread policy and
600-second limits remain unchanged. This is an observed difference, not an
attribution to an earlier failure or permission to normalize diagnostics.
Later revisions, full same-input HE and final pipeline source tests were not
run. No source or product qualification is claimed.

Independently, that revision recorded zero preparation HITs and four provenance
refusals. The original gate requires actual checked HITs too; NOTE ordering
does not remove this blocker. A bounded exact-target state diagnostic proved
the full module 9/0 but emitted zero matching records, which remains unavailable
evidence. An authorized separate unfiltered diagnostic then captured the first
refusal: imported `vstd/function.rs:151:20: 151:65 (#0)` was **Delayed at both
constructor capture and pre-focus validation**. Its original span was
411214–411259 inside original file range 406973–411571. The raw expected hash
is retained without inferring its original hash algorithm. No current Loaded
coordinates were supplied. The Loaded-only policy correctly refused this
binding before reaching the previously targeted div-mod span.

Both diagnostics used the unchanged admitted 4 MiB/4,096-record observer and
600-second compiler bound in fresh owned contexts; they do not continue or
replace the failed original pipeline. All thirteen runtime artifact hashes,
the exact direct-file inventory, all six source hashes and the source lock
matched after these runs. The candidate remains frozen and unselected. Any
future resolution policy requires separate review, a new patch identity and
the original qualification gates again.
