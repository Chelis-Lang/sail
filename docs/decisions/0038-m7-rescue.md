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
   - an engine-versus-stock conformance arm, including the cached-versus-fresh lowering oracle where available, runs over the standard corpus;
   - persistent cross-edit reuse of prepared lowering is **off by default**. Re-enabling it by default needs its own ADR with content-addressed keys over the complete lowering inputs (charter rule 6) and the conformance arm at zero differences.
4. **Evidence policy.**
   - Each acceptance gate gets one concise Markdown record. Failed and interrupted attempts stay visible as one indexed line each.
   - Cleanup, retirement, ownership and disk-hygiene records do not go into git.
   - Evidence lands in the PR it qualifies; there are no evidence-only commits.
   - Raw logs stay outside the repository and are cited by sha256.
   - The roadmap's current state is a short gate table, not a log.
5. **Landing.** PR #74 is replaced by small PRs against `main`. Pre-rescue history is preserved in the tag `archive/m7-contracts-f7103bf`.
6. **Direction after M7, ranked by agent authoring-loop latency.** Each item starts with its own ADR and a measurement:
   1. a warm Verus frontend across edits (M8);
   2. a Rust pre-solver for the easy fragment, provisional by default with background Z3 confirmation, gated by differential fuzzing against Z3 (M9);
   3. fail-fast, edit-prioritised streaming of results;
   4. opt-in stability modes (unsat-core pruning, instantiation replay, racing portfolios);
   5. sharing between many concurrent agents;
   6. more backends and certificates;
   7. other verifiers.

## Consequences

- Performance iteration takes minutes instead of tens of minutes, and timing tests run at their real speed.
- The M7 speed gate becomes measurable. If it fails on a catalogue, that failure is recorded together with its phase profile, and the speed claim is re-scoped here rather than met by adding mechanism.
- The repository stays reviewable. Detailed history remains reachable through the archive tag.
