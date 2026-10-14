# Roadmap review, 2026-10-13

This review covers what to do after the M7 rescue, judged by what a user would
notice. It draws on the
[new-user walkthrough](2026-10-13-new-user-walkthrough.md), the rescue lane
reports under [rescue/](rescue/), and the
[deep-tier validation](../validation/deep-tier.md).

## Why someone would use Sail

An agent or developer editing Verus code wants two things:

- the same verdicts and errors as `cargo verus verify`;
- getting them much faster on every loop after the first, from the CLI, an
  editor watch, or an agent over MCP.

Sail matters only if both hold for that person on their machine. Trust is the
non-negotiable half. Speed is the reason to install it at all.

## Where we are

**Trust is in good shape.** These checks found 0 differences from plain Verus:

- the fast, standard and deep conformance tiers;
- Verus's own 4,695-test suite;
- all 24 walkthrough comparisons.

The opt-in engine also matches plain Verus. Where the engine lacks a Verus
feature (Singular), Sail falls back to stock Verus.

**Speed is real but narrower than the README implies.**

- **Where it helps:**
  - edit loops on solver-heavy projects: vest 1.6–1.8×;
  - repair loops: about 2.1× on HumanEval and 1.8× on Vest against M6;
  - unchanged re-verification on Linux: 0.3–1.4 s;
  - agents on separate workspaces: four finish 2.7× sooner;
  - team-cache hits.
- **Where it doesn't:** small projects, cold runs, and the Rust/Verus frontend,
  which sets a floor of 24–30 s per vest edit even when every solver reply is
  cached.

**A new user mostly can't get to the good part.** The walkthrough found:

- MCP is broken in Claude Code;
- MCP replies run to 4–16 MB;
- macOS never reuses work on the managed path;
- installing Verus takes about seven undocumented steps;
- the repository is private, and the repair engine needed private access (the fork is now public).

## Opportunities, ranked by user value

The cost scale is S (a day or less), M (a few days) and L (a week or more).

### A. Make it usable. Do these first.

These block the core use case or mislead the user. None of them needs research.

| # | What | Value to a user | Cost |
|---|---|---|---|
| A1 | MCP works in Claude Code. Send `ttlMs` and `cacheScope` on 2026-07-28 list results, or stop advertising that revision. Add a real-client smoke test to e2e. | Agents can use Sail at all | S |
| A2 | Agent-sized MCP replies. Return only the requested targets, send the JSON once, put a short failures-first summary at the top, and keep source-level counterexample variables while dropping `fuel%vstd` noise. Each evidence tool's summary states its actual result. | Replies fit in an agent's context, and the useful part comes first | M |
| A3 | Honest counts. A failing vest edit reads "0 verified, 1 failed, 3127 unknown" where Verus says "3237 verified, 1 error". Report functions that are unattributed but not failing as such, and explain the counting unit. | Agents don't chase 3,000 phantom problems | S–M |
| A4 | macOS managed reuse. Every `--incremental`, MCP or watch run on macOS re-verifies vstd (26–69 s). Find out why, and fix it or document plain `sail verify` as the macOS path until it is fixed. | Mac developers, and agents on laptops, get the speed | M (unknown cause) |
| A5 | Install and first run. A user README covering install, verify, MCP and limits. `fetch-verus` installs the toolchain it names and prints the `PATH` line. Prerequisites are listed. Validation status moves out of the user guides. | Zero to first result without guessing | S–M |
| A6 | Papercuts. Warn when the daemon can't start (the long socket path silently disables caching for 5.5 s per run), and shorten the socket name. `--help` works everywhere. `sail jobs list` shows the real currentness. `repair-check` without an engine reports `SAIL3120` up front. Fix the doc errors table from the walkthrough. | Fewer silent degradations and dead ends | S |

### B. Sharpen the value. Do these next, in this order.

| # | What | Value to a user | Cost |
|---|---|---|---|
| B1 | Reuse a verified vstd across projects and worktrees for the same toolchain. Agents that open many worktrees pay about 30 s per new one today. Needs its own ADR and provenance label (ADR 0040 notes). | The first-contact cost drops from about 30 s to about 1 s per new worktree | M |
| B2 | Single-file `verus file.rs` projects: human-eval-verus style, examples, teaching, agent benchmarks. Today they fail with "could not find Cargo.toml". At minimum, document the Cargo requirement. | Covers how many people and agent benchmarks use Verus | M |
| B3 | Disk cleanup for managed targets (1.7–2.7 GB after a few projects): `sail cache projects list\|prune`. | Sail doesn't quietly eat disks | S |
| B4 | Keep receipts for more than the latest revision, so reverting reuses work. | Common undo/redo agent pattern is instant | S |

### C. Decisions (owner, 2026-10-14)

- **Distribution.** Sail stays private for now. The owner will review the work
  internally and then prepare an open-source release. Users will eventually
  download macOS and Linux binaries from GitHub releases. A5's install docs
  should therefore target release binaries, with source builds as the
  contributor path.
- **Repair engine.** It ships with Sail. Release binaries include prebuilt
  engine runtimes for Linux and macOS.
- **Verus fork.** `Chelis-Lang/verus` is now public, so building the engine no
  longer needs organisation access.
- **Running default `sail verify` on the fork** (pending since 2026-10-12): not
  now. B1 and A4 are bigger wins. Revisit with the release.

### D. Defer until there are users asking

| Item | Why defer |
|---|---|
| Warm Verus frontend (M8 bucket memo) | Measured: unchanged-bucket work is 9–12% of an edit cycle (L18). The frontend floor is rustc and VIR, which needs deep incremental work in Verus. Real, but expensive, and the right shape depends on user data. |
| Fail-fast streaming of per-function results | Useful for large crates. Do it after A2, because the reply shape comes first. |
| Stability and fragility modes, portfolios | They exist as opt-in tools. No evidence of demand yet. |
| More SMT backends, certificates, other verifiers | No user pull. They add trust surface. |

### E. Closed

- Rust pre-solver: 0.1% of solver time (L6).
- Warm solver sessions: about 0.1 s per cycle (L11, ADR 0040).
- Prepared-lowering reuse: removed (L7a).

## Proposed sequence

1. Ship A1–A6 as small PRs, with A1, A3 and A6 first because they are cheap. Then
   repeat the walkthrough on Claude Code and a clean Mac.
2. Add a release workflow that builds macOS and Linux binaries of Sail and the engine runtime as GitHub release assets (kept private until the open-source release).
3. Then B1–B4, judged against the repeated walkthrough.
4. Revisit section D only with feedback from real users.
