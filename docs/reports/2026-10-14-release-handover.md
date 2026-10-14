# Sail 0.1.0: handover for review

This note is the starting point for reviewing Sail before its first open-source
release. The candidate is the draft release **`v0.1.0-rc.2`** on
`Chelis-Lang/sail`, built from `main` at `8f427e81`.

## What Sail is, in one paragraph

Sail sits between Verus and its pinned Z3 solver. You run `sail verify`
instead of `cargo verus verify` and get the same verdicts and the same error
messages, faster on repeated runs, because Sail caches solver answers. It also
serves agents over MCP (`sail mcp`), watches a workspace (`sail watch`), and
runs proof repairs through an optional engine built from Sail's fork of Verus.
See the [README](../../README.md).

## Start here

| To review | Read |
|---|---|
| What a user sees and does | [README](../../README.md) |
| What is faster, and what is not | [Performance](../performance.md) |
| Evidence that results match plain Verus | [Deep tier](../validation/deep-tier.md), [M7 gates](../validation/m7.md) |
| This release candidate | [Release-candidate validation](../validation/release-candidate.md), [clean macOS](../validation/clean-macos.md), [macOS engine](../validation/macos-engine.md) |
| How releases are built | [Releasing](../releasing.md) |
| The Verus fork | [`Chelis-Lang/verus` SAIL.md](https://github.com/Chelis-Lang/verus/blob/sail/SAIL.md), [ADR 0039](../decisions/0039-private-verus-fork.md) |
| Known limits and open defects | [Known issues](../known-issues.md) |
| What comes after 0.1.0 | [Roadmap review](2026-10-13-roadmap-review.md) |

## What was validated

- **Same results as plain Verus.** Every conformance tier (fast, standard,
  deep) showed 0 differences in every arm, including the engine. So did
  Verus's own 4,695-test suite run through Sail, with and without the cache.
  Its few "unstable" results are tests where plain Verus disagrees with
  itself. Projects that need Verus's Singular feature fall back to stock
  Verus in the engine.
- **The release archives install and work from scratch:**
  - **Linux:** on a fresh Linux host;
  - **macOS:** on a clean EC2 Apple M4, following the README;
  - **On both:** `sail verify` matched plain Verus at every step, the bundled
    engine was found with no setup, and a repair was accepted.
- **Agents.** The real Claude Code CLI fixed a failing proof through
  `sail mcp` in six turns, with replies of a few KB.
- **`v0.1.0-rc.2`** ([record](../validation/release-candidate.md)):
  - **Checks:** gate (565 tests), solver, all end-to-end phases, the PR
    conformance subset and the full standard tier (159/159 units, 12,726
    functions). Every check passed with 0 differences in every arm.
  - **Smoke tests:** both archives report `sail 0.1.0-rc.2` and pass.
  - **Install:** a fresh Ubuntu install from the README worked end to end,
    with no Sail checkout. The README's prerequisite order was fixed after
    the build (docs only, #114).
  - **The three fixes since rc.1 hold:**
    - after a Rust compile error, vest takes 7.1 s, not 37.9 s, against
      5.6 s for plain Verus;
    - comparing after a fix reports "1 recovered" in 2.2 KB, not 1.26 MB;
    - `sail --version` reports the exact build.
  - **Archives:** Linux 42.0 MB (sha256 `0b343f6f…933e`), macOS 35.0 MB
    (`b6ccb419…7ff8`).

## Performance, honestly

From [Performance](../performance.md). Medians of three rounds on idle EC2 hosts.

- **Edit loops on a solver-heavy project** (vest): 1.3–1.5× faster on Linux,
  1.5–1.6× on macOS.
- **Repair loops with the engine:** 2.1× (HumanEval) and 1.8× (Vest) faster
  than verifying the function and then the crate.
- **Unchanged code:** an agent gets a complete report in about 0.3–1.8 s.
  This is not faster than Cargo's own no-op; its value is the report.
- **Several agents on one daemon** run concurrently: 4 vest agents finish
  2.2× sooner on Linux.
- **No gain** on small projects or on a project's first (cold) run. Edits
  on large crates still pay the Rust and Verus frontend, about 24 s per vest
  edit on Linux.

## Known limitations (documented, not blocking)

- **Cargo projects only.** Single-file `verus file.rs` projects aren't
  supported.
- **vstd on first contact.** A new project's or worktree's first run
  verifies vstd, as plain Verus does (about 30 s).
- **macOS freshness.** A Cargo configuration file created and deleted
  within a single run isn't noticed ([freshness](../freshness.md)).
- **macOS engine build.** Building the engine from source on macOS is not
  bit-identical to the pinned runtime. The release ships the pinned,
  qualified runtime.
- **Summary wording after failures (low).** A failing managed summary leads
  with `0 verified` before `… passed by Verus's totals`. After a Rust compile
  error it reads `0 verified, 0 failed, 0 unknown`. Verdicts and errors are
  correct; only the count line is unclear.
- **Open issues:**
  - #76: shared-cache refresh;
  - #77: repair input coverage after a warmup;
  - #78: Cargo `-F` aliases;
  - #102 and #103: flaky tests;
  - #112: a CI job close to its time limit on cold builds.

## Decisions for you

1. **Repository visibility.** `Chelis-Lang/sail` is private. Making it public
   is the open-source step. Everything else (LICENSE, CONTRIBUTING,
   README) is in place.
2. **Publishing.** Publish the draft `v0.1.0-rc.2` as `v0.1.0` (rebuilding
   with `scripts/release.sh 0.1.0` from the same commit), or ask for changes
   and a further candidate. Moving CHANGELOG's `0.1.0 (unreleased)` heading to
   a dated `0.1.0` is part of that step.
3. **The fork's homepage.** Point `Chelis-Lang/verus` at `Chelis-Lang/sail`
   once Sail is public.

## Housekeeping after this note

- The EC2 Mac dedicated host releases itself at 2026-10-15 15:03 UTC (24-hour
  licensing minimum). After that, delete its IAM role and schedule group:

  ```
  aws ec2 describe-hosts --region us-east-1 --host-ids h-067e52c2554bc9356 --query 'Hosts[0].State'
  aws iam delete-role-policy --role-name sail-l39-mac-release --policy-name release-host
  aws iam delete-role --role-name sail-l39-mac-release
  aws scheduler delete-schedule-group --region us-east-1 --name sail-l39-mac
  ```

- No Linux hosts are left running (`scripts/perf-host.sh list` is empty).
