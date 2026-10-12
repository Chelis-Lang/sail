# Generated-source binding audit on product 730

This file-only audit binds production source
`730a3210151ca51b7e011121c4795553b0cfa3e5`. The
[source record](m7-benchmark-namespace-730-source.json) distinguishes those
bytes from the added, unexecuted component tests. No Docker call, target read,
compiler, Cargo invocation, proof or benchmark ran in this scope. Production
checks, ordering, root selection and publication remain unchanged.

The lead reports that the original core-validated benchmark stopped during its
second required HumanEval `compute_gcd` repair warmup, before measured samples.
Verification exited zero, but both focus and full reports were Unknown and
`accepted` was false. Notes included
`native-observation-invalidated; barrier=post-verification; detail=namespace-root-binding-changed`
and the build-script declaration-establishment note. This review has not opened
the retained native archive and does not independently certify its contents.
The separately recorded [complete original reader pass](m7-reader-730a321-terminal.md)
remains a different scope; it does not admit the failed benchmark warmup.

## Exact source path

| Step | Source in `crates/sail-verus/src/managed/` | Consequence |
| --- | --- | --- |
| Recover prior semantic source declarations | `source_inputs.rs::load`, `runner.rs::run_engine` | Previously observed compiler dependency paths populate `sail_source_inputs`. `source_inputs::update` derives these paths from compiler-artifact dep-info, including generated Rust files. |
| Recover ordinary script observations | `build_inputs_family.rs::begin` | The family export is consumed before shared-target mutation. Without a valid export, prior local records retain observation paths but lose reuse authority (`covered=false`, `output=None`). |
| Choose initial roots | `input.rs::snapshot_metadata_home` | Package directories enter roots. Every prior `sail_source_inputs` path is an explicit root, including files under Cargo's excluded target directory. Symlink resolution can add further exact roots. Ordinary additional regular declaration files are read, but do not automatically become separate root bindings. |
| Clean stale packages | `runner.rs::run_engine`, initial snapshot through `InputGuards` | The initial snapshot is taken **before** stale-script clean. The original precise observer is acquired **after** that clean. Root paths survive in the initial snapshot even if clean removes a generated file. |
| Establish monitored input revision | `runner.rs::run_engine`, `MonitoredSnapshot` and `InputBarrier` | A second snapshot replaces the revision and observation-path inventory, not the initial root list. A missing declared generated file makes this snapshot incomplete. The original Input barrier remains required. |
| Recheck reusable scripts | `build_inputs_family.rs::recheck` | Live-guard recheck can require another clean, after watcher acquisition. It rewrites the local declaration ledger, but that ledger is not automatically a proof-time root. |
| Observe output creation/replacement | `precise.rs::Observer::begin_with`, `identity`, `clean` | Each supplied root is bound to resolved device/inode, or absence, after watcher construction. `clean` checks mounts first, then these bindings, then drains native events. Absent-to-present, present-to-absent, different identity or metadata failure refuses. This predicate does not compare directory mtime. |
| Finish proof and publish | `runner.rs::run_engine`, `PostVerification`; `publish_compiler_generation` | Original source, script, runtime and native barriers still determine Currentness. Unknown cannot export the compiler-family observation generation. Proof receipts are not ordinary dependency observations. |

The lead's retained-metadata inspection reports 551 source paths in each of the
stock and interactive contexts, with exactly one generated target-owned input:
`debug/build/serde_core-90770f6d060cf0d0/out/private.rs`, relative to each compiler
family target. Both family exports were absent. These are reported inventory
facts, not captured old/new device/inode evidence from this review.

That input is a concrete candidate for a bootstrap cycle: missing export forces
regeneration, cleanup removes a previously declared generated semantic input,
and recreation under the original proof observer makes Current unavailable.
Without Current, no family export survives for the next invocation. The source
supports this mechanism; it does not identify the historical first mismatching
binding or prove that serde's output was the failing root.

There are two distinct refusals in the pre-watch-clean sequence. The monitored
snapshot is incomplete while the generated source is absent. Its later
recreation also changes the observer's absence binding. Suppressing one note,
refreshing the binding after compilation or excluding the generated file would
neither preserve the existing authority contract nor resolve both refusals.

