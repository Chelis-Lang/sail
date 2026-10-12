# Bootstrap 602 positive-control failure

Frozen source `602ec67d94f9d1b7918933892ad780d4ad1246b6` passes strict compilation
but fails the complete workspace gate: 649 tests run, 646 PASS, three FAIL and
41 solver cases skipped. The enclosing gate takes 456.378 seconds; nextest takes
210.302 seconds. The original solver suite is unattempted after this failure.

The failed controls are the actual-Cargo publication positive, empty-generation
recovery positive, and the successful baseline inside the reused-binary/dep-info
negative control. Their unchanged publication assertions fail. Five other
bootstrap negative controls pass, but cannot qualify a publication route whose
positive baseline fails. The [raw identity record](m7-bootstrap-602ec67-gate-failure.json)
preserves all host file lengths/hashes, exact source archive, logs and controller
outcome. Two compiler jobs, one independent nextest case and original internal
fixture behavior remain unchanged.

A separate fresh tiny-Cargo format diagnostic passes. Its metadata and build
phases take 1.624 and 2.859 seconds, each under the existing 60-second bound. The
single 290-byte custom-build dep-info file has an absolute first-rule header
equal to its actual dep-info path, but the prerequisite is relative `build.rs`.
Its matching custom-build artifact reports `fresh=false`. The diagnostic runs
Cargo in the fixture package directory and seals that actual source identity.
With one package, package and workspace directories coincide; this capture
cannot distinguish them. It does not establish successful bootstrap publication, reproduce the full
project's concurrency, or identify the historical observer mismatch.

The failing candidate resolves that relative prerequisite using the Sail process
working directory. That is different from this Cargo workspace directory.
A second actual workspace-member capture resolves the ambiguity: its 325-byte
dep-info prerequisite is `member/build.rs`, matching the custom-build source
only when joined to the exact metadata workspace root. Metadata and build pass
in 1.573 and 2.796 seconds under the same 60-second bounds. Joining that spelling
to the package directory would be incorrect. These independently hashed records
describe actual Cargo output; no new native actor ran in this review.

A third fresh capture launches Cargo from the member directory itself. It still
emits `member/build.rs` relative to the metadata workspace root; metadata and
build pass in 2.276 and 3.446 seconds. This distinguishes workspace-based
prerequisites from both package directory and Cargo launch directory. All three
captures retain their exact dep-info headers, source artifact roles and raw
hashes. The checks operate within the documented trusted private Cargo-artifact
boundary, not an authentication claim for maliciously omitted dependencies.

The narrow correction must use the exact canonical absolute metadata workspace
root and custom-build source identity, retain exact header matching, and require the same
original independently guarded inventory and raw-alias refusal. Unsupported
directory identity must refuse the optional export. No declaration or semantic
input may be excluded to make the positive pass.

The independent `0a353ee` review ACKs non-UTF-8 refusal: any unrepresentable key
or value declines optional bootstrap capture before native acquisition, and an
unrepresentable declared value refuses ordinary-family matching instead of
being treated as absent. The isolated child control does not mutate the parent
process environment. It remains a separately pending source revision, not a
passing result from this 602 scope.

Original recorded source/protected/PID1 checks and before/after container identity
match. The gate's known groups have no live survivor or unavailable row and seven
complete all-dead task records. Both tiny-Cargo phases have empty known-group
closure records. These are original bounded closure observations, not a new
native census or general orphan-absence certificate. This file-only review hashes
the retained host artifacts and embedded dep-info bytes without launching actors.
The [earlier 244 PASS and two strict failures](m7-core-244d51d-validation.md) remain
separate. Corrected positive controls, sensitivity, combined gate/solver and
unchanged original product scenarios are still required; no proof, installed
product or benchmark acceptance follows here.
