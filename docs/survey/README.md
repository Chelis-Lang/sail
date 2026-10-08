# Survey

Research recorded before building (charter: "Survey before building"). Each
document states its sources and what it confirms empirically.

| Document | Covers | Status |
|---|---|---|
| [verus-z3.md](verus-z3.md) | How the pinned Verus finds, starts, drives and stops Z3; protocol, limits, labelling, nondeterminism, shim requirements | Source-confirmed. Empirical confirmations recorded in ADR 0003 |
| [prior-art.md](prior-art.md) | Dafny/Boogie caching, F* hints, Mariposa and follow-ups, result caches and solver proxies elsewhere, Z3 determinism | Literature and source review |
| [corpus.md](corpus.md) | In-tree and external Verus projects, compatibility with the pinned release, tiers, history-replay candidates, draft `corpus.toml` | Partly measured with the pinned release |
| [solver-behavior.md](solver-behavior.md) | Z3 4.16.0 replay determinism, seed stability, history dependence and rlimit semantics, measured on the captured query corpus (examples and vstd); the query-time profile | Measured with the pinned release (M3) |

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
  two runs. Replaying a captured session reproduces answers and models
  exactly; `rlimit-count` too, except for a few units of jitter in
  nonlinear-arithmetic spinoffs (`corpus/repros/z3-nla-rlimit-nondeterminism`).
- Answers are a function of the assertion stack on the whole captured corpus
  (3,645 of 3,645 checks), but model bytes and `rlimit` effort depend on
  process history. Query key v1 (stack only) is therefore not sufficient for
  default mode (solver-behavior.md, ADR 0004).
- `(set-option :rlimit N)` is relative to the cumulative count, and Z3 4.16.0
  reports rlimit exhaustion as `canceled`.
- Verus already maintains `verita`, a cross-project runner. The corpus starts
  from its project list, pinned to exact commits.
