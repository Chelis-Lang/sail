# Reconstructed HE lease refusal: installed executable ownership

After the [original functional prerequisite failed](m7-retained-he-functional-1e.md),
one separately frozen, unselected test reconstructed its completed repair
scope. It called production `refresh_context` without Cargo, built the original
native Observer inventory and attempted original read-lease acquisition once.
Only test-side acquisition-stage and bounded helper error capture were added;
roots, exclusions, limits, lease operations and successful readiness frames
were unchanged. [The diagnostic patch](m7-retained-he-lease-refusal-1e.patch)
and [portable evidence](m7-retained-he-lease-refusal-1e.json) record their hashes.

The test exited 0 in 23.524 seconds, recording a refusal rather than a Current
proof. The Observer succeeded and remained clean. Acquisition stopped at
`helper-readiness-frame` after 1055.372 ms: the helper's
`F_SETLEASE(F_RDLCK)` returned errno 13 (`EACCES`) on the sealed Sail executable.
That file was owned by UID 501, whereas the diagnostic process ran as UID 0
without `CAP_LEASE`. The helper is launched without a credential switch.

The exact reconstructed Observer inventory contained 1413 files; the combined
acquisition bindings, including the helper closure, contained 1415. All current
device/inode bindings matched. Of those files, 1413 were owned by UID 0 and
exactly two by UID 501: the sealed Sail and shim executables. A bounded
metadata-only scan examined 613 processes and 344 descriptors in 23.109 ms,
found no holder of the refused inode, and recorded one vanished-proc-entry
error. It is not a historical holder scan.

Linux checks matching mapped inode ownership or `CAP_LEASE` before establishing
a lease, returning `EACCES` otherwise.
[The kernel implementation](https://github.com/torvalds/linux/blob/v6.17/fs/locks.c#L1893-L1906)
supports this explanation for the reconstructed ownership refusal. It does
not establish a scheduling or conflicting-writer cause. The original proof
reports did not capture stage/path/errno; those historical details remain
unavailable.

For this audited environment, the minimal setup correction is to create new
native copies of both executable contents as the service UID before recording
a new freeze. Existing sealed copies must remain untouched. When an actual
complete runtime inventory contains other foreign-owned files, a dedicated
Linux container with only the additional `LEASE` capability is an alternative;
normal guard validation is still required. Ownership or capabilities alone
never establish Current.

There was no Cargo or proof invocation during the test, no fill, source change,
lease retry or acceptance repeat. Original retained input-state, source-input
and build-input hashes matched before and after. The original owned helper
shutdown/drop behavior was retained. The source archive and executable remain
unselected. Environment values were retained privately but excluded from the
raw export; their exact hash is recorded. The raw archive contains 34,344,960
bytes, SHA-256
`2446a7e8797f64879649008beb4f9fd46b846b713b186c0db07c16d0c04b16df`.

See [freshness prerequisites](../freshness.md) and
[the lease trust boundary](../trust.md#retained-freshness-and-kernel-lease-ownership).
