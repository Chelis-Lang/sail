# 0043: Default mode runs Sail's Verus, built from the fork

- **Status:** accepted (owner direction 2026-10-15)
- **Date:** 2026-10-15
- **Supersedes:** the *Scope* line of [ADR 0039](0039-private-verus-fork.md)
  ("The fork serves the opt-in engine only")
- **Amends:** charter goal 3 ("no changes to their … Verus installation") and
  the reading of correctness rule 1 (charter appendix entry 4)

## Context

- **Owner direction (2026-10-15):** "you gotta get that randomness out.
  That's part of the point of the fork. You can't run stock verus." The
  owner made this a standing principle for Sail and the fork: *randomness is
  always the enemy of caching and should be driven out as needed.*
- **Stock Verus is not deterministic.** It emits SMT in run-dependent orders
  for identical input: declaration order (VI-0001), automatic trigger choice
  (VI-0015), and output orders (VI-0007, VI-0008, VI-0012). Default-mode
  keys bind the exact process prefix ([ADR 0008](0008-default-mode-keys-bind-the-process-prefix.md)),
  so every run-dependent byte is a miss. In VI-0001's measurements, ten runs
  of identical input gave two distinct SMT streams for HumanEval's
  `human_eval_160` and four and two for Vest's `cbor::value` and
  `asn1::integer` without the fork's fixes, and one stream each with them;
  on the pinned `13a1176c` runtime no measured bucket's SMT digest varied
  (0 of 28 HumanEval and 0 of 14 Vest samples, rescue L22).
- **Default mode ran stock.** `sail verify` ran whatever `cargo-verus` was on
  `PATH` (or `SAIL_CARGO_VERUS`). Only the opt-in repair engine ran the
  fork. Users also had to install Verus themselves.
- **The fork runtime is already a complete Verus.** The pinned runtime
  (`engine-runtime-13a1176c-linux` and `-macos`) holds `verus`,
  `cargo-verus`, `rust_verify`, the builtin and macro libraries, vstd
  (`libvstd.rlib`, `vstd.vir`, built the way the release builds it) and the
  pinned Z3, which is what `cargo-verus` requires
  (`cargo-verus-toolchains/src/installed.rs`). On a fresh `cargo verus new`
  project, warm `cargo verus verify` with the fork runtime printed exactly
  the stock release's stdout and stderr and exit code (macOS, L41). Without
  engine controls the runtime runs Verus's ordinary path; the engine's
  focused phase runs only when Sail's repair driver asks for it.
- **One gap: `cargo verus new`.** Verus's release workflow writes a toolchain
  manifest for its own version before building `cargo-verus`
  (`create-manifest`, `.github/workflows/build.yml`); Sail's source build did
  not, so the runtime's `cargo verus new` fails with "no known toolchain
  manifest for Verus version 0.2026.10.13.13a1176" (VI-0018).

## Decision

1. **Sail runs its own Verus.** Every verification Sail starts (`sail
   verify`, `--incremental`, `--output json`, `watch`, MCP, jobs, the
   portfolio, `fragility`, `repair-check`) runs the pinned fork runtime: one
   runtime, pinned by `engine/verus/artifact-*.json` and `fork.toml`, serves
   default mode and the repair engine. Release archives ship it under
   `engine/<variant>/`. Users install Sail and a Rust toolchain only.
2. **Stock Verus is only the oracle.** Plain stock Verus with the pinned Z3
   (`corpus/toolchain.toml`, installed by `cargo xtask fetch-verus`) is what
   the conformance harness compares against. Sail never runs it to answer a
   user.
