# VI-0012: Filtered verification prints bucket-start notes in arrival order

- **Status:** open; source mechanism established, identical-runtime repeated
  stock control pending. Recorded locally, not filed upstream.
- **Verus:** pinned `0.2026.10.04.426d8b0`, source commit
  `426d8b01e7ffb910a36c15e1f870ff985bb61eef`.
- **Found:** 2026-10-11.
- **Reproduction:** the frozen revision-zero control in
  `engine/verus/candidates/test-private-span-cross-edit.py`, selecting the
  pinned HumanEval module `human_eval_013`. This existing input contains
  separately scheduled `lemma_div` and `lemma_div_converse` buckets. A smaller
  executable stock-only reproduction has not yet been run.
- **Impact:** candidate `50667954` failed the original exact stderr comparison
  before revisions one and two. Neither transcript normalization nor a thread
  override is applied. The source candidate remains unqualified.
- **Workaround:** none adopted. `--no-report-long-running` does not suppress
  these bucket-start notes. See also [VI-0007](VI-0007-nondeterministic-error-order.md)
  for the separately reproduced ordering of errors from different modules.

## Observed failure

The original candidate control produced 234 bytes of stderr on each side.
Both began with `note: verifying module human_eval_013`. Stock then printed
`lemma_div_converse` followed by `lemma_div`; the engine printed `lemma_div`
followed by `lemma_div_converse`. Both completed nine verified functions and
zero errors. The exact byte comparison failed and remains unchanged.

The stock launcher SHA-256 was
`2fe2fca366846ec5aaaaaf80edf061f7d19564c097cb9840c7638719e8bc1854`, and its
driver was
`b8c11e7b626cc85d8a46b90036f0adfec443b7cde1c69d8b182175b92c708af7`.
The reversed stock transcript SHA-256 was
`e30eada43688ea0fa26949feaac4b79e94146226645952a840b3fcba52d857ee`;
the engine transcript SHA-256 was
`63efc95648ca075bb98c1c1689c77373d82310e78a9dafc5da0469563bcbcb16`.
Raw files are retained under the task-owned validation evidence directory
`original-qualification/previous-revision-focus/0/`.

Earlier retained stock transcripts from the `f833` independent-context and
`c05` qualification controls have the latter hash and opposite note order.
Their `tasks/lib.rs` and `tasks/human_eval_013.rs` content hashes match the new
control. The historical launcher/driver and complete command identities still
need to be sealed before this is described as demonstrated repeated stock
instability under identical runtime and flags. These observations alone do not
rule out an engine scheduling defect or a differing historical context.

The same revision-zero candidate also reported zero prepared hits and four
provenance refusals. That independent source-reuse failure is not explained or
waived by note ordering.

## Pinned source mechanism

The frozen test passes `--no-report-long-running` to both arms and leaves the
stock thread count unchanged. In `source/rust_verify/src/config.rs`, parsing
that flag sets `report_long_running` to false. The default thread count is
`max(available_parallelism - 1, 1)`.

In `source/rust_verify/src/verifier.rs`:

- `verify_bucket_outer` emits the start note with `report_now` when tracing is
  enabled **or** the user filter is not everything (lines 1943–1947 in the
  inspected candidate). This condition does not inspect `report_long_running`.
- `QueuedReporter::report_now` delegates to `report_as_now`, which sends
  `ReporterMessage::Message(..., true)` (lines 241–256).
- The receiver prints messages marked `now` immediately instead of buffering
  them for ordered bucket output (lines 2270–2275).
- Query progress uses a separate `do_report_long_running` guard (lines
  770–784). Disabling query notices therefore leaves bucket-start notes active.

These are unchanged stock paths surrounding parallel bucket workers. Arrival
order is not a declared deterministic order. This static finding explains why
the flag cannot stabilize this particular output; it does not by itself prove
which worker scheduling caused the recorded comparison failure.

## Required follow-up

Seal the historical stock identity or run a separately authorized bounded
stock-only reproduction with exact source, launcher, driver and flags. Retain
every raw output and order rather than retrying qualification until it passes.
Any proposed treatment of progress notes needs an explicit correctness decision;
the original diagnostic check remains mandatory in the meantime.
