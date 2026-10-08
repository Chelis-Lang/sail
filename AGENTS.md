# AGENTS.md

Operating guide for humans and coding agents working on **sail** (S.A.I.L., the
Satisfiability Analysis & Inference Layer). It is a Rust service and toolchain
that sits between Verus and its pinned Z3 to make SMT-backed verification fast,
stable and agent-friendly, without ever changing a verdict in default mode.
`CLAUDE.md` is a symlink to this file.

`docs/CHARTER.md` is the project charter, and this file must stay consistent
with it. When the charter and a later instruction conflict, ask the owner before
acting. The charter's appendix records corrections made since it was written.

## Ground rules

- **Correctness contract first.** The charter's ten-rule correctness contract
  is not negotiable for speed. In default mode, `sail` must produce the same
  per-function verdicts and the same diagnostics as plain Verus with the pinned
  Z3. The differential harness (`sail-conformance`) is the definition of
  correctness.
- **Never weaken a check.** Never weaken a conformance check, golden, test or
  benchmark baseline to make a change pass. If a check looks wrong, raise it
  with evidence.
- **Fail safe.** Any service, cache or pool misbehaviour must end in either a
  correct answer or a fallback to the pinned Z3 directly. It must never end in a
  wrong answer. The shim's forwarded byte stream must never depend on code that
  can fail.
- **No normalization by default.** Canonicalizing queries needs an ADR and
  whole-corpus validation with the solver as oracle (charter rule 5).
- **Provisional is not verified.** Nothing provisional is ever reported as
  verified. Confirmed results record their provenance.
- **Verdict-changing modes** (for example "accept a proof from any seed") are
  opt-in, named, recorded in `docs/divergences.md`, and cached under keys that
  cannot collide with default-mode keys.
- **Surprising Verus or Z3 behaviour:** capture a minimal reproduction in the
  corpus and document it before working around it. Verus problems are
  tracked in `docs/verus-issues/`.
- **Never file issues, PRs or comments in other projects' repositories**
  (Verus, Z3, corpus projects). Record upstream problems locally
  (`docs/verus-issues/`). Whether to report upstream is a human decision.
- **Decisions** go in numbered ADRs in `docs/decisions/` (copy `0000-template.md`).
  Changing the crate dependency graph is a decision.
- **Solver identity is explicit.** Never assume a single solver in the cache,
  verdict model or query representation.

## Commit messages and authorship

Use plain conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `test:`,
`refactor:`, `perf:`, `ci:`) under the configured human author. Commit messages
and PR bodies must not contain Claude, Codex, Copilot or other AI attribution,
AI co-authorship trailers, "Generated with" lines, or links to AI sessions.

Three layers enforce this:
1. **Local hook.** `.githooks/commit-msg`, installed by `cargo xtask install-hooks`.
   Run this once per clone. It covers every worktree.
2. **CI.** The `No AI authorship markers` job runs `scripts/check-ai-markers.sh`
   over every pushed or PR commit, checking messages, author and committer.
3. **Agent config.** `.claude/settings.json` disables Claude Code's attribution.
   This overrides any default instruction to add `Co-Authored-By` lines.

Never bypass the hook with `--no-verify`. See ADR 0002. The same install adds
a `pre-commit` hook that refuses commits `rustfmt` would change.

## Workflow mode

**Current mode: PR mode.** This mode has applied since 2026-10-08, after M0's
bootstrap pushes (ADR 0002).

In PR mode:
- `main` is protected, and every change lands through a squash-merged PR.
- Five checks are required (see Gates). Admins are not exempt.
- Protection is applied by `cargo xtask protect-main`, which is idempotent.

**Keep `main` green.**
- Run the full local gate (see Gates) before you push.
- Never merge on red.
- If `main` goes red, fixing it takes priority over new work.

**Each PR states** which corpus entries it affects and the conformance results,
plus benchmark deltas for any performance change.

**Parallel agents** work in separate worktrees under `.claude/worktrees/` on
their own branches. The lead reviews and merges.

## Repository layout

```
crates/sail-*        one crate per charter component (see docs/CHARTER.md)
xtask/               cargo xtask: install-hooks, check-layering, protect-main
corpus/              pinned Verus/Z3 toolchain and corpus project pins
bench/               benchmark result data (machine-readable)
docs/                charter, roadmap, ADRs, survey, trust model, divergences,
                     open questions, benchmarks (generated)
scripts/             small shell helpers used by hooks and CI
                     (`cargo xtask fetch-verus` installs the pinned Verus)
.githooks/           tracked git hooks
```

