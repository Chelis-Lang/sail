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
  corpus and document it before working around it.
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
- Two checks are required: `No AI authorship markers` and `Workspace gate`.
  Admins are not exempt.
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

Run the gate locally before pushing. This is exactly what CI's `Workspace gate` runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked          # or: cargo test --workspace
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo deny --locked check
```

**Testing tiers** (charter §Engineering practice):
- *Fast:* the gate above. It runs on every push and PR, and gains a small
  conformance subset once the harness exists (M3).
- *Standard:* conformance over the main corpus in default mode, plus cache tier
  and shim fallback tests. This tier gates merges once it exists.
  `.github/workflows/conformance.yml` runs a fast-tier subset on PRs and the
  whole fast tier nightly; dispatch it with `tier=standard|deep` (ADR 0006).

Run conformance locally (needs `cargo xtask fetch-verus` once):

```sh
cargo build --release -p sail-shim -p sail-conformance
./target/release/sail-conformance run --tier fast --jobs 4
```

The standard tier adds the external `cargo verus` and `verus` projects
(`--tier standard`). It also needs Verus's Rust toolchain
(`rust_toolchain` in `corpus/toolchain.toml`) installed with rustup, and
network access for the projects' crates.io dependencies. Projects whose
`requires` are unavailable are skipped and listed in the summary line.
- *Deep:* full corpus, history replay, portfolio and fragility runs, and full
  benchmarks. Scheduled, and run before releases.
- *Not in the fast gate:* benchmarks, profiling and long fuzzing. Record
  machine details with every benchmark result.

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
