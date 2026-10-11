# Pinned compiler diagnostic owner correction

Status: unselected. The actual pinned ownership API control and full source
library/strict checks pass. The fresh ordinary runtime build passed, but the original fourteen-stage
qualification failed at its required positive-hit assertion in revision 0.
[Identity](identity.json) binds the [complete candidate](../../deferred-import-locator-dynsend-owner.patch)
and [minimal delta](from-rc.patch).

The [original Arc lint failure](../source-gate-a61-failure.json) and
[subsequent Rc compilation failure](../source-owner-correction/source-gate-4a3-failure.json)
remain separate frozen results. The first candidate passed 38 VIR and 55 driver
controls but failed strict driver Clippy. The second ordinary build passed
fresh vstd 2060/0 and 38 VIR controls, but the actual driver controls could not
compile: the diagnostic emitter requires rustc's `DynSend` marker, explicitly
unsupported for `Rc`. Strict checks and original qualification did not run.

The pinned compiler's `rustc_session/src/parse.rs` itself owns its source map
with `Arc` and exposes `clone_source_map`. Its `ParseSess::new` uses fixed loader
and hash inputs, so that factory cannot preserve this fixture's custom loader.
`rustc_errors/src/emitter.rs` defines `DynEmitter` as `dyn Emitter + DynSend`;
`rustc_data_structures/src/marker.rs` supports `Arc` using compiler
`DynSend`/`DynSync` markers while explicitly rejecting `Rc`. These markers are
different from standard Rust `Send`/`Sync`. There is no `Lrc` alias in this
pinned `sync.rs`, and `DiagCtxt` does not expose a borrowed shared-map factory.

The correction restores the original test's `Arc` field and constructor with a
single documented `clippy::arc_with_non_send_sync` allowance on that binding.
The owner remains on the existing rustc compiler thread. No unsafe marker,
constructor-hiding conversion, production behavior, Reporter mode, level,
order, label, expected output, or assertion changes. Removing only the comment
and binding allowance yields the byte-identical a61 full test module.

[API controls](api-controls.json) retain actual pinned compiler compile,
strict-Clippy, execution, and negative controls. The positive control preserves
a custom loader, captures an actual warning/primary span, and observes owner
drop. Its two negatives reproduce the unallowed Arc lint and Rc trait failure.
Two initial new-control API/working-directory mistakes remain hashed in that
record. Run the control with:

```sh
python3 engine/verus/candidates/deferred-import-locator/dynsend-owner/test-owner-api.py \
  --work-dir target/deferred-dynsend-api-controls
```

This small control does not execute the full source Reporter/ReplayMessages
fixture, prepared codec, VIR controls, preparation-budget proof, or any proof
qualification. Those checks remain mandatory against these exact source bytes.
The actual full Reporter fixture still compares four scalar replay methods,
levels, primary spans and labels, plus direct stock grouped calls on both arms;
it does not establish formatted stderr or prepared-group replay equivalence.
Original raw-diagnostic checks and 600-second compiler limits remain unchanged.
The proposed v6 authority, one 8 MiB mapping family and prospective 500 MiB
phase maximum are unchanged. The selected runtime remains unchanged.

[Source-control launch](source-control-launch.json) records the one authorized
private pinned-source overlay, exact lock/archive, compiler-only closed cache
seed, two build jobs and serial independent cases. It precedes any new ordinary
variant build. All source library controls and strict checks remain required;
the failed Rc variant's source and thirteen runtime bytes are protected.

[Actual source controls](source-controls-pass.json) passed 38 VIR and 55 driver
library cases, plus both all-target Clippy checks with warnings denied, on the
exact 886b overlay and pinned lock. All seven source hashes were checked again
afterward, and no owned compiler remains. The preceding failed 29829 variant's
complete thirteen-artifact inventory and hashes stayed unchanged. The run uses
closed compiler-only copies, excludes changed VIR/driver objects and every
runtime/proof/seal output, and retains the full seed inventory and raw logs.
The earlier Arc lint and Rc trait failures remain failed records. These source
controls do not supply a new runtime manifest, vstd receipt, original oracle
result, or permission to select the candidate.

