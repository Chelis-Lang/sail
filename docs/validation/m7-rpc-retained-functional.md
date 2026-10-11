# Separate real retained CLI/MCP cursor disagreement

The original retained functional harness failed after normal product restart
re-established the genuine completed baseline as publicly Current. This is
separate from the [failed reader workflow](m7-rpc-stage-workflow.md): it does not
change its Result p95 or make the unattempted Rust census pass.

The [portable record](m7-rpc-retained-functional.json) binds exact product,
original script and selected b346 runtime identities, every re-establishment
status, original operations, bounded owned closure and raw archive. No report,
receipt, proof target or digest authority was copied. The existing baseline
`1791711182962-716934-2` retained input revision
`f2197351d5d52c79a315175829a8634eea4b557f7d33b0bed25ccdd1804ae926`.

Normal background projection establishment took 18.412056 s, including 32
five-second-bounded status reads and the complete Current Result read. The
first public status was Unknown; the final complete Result was admitted using
the unchanged full-report and exact-function checks. The separately recorded
observation window was bounded at 120 s and issued no proof or Prepare request.

The unchanged original harness then ran with its original 120 s whole,
30 s CLI and 5 s MCP exchange limits. CLI retained status, stable missing-job
error, immutable baseline save/self-compare/overwrite refusal, actual MCP
complete Result, exact function explanation, missing-job/function errors and
the initial proof event page passed. After 5.337185 s, the wrong-stream control
failed: MCP returned `isError=true`, wrapping `SAIL0902: cursor belongs to
another proof stream`, while the harness expected a non-error page with a gap.

This is an assertion/contract disagreement, not a transport timeout. The
production journal explicitly rejects a nonempty cursor naming another stream
before reading events. The API describes a cursor as meaningful only within
its named stream; a gap describes missing or rejected events within a stream.
No assertion, deadline or production behavior was changed. The original
failure is retained for review rather than silently accepting either outcome.
Reconnect, cursor replay, MCP comparison and owned forced submission/cancellation
were not reached; no new proof was submitted.

## Reviewed harness correction

The wrong-stream assertion now requires that exact existing `SAIL0902` MCP
error projection, including `isError=true` and complete exit-code evidence.
This follows the existing explicit-refusal contract in
[the workflow guide](../proof-repair.md#resumable-observations), the stream-bound
[`Cursor` API](../../crates/sail-api/src/proof.rs), and the early rejection in
[`Journals::page`](../../crates/sail-daemon/src/proof_journal.rs).
[ADR 0027](../decisions/0027-interactive-proof-engine-and-repair-workflow.md)
requires visible gaps and completion within the journal; it does not authorize
interpreting another stream's cursor as a page of this stream.

Three new controls use the retained actual error to reproduce the original
non-error assertion failure, require the complete corrected projection, and
reject non-error gap pages, wrong error messages, incomplete results or wrong
exit codes. All twelve harness controls pass. Only the mistaken wrong-stream
expectation changed; reconnect, gap/truncation/duplicate checks and all
120/30/5 deadlines remain unchanged. Any subsequent real run must identify this
corrected harness revision; the archived original failure remains a failure.

The one [separately labelled corrected follow-up](m7-rpc-retained-corrected.md)
passed complete functional coverage with the original bounds and normal fresh
Current admission. It does not replace this original failure or the reader
latency failure.

The wrapper took 24.974297 s inclusive. Its owned daemon exited normally with
code 0 in 0.135078 s; cleanup errors were empty. Product/script byte seals and
the original exported stage-recorder bytes remained unchanged. The earlier
reader failure and its 43-row attribution subset remain intact.

See the [repair workflow](../proof-repair.md), [reader scope](../reader-acceptance.md),
[known issues](../known-issues.md) and [roadmap](../ROADMAP.md).
