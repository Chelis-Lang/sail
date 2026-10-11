# Nonterminal Verismo tspec process observation

The new complete default standard attempt has 155 of 159 units classified Same
at this observation. Its `verismo_tspec` plain arm is still running. This is
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

These observations establish a sleeping pipeline. They do not identify the
responsible lock, prove a deadlock, or establish emulation or overlapping work
as its cause. No debugger attachment, pipe consumption, retry, source change,
solver flag change or deadline change was made. The original 3,600-second unit
limit and one-job full-suite controller continue unchanged.

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
