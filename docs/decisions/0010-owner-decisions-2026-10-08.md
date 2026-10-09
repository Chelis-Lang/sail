# 0010: Owner decisions on the bootstrap open questions

- **Status:** accepted (owner decision, 2026-10-08)
- **Date:** 2026-10-08

## Context

The bootstrap surveys raised eight questions for the owner
(`docs/open-questions.md`, items 1–8). Items 7 and 8 are recorded in ADR
0009 and in the update to ADR 0008. The rest are recorded here.

## Decision

1. **License.** Dual MIT OR Apache-2.0, which is the Rust default and
   compatible with Verus and Z3. It is set at the first public release; until
   then the workspace stays `publish = false` with no license field.
2. **Reference platform.** Published benchmarks use Linux x86-64, where CI
   and most Verus users run. macOS arm64 stays a secondary pinned target and
   remains the development machine. Numbers measured on it are labelled as
   such.
3. **Attribution.** Function attribution uses `--log smt-transcript` plus
   structure, because that flag leaves the pipe bytes unchanged. `--log smt`
   is not used in default mode. If sail ever adds flags that change Z3's
   input, the conformance baseline must run plain Verus with the same flags.
4. **Corpus scope.**
   - `rust_verify_test` is included in the deep tier only.
   - Singular is optional, and queries that need it are tagged.
   - Fork pins are allowed if they are pinned by commit and archived locally,
     since forks can vanish.
   - A second Verus release is added as a secondary compatibility target.
5. **Baseline.** The median of 3 runs, with the spread reported alongside it.
   Benchmarks are not held back waiting for an idle machine. The project is
   in R&D on a development machine, so results record their load conditions
   and proceed.
6. **Shared tier.** Entries are scoped by solver binary hash plus platform
   class. Hits across platform classes are allowed only in a named opt-in
   mode.

**Update to ADR 0008 (item 8).** A stack-only key (v1) is approved as a
*named opt-in mode*, with models recomputed on demand. It must be registered
in `docs/divergences.md` when it is built. rlimit consumption stays out of
keys entirely (ADR 0009).

## Consequences

- **Corpus work follows from item 4:**
  - Singular-tagged entries;
  - local archives of fork pins (`<cache>/archive/`, recorded in the
    manifest);
  - a second Verus pin in `corpus/toolchain.toml` with its own
    compatibility column.
- **Published numbers come from Linux.** `docs/benchmarks.md` labels macOS
  results as secondary until there is a Linux baseline, which comes from CI
  or a Linux machine.
- **The trust model** (`docs/trust.md`) gains the platform-class scoping
  rule.

## Update 2026-10-09: the secondary pin is in place

`corpus/toolchain.toml` holds the primary pin plus `[secondary."<release>"]`
pins. The first is `0.2026.09.06.8dea4a2`, the release vest's recent history
was written against, with Z3 4.16.0 and Rust 1.98.0.

- **Selecting it:** setting `SAIL_VERUS_RELEASE=<release>` selects a
  secondary pin for `cargo xtask fetch-verus`, the harness and the
  benchmarks.
- **Corpus validation:** the corpus is validated against the primary pin. A
  selected secondary pin skips that check, and every report records which
  release ran.
- **Where it runs:** secondary runs are local only (ADR 0013).
- **Results:** in `docs/compatibility.md`.
- **No bundled z3:** this release's archive has none (VI-0006). Its pin uses
  the primary's z3 (`z3 = "primary"`, the same 4.16.0 build), and the harness
  then names it for plain Verus too.

## Update 2026-10-09: `rust_verify_test` runs in the deep tier

Item 4's `rust_verify_test` is in place (ADR 0019):
- **Build.** `cargo xtask build-verus-source` builds the pinned commit with
  vargo under `<cache>/verus-src-build/<commit>`, with the pinned z3.
- **Run.** `ci/local.sh deep-rvt` (or `ci/run.sh deep-rvt` on the host)
  compares plain Verus and `sail-z3` test by test. It is local-only.
- **Singular** is still not built: a Singular-enabled source build is
  possible, but using it for the Singular corpus entries needs more than
  the build (ADR 0019).
