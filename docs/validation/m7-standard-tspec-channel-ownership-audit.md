# Tspec: pinned AIR channel and child ownership

This is a read-only audit of Verus commit
`426d8b01e7ffb910a36c15e1f870ff985bb61eef` and the original Sail runner
`e0ca5f5154e74326443181f57f0fd928488159d3`. It identifies stack targets and
falsifiable wait hypotheses. It establishes no cause for the retained tspec
timeout and launches no proof, debugger or build.

## Normal request path

`crates/sail-verus/src/run.rs:53` adds `--no-report-long-running` to both arms.
Pinned Verus `config.rs:836` therefore disables the long-running callback.
`air/src/smt_verify.rs:225–237` uses `CommandsHandle::wait` directly. Even with
the callback enabled, expiry reports progress and then calls `wait`; it does
not cancel Z3. The initial declaration/version roundtrip at `smt_verify.rs:178–181`
also uses the direct wait. The original unit's 3,600-second deadline belongs
to the Sail harness, not to these AIR waits.

In `air/src/smt_process.rs`, each solver owns three channel pairs:

| Channel | Producer → consumer | Ownership |
| --- | --- | --- |
| Command batches | `SmtProcess` → writer thread | The private `requests` field is the only sender; the closed source scan finds no sender clone. |
| Read requests | `SmtProcess` → reader thread | Transfers the sole `BufReader<ChildStdout>` for the next response. |
| Completed responses | Reader thread → `CommandsHandle` | Returns that same reader with ordered response lines. |

`send_commands_async:167–191` enqueues the command batch before taking the
reader/response-receiver pair and sending the read request. Its mutable borrow
and `responses_buf_recv.take()` enforce one outstanding response per process.
`wait:214–218` restores that pair after receiving the completion. A caller that
does not wait cannot issue another ordinary batch successfully; the absent
pair triggers the existing invariant failure.

The writer at lines 48–59 writes the batch, newline, echoed completion marker
and flush. The reader at lines 68–100 reads until the exact marker, then sends
the response. EOF returns empty lines and causes the existing panic after three
empty reads; it is not an explicit infinite EOF loop. Ordinary send/read errors
and channel disconnection panic rather than silently dropping a response.
No normal source branch skips the writer enqueue, read enqueue or completion
send. This does not prove the actual runtime executed those branches.

## Wait hypotheses and distinguishing evidence

| Possible phase | What a real stack would need to show | Static constraint |
| --- | --- | --- |
| Pending AIR response | Worker in `CommandsHandle::wait`, paired writer and reader positions | Empty kernel pipes do not identify the command queue, reader buffer or completion state. |
| Solver shutdown | Worker in `SmtProcess::drop` → `Child::wait` | Drop takes and closes the sole writer sender **before** waiting. The writer must finish any queued batch before dropping solver stdin. There is no owned writer join or internal shutdown deadline. |
| Missing bucket completion | Main thread in reporter `recv`, worker positions and assigned bucket | Every normal worker bucket path calls `reporter.done()`. A worker panic is forwarded after `catch_unwind`; unwinding that blocks in child destruction could delay that forwarding. No such panic or destruction stack was observed. |
| Main waiting for worker exit | Main in worker join after all bucket Done messages | The join loop is reached after `num_done == bucket_ids.len()`. It is distinct from waiting for an AIR response or a Done message. |

At `rust_verify/src/verifier.rs:2163–2206`, the task mutex is explicitly dropped
before verification. At `2220`, the main reporter loop blocks receiving before
its finished-worker checks at `2470`. Its own sender is retained, so receiver
disconnection is not an independent worker-liveness check. This is a visibility
limitation, not proof that a normal worker can disappear without Done or panic
forwarding. The existing catch and Done paths provide those notifications.

A stable observation of child stdin blocked on input, writer blocked on its
request receive, and the same process already inside `SmtProcess::drop` would
need explanation: the source closes the sole request sender before the child
wait. The retained kernel observations do not establish this combination.
Likewise, sleeping futex metadata alone cannot identify any of these channels.

The [retained timeout audit](m7-standard-tspec-retained-audit.md) and
[whole standard result](m7-default-standard-e0ca-terminal.md) retain their
original failed scope. The [debugger preflight](m7-tspec-stack-preflight.md)
recovered guest frames in tiny fixtures but failed transparent child-lifecycle
controls. That route is not approved for stock tspec. No assertion, flag,
thread count, stream, classifier or deadline change follows from this audit.
