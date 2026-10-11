# Namespace request visibility correction

The preceding frozen candidate passed every VIR and driver library control but
failed strict driver compilation with `private_interfaces`. This delta changes
only `NamespaceRequest` from parent-visible to crate-visible, matching the
existing reporter enum. Its fields remain private; no endpoint, SourceMap
transport, diagnostic behavior, quota, assertion or lint policy changes.

The preceding failed source scope remains independently retained. The new
controller validates that exact failure and source before applying this delta,
then repeats all complete-source tests and strict checks in the same closed
private compiler target. The old runtime/source/lock inventories stay protected.
This candidate remains unselected; no runtime build or qualification has run.

The corrected narrow patch uses the pinned Verus repository's `source/` prefix,
as does the full patch. A private byte-equality preflight rejected the first
file-only apply attempt, then verified the exact repository-root layout.
No native source actor ran with the incorrect narrow-patch path.

The corrected actual complete-source run passed VIR41/41, driver88/88 and both
all-target strict checks with `-D warnings`. `source-controls-pass.json` binds
each command, log and tested executable; no check or lint policy changed.
The exact candidate source/lock and all three old runtime/source/lock seals
remained unchanged, with no remaining dual-container holders. The preceding
strict failure stays failed. Fresh ordinary vstd/runtime construction and the
original qualification remain separate required scopes.
