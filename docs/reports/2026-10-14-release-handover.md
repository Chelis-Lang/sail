# Sail 0.1.0: handover for review

This note is the starting point for reviewing Sail before its first
open-source release. The candidate is the draft release **`v0.1.0-rc.5`** on
`Chelis-Lang/sail`, built from `main` at `c3687990`. The owner decided that
0.1.0 is cut from rc.5 ([question 29](../open-questions.md)).

## What Sail is now

Sail sits between Verus and the Z3 solver. You run `sail verify` instead of
`cargo verus verify` and get the same verdicts, the same error messages and
the same exit code. Repeated runs are faster because Sail caches solver
answers and keeps proof receipts for unchanged crates. Since rc.3 it has
changed in five ways:

- **Sail runs its own Verus.** Every verification uses Verus built from
  Sail's fork (`Chelis-Lang/verus`, commit `73dea021`) at the pinned release
  `0.2026.10.04.426d8b0`, with Z3 4.16.0. Both release archives bundle it,
  so users install only Sail and a Rust toolchain
  ([ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)). The fork
  fixes stock Verus's nondeterministic output order, which makes cache hits
  more reliable.
- **Managed verification is the default.** Every `sail verify` builds in
  Sail's own Cargo target, keeps a proof receipt per crate, and reuses
  unchanged crates
  ([ADR 0046](../decisions/0046-managed-verification-by-default.md)). Human
  output is still plain `cargo verus verify`'s. `--output json`, MCP, jobs
  and `watch` return a complete per-function report, including crates that
  were reused. Setups whose inputs Sail cannot bind still verify exactly as
  plain Verus, with one note line and no reuse: tests, `--target-dir`, and
  loader variables such as `LD_LIBRARY_PATH`. Nothing that plain Verus
  accepts is refused.
- **vstd is verified once per machine.** The first run on a machine
  verifies vstd. Later new projects and worktrees start from Sail's local
  dependency store
  ([ADR 0045](../decisions/0045-dependency-store.md)): a new demo project
  takes 1.4 s against 39 s for plain Verus.
- **`sail new`** creates a project that verifies as created. Its files are
  those of `cargo verus new`, with the matching vstd.
- **Output schema v2.** Reports record how each verdict was established and
  its evidence tier ([output schema](../output-schema.md)).

The MCP server (`sail mcp`), background jobs, `watch`, the signed team cache
and the experimental proof-repair engine (`sail repair-check`) are unchanged
in scope. See the [README](../../README.md).

## Start here

