# Missing repair selector phase review

This read-only audit distinguishes the original full repair timeout from a
subsequent reduced success-prefix diagnostic. No new build or proof actor was
started by the review.

The source preparation engine clears caching before selection. An exact
selector with zero or multiple matches emits an unavailable observation and
returns before imported-source handoff, Sources publication, cache admission or
focused verification. Full verification still runs normally. The later bucket
paths test disabled caching before acquiring the preparation gate or namespace
broker. Full completion requires its normal full compiler seal; a focused seal
is not required for an unavailable selection. Missing focus artifacts do not
block the adapter. The public repair envelope then retains the full result and
reports `SAIL3122`, no focused report and `accepted = false`.

The compiler-family ownership locks use nonblocking acquisition and remain
held through clean, invoke and receipt publication. No static selector-specific
broker or family-lock deadlock was found. The daemon's `Running` state includes
synchronous freshness establishment after the private worker exits; that state
alone does not identify an active compiler or the failed phase.

The closed reduced `cdb6e6d` diagnostic's result record has SHA-256
`e23bf70ea68933dc0bd7347ad68d7eb751d61ed6ec6085ff24caaad67afe0125`.
Its primary scope completed in 77.045656 seconds, with the missing job submitted
at 47.857549 seconds and completed 29.042 seconds after its recorded start.
The full missing result was Current, exit 0 and two functions verified; repair
was refused with exact `SAIL3122`, no focus and no batching. Preparation remained
disabled with zero hits and misses. Its source stream contains the exact
zero-match unavailable observation, ordinary full function observations,
phase-finished and compiler-sealed. Product, shim and all 13 runtime hashes
matched before and afterward; daemon exit was 0.

At missing plus 15 seconds, the limited birth-bound snapshot recorded Cargo
and ordinary `syn`/`verus_syn` compiler descendants. Neither was marked as a
Verus proof crate. This identifies dependency compilation in that snapshot,
not a selected proof or preparation-broker wait. The observer is partial and
cannot establish exhaustive ownership or a deadlock from sleeping states.

The original whole repair fixture still failed its unchanged 120-second bound
at 120.314 seconds. The reduced scope omits later original fixture operations
and is not whole-suite acceptance. Its success does not explain or erase that
original failure, prove a performance improvement or authorize receipt reuse.
