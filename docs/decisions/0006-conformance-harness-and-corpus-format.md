# 0006: Conformance harness and corpus manifest

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Charter rule 1 asks for the same per-function verdicts and the same
diagnostics as plain Verus, with the differential harness enforcing this on
the whole corpus. The corpus survey proposed a manifest format compatible
with verita (`docs/survey/corpus.md` §4).

The Verus-internals survey found:
- `--output-json --time` gives per-function `success`, rlimit and time;
- `--no-report-long-running` removes timing-dependent diagnostics (§6, §8).

## Decision

- **Manifest.** `corpus/corpus.toml`, schema v1, is the survey's draft
  restricted to what the runner supports today:
  - `build = "verus"`, with `crate_roots` or `crate_roots_dir`, `exclude`,
    `extra_verus_args` (with `{out}` substitution), `env`, `requires`,
    `expected` and `expected_from`;
  - unknown fields are rejected;
  - `cargo-verus` and `script` builds are parsed but refused by the runner
    until they are implemented;
  - projects are fetched shallowly at their pinned SHA into
    `<cache>/corpus/<repo>-<sha12>`;
  - the manifest records the Verus release it was validated against, and the
    harness refuses to run if that does not match `corpus/toolchain.toml`.
- **A unit is one Verus invocation.** Both sides run with identical
  arguments, plus `--output-json --time --no-report-long-running`. Plain
  Verus runs with `VERUS_Z3_PATH` cleared, so the launcher uses its bundled
  Z3. The sail side sets `VERUS_Z3_PATH=sail-z3` and
  `SAIL_Z3_PATH=<pinned z3>`.
- **Comparison.** A unit is `same` only if all of these are identical:
  - exit code;
  - stderr, byte for byte;
  - the `verification-results` and `func-details` JSON;
  - the set of functions and each function's `success`.

  No normalization is applied. Per-function rlimit differences are reported
  as *drift*, not as differences, because Z3 itself produces them
  (`corpus/repros/z3-nla-rlimit-nondeterminism/`). Harness failures are
  `infra-error`, never a conformance verdict.
- **Reports.** `sail-conformance run` writes `report.json` (schema v1, with
  machine details) under `<cache>/conformance/<timestamp>/`. It exits 1 on
  any difference and 2 on infrastructure errors.
- **CI.** `.github/workflows/conformance.yml` runs on Linux:
  - PRs: a small subset of the fast tier;
  - nightly: the whole fast tier;
  - manual dispatch: any tier.

  It is not yet a required check; it becomes one once it has been stable for
  a week.

## Consequences

- **Per-function verdicts come from Verus's own breakdown.** The survey notes
  that `success` misses bit-vector-only failures, but stderr equality covers
  them.
- **Comparing stderr byte for byte is strict.** Any timing-dependent
  diagnostic that survives `--no-report-long-running` will show up as a
  difference to investigate, not to hide.
- `cargo verus` projects (the standard tier) need runner support. That is the
  next corpus step.
