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
- **Randomness is the enemy of caching.** Default-mode keys bind the exact
  solver input bytes (ADR 0008), so any run-to-run variation in what Sail or
  Verus emits becomes a cache miss. Treat nondeterminism in Sail or in the
  Verus fork's output (hash-order iteration, run-dependent numbering or
  ordering) as a defect and drive it out: fix Verus's in the fork under ADR
  0039's rules, never by normalizing queries in Sail. Sail runs the fork's
  Verus, never stock Verus; stock is only the conformance oracle.
- **Verdict-changing modes** (for example "accept a proof from any seed") are
  opt-in, named, recorded in `docs/divergences.md`, and cached under keys that
  cannot collide with default-mode keys.
- **Surprising Verus or Z3 behaviour:** capture a minimal reproduction in the
  corpus and document it before working around it. Verus problems are
  tracked in `docs/verus-issues/`.
- **Where agents may file.** Agents may open issues, PRs and comments only in
  this line of work's repositories: `Chelis-Lang/sail`, the Verus fork
  `Chelis-Lang/verus`, `Chelis-Lang/dive` (D.I.V.E.) and Oyster's repository
  under `Chelis-Lang` once it exists. Never in upstream Verus, Z3, corpus
  projects or any other repository.
  Record upstream problems locally (`docs/verus-issues/`). Whether to report
  upstream is a human decision.
- **Nothing is published until the owner's rollout.** This work is intended
  for open source but stays staged: nothing is open-sourced or published to
  crates.io until the owner orchestrates the rollout. The Verus fork is the
  only public repository in this line of work. The workspace keeps
  `publish = false` (ADR 0010).
- **Fork a backend only for a proven gain.** Forking a backend or tool (as
  ADR 0039 forks Verus) needs a rigorously measured gain, proven before any
  Sail use of the fork ships.
- **Decisions** go in numbered ADRs in `docs/decisions/` (copy `0000-template.md`).
  Changing the crate dependency graph is a decision.
- **Solver identity is explicit.** Never assume a single solver in the cache,
  verdict model or query representation.
- **macOS and Linux both.** Code must build, lint and test on macOS and
  Linux. Linux-only code is `cfg`-gated. CI runs only Linux, so run
  `ci/run.sh host` on a Mac before pushing changes that touch
  platform-specific code.

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
- Six checks are required (see Gates). Admins are not exempt.
- Protection is applied by `cargo xtask protect-main`, which is idempotent.

**Keep `main` green.**
- Run the full local gate (see Gates) before you push.
- Never merge on red.
- If `main` goes red, fixing it takes priority over new work.

**Each PR states** which corpus entries it affects and the conformance results,
plus benchmark deltas for any performance change.

**Parallel agents** work in separate worktrees under `.claude/worktrees/` on
their own branches. The lead reviews and merges.

**Native Linux hosts.** Linux validation, gates and all performance work run
on native x86-64 EC2 hosts (ADR 0038). Use `scripts/perf-host.sh up LANE`,
`sync`, `run` and `down`, one host per lane so builds and benchmarks never
contend. `ci/local.sh` builds the CI image natively there. Hosts are tagged
`Project=sail`, terminate themselves after `SAIL_PERF_TTL_MIN` minutes, and
are torn down after use; `scripts/perf-host.sh list` must be empty at the end of a session.
The local emulated container is for quick correctness checks only; never
measure performance under emulation. Clean macOS numbers come from an EC2 Mac
host (`scripts/perf-host-mac.sh`), which needs the owner's approval: Apple's
licence bills a Mac dedicated host for at least 24 hours, so `up` schedules
its release on the AWS side and `down` terminates the instance at once.

**Workspace hygiene.** Remove worktrees once their branch is merged or
archived, after checking for uncommitted source. Discard regenerable build
intermediates freely. Cleanup is routine and is not recorded in git.

## Repository layout

