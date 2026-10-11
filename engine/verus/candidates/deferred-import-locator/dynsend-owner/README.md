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
