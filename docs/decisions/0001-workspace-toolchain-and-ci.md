# 0001: Workspace, toolchain, lints, layering and CI tiers

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter asks for:
- a Cargo workspace of small crates with strict dependency direction;
- a stable toolchain with a pinned MSRV;
- forbidden `unsafe`;
- pedantic clippy;
- tiered testing.

The closest peer, Chelis-Lang/dive (also created 2026-10-08), set its
conventions from the org's Rust repos (chelis, buoy, barnacle, c-note) in its
ADR 0001:
- resolver 3 and edition 2024;
- exactly pinned toolchains;
- `[workspace.lints]` with pedantic deny and `unwrap`/`expect`/`panic` deny;
- `cargo deny`;
- an xtask for repo automation;
- lean CI on `ubuntu-latest` with `Swatinem/rust-cache` saving only on main;
- SHA-pinned actions.

sail adopts them so the org's repos stay uniform.

The charter's dependency text says conformance and benchmarks "depend on the
CLI and API". The CLI is a binary crate, though, and the charter also gives the
CLI `sail bench`, so a literal crate edge in either direction is awkward or
cyclic. The charter also says the shim talks to the daemon through "a thin
client" but names no crate for it.

## Decision

- **Workspace.** One crate per charter component under `crates/`, created now
  as documented placeholders. `xtask/` holds repo automation.
- **Toolchain.** Rust 1.99.0 pinned exactly, with `rust-version = "1.99"` and
  edition 2024. No nightly.
- **Lints** match dive: forbid `unsafe_code`; deny `missing_docs`,
  `clippy::all`, `clippy::pedantic`, `unwrap_used`, `expect_used`, `panic`,
  `todo`, `unimplemented` and `dbg_macro`.
- **Layering.** Permitted internal edges live in one table,
  `xtask/src/layering.rs`, which `cargo xtask check-layering` checks in CI.
  - `sail-api` owns the structured output schema, the daemon wire protocol and
    the thin client. The shim's only internal dependencies are `sail-smtlib`,
    `sail-session` and `sail-api`, and a unit test forbids it from reaching
    the daemon, cache, solver pool, portfolio or Verus crates.
  - `sail-conformance` and `sail-bench` depend on `sail-api` and `sail-verus`
    and drive the built `sail` and `sail-z3` binaries as processes. They do
    not link `sail-cli`. `sail-cli` may link them to expose `sail bench`.
  - A unit test keeps `sail-smtlib`, `sail-session`, `sail-key` and
    `sail-cache` free of `sail-verus`, so the foundations stay
    verifier-agnostic.
- **CI tiers.**
  - `ci.yml` runs on every push to main and every PR. Its `No AI authorship
    markers` job runs first, then `Workspace gate`: fmt, clippy, nextest,
    doctests, rustdoc, layering and cargo-deny.
  - The standard and deep conformance tiers get their own workflow in M3, once
    the harness exists. No placeholder workflow is added that would report
    green without checking anything.
  - Benchmarks, profiling and long fuzzing never run in the fast tier.
- **Hosting.** GitHub-hosted `ubuntu-latest` only for the fast tier.
  Development and the first baselines happen on macOS arm64. A macOS CI job is
  deferred until the shim has platform-specific behaviour worth gating, because
  macOS minutes cost more.
- **Profiles.** `debug = "line-tables-only"` in the dev, test and release
  profiles, plus a `profiling` profile.

## Consequences

- Placeholder crates make layering violations visible from day one.
- Toolchain bumps are deliberate and touch both `rust-toolchain.toml` and
  `Cargo.toml`.
- Conformance does not gate merges until M3. Until then, the correctness
  argument for any change rests on unit and property tests.
- Recorded in the charter appendix, item 1.