## Distinguish the two observer inventories

Proof-time `coherent::Guard::begin` receives `before.roots`. The later
`runner::observation_plan`, used by `FreshnessIndex`, explicitly adds
`input-state.json`, `source-inputs.json`, `build-inputs.json` and runtime selection
sidecars. These inventories must not be conflated.

`runner::runtime_paths` decodes the persisted runtime identity list. Standard
`context` producers add executables, engine artifacts, helper files, actual
Rust/Verus sysroots, existing configuration files and an explicit target JSON.
They do not ordinarily add the three local context ledgers. A local ledger can
still be a proof root if an actual semantic declaration or configuration names
it; membership must come from the acquired root inventory. Atomic local-ledger
replacement by `recheck` alone is therefore not established as this failure's
direct binding cause.

The declaration note refers to **initial** script coverage. It does not prove
that declarations were newly different, or that a local ledger caused the
native failure. A successful verifier exit does not turn Unknown into Current.

## Component controls prepared, not executed

Three added Linux tests use original `snapshot_metadata_home` root derivation
and the original `Observer` without changing its predicates:

1. `unchanged_generated_source_remains_guarded_inside_excluded_target` requires
   the generated file in roots and observed files, complete equal revisions and
   clean barriers, despite the excluded target ancestor.
2. `generated_source_clean_before_watch_then_recreation_refuses_root_binding`
   models initial snapshot, fixture-file deletion, original acquisition,
   incomplete monitored snapshot and equal-byte regeneration. It requires the
   exact root-binding failure and sticky refusal. A new independent original
   acquisition after regeneration can be clean; it does not certify the old run.
3. `generated_source_replaced_after_watch_refuses_even_with_equal_content`
   retains the old inode under another name, installs identical bytes at the
   explicit generated path and requires exact root-binding refusal, even though
   the later content revision equals the initial revision.

These are bounded owned-file mutations, not Cargo execution, actual serde
regeneration, a proof, acceptance or an explanation of the historical benchmark.
Compilation, Linux execution, strict checks and original-predicate sensitivity
remain pending. Removing explicit source-root admission should fail the root
membership assertions; removing the original binding comparison should fail
the exact-reason assertions. Existing mutation and exclusion controls remain.

## Minimal historical-path discriminator

The next diagnostic should retain the **first** failing root's ordinal, exact
path-byte SHA-256, captured old device/inode-or-absence, and the already obtained
new device/inode-or-absence or syscall errno. Seal the ordered root inventory at
actual acquisition so an ordinal/hash can map to an exact private path. Do not
infer that inventory from later artifacts or disclose paths, environments or
file contents in public notes.

A narrow test-only recorder can reuse the one `identity` result already needed
by the existing binding loop. It must keep mount-first order, root order, first
mismatch short-circuit, sticky refusal and native-drain ordering. It should
retain one fixed-size row, no extra per-root metadata calls, no stdout protocol
change and no blocking trace file write on the barrier. Bound path hashing in
the diagnostic; an oversized path must remain an explicitly incomplete mapping,
never a successful check. Export after owned closure, retaining observer failure
and any missing/lost record. Test absent-to-present, replacement and metadata
error representations, plus no record on unchanged roots. This plan introduces
no production recorder or new proof invocation.

If the generated-file mechanism is confirmed, the solution needs an independent
ordinary build-script generation transaction, or preparation that completes
that generation before acquiring the original full proof owner. It cannot
certify Unknown proofs or skip generated semantic inputs. An independent script
transaction would have to guard complete **preknown** script declarations,
effective environment, source/config/tool/runtime inputs and the exact generated
output contract, refuse newly discovered declarations or ambiguous ownership,
and preserve the unchanged full proof barriers. That broader authority seam
requires separate design and controls; this audit does not implement it.

Users should treat this benchmark as a prerequisite failure with no measured
performance result. Developers must keep the already passing reader criterion,
the failed benchmark warmup and any future component diagnosis separate.
