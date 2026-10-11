# Pinned compiler diagnostic owner correction

Status: unselected. The actual pinned ownership API control and full source
library/strict checks pass. A fresh ordinary runtime build and the original
fourteen-stage qualification have not run on this correction.
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
from this build; original fourteen-stage qualification has not launched.

[Ordinary build closure](ordinary-build-pass.json) now records successful
completion in 2,589.219 seconds (the official builder reports 2,520 seconds),
with a fresh vstd proof of 2,060 verified and zero errors. The new manifest, all
thirteen direct runtime artifacts, pinned lock and all seven source hashes
were checked after completion. Owned build actors are closed, and both
preceding complete runtime seals remain unchanged. This is an unselected
build/source-gate result, not original-oracle or product qualification.

[Prepared qualification handoff](qualification-handoff.json) binds the fourteen
original Python source files and the [future controller](run-qualification.py)
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
No qualification process has launched. Root must first review the fresh new
manifest and this controller binding.

[Source-control compiler retirement](source-control-compiler-retirement.json)
removes only the closed source-control target's regenerable compiler
subdirectories after the ordinary builder finishes copying its verified seed.
It retains both actual test executables, source snapshots, all raw check logs
and the seed inventory. Holder checks are empty, and both preceding complete
thirteen-artifact runtime seals stay byte-exact. File-allocated retirement and
native/host free-space observations are separate; shared backing-space changes
are not attributed to this cleanup. The active ordinary and common targets
remain protected.
