# Pending first-command boundary in the failed workflow

The bounded read-only [supplemental record](m7-rpc-binding-reader-wait-audit.json)
identifies a concrete command/response boundary for further investigation. It
does not identify the cause of the [600-second prerequisite failure](m7-rpc-binding-reader-terminal.md)
or recover the verifier/shim processes missing from its partial snapshot.
No replay, proof or endpoint call was launched during this audit.

The failed ordinary run retains one HumanEval invocation-start marker, no
invocation-completion document and no pending receipt. Its sidecars contain
100 log files (33,076,471 bytes) and 45 small shim provenance documents. Every
recorded provenance document represents a completed shim session; it does not
describe any session that never reached completion. Retained logs stop updating
near the start of the invocation, but modification times do not prove that a
process was blocked or establish which stage stopped progressing.

Of 50 SMT transcripts, `human_eval_052.smt_transcript` alone has one QUERY and
no RESPONSE block. Its 358,318 bytes are sealed with SHA-256
`f59d7790f0f4f986691940dd5eb6d6a17b2b98041f399927364f005fd06fcc93`.
The pending batch ends in `(push)` and `(get-info :all-statistics)`. Transcripts
for modules 054, 056 and 057 are empty. This first batch contains declarations
and a statistics request, rather than a completed function verdict.

The actual selected b346 `source/air/src/smt_process.rs` is byte-identical to
the pinned stock commit's file: 9,208 bytes, SHA-256
`d11353d624acb6bb9f0e8c104cbcb705f89a4ac0ce479e283cdbc26848e46201`.
The source orders these operations:

1. `send_commands_async` writes and flushes a QUERY log before sending the
   request to its writer channel.
2. The writer independently writes the original command vector, then a newline
   and `(echo "<<DONE>>")`, and flushes solver stdin.
3. The reader collects solver lines until DONE. `CommandsHandle::wait` logs a
   RESPONSE only after receiving that completed reader result.

Consequently the unmatched log is evidence of intent followed by an absent
completed receiver result. It cannot discriminate an unscheduled writer,
blocked write/flush, solver read, missing DONE, reader-channel handoff or a
different failure. Replaying the log file itself would be incorrect: logger
markers and response formatting are not the solver's accepted byte stream.

## Expected re-verification in the original workflow

The original reader controller sets `SAIL_MANAGED_FORCE=1` for all commands.
The runner includes that setting in its forced-clean decision, so the later
ordinary check deliberately re-verifies the selected target. Its context and
input revision exactly match the successful ordinary preparation: context
`4bc68866277c17e21676a56e3eb6ce912582a6752da73feb8049763196203ad5`
and revision
`821414360c2a5f474ff8cb6124db72606ab68366c26a240771c0b4a3cd24b947`.
Repair preparation uses distinct context
`7dffdcde165b452207de1ef985d19f39880c9344aa4709a71e84a29871646f15`
with the same input revision. The postclosure repair context still has its
HumanEval receipt, established by run `759104-1791714671431134833`; the
ordinary context retains its vstd receipt. The supplemental record seals these
small metadata projections and each complete receipt file by hash. This
postclosure observation does not recover historical receipt-removal timing,
and no shared target-directory overwrite was observed. No force policy or
workflow check was changed.

## Proposed single failure-point control

Prepare one separately frozen, ownership-bound diagnostic that exercises this
one pending batch through the selected stock AIR transport. Preserve the exact
command vector extracted between the logger's literal markers, and let the
unchanged writer add its original newline/DONE framing. Seal both the intent
vector and resulting expected writer stream; call them reconstructed bytes,
never captured historical wire. Keep the selected runtime and current-product
identities, solver options `-smt2 -in`, original cache configuration and a single
600-second whole-operation deadline. This control is not a Verus proof, a new
reader run or a timing acceptance scope.

A small unselected diagnostic harness can use the actual stock transport source
with observation-only markers immediately before/after channel enqueue,
`write_all`, newline/echo/flush, `read_line`, response send and receiver wait.
Use a fixed metadata datagram format and nonblocking writes to an owned bounded
collector; dropped observations must remain explicit. Do not add another pipe
reader, modify commands, replace flushes, tune thread scheduling or use marker
data as proof authority. The markers add observation work, so their durations
cannot explain the original run's timings retrospectively.

The controller must retain its Popen handle and root birth ticks, use the
[existing bounded all-task observer](m7-reader-process-observation.md), and
capture only metadata at stalled phase boundaries or timeout. It must never
consume the solver's pipes or signal discovered descendants. Original owned
process-group termination and reaping apply after the unchanged deadline;
evidence failure must not mask that timeout. A completed control can establish
that this input/transport combination makes progress; it cannot prove the old
process made the same progress. A stalled control can localize its own last
confirmed boundary, including explicit observation loss, without attributing
the older timeout automatically.

The [concrete component plan](m7-stock-smt-boundary-plan.md) and controller are
now prepared with exact reconstructed-byte seals and static controls. No
executable replay or sensitivity control has run; source/lifecycle review
precedes execution. See [reader acceptance](../reader-acceptance.md),
[RPC diagnostics](../result-rpc-diagnostics.md), [known issues](../known-issues.md)
and [the roadmap](../ROADMAP.md).
