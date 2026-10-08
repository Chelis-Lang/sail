# Verus issues (tracked locally)

Problems and surprises in Verus that affect sail are recorded here as
documentation. Each has a reproduction, its impact on sail, and any
workaround.

**We do not file issues in the Verus repository.** This applies to agents,
bots and automation, and is a project rule (AGENTS.md). If a person decides
to report one upstream, each entry's description is written so it can be
filed as is.

| Id | Title | Status | Reproduction |
|---|---|---|---|
| [VI-0001](VI-0001-nondeterministic-declaration-order.md) | Nondeterministic SMT output from `arrow_*` accessor order | open | `corpus/repros/verus-nondeterministic-declaration-order/` |
| [VI-0002](VI-0002-page-table-rlimit-on-pinned-release.md) | page-table fails one check on the pinned release | open | corpus entry `page-table` |

Z3 behaviour is recorded the same way, as reproductions under
`corpus/repros/` (for example `z3-nla-rlimit-nondeterminism`).

## Adding an entry

Use the next `VI-NNNN` number. Record the Verus version, the date found, a
reproduction (prefer a minimal one under `corpus/repros/`), the impact on
sail, the workaround, and a description written as an upstream report would
be. Update this table.