```
crates/sail-*        one crate per charter component (see docs/CHARTER.md)
xtask/               cargo xtask: install-hooks, check-layering, protect-main,
                     fetch-verus, build-verus-source
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
ci/local.sh e2e [PHASE]           # `sail verify` vs `cargo verus verify`: identical output (all phases + extra)
ci/local.sh conformance fast pr   # the PR conformance subset: pass-through, cache-on, engine vs plain
ci/run.sh host                    # on the host (macOS or Linux): clippy and non-solver tests
```

The image has two packaging targets from one Dockerfile: `core` retains every
pinned toolchain and verifier/runtime; `mcp` adds the pinned real MCP client.
CI's managed e2e phase and local `e2e managed`/full `e2e` use `mcp`; other
phases use `core`. `ci/local.sh` selects the matching target automatically.
Direct Dockerfile builds still default to the complete `mcp` target. The
[startup record](docs/validation/ci-image-startup.md) distinguishes the
packaging candidate from measured CI acceptance.

On the host, the fast loop is still `cargo nextest run --workspace` plus the
gate commands in `ci/run.sh`. `cargo xtask fetch-verus` installs the pinned
Verus for host runs.

CI serves rapid iteration: it catches regressions quickly and cheaply, and each
job should finish in about four minutes. Acceptance-, evidence- and
release-style checks belong in the local tiers below, never in CI.

**Required on every PR** (branch protection): `No AI authorship markers`,
`Workspace gate`, `Solver tests`, `End to end`, `Conformance`, `CI completion`.
The completion guard runs even when another job fails and requires every
gate, including image creation, to have succeeded. Branches must be current
with `main` before merging. The `End to end` check aggregates nine parallel
phases and succeeds only when all complete successfully:
- `legacy`: `sail new`, then `sail verify` vs `cargo verus verify` cold and
  warm, `--output json`, one MCP `verify` call and the portfolio;
- `output`: the edit loop (combined output, passing, unchanged, compile error,
  partial verification);
- `flags`: the setups verified as plain Cargo-Verus verifies them;
- `loader`: a loader variable (no proof reuse, one note);
- `managed`: managed receipts, failing-first reuse, prune, durable jobs, MCP
  and Claude Code's tool listing;
- `managed-copies`: two fresh member workspaces establish their own vstd and
  requested-target proofs, then reuse only their own receipts;
- `managed-native`: fresh plain output and an actual native compiler replacement
  at unchanged `PATH`, rejected by both stock and Sail;
- `scripts`: managed reuse across build scripts and generated includes;
- `repair`: the repair CLI/MCP against the prebuilt engine.

CI builds the release binaries once per run (`End to end (build)`,
`ci/run.sh e2e-build`) and every phase job unpacks them; a failed build
skips the phases, and a skipped phase fails the aggregate. `ci/local.sh e2e`
builds, then runs every phase (all managed cases on one project) plus `extra` (`-- --output-json --time` and a
`-p`/`--features` workspace, four cold builds, local-only);
`ci/local.sh e2e PHASE` runs one phase (or `extra`) for diagnosis. No phase
may be skipped to authorize a merge.
Evidence directories and conformance reports are uploaded only when a job
fails (managed phases also request upload on cancellation). A hard job timeout
can prevent the upload step from running. See [testing](docs/testing.md) for
phase entry points and timeout diagnosis.
The optional repair engine is a locally qualified prebuilt runtime pinned in
`engine/verus/artifact-linux.json`. The CI image validates its archive, source
patch, source lock and every runtime file; CI never compiles Verus from source.
The runtime is a public release asset of the Verus fork (`Chelis-Lang/verus`),
so CI, `ci/local.sh` and native hosts build the image with no token or secret.

**Testing tiers** (charter §Engineering practice). CI runs only the fast
tier, and every CI job must finish well inside 10 minutes. Anything longer
is local-only, and `ci/run.sh` refuses to run it in CI (ADR 0013):
- *Fast (CI, every PR):* the six required checks above.
- *Standard (local):* `ci/local.sh conformance fast`, then
  `ci/local.sh conformance standard` (includes anvil). Run it before merging
  changes to the shim, cache, daemon, keys or harness, and put the summary
  lines in the PR.
