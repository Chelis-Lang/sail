# 0110: Interrupted first observations are discarded, not trusted

- **Status:** accepted (the conservative rule below only)
- **Date:** 2026-11-10
- **Amends:** [0023](0023-managed-incremental-workspace-verification.md)
  (first observation of a proof context),
  [0026](0026-live-workspace-dependencies.md) (absent-configuration queue),
  [0014](0014-attribution-ledger-and-output-schema.md) (unsourced checks)

## Context

A managed result is Current only when every currentness predicate holds
(ADR 0023, ADR 0041). One of them is compiler-declared source coverage: a
proof context's `source-inputs.json` records the files each compiled unit
declared in its dep-info (for example `include_str!` targets), and the next
run's input snapshot includes them. A context's **first observation** has no
such record yet, and none is needed: its run records its own declarations and
compares them after the run. A missing record is therefore complete only when
the context has never been observed (`source_inputs::load`): once
`input-state.json` exists, a missing record is incomplete and the result is
Unknown (`SAIL3114`).

[Sail issue 696](https://github.com/Chelis-Lang/sail/issues/696) reported
intermittent Unknown results. With a reason code recorded per predicate, the
v0.3.0 spike reproduced two causes on native Linux hosts:

1. **An interrupted first observation.** The first run of a context writes
   `input-state.json` before Cargo starts and `source-inputs.json` after Cargo
   finishes. A run that ends in between (a cancelled job, or the service's
   new-project preparation preempted by a job about a second after `sail new`)
   leaves a context that reads as observed but has no declarations. The next
   run is then Unknown: 9 of 11 jobs with a 1-3 s gap after `sail new`, and 53
   of 83 jobs under heavy host load, 46 of them for this reason alone.
2. **Absent-configuration queue overflow.** The inotify observer of absent
   Cargo configuration paths (ADR 0026) watches the nearest existing ancestor
   of each path, which includes `/tmp` and `/` for a workspace under `/tmp`.
   It drained the kernel queue (16,384 events by default) only when the run
   ended. 1,000 unrelated create/delete pairs per second in `/tmp` overflowed
   it in 5 of 5 jobs.

Both failures are conservative: nothing unproved was reported as verified.
But a correct result read Unknown until it was resubmitted.

The simple repair of cause 1, treating a context whose earlier runs were all
interrupted as still at its first observation, relaxes a predicate: the
interrupted run may have compiled and verified units into the compiler
family's Cargo target, and Cargo can later report those units fresh. Their
dep-info describes the inputs as they were then. A first observation that
reuses them would learn its declarations from outputs no recorded observation
covered.

## Decision

1. **Marker.** A run that begins a proof context's first observation (no
   `input-state.json` and no `source-inputs.json`) writes
   `first-observation.started` in the context before it writes the first input
   state, and removes it only after the run's compiler-declared inputs are
   recorded.
2. **Discard.** A run that finds the marker discards everything the
   interrupted run could have left, before it reads any context state, in this
   order:
   1. every proof receipt of every context of the same compiler family, so no
      receipt outlives the target it describes (the rule pruning already
      keeps);
   2. the family's Cargo target, by the same whole-family clean as a compiler
      protocol migration (`clean_compiler_family`, which refuses an artifact
      layout Sail does not own);
   3. every record of this context except its run directories.

   Then it removes the marker and continues as a genuine first observation.
   The marker is removed last, so a discard that is itself interrupted is
   repeated in full by the next run. Any failure is an error of the run, never
   a relaxed result.
3. **Reasons.** Every predicate that withholds Current is named: the
   `SAIL3114` note lists them (for example
   `SAIL3114: inputs could not be confirmed unchanged (absent-queue-overflow);
   re-run for a current result`), and a `currentness-decision` job event
   carries `currentness` and `reasons`. The service's own read-back failure is
   named `readback-unconfirmed`. The project-result schema is unchanged.
4. **Watcher.** The absent-configuration observer drains its queue
   continuously on its own thread (a pass every 5 ms) and once more after the
   run. An overflow, a removed watch or an unreadable queue still makes the
   observation incomplete: lost events cannot be reconstructed by a later
   rescan, because a configuration created and removed during the run leaves
   no trace in the final state. The observation therefore fails closed, and
   now names its reason.
5. **Unsourced checks.** A target report with a check that has no source (an
   answer the cache never recorded, for example after the shim fell back to
   the pinned solver on a failed handshake,
   [#683](https://github.com/Chelis-Lang/sail/issues/683)) publishes no
   reusable receipt, replay record, machine-local dependency build or
   standalone proof, and a stored one with such a check is not reused.
   Standalone service runs now set `SAIL_PROVENANCE_DIR`, so their checks are
   sourced ([#684](https://github.com/Chelis-Lang/sail/issues/684)).

## Soundness

The marker exists from before the first input state is written until after
the declarations are recorded, so every run that ended in that window leaves
it. Runs of one family are serialized by its exclusive ownership lock, so no
other run observes the window. After the discard the context has no records
and the family target holds no compiled output (or is new), which is exactly
the state of a context that was never observed in a family whose target was
never built. A first observation from that state is the one ADR 0023 already
accepts; nothing the interrupted run produced can be read by it, and no
receipt of a sibling context can name an output that no longer exists. A
context without the marker behaves exactly as before, including a missing
record after an earlier complete observation (still Unknown). Reasons,
continuous draining and the unsourced-check rule only add information or
withhold reuse; none of them makes any result Current that was not Current
before.

## Consequences

- The run after an interrupted first observation is a cold run of the whole
  compiler family, including dependencies, and sibling contexts of that
  family verify their targets again. This happens only after an interruption
  inside the first observation.
- A run interrupted after its declarations were recorded keeps the earlier
  behaviour; such runs were not a cause of #696.
- A long-lived thread per managed run watches absent configuration paths.
  Overflow now needs millions of events per second in the watched
  directories.
- An unsourced target is verified again on its next run instead of reused, so
  its checks can become sourced.
- What makes the shim's handshake fail organically (#683) remains unknown.