**Layering.** `xtask/src/layering.rs` is the single table of permitted internal
dependency edges, and `cargo xtask check-layering` enforces it in CI. Two
structural rules are tested:
- `sail-shim` must not reach the daemon, cache, solver pool, portfolio or Verus
  crates, so its fallback path stays small;
- the protocol, session, key and cache layers must not depend on `sail-verus`.

## Rust conventions

- **Toolchain.** Pinned exactly in `rust-toolchain.toml` (1.99.0). MSRV is
  `rust-version` in `Cargo.toml`. Bump both together, on purpose. Verus needs
  its own toolchain (pinned in `corpus/toolchain.toml`). That is Verus's
  concern, and `sail` sets it only when invoking Verus.
- **Edition and features.** Edition 2024, stable only, no nightly features.
- **Unsafe code.** `unsafe_code = "forbid"` workspace-wide, and every crate root
  also has `#![forbid(unsafe_code)]`. Any exception needs an ADR with owner
  approval, an isolated module and a written safety argument.
- **Lints.** Configured in `[workspace.lints]`: `clippy::all` and
  `clippy::pedantic` deny, plus `unwrap_used`, `expect_used`, `panic`, `todo`,
  `unimplemented`, `dbg_macro` and `missing_docs`. Document any local `#[allow]`
  with a reason. `clippy.toml` lists domain words for `doc_markdown`.
- **Errors.** No panics on any input. Every user-facing error has a stable code
  of the form `SAILnnnn`. Internal invariant violations are reported as bugs
  with reproduction context, and trigger the fail-safe path.
- **Environment and config.** Every environment variable, config key, crate and
  binary uses the `sail` prefix (`SAIL_*`, `sail-*`).
- **Dependencies.** Add third-party crates to `[workspace.dependencies]` first
  and reference them with `workspace = true`. Every new dependency should earn
  its place, and `sail-shim`'s dependencies especially. `deny.toml` holds the
  licence allowlist and source policy.

## Gates

CI runs every check inside the CI image (ADR 0013), through `ci/run.sh`.
Run the same steps locally in the same image with `ci/local.sh`:

```sh
ci/local.sh gate                  # fmt, clippy, tests, doctests, rustdoc, layering, deny, benchmarks page
ci/local.sh solver                # every test that needs the pinned Z3 (incl. fault injection)
ci/local.sh e2e                   # `sail verify` vs `cargo verus verify`: identical output
ci/local.sh conformance fast pr   # the PR conformance subset, pass-through and cache-on
```

On the host, the fast loop is still `cargo nextest run --workspace` plus the
gate commands in `ci/run.sh`. `cargo xtask fetch-verus` installs the pinned
Verus for host runs.

**Required on every PR** (branch protection): `No AI authorship markers`,
`Workspace gate`, `Solver tests`, `End to end`, `Conformance`.

**Testing tiers** (charter §Engineering practice). CI runs only the fast
tier, and every CI job must finish well inside 10 minutes. Anything longer
is local-only, and `ci/run.sh` refuses to run it in CI (ADR 0013):
- *Fast (CI, every PR):* the five required checks above.
- *Standard (local):* `ci/local.sh conformance fast`, then
  `ci/local.sh conformance standard` (includes anvil). Run it before merging
  changes to the shim, cache, daemon, keys or harness, and put the summary
  lines in the PR.
- *Deep (local):* `ci/local.sh conformance deep`, before releases.
- *Secondary Verus (local):* `SAIL_VERUS_RELEASE=<release>` runs any of the
  above against a secondary pin (`corpus/toolchain.toml`). Record results in
  `docs/compatibility.md`.
- *Benchmarks (local):* `ci/local.sh bench cold|warm|edit TIER ROUNDS`, or
  `sail-bench` on the host. Never part of the gate. Record machine details
  with every result; reference numbers come from Linux x86-64.
- *Fault injection:* `docs/fault-injection.md`, run as part of `solver`.

## Tracking

- **Milestones.** GitHub milestones on `Chelis-Lang/sail` track phases, and
  `docs/ROADMAP.md` is the narrative.
- **Issues.** One issue per deliverable or defect. Every conformance
  disagreement becomes an issue containing a minimal reproduction.
- **Owner questions** live in `docs/open-questions.md`. Add to it rather than
  guessing.

## Documentation style

Docs describe the current state. Use repo-relative paths only: no absolute local
paths and no personal emails. Public APIs carry rustdoc. Record user-visible
changes in `CHANGELOG.md`. `docs/benchmarks.md` is generated from data under
`bench/` and never edited by hand.