- *macOS (local):* `ci/run.sh host` on a Mac: clippy and the non-solver tests.
  Run it before pushing changes that touch platform-specific code.
- *Deep (local):* `ci/local.sh conformance deep`, before releases. Also
  `ci/local.sh deep-rvt [cache] [ARGS...]`: Verus's own `rust_verify_test`
  suite, the pinned fork with and without `sail-z3`, with exact output checks,
  then the original stock-with-and-without-shim control (ADR 0042's
  complete diagnostic messages). Corpus conformance separately compares stock
  against the fork. Nonignored fork libtests must pass on both sides. The step
  builds isolated fork-test and stock
  source variants first (`cargo xtask build-verus-source --fork` and without
  `--fork`). Fork-only regression tests run in the exact fork arm.
  `cache` adds rounds with the cache daemon; ARGS such as `--filter modes::`
  or `--every 10` select a subset. It runs on the host too (`ci/run.sh
  deep-rvt`). See ADR 0019.
- *Secondary Verus (local):* `SAIL_VERUS_RELEASE=<release>` runs any of the
  above against a secondary pin (`corpus/toolchain.toml`). Record results in
  `docs/compatibility.md`.
- *Singular (optional, local):* `ci/local-singular.sh` builds the pinned source
  with Singular and checks tagged deep-tier projects (ADR 0021). It is never a
  required CI job. `SAIL_VERUS_SOURCE_BUILD` explicitly selects a validated
  source variant; binary releases do not support Singular.
- *Benchmarks (native host):* `ci/local.sh bench cold|warm|edit TIER ROUNDS`, or
  `sail-bench`, on a `scripts/perf-host.sh` host. Never part of the gate.
  Record the instance type and region with every result. A benchmark records
  every sample, including protocol refusals, and continues; gates are judged
  when the report is generated and a refused sample counts as a failure,
  never as a dropped sample (ADR 0038).
- *Fault injection:* `docs/fault-injection.md`, run as part of `solver`.
- *Compiler-boundary witnesses (local):* the owning bounded acceptance runner
  invokes `ci/run.sh boundary-witness admission|shapes` with frozen actual
  compiler inputs. These two ignored witness entrypoints are not pinned-Z3
  solver fixtures; their missing-input checks remain strict and their results
  are recorded separately from the solver suite.
- *Determinism (local, before re-pinning the fork):* `scripts/verus-determinism.py
  --runs 3` over the fast tier with the new runtime; see
  `docs/validation/determinism.md`.

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

Documentation is part of implementation and every agent handoff:
- Update the relevant user guide with commands, setup, observable behavior,
  errors and material limitations when a feature changes. Link new guides from
  `docs/README.md` and the main README where users need them.
- Keep internal design and `docs/ROADMAP.md` current. Distinguish implemented,
  built, qualified, selected and accepted; record remaining gates and dependencies.
- Record surprising upstream behavior with a minimal reproduction and local
  issue entry. Keep Sail defects and unresolved validation failures visible in
  `docs/known-issues.md`; do not attribute a cause without evidence.
- Validation records identify source/runtime/environment, actual scope, result
  and limits. Never describe static review, a fixture pass or a candidate
  build as real-project acceptance.
- **Evidence policy (ADR 0038).** One concise Markdown record per acceptance
  gate (`docs/validation/`), stating current status and the best evidence.
  Failed and interrupted attempts stay visible as one indexed line each
  (commit, date, result, cause or "unknown"). Raw logs and JSON dumps stay
  outside the repository, cited by sha256. Evidence lands in the PR it
  qualifies: no evidence-only commits, and no records of cleanup, retirement,
  ownership or disk hygiene. `docs/ROADMAP.md` shows current state as a short
  gate table, never as a log. Reports and evidence are Markdown in this
  repository.
- Each agent handoff and PR lists the documentation changed or explains why the
  change has no documentation impact. The lead reviews docs alongside code.
