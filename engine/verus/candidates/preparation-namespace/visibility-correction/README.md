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
This candidate remains unselected. Its executed source, ordinary build and
qualification scopes are recorded separately below.

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

The ordinary build is now PASS:479.578 seconds, fresh vstd2060/0 and the complete
new thirteen-artifact manifest, with exact source8 and source lock. The build
actor closed; all three prior runtime/source/lock seals remained unchanged.
`ordinary-build-pass.json` binds the manifest, raw logs, compiler-only seed and
controllers. An independent review checked the actual closed bytes and original
qualification launcher. This is an ordinary build result, not a qualification
or product acceptance result; the candidate remains unselected.


The one original qualification failed after 186.002 seconds: eight of thirteen
actual pipeline stages passed, the ninth (`previous-revision-focus`) failed, and
four were not attempted. The frozen inventory contains fourteen scripts; it is
not a count of executed stages. Domain migration and the checked same-input hit
controls passed. No script, environment selector, assertion or 600-second
compiler limit changed.

The failed stage stopped in revision 0 with zero hits, zero same-callback checks
and zero focused restores. Stock and engine both verified 9 functions with no
errors, identical per-function results and exact stderr. The assertion label
`unchecked actual hit` describes the missing positive hit here; it does not
identify an unchecked nonzero hit. Previous-revision focused reuse and the full
HE gate remain unestablished.

Three events refused used-binding namespace validation, and one `source-map`
event separately refused service availability. The latter is not inferred to
wrap an inner namespace refusal: the generic return preserves `Some(None)` for
that case. Existing events identify neither the first rejected binding nor its
exact role/namespace reason. The dormant import trace runs after successful
namespace validation and cannot identify this refusal by itself. No cause is
assigned to a malformed binding, changed owner, unsupported state or broker
admission without further evidence.

[qualification-failure.json](qualification-failure.json) binds the terminal
result, thirteen-stage count, fourteen-script hashes, failed revision and raw
archive. Independent postflight verified the complete new thirteen runtime
artifacts, eight source files and lock, all three older runtime/source/lock seals
and zero remaining owned actors in both containers. The original failed run
remains failed; no retry, acceptance credit or runtime selection follows it.
