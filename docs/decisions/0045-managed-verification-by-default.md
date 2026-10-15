# 0045: Managed verification is the only `sail verify`

- **Status:** accepted (owner decision 2026-10-15)
- **Date:** 2026-10-15
- **Amends:** [ADR 0023](0023-managed-incremental-workspace-verification.md)
  ("The default CLI … unchanged"; "ordinary verification remains
  available") and [ADR 0014](0014-attribution-ledger-and-output-schema.md) (what
  `sail verify --output json` prints)

## Context

- **Two paths.** `sail verify` ran a thin pass-through: `cargo verus verify`
  in the user's target with `sail-z3` in front of Z3, and nothing else. The
  managed runner (ADR 0023, ADR 0036, ADR 0041: Sail's own Cargo target,
  proof receipts that skip unchanged targets, complete per-target reports,
  durable jobs) ran only with `--incremental`, `--background`, jobs, `watch`,
  MCP and repairs.
- **Owner direction (2026-10-15).** "Why would we allow skipping past our
  core infra to run cargo and be knowingly slower? What is that for?" Then:
  "if its tech debt, id just scrub it out as a legacy config and default to
  fast by default. seems otherwise actively harmful to have a suboptimal
  default."
- **What managed printed.** Measured on a `cargo verus new` project with one
  passing and one failing function (macOS, Sail's Verus `73dea021`), managed
  verification differed from plain `cargo verus verify` on stdout in every
  run. It printed its own `Current evidence: …` line instead of Verus's
  `verification results:: 1 verified, 1 errors`, because the driver always
  asks Verus for `--output-json --time` and kept that document. A user's own
  `-- --output-json` or `-- --time` failed outright ("Option 'time' given
  more than once"). Stderr and exit codes were already Cargo's and Verus's.
- **What managed cost.** See [Measurements](#measurements).

## Decision

1. **One path.** Every `sail verify` runs the managed runner. The
   pass-through mode is removed from the product: no flag, environment
   variable or fallback runs it. `--incremental` is accepted, does nothing,
   and is documented as deprecated, so existing scripts keep working.
   `--background`, `--engine` and `--output json` keep their meaning. Jobs,
   `watch`, MCP and repairs already ran this path.
2. **Rule 1 output.** Without `--output json`, `sail verify` prints what
   plain `cargo verus verify` prints, from the same work:
   - stderr: Cargo's status lines and Verus's diagnostics, relayed as they
     arrive (unchanged);
   - stdout: Verus's own stdout. The managed driver passes `--output-json`
     and `--time` only when the user did not, and rebuilds the user's stdout
     from the document: the user's own `--output-json` document (without
     `times-ms` unless they asked for `--time`), or Verus's
     `verification results::` line under exactly the conditions Verus
     prints it (`rust_verify/src/verifier.rs`, `finish_verus`), followed by
     Verus's `--time` report when asked for. Cargo forwards that stdout; the
     runner relays every line that is not one of Cargo's JSON messages;
   - the exit code: Cargo-Verus's.

   Sail prints nothing of its own on stdout or stderr, apart from the
   existing one-line `warning[SAIL0003]` when the cache daemon is unavailable
   and errors that stop a run. `--output json` prints the
   `sail-project-result` envelope (a schema-v2 report per target) instead
   of a single schema-v2 report.
3. **What differs, and why it is not a rule 1 difference.** Sail's target is
   its own, and freshness there follows content and arguments:
   - the first run in a project, worktree or new argument set compiles the
     dependencies and verifies vstd in Sail's target, so it prints their
     `Compiling` lines and vstd's summary even when `target/` is current;
   - plain Cargo-Verus treats a crate as fresh when only its Verus arguments
     changed and prints nothing; Sail verifies again;
   - a target with a receipt but whose artifact Cargo found stale, or the
     reverse, is verified again (a retry runs Cargo twice, so its lines
     appear twice).

   In each case Sail's output is exactly what plain `cargo verus verify`
   prints for the same verification work on the same target state.
   Verdicts and diagnostics are never reused from a failed or incomplete
   run: such targets get no receipt and are verified again, as Cargo does.
4. **Unsupported setups fail; they do not fall back.** Managed verification
   owns the target and Verus's logs and binds every input it reuses. It
   refuses `--target-dir`, Verus's `--log` options, `--no-verify`, tests,
   benches and examples, `--sysroot`, a chained `RUSTC_WORKSPACE_WRAPPER`,
   Cargo `--config` and configuration it cannot identify (`SAIL3100`,
   `SAIL3112`), and points at `cargo verus verify`. A silent fallback would
   be the removed pass-through under another name.
5. **Explicit experiments stay explicit.** `--measure-fragility` and the
   verdict-changing `--portfolio accept-first-proof` (with its single-file
   `--verus-source`) still run Cargo-Verus with `sail-z3` directly and print
   one schema-v2 report; they are measurements, not a verification mode, and
   refuse `--background` and `--engine` (`SAIL0110`).
6. **Conformance covers the default.** `sail-conformance run` compares, on
   its Sail side, Verus run as `sail verify` runs it: Sail's Verus, `sail-z3`
   and the logging flags the managed driver adds
   (`adapter::OBSERVATION_LOG_FLAGS`, one constant for both), with no daemon,
   then with the cache filling and warm (`ci/run.sh conformance`). The bare
   shim stays available as `--sail-arm shim`, test infrastructure for
   telling a logging-flag difference from anything else, like
   `--sail-verus stock` for the fork (ADR 0043). The corpus's fast tier is
   single-file Verus, which `sail verify` does not run, so the CLI itself is
   compared with plain `cargo verus verify` byte for byte by the CI
   `End to end` legacy phase (cold and warm, then `--output json` and MCP).

## Measurements

TBD

## Consequences

- **One product path.** Every user and agent gets receipts, complete
  reports, durable jobs and the cache; there is no slower mode to fall into.
- **Drop-in, with limits.** Projects that verify tests, benches or examples,
  or use `--target-dir`, Verus log options or Cargo `--config`, must run
  `cargo verus verify` without Sail until managed verification supports
  them. Charter appendix entry 6 records this against success criterion 3.
- **Disk.** Each project and argument set has its own target under the Sail
  cache (several GB for a mid-sized project); `sail cache projects` prunes
  them ([disk use](../workspace-hygiene.md)).
- **Output format.** `sail verify --output json` consumers get the
  project-result envelope; the requested targets' reports are the old
  documents. MCP and jobs already used it.
- **Remaining costs** are listed under Measurements, with numbers; L47's
  pre-verified vstd addresses the first-run cost.