[Closed compiler retirement](preceding-compiler-retirement.json) removes only
regenerable compiler subdirectories from the two closed failed variants after
exact runtime-seal and holder checks, retaining their actual test executables,
source snapshots and raw failures. Allocated files retired and native/host free
space observations are recorded separately; shared backing-space changes are
not attributed to cleanup. No selected/new/qualification runtime file moves.

[Ordinary build launch](ordinary-build-launch.json) records the subsequently
authorized 73bd feature-bound variant, frozen private builder archive and
normal-priority two-job actor. The compatible compiler-only seed comes from the
closed source-control target with changed VIR/driver crates excluded. No
runtime files, proof receipts or acceptance records are copied. Fresh ordinary
vstd verification and a complete thirteen-artifact manifest are required
from this build before original fourteen-stage qualification could launch.

[Ordinary build closure](ordinary-build-pass.json) now records successful
completion in 2,589.219 seconds (the official builder reports 2,520 seconds),
with a fresh vstd proof of 2,060 verified and zero errors. The new manifest, all
thirteen direct runtime artifacts, pinned lock and all seven source hashes
were checked after completion. Owned build actors are closed, and both
preceding complete runtime seals remain unchanged. This is an unselected
build/source-gate result, not original-oracle or product qualification.

[Prepared qualification handoff](qualification-handoff.json) binds the fourteen
original Python source files and the [frozen controller](run-qualification.py)
to this candidate. The frozen directory also contains four generated bytecode
files; they are not additional authoritative source scripts. The controller
asserts every source hash before launching. Its [review delta](qualification-controller-review.diff)
contains candidate paths, completed source-control and fresh manifest admission,
and no extra compiler environment settings; the original fixture files remain
byte-exact.
Their assertions,
raw stock diagnostic comparisons, prior-revision FOCUS requirement and original
600-second compiler bounds stay unchanged. The [trace audit](trace-supplement-review.md)
found that original fixtures strip inherited engine settings, so the previously
prepared trace environment was ineffective and was removed before any launch.
The source trace stays dormant; no actual selected-state evidence is claimed.
The reviewed controller subsequently launched once, as recorded below.

[Source-control compiler retirement](source-control-compiler-retirement.json)
removes only the closed source-control target's regenerable compiler
subdirectories after the ordinary builder finishes copying its verified seed.
It retains both actual test executables, source snapshots, all raw check logs
and the seed inventory. Holder checks are empty, and both preceding complete
thirteen-artifact runtime seals stay byte-exact. File-allocated retirement and
native/host free-space observations are separate; shared backing-space changes
are not attributed to this cleanup. The active ordinary and common targets
remain protected.

[Original qualification launch](qualification-launch.json) records the one
subsequently authorized fourteen-stage actor on the sealed new manifest. It
uses the reviewed exact controller at normal priority, serial independent
cases and unchanged internal concurrency. The original fourteen fixture
sources, 600-second compiler bounds, stock diagnostic and same-input checks,
and genuine previous-revision FOCUS requirement stay unchanged. No trace
supplement is forwarded. The run is terminal failed; qualification and runtime selection are refused.
Its primary result was retained before separate source/runtime and actor
closure checks.


[Original qualification failure and closure](qualification-failure.json) retains
the one original run: eight stages passed, then `previous-revision-focus` failed
in revision 0 at line 77 after 433.841 seconds overall. The assertion label
`unchecked actual hit` accompanies `hits == len(checks) and hits > 0`; here both
counts are zero. No unchecked nonzero hit occurred. Four preparation misses
record only `stage=provenance` (one focus, three full); the saved data does not
identify a particular binding or resolver state. Revisions 1 and 2, genuine
previous-revision focus reuse and the remaining full qualification did not run.

