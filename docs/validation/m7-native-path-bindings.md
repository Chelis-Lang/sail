# Exact native path bindings

The [complete Linux gate](../../bench/validation/m7-linux-binding-gate-46e1da6-emulated-2026-10-11.json)
passes all 566 tests, strict linting, doctests, rustdoc, layering, dependency
policy and the generated benchmark check. The separate
[solver suite](../../bench/validation/m7-linux-binding-solver-46e1da6-emulated-2026-10-11.json)
passes all 39 original controls, including restored bind/unmount invalidation.
The local cold gate took 823.686 seconds after regenerable library retirement;
the solver scope took 117.377 seconds. These are emulated local checks, not
native CI or published performance measurements.

At each lease barrier the current path is opened and its exact inode and kernel
pathname compared. Any uncertainty falls back to the original predicate.
Existing namespace, mount, kernel lease, interpreter and helper barriers remain.
No timestamp, new cohort grace period or retained identity memo authorizes reuse.
See [ADR 0032](../decisions/0032-kernel-mount-generation-coverage.md).

The new native regressions compare symlinks, hardlinks, parent traversal, native
byte names, missing paths and unavailable procfs fallback with the old predicate.
An opened-and-unlinked file must not impersonate a live same-inode hardlink
whose literal name ends in the kernel deleted marker. Both new tests pass. The [actual negative control](../../bench/validation/m7-linux-binding-negative-46e1da6-emulated-2026-10-11.json)
removes only the deleted-marker guard in an isolated test-only source copy. The
unchanged hardlink regression then fails at its intended assertion in 12.060
seconds inclusive of compilation. The original passing test ELF is sealed and
archived, and the production source remains unchanged. No unqualified test-only
artifact grants proof or runtime authority.

The [first compile failure](../../bench/validation/m7-linux-binding-first-compile-a9300ac-emulated-2026-10-11.json)
remains recorded; the extracted comparison was corrected to borrow a native
Path instead of a PathBuf. The [bounded syscall component](m7-binding-paths-component/README.md)
is synthetic and does not replace live reader acceptance. The
[sealed release](m7-binding-product/installed-product.json) is now installed,
and one unchanged live-reader workflow is running against its owned copy.
The first release process stalled and was closed after an exact PID and birth
identity check; its [failure record](m7-binding-product/stalled-build.json) and
[kernel observations](m7-binding-product/stall-observation.json) remain
separate. No userspace cause is established. A fresh process built the same
source in 108.350 seconds. The [original managed CLI/MCP scope](m7-managed-current-product.md) passes
against this release, with its 240-second verification bound unchanged.
The [new native CI](../../bench/validation/m7-linux-ci-4e101c8-native-2026-10-11.json)
passes all ten jobs in 374 seconds, including 566 workspace tests, 39 solver
controls and all three 33/33 conformance rounds. The 403.341 ms failure remains
failed, and the optional source engine remains unselected.
