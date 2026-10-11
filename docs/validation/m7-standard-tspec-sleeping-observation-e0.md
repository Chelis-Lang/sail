# Verismo tspec sleeping stage and original unit timeout

The plain `verismo_tspec` arm subsequently reached its original 3,600-second
deadline. The harness classified it as infrastructure error `SAIL3022` and
killed its process group. Its retained stdout, stderr and time files are empty;
the Sail arm was not started. A bounded scan found six remaining members of the
owned process group, all dead zombies awaiting reaping. The complete attempt
continues with 155 Same and one infrastructure error; this is a unit outcome,
not the full-suite terminal result.

The new complete default standard attempt had 155 of 159 units classified Same
at the earlier observation below. Its `verismo_tspec` plain arm was running. This is
process-stage evidence, not a terminal conformance result or an identified cause.
The [machine record](m7-standard-tspec-sleeping-observation-e0.json) binds the
frozen executable, stock driver/Z3, source and observer identities.

Across 141 retained samples of the same verifier identity, the last CPU change
was at observer elapsed 4,737.364 seconds, reaching 23,974 ticks. At elapsed
5,241.089 seconds the count was unchanged: about 504 seconds without observed
CPU advancement. All 16 observed verifier threads were sleeping, principally
in `rt_mutex_schedule`; three direct pinned-Z3 children were in `anon_pipe_read`.
The separately checked stdout, stderr and time files were still empty.

A separate bounded `/proc` metadata read retained all 16 reported thread
syscalls, with leader and verifier identities checked before and after. They
report syscall 98: fifteen operations `0x86` and one `0x80`. The container's
Linux header constants identify futex 98, private flag 128, lock-PI operation 6
and wait operation 0. The reported common PC is absent from the complete
recorded maps, so this does not supply a mapped user stack or a lock owner.
The read attached no debugger and consumed no memory or pipe contents.

A later non-consuming `FIONREAD` check found zero available bytes in each of
the three solver children's stdin, stdout and stderr pipes. Their matching
verifier pipe descriptors were recorded, with all process start identities
checked before and after. There was no queued pipe input or unread solver reply
at that instant; this does not identify the reason for the wait.

These observations establish a sleeping pipeline. They do not identify the
responsible lock, prove a deadlock, or establish emulation or overlapping work
as its cause. No debugger attachment, pipe consumption, retry, source change,
solver flag change or deadline change was made. The original 3,600-second unit
limit was unchanged, and the one-job full-suite controller continues unchanged.

The bounded raw snapshot is retained outside the corpus scratch and copied to
the host with matching SHA-256 and length. It is supplementary to the
[corrected metadata observer](m7-conformance-observer-disappearance.md); the
original observer's disappearance failure is retained too. Neither observer
can change the harness verdict or deadline.

The [earlier complete failed standard attempt](../../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json)
and [isolated bitflags result](m7-verismo-bitflags-e0ca-diagnostic.json) remain
separate. A complete terminal record is still required before considering
current-product cache-fill or warm rounds. This observation makes no optional
source-engine, reader, performance or M7 acceptance claim.
