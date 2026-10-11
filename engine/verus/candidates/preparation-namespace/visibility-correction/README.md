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