| To review | Read |
|---|---|
| What a user sees and does | [README](../../README.md) |
| What is faster, and what is not | [Performance](../performance.md), [real-project report](2026-10-15-real-project-performance.md) |
| This release candidate | [Release-candidate validation](../validation/release-candidate.md) |
| Evidence that results match plain Verus | [Deep tier](../validation/deep-tier.md), [M7 gates](../validation/m7.md) |
| How releases are built | [Releasing](../releasing.md) |
| The Verus fork | [`Chelis-Lang/verus` SAIL.md](https://github.com/Chelis-Lang/verus/blob/sail/SAIL.md), [ADR 0039](../decisions/0039-private-verus-fork.md) |
| Known limits and open defects | [Known issues](../known-issues.md) |
| What comes after 0.1.0 | [Roadmap](../ROADMAP.md) |

## What was validated on rc.5

All checks ran on `c3687990` on 2026-10-15 and 2026-10-16. Linux checks ran
on native EC2 hosts; macOS checks ran on the developer Mac
([record](../validation/release-candidate.md)).

- **Same results as plain Verus.** Every conformance tier showed 0
  differences in every arm, including the engine arm: the fast PR subset
  (33 units), fast (147), standard (159 units, 12,726 functions) and deep
  (159). One round of Verus's own 4,695-test suite also showed 0
  differences. Its 4 "unstable" tests are ones where stock Verus prints
  output in hash order (VI-0008).
- **Gates.**
  - Linux: gate (620 tests), solver, all three end-to-end phases, and the
    real Claude Code CLI listing Sail's 15 tools.
  - macOS: `ci/run.sh host` (609 tests) and the managed end-to-end phase.
- **The archives work from scratch.** Both smoke tests pass. Following the
  README in a fresh Ubuntu 24.04 container and in a fresh macOS directory,
  each of these worked:
  - install, then `sail new` and `sail verify`;
  - a second new project (1.3 s on Linux);
  - a workspace member run from its own directory (#157 fixed);
  - a project whose first run fails, then MCP `verify`, the fix and
    `repair-check`;
  - a run with `LD_LIBRARY_PATH` set, identical to plain Verus apart from
    the one note line;
  - `--verify-function`, identical to plain Verus;
  - the reproductions of #119–#121, all of which now reuse.
- **Archives.** Linux: 42.3 MB, sha256 `ea4cfb78…2f5e`. macOS: 35.2 MB,
  sha256 `303e0750…d309`. Each reports `sail 0.1.0-rc.5`.

## Performance, honestly

From [Performance](../performance.md): rc.5, medians of three rounds on an
idle EC2 c7i.4xlarge. The macOS figures there are from an earlier build.

| | Plain Verus | Sail | Speedup |
|---|---:|---:|---:|
| vest edit (catalogue edit) | 37.5–42.1 s | about 28 s | 1.3–1.5× |
| New project after the first | 39.0 s | 1.4 s | |
| New vest worktree | 89.1 s | 33.1 s | |

- **Real projects gain less than vest's catalogue edit.**
  - An edit confined to one module is 1.2–2.3× faster.
  - A change to a widely used spec function is about 1.0×, because Verus
    re-solves each changed module's earlier queries.
  - The [real-project report](2026-10-15-real-project-performance.md)
    ranks the levers. Making a cache miss cost only the changed check
    (#158) is the largest.
- **Small projects and unchanged code cost a little.** Each run takes
  about 0.3 s more than plain Verus, for receipts and complete reports:
  - demo edit: 1.17 s against 0.88 s;
  - unchanged demo: 0.36 s against 0.12 s;
  - unchanged vest: 0.97 s against 0.22 s.
- **The first run on a machine is plain Verus speed or 1–2 s slower.**
- **Repair loops with the engine:** HumanEval 2.2× and Vest 1.8× faster
  than verifying the function and then the crate.
- **Several agents on one daemon run concurrently:** 4 vest agents finish
  2.2× sooner on Linux (measured on an earlier build).

## Known issues (documented, not blocking)

- **New in rc.5:**
  - The repair benchmark's outcome check fails on schema v2, because it
    hashes the renamed `tier` field. Verdicts and diagnostics match. This is
    a harness issue only (#179).
  - Reports say `sail_version` `0.1.0` for rc builds (#180, low).
  - One workspace shape got its own dependency-store entry, so it verified
    vstd once more (low).
- **Setups without reuse.** Tests, benches, examples, `--target-dir`,
  Cargo `--config`, Verus log options and loader variables verify as
  plain, with no reuse. Jobs, `watch` and MCP refuse all but the loader
  variables (#165).
- **Cargo projects only.** Single-file `verus file.rs` projects are not
  supported (#164).
- **macOS freshness.** A Cargo configuration file created and deleted
  within one run isn't noticed ([freshness](../freshness.md)).
- **Open issues:**
  - #76: shared-cache refresh;
  - #77: repair input coverage;
  - #78: Cargo `-F` aliases;
  - #102 and #103: flaky tests;
  - #112: a CI job near its time limit;
  - #124: `deep-rvt` output order;
  - #166, #174 and #177: managed-reuse edge cases.

The full list is in [known issues](../known-issues.md).

## Decisions for you

1. **Publishing.** Rebuild with `scripts/release.sh 0.1.0` from `main` and
   publish it, or ask for changes and a further candidate. The commit
   should be `c3687990` or a later commit that changes only documentation;
   anything else needs a new validation. Moving CHANGELOG's `0.1.0
   (unreleased)` heading to a dated `0.1.0` is part of this step.
2. **Repository visibility.** `Chelis-Lang/sail` is private. Making it
   public is the open-source step. LICENSE, CONTRIBUTING and README are in
   place.
3. **The fork's homepage.** Point `Chelis-Lang/verus` at `Chelis-Lang/sail`
   once Sail is public.
4. **The repair benchmark (#179).** Decide whether its outcome check must be
   fixed before 0.1.0. It affects only the benchmark's acceptance verdict,
   not the product.

## Housekeeping

- The rc.4 draft release is deleted. The rc.5 draft is the only candidate.
- The EC2 Mac dedicated host from the clean-macOS run is released. Its IAM
  role and schedule group are gone.
- Lane L56 tore down every Linux host it started.
