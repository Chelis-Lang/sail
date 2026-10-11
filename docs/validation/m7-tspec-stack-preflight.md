# Tspec stack-capture preflight

Five preflight runs on small owned C fixtures tested debugger access in a new Sail-only container.
No Verus, solver, corpus or conformance actor was launched. The two existing
acceptance containers were unchanged. The [machine record](tspec-stack-preflight/summary.json)
binds the image, packages, tools, controls, raw output and closed actors.

All five controls failed their complete unwind/detach/lifecycle criteria:

| Control | Actual result |
| --- | --- |
| GDB ptrace with `SYS_PTRACE` and `seccomp=unconfined` | Guest register reads failed with `Input/output error`. |
| Translator remote debugger | Reached the known guest breakpoint; worker libc frames did not unwind. |
| Remote debugger with exact owned-process library biases | Unwound both known guest marker frames. Remote detach reported `Remote connection closed`; the fixture subsequently exited normally. |
| The same route followed by ordinary fork/exec | Unwound both markers, then terminated with SIGTRAP and `unfair_lock_lock failed: thread holding lock no longer exists`. |
| Keeping the remote connection through fork | Reached the known breakpoint/library setup, then hit the original five-second preflight bound. Fork behavior remains unresolved. |

The remote route used `ROSETTA_DEBUGSERVER_PORT` only on the standalone fixture.
Its existence was investigated from the [Docker issue](https://github.com/docker/for-mac/issues/6921);
the results above are actual local measurements, not an assumption that this
setting is a supported or transparent compiler debugger. The child control
prevents treating successful guest register access as a safe compiler lifecycle.
It does not attribute the historical Verismo timeout to the translator.

Each debugger had a five-second absolute preflight bound and 256 KiB output cap.
The library-bias read was limited to 64 KiB of the owned fixture's mappings.
Only symbols and twelve-frame stacks were requested, with arguments suppressed;
there were no compiler environment, input/configuration-content or compiler stream reads.
The fixture completion bound was two seconds. All owned fixtures and debuggers
closed; the child failure was a measured SIGTRAP, not forced cleanup converted
into a pass. Original failed runs remain individually retained. The last control did not drain
fixture stdout/stderr after its timeout; that evidence limitation is recorded,
and its bounded cleanup is not a successful lifecycle measurement.

The pinned AIR source provides useful stack targets: writer and reader channel
waits, `CommandsHandle::wait`, `SmtProcess::drop`'s child wait, and the verifier's
reporter receive/worker joins. `wait_timeout` falls back to an unbounded handle
wait after a progress report; it is not a solver cancellation deadline. These
are static source facts, not observed tspec stacks. The stock driver retains
symbol and unwind tables but lacks source debug sections, so a functioning
route could identify functions without claiming source-line or local-variable
evidence.

The [original retained audit](m7-standard-tspec-retained-audit.md) and
[whole standard result](m7-default-standard-e0ca-terminal.md) are unchanged:
158 matching units and one plain-arm infrastructure timeout at 3,600 seconds.
No deadline, flag, thread policy, stream or classifier has changed. Before a
new unit diagnostic, the debugger transport must preserve child execution or
the altered execution scope must be reviewed explicitly. Merely setting the
remote port throughout Cargo and its descendants is not an approved plan.
