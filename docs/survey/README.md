# Survey

Research recorded before building (charter: "Survey before building"). Each
document states its sources and what it confirms empirically.

| Document | Covers | Status |
|---|---|---|
| [verus-z3.md](verus-z3.md) | How the pinned Verus finds, starts, drives and stops Z3; protocol, limits, labelling, nondeterminism, shim requirements | Source-confirmed. Empirical confirmations recorded in ADR 0003 |
| [prior-art.md](prior-art.md) | Dafny/Boogie caching, F* hints, Mariposa and follow-ups, result caches and solver proxies elsewhere, Z3 determinism | Literature and source review |
| [corpus.md](corpus.md) | In-tree and external Verus projects, compatibility with the pinned release, tiers, history-replay candidates, draft `corpus.toml` | Partly measured with the pinned release |
| solver-behavior.md | Z3 4.16.0 determinism under fixed seeds and rlimit, history dependence, seed stability on the captured query corpus | Pending (M3) |

## Findings that drive decisions

- `VERUS_Z3_PATH` is a faithful integration point. Verus needs no changes
  (ADR 0003).
- Function names never reach the solver pipe. Attribution needs Verus's own
  logs.
- Each Z3 process serves a whole bucket of queries in sequence. The prior-art
  survey cites Z3 issues showing that answers can depend on process history
  and on the platform, so cache keys are measurement-only until M3 measures
  this (ADR 0004).
- Statistics replies carry wall-clock time and memory and differ between any
  two runs. Verdicts, models, version replies and `rlimit-count` were
  identical across runs of the recorded toy session.
- Verus already maintains `verita`, a cross-project runner. The corpus starts
  from its project list, pinned to exact commits.
