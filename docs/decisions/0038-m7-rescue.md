# 0038: M7 rescue: native reference hosts, evidence policy and engine conditions

- **Status:** accepted
- **Date:** 2026-10-12

## Context

The owner stopped the previous M7 agent and handed M7 to a rescue lead with a broad mandate. The lead may resolve the remaining open questions; this ADR records those resolutions. [The rescue review](../reports/2026-10-12-m7-rescue-review.md) found three problems:

- default-mode correctness intact, but no accepted M7 speed sample, and every paired sample slower than M6;
- velocity consumed by evidence bookkeeping, by x86 emulation on an Apple M3, and by benchmarks that aborted with zero samples;
- engine risks: authoritative engine output with no stock fallback, persistent prepared-lowering reuse that depends on dependency tracking, and engine results labelled as default mode.

The owner also set the direction beyond M7: rank work by agent authoring-loop latency, and run work as concurrent lanes.

## Decision

1. **Reference hosts.** Performance and Linux validation run on native x86-64 Linux EC2 hosts launched with `scripts/perf-host.sh`. Hosts are tagged `Project=sail`, terminate themselves, and are torn down after use. This is the charter's reference platform (ADR 0010), and it replaces the emulated local container for performance work. The ADR 0027 note that permitted emulation is superseded. Every result records the instance type and region.
2. **Benchmarks record, then judge.** A benchmark run keeps every sample with its refusal code and continues. Gates are applied when the report is generated, and no gate is weakened: an M7 sample that violates the protocol is a failing sample, never a dropped one. "Median" in the M7 speed gate means, per catalogue, the median over at least three rounds of the whole-loop total, with arm order alternating each round. Per-cycle medians are also reported.
3. **Interactive-engine conditions.** Repair results from the engine's full phase stay authoritative, because requiring a stock re-run would remove the speed goal. In exchange:
   - output names the engine in provenance;
   - an engine fault falls back to stock verification, as ADR 0027 already requires;
   - an engine-versus-stock conformance arm runs over the standard corpus, comparing verdicts and diagnostics;
   - the engine prepares no lowering for reuse (see the 2026-10-12 removal note below). Reuse across edits returns only through its own ADR with content-addressed keys over the complete lowering inputs (charter rule 6) and the conformance arm at zero differences.
4. **Evidence policy.**
   - Each acceptance gate gets one concise Markdown record. Failed and interrupted attempts stay visible as one indexed line each.
   - Cleanup, retirement, ownership and disk-hygiene records do not go into git.
   - Evidence lands in the PR it qualifies; there are no evidence-only commits.
   - Raw logs stay outside the repository and are cited by sha256.
   - The roadmap's current state is a short gate table, not a log.
5. **Landing.** PR #74 is replaced by small PRs against `main`. Pre-rescue history is preserved in the tag `archive/m7-contracts-f7103bf`.
6. **Direction after M7, ranked by agent authoring-loop latency.** Each item starts with its own ADR and a measurement:
   1. a warm Verus frontend across edits (M8);
   2. deterministic Verus output wherever Sail runs Verus (VI-0001 alone re-solved whole buckets in a third of measured edit cycles, about 2.7 s each; ADR 0039, ADR 0040);
   3. fail-fast, edit-prioritised streaming of results;
   4. opt-in stability modes (unsat-core pruning, instantiation replay, racing portfolios);
   5. sharing between many concurrent agents;
   6. more backends and certificates;
   7. other verifiers.

   Warm solver sessions were measured at about 0.1 s per edit cycle and are deferred until after M8 (ADR 0040). A Rust pre-solver for the easy fragment is not on this list either. The L6 spike ([report](../reports/rescue/l6-presolver-coverage.md)) found that it covers 8.2% of edit-loop cache misses by count and 0.1% by solve time, and that no edit cycle was fully covered, so it would not shorten an agent's time to a verdict. About 59 ms of a 72 ms median miss is Z3 start-up and prefix replay, which the shim pays once per missed bucket, mostly off the critical path. The pre-solver is reconsidered only if arithmetic-heavy projects measure differently.

## Consequences

- Performance iteration takes minutes instead of tens of minutes, and timing tests run at their real speed.
- The M7 speed gate becomes measurable. If it fails on a catalogue, that failure is recorded together with its phase profile, and the speed claim is re-scoped here rather than met by adding mechanism.
- The repository stays reviewable. Detailed history remains reachable through the archive tag.

## Removal (2026-10-12): prepared-lowering reuse and the same-input oracle

This replaces the original condition that persistent prepared-lowering reuse
be off by default, and the same-day amendment that classified oracle
mismatches with confirming fresh lowerings. Both features were removed from
the engine (rescue L7a):

- **Reuse paid for nothing.** L1 measured no in-batch hits on HumanEval and
  few or none persistent; every miss still paid for key encoding and
  admission under one global lock, about 6.7 s per repair on HumanEval and
  40 s on Vest, which cancelled the shared callback's saving.
- **The oracle could not stay stable.** It needed confirming fresh arms and
  an unstable class to stop flaking on stock trigger nondeterminism
  (VI-0015).
- **M8 replaces it.** Reuse across edits is redesigned with content-addressed
  keys (L9).

The engine now lowers every bucket fresh, as stock Verus does. The engine
conformance arm keeps its comparison with plain Verus on verdicts and
diagnostics and its completion-seal check. The repair gate's preparation
telemetry check (`SAIL7105`) is retired.
