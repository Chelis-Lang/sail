# Tspec: distinguish context disposal from an outstanding response

This file-only follow-up preserves the complete [e0ca failure](m7-default-standard-e0ca-terminal.md)
and the separately [interrupted transcript diagnostic](m7-tspec-transcript-interrupted.md).
No verifier, solver, debugger, build or cleanup runs here. VI-0013 remains
unresolved; the environment description is not a cause attribution.

## What the retained artifacts narrow

The original stock arm timed out at 3,600 seconds with no result. Its empty
kernel pipes cannot expose AIR's command channels, `BufReader` contents,
completion receiver or child-wait ownership. The failed debugger child controls
still exclude that debugger route from an ordinary compiler diagnostic.

The later transcript diagnostic retained 542 files, 161,009,268 bytes and
3,880 complete QUERY/RESPONSE pairs, with no unmatched query or open section.
The newest `proof_bit64_and_rel_mod._47` transcript is empty; the bucket's
`math__bits_p` transcript has 35 completed pairs. This is evidence from the
interrupted diagnostic, not the historical timeout. It offers no exact
unanswered batch to replay. An empty new log does not establish which operation
blocked, whether its query ran, or that suffix 47 means macro iteration 47.

Host Git objects were read at Verus `426d8b01e7ffb910a36c15e1f870ff985bb61eef`
and Verismo `6bc6dd326d8b9ca30ffec57a544b13b5fadf3cb3`, without consulting a
mutable native build tree. They confirm these seams:

- `source/verismo_tspec/src/math/bits_p.rs:527` expands `N in 0..64` into
  separate `by(bit_vector)` assertions in `proof_bit64_and_rel_mod`.
- `source/rust_verify/src/verifier.rs:1211` creates the transcript;
  `:1226–1245` skips the prelude for a bit-vector context. At `:1575`, the
  right-hand side constructs the new context before assignment disposes the
  previous `spinoff_context`.
- `source/air/src/context.rs:206` launches a solver lazily. Therefore creating
  the new empty transcript can precede both prior-context disposal and the
  new solver's first launch/send.
- `source/air/src/smt_process.rs:235` disposes the sole command sender before
  waiting for the child. Its writer/reader threads are not joined there.
  The sender close, writer channel EOF, physical solver-stdin disposal and
  child exit are distinct boundaries.

The exact source hashes are `d11353d6…` for `smt_process.rs` and `f28f141f…`
for `verifier.rs`, matching the [retained audit](m7-standard-tspec-retained-audit.json).
`context.rs` is 21,085 bytes with SHA-256
`916576ec45e0af2341d987fa4103b2dc4fb29a5580a1316d891fafbaf51f0a26`;
`bits_p.rs` is 13,903 bytes with SHA-256
`0dfdb1ca44f2b4863b14eaa548cb43242502d0ee9da1c7cbf1e247621535c236`.

## Next separately reviewed diagnostic

Prefer a diagnostic-only pinned-source phase observer over another
transcript-only run. Preserve the entire tspec package, prepared Cargo target,
stock solver bytes, default verification thread policy, common flags and
original 3,600-second unit deadline. Do not filter to the apparent function,
add a solver cancellation limit, extend a wait or change classification.
This altered observer build earns no stock conformance credit even if it
completes. Freeze its source delta, image, project inventory and invocation
before any execution; a new build and run require a separate grant.

The observer must distinguish these exact operations without taking ownership
of compiler or solver streams:

| Actor | Required entry/completion observations |
| --- | --- |
| Verifier worker | New context begin/ready; prior context replacement begin/end; bucket completion and panic forwarding |
| AIR owner | Solver launch begin/end and child PID; batch sequence; command enqueue and read-request enqueue; response wait enter/return; reader ownership restored |
| Writer | Request receive enter/return; write/marker/flush completion; channel EOF; stdin disposal |
| Reader | Read-request receive enter/return; pipe read; delimiter recognized; completion send |
| Destructor | Command sender disposal; child wait enter/return; child identity |
| Main verifier | Reporter receive versus final worker join |

Use fixed numeric context/solver/batch/role identifiers and fixed phase fields,
not query bytes, file paths, environment values or raw lock addresses. A
bounded diagnostic design should pre-admit at most 4,096 owner slots with
256 bytes per slot (1 MiB), retain current phases and sequence counters rather
than depend solely on an overwriting event ring, and refuse new observation
slots on exhaustion without changing verification. Snapshot consistency must
be explicit: a changed version or incomplete role mapping is unavailable,
never a certified wait dependency. Avoid a shared logging mutex in the
observed path, blocking observer writes, and synchronous stack capture there.
The implementation's allocation, snapshot transport and any extra observer
thread need review before a claimed bound; this is a plan, not an implemented
resource proof.

Supplement with birth-bound non-consuming process/task/FD-link snapshots so
each child and its pipe endpoints can be correlated with the numeric solver
owner. Reuse existing bounded census and all-task dead-state guards; no stream
reads, ptrace or guessed post-reap PID authority. A proposed observation cap is
24 MiB, one snapshot per five seconds, and at most 256 tasks, 4,096 FD links
and a one-second sweep per snapshot. Exhaustion, changing membership or
unreadable rows must remain partial. Keep raw stdout/stderr on their original
independent drains. A missing observer record does not establish a missing
compiler action. The unchanged harness owns its timeout kill; diagnostic
cleanup retains its own uncertainty and never converts a timeout into a result.

Only correlated phase evidence can choose the next correction. For example,
child-wait entry plus sender disposal but no writer EOF narrows a shutdown
handoff; completed flush plus response-wait and active reader narrows a
different path. Neither combination has been observed. If instrumentation
changes the symptom or the unit finishes, retain that scoped outcome without
claiming the original timeout is explained. A subsequent unmodified standard
comparison remains necessary for acceptance.