The stock and engine module proofs exited successfully with nine verified and
zero errors. Full module results and function details matched, and raw stderr
matched byte for byte (234 bytes). Those earlier checks passed; they do not
replace the missing positive-hit requirement. No deadline, assertion, source
fixture or diagnostic comparison changed, and no retry was launched.

The separate post-run check confirms all thirteen direct runtime bytes and
inventory, seven source identities and pinned lock match the pre-run seals.
No active pipeline-group or candidate/owned-path process was found; a second
scan includes separately-sessioned processes through exact inherited sidecar
and preparation selectors. The failed stage did not enable optional actor
observations, so no complete historical per-child PID/group census is claimed.
Raw failure streams, summaries and events are retained independently. The
candidate stays unselected and unqualified.

A [separate diagnostic controller](run-provenance-diagnostic.py) is prepared for
review, with [static input/flag binding](provenance-diagnostic-review.json).
It was reviewed before the one separate run below. It binds the already-failed
revision 0 source path and all 265
source-copy hashes, exact sealed runtime and original compiler flags/context.
Only fresh owned output paths and the existing unfiltered
`SAIL_ENGINE_IMPORT_TRACE=1` observation differ; the target filter is absent.
No preparations, oracle results or receipts are copied. The existing compiler
helper keeps its original 600-second limit and can record the exact owned
process group without supplying proof authority. Primary streams and outcome
precede independent post-run runtime/source and present actor closure.

The existing trace emits during focus only after `provenance_key` produces a
valid Imported key. It can distinguish a whole capture error, record lookup
failure or selected unavailable resolver state when that path is observed.
It does not observe malformed unit/unknown raw owners or Local provenance
refusal. A first ineligible trace record is not automatically the binding that
caused the first complete-store refusal; ordering and fields must establish
that correlation. Zero records or only eligible imports leave the cause
unavailable. The bounded observer records its own refusal marker on overflow;
no trace failure changes the full proof or preparation eligibility. The
original failed qualification stays failed regardless of this diagnostic.


[Separate diagnostic result](provenance-diagnostic-result.json) completed within
the same 600-second bound: 47.950 seconds for invocation/observation and 57.346
seconds for the controller including seal checks. It verified the same nine
functions; stderr remained byte-exact against the original 234-byte stream and
typed module/function results matched. Runtime/source/lock seals and the
265-file input inventory remain unchanged, with no active owned process.
The actual engine process identity and bounded process observations are hashed
in the result. No observer cap or unavailable marker occurred.

Six focus import-state records expose five eligible Loaded bindings from
`vstd/arithmetic/mul.rs`, followed by ineligible imported binding 5705 at
`vstd/function.rs:138:2: 138:2 (#0)`. That record has initial and pre-focus state
Delayed, original range 406973–411571 and reason Delayed. It immediately
precedes the focus fresh-store provenance refusal. The store loop holds
`PREPARATION_GATE`, and the immutable unavailable record makes provenance
return before batch/disk publication, which correlates this focus refusal with
the observed binding. It also excludes a whole-family capture error.

The reason still merges an unsupported absolute/UTF-8/bounded descriptor path
and a missing matching current Foreign source. The unavailable-state trace
does not emit the path or match count, so neither cause is established here.
The three full-phase refusals were not individually traced. This diagnostic
does not demonstrate a hit, previous-revision reuse, or original qualification
pass; it supplies narrower evidence for a separately reviewed correction.


[Original-descriptor role review](original-descriptor-role-review.md) proposes a
distinct authority for stock delayed recipes whose original metadata file has
not yet acquired current Foreign metadata. The existing Foreign-associated
policy and negatives stay intact; actual path association, namespace absence,
current binding and no-loading controls remain required. This is an unexecuted
design under a proposed ADR amendment, not the diagnosed narrower branch for
binding 5705 or an implemented correction.
