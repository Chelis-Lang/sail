# Cleanup-first fault-session controls

The final test-support source passes both focused lifecycle controls, the
unchanged GC/clear case and affected all-target strict Clippy. The original
combined `c97de63` solver attempt remains failed at 40 of 41; these scoped passes
do not replace it or identify its historical cause. The
[machine record](m7-fault-session-cleanup-controls.json) binds all commands,
source hashes, tested images, earlier failures and the verified retained archive.
The archive contains 25 listed artifacts plus its `inventory.json` member, for
26 regular members in total.

The actual paused-sink control returns after killing/reaping its directly owned
child while the real reader buffer is still locked. It reports unavailable bytes
and incomplete reader retirement. The control then releases the pause and
observes reader buffer ownership retirement before asserting. A normal-exit
control requires joined EOF and the exact full output bytes.

Reintroducing only the old blocking observation before kill makes that same
pause control fail at its bounded receive: zero passed, one failed, exit 101 in
3.06 seconds. Its pause is released and its waiter joined before the failure.
The actual negative image remains retained. This proves sensitivity to the
cleanup-lock defect, not that it caused the earlier solver timeout.

The first source restoration preserved an old mtime. Cargo performed no
recompilation and ran the byte-identical negative executable beside positive
source bytes; that setup attempt failed and remains retained. Advancing the
source mtime and applying the equivalent test-only poison-recovery Clippy
spelling caused actual recompilation. Both final controls pass in 1.17 seconds.
No source predicate, assertion or original deadline changed.

The original GC/clear case ran once on that final image using the unchanged
nextest CI profile and all concurrent sessions/cache/reply assertions. It passed
in 13.902 seconds; its final sessions began at 11.366 seconds and completed at
13.753 seconds. Affected release all-target strict Clippy then passed. Root owns
the separate full-suite validation. Both-container postflight found zero holders
or unreadable processes for the private source/target scope; this is not a
global orphan-absence claim. No production code, solver query, cache authority,
selected runtime, receipt or performance baseline changed.

An audit of the requested owned host target found no regular `.rlib` or `.rmeta`
files and reclaimed nothing. All binaries, raw evidence and runtimes remain.