3. **Correctness rule 1 now reads:** in default mode, `sail verify` produces
   the same per-function verdicts and the same diagnostics as plain *stock*
   Verus with the pinned Z3 (ADR 0009's instability rules and ADR 0042's
   diagnostic set comparison still apply). The fork's rules (ADR 0039: a fix
   may only pick one order stock can already produce, or correct structured
   output) are what make this hold. Enforcement:
   - `sail-conformance run` compares Sail-on-fork (fork runtime with
     `sail-z3`, with and without the cache) against plain stock, on every
     tier: `fast pr` in CI, `fast` and `standard` before merging shim, cache,
     daemon, key or harness changes, `deep` before releases. The previous
     stock-on-stock pass-through stays available as `--sail-verus stock`, to
     tell a shim difference from a fork difference.
   - The CI `End to end` legacy phase compares `sail verify` with stock
     `cargo verus verify` on the same project, byte for byte.
   - `deep-rvt` keeps comparing the stock source build with and without
     `sail-z3`. The fork's own `rust_verify_test` suite and its `sail_vi*`
     regression tests run on every fork upgrade ([engine README](../../engine/verus/README.md#upgrading-verus)).
     A per-test fork-versus-stock `deep-rvt` arm is a remaining gate, not yet
     built.
4. **No silent stock use.** Sail no longer looks for `cargo-verus` on
   `PATH`. It finds its Verus, in order, at `SAIL_VERUS_BUILD` (a fork build
   directory; used for CI images and development), the Sail cache
   (`<cache>/verus-src-build/<variant>`, from `sail engine install`,
   `cargo xtask build-verus-engine` or `ci/engine-artifact.py fetch`), and
   the release archive's bundled `engine/<variant>`. Each candidate passes the
   same identity and artifact-digest validation; nothing else is accepted.
5. **One explicit development override.** `SAIL_CARGO_VERUS=PATH` still runs
   another `cargo-verus`. It exists for tests that replay recorded Verus
   runs through fake verifiers, and for diagnosing a fork-versus-stock
   difference by hand. It is never set by Sail, and every command that uses
   it prints `warning[SAIL0108]` naming the binary and saying the results are
   not Sail's default mode. Content-addressed keys (charter rule 2) keep its
   cached answers correct: they bind query bytes and solver identity, not the
   Verus that produced them.
6. **Missing or corrupt Verus is an error, never a fallback.** If no
   candidate validates, verification stops with `error[SAIL0101]`, which says
   what is wrong and how to obtain the runtime. There is no fallback to any
   other Verus. Validation reads every artifact's full content digest once
   per install and records the files' device, inode, size and modification
   time under `<cache>/runtime-identities/`; later runs compare those stats,
   as ADR 0041 already does for toolchain files. Any change forces the full
   check again.
7. **Rust toolchain.** `cargo verus` compiles the user's crate with the Rust
   toolchain the runtime was built with, `1.98.1` (Verus's own pin), through
   rustup. Users install rustup and that toolchain; `sail verify` reports the
   requirement if it is missing (Verus's own message).
8. **vstd and the builtin crates.** Under `cargo verus`, Cargo builds `vstd`,
   `verus_builtin`, `verus_builtin_macros` and `verus_state_machines_macros`
   from the project's own dependency source (crates.io for user projects, as
   recorded in the project's `Cargo.lock`), compiled and checked by the fork's
   `rust_verify`; the runtime's bundled `libvstd.rlib` and macros serve
   direct `verus FILE` runs. The fork changes no vstd or builtin source; its
   only macro change (`9a5b2cc8`, VI-0001) is made independent of the macro's
   origin by `af212563`, which declares enum accessors in source order in the
   verifier. The SMT stream is therefore the same whichever source Cargo
   resolves, and `Cargo.lock` makes the resolution itself reproducible.
   Sail's corpus keeps patching these dependencies to the pinned upstream
   source (`patch_verus_deps`), exactly as before. A future fork fix in vstd
   or macro source would not reach `cargo verus` projects and needs its own
   decision.
9. **Build.** `cargo xtask build-verus-engine` writes Verus's toolchain
   manifest for the build's own version before building, as the release
   workflow does, so `cargo verus new` works with the runtime (VI-0018). This
   changes no verifier byte, so the variant identity is unchanged; runtimes
   built before this change lack the manifest until the next re-pin.

## Consequences

- **Deterministic default mode.** Fork fixes now reach every default run, so
  identical input gives identical SMT, and exact-prefix keys hit. This is
  what Sail's cache needs.
- **Charter goal 3 changes.** Switching to Sail no longer needs, or touches,
  a Verus installation; a user's own Verus stays theirs, and `cargo verus
  verify` still switches back. Charter appendix entry 4 records this.
- **Every fork commit must keep rule 1.** A re-pin needs the conformance
  tiers above to pass with Sail-on-fork against stock, plus the fork's
  regression tests, before it lands.
- **Upgrades.** Moving the Verus pin moves both `corpus/toolchain.toml` (the
  oracle) and `fork.toml` (the runtime) together, as ADR 0039 already
  requires.
- **Stock-mode workarounds** recorded in `docs/verus-issues/` and
  `docs/known-issues.md` for VI issues the fork fixes no longer apply to
  default mode.
- **Cache continuity.** Results cached from stock runs stay valid (keys bind
  bytes, not producers) but match fork queries only where the bytes agree.
- **CI.** The image already validates the runtime and sets
  `SAIL_ENGINE_BUILD`; `ci/run.sh` passes it as `SAIL_VERUS_BUILD`. No job
  builds Verus.
