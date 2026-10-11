# Bounded correction to the stock transport recorder

The [first normal control](m7-stock-smt-boundary-terminal.md) transferred all
46 expected wire bytes and exited successfully, but dropped two metadata rows.
Its recorder failed; the four fault controls and real forwarding arm were not
attempted. That failed scope and its frozen sources remain unchanged.

The separate correction in the [diagnostic controller](../../scripts/m7-stock-smt-process-diagnostic.py)
replaces direct datagram sends from stock threads with a fixed 64-row queue.
Each row contains four integers and occupies 32 bytes, so queued payload storage
is 2 KiB. A marker only attempts `try_send`; it never waits on metadata output.
One owned metadata writer sends the rows with a timeout of at most 600 ms and
no more than the remaining whole-operation budget. This accommodates the
observer's documented 0.5-second snapshot window plus collector polling; it does
not guarantee delivery if that finite work bound or queue capacity is exceeded. Queue-full, disconnected,
send-error and deadline losses have separate counters. Nothing retries a
failed metadata send or changes stock command bytes, solver options, channels,
flushes, reads or proof authority. This diagnostic is not a product change.

On normal completion, the writer drains and joins only after the original stock
`Drop`. The controller's existing whole-operation timeout still owns process-group
cleanup, including a stuck join; no new time is added to the operation budget.
A killed fault actor cannot report that it joined or supplied final counters.
The collector retains its bounded final drain and explicitly records unknown
general tail loss when completion counters are absent.

The fixed fake peer reads exactly one request and emits one response line plus
DONE. Source-bound stage multisets require exactly 1, 7, 14 and 19 rows for the
enqueue, write, read and delivery pauses, respectively; normal completion requires
23 rows. They include every repeated read marker, contiguous sequence numbers,
zero reported loss and exact untouched command/newline/DONE wire bytes. Reordering
between threads is allowed; missing, duplicated or changed markers fail. These
counts certify only this fixed program's expected prefix, not an arbitrary
interrupted real arm or historical observation. Normal completion also requires
complete zero-loss sender accounting and one response.

Nine [pure preparation controls](../../scripts/test-m7-stock-smt-process-diagnostic.py)
pass on the host and Linux. They cover exact prefix multiplicities, sequence/loss refusal,
complete loss-class accounting, source boundaries, byte extraction and compiler
timeout preservation. The [static freeze](m7-stock-smt-recorder-correction.json) binds the corrected
controller, generated sources, argv, complete runtime/tools and unchanged input.
An earlier 25 ms sender timeout was superseded before execution because it
did not accommodate that observer window. An initial static preparation used an
incorrect transcript filename; its missing-file
error is retained, and the corrected path resolves the same sealed transcript.
No compiler or solver was launched. Executable controls remain pending. Neither pure
controls nor a successful component can explain the original full-project wait
or count as reader, proof, performance or M7 acceptance. The original
[plan](m7-stock-smt-boundary-plan.md) and [focused ordinary diagnosis](m7-ordinary-wait-diagnostic-plan.md)
retain their independent scopes.
