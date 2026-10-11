# GC/clear solver timeout and isolated diagnostic

The combined `c97de63` source gate passed 605 tests. Its original solver suite
failed: 40 of 41 cases passed, and `gc_and_clear_during_sessions_only_cost_misses`
reached the unchanged 180-second test limit plus termination grace. The case
finished its maintenance join at 7.129 seconds and began its final concurrent
sessions at 7.174 seconds. That failed scope remains required evidence.

The [machine record](m7-gc-c97-isolated-diagnostic.json) binds the original scratch
archive, original solver log, exact tested executable and one separately authorized
diagnostic. The diagnostic ran the unchanged case with its 60-second session
limit and 180-second test limit. It passed all sessions in 8.78 seconds. Its owned
process group was empty at closure. This isolated pass neither explains the
historical failure nor replaces the complete solver-suite result.

The original partial live observation reported a pass-through `sail-z3-out`
thread waiting in `anon_pipe_write`; most test-process threads waited in
`rt_mutex_schedule`. No original pipe descriptor mapping or user stack was
retained, so those observations do not establish a cache, reader, kernel or
emulation cause. The diagnostic sampled normal active readers and completed
without reproducing the stall. No production deadlock was found in the audited
cached/pass-through output handling.

The test-support timeout does have a lifecycle gap: it acquires the output
buffer mutex for its diagnostic snapshot before killing the timed-out child.
A stalled buffer owner could prevent that cleanup. Its subsequent reader join
is also unbounded. A targeted paused-buffer control and a cleanup-first,
nonblocking snapshot correction remain separate work; neither is yet evidence
of the original timeout's cause. No assertion, query, solver/cache authority,
internal concurrency or original deadline changed in this diagnostic.
