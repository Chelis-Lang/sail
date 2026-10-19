# 0058: Managed builds use the engine's pinned Cargo

- **Status:** accepted (owner decision, 2026-10-19)
- **Date:** 2026-10-19

## Context

Verus already requires its pinned Rust compiler (`pins.verus.rust_toolchain`
in `corpus/toolchain.toml`, currently `1.98.1`): the launcher runs its driver
with `rustup run <toolchain>`, whatever the caller's default is. Only the Cargo
layer followed the caller. Managed verification ran `$CARGO` or `cargo` on
`PATH`, which for a rustup user is the default toolchain's Cargo.

The shipped vstd proof bundle (ADR 0048) is keyed to the exact compiler
arguments a Cargo emits, and Cargo versions add different flags even when they
invoke the same compiler. The bundle is recorded with Cargo `1.98.1`. Most
users' default Cargo is newer (the CI image's is `1.99.0`), so they silently
missed the shipped proof and verified vstd again: a slow first run, with the
fast path reachable only by typing `RUSTUP_TOOLCHAIN=1.98.1 sail verify`.

Sail is primarily a command-line tool. The owner decided that managed builds
use the engine's pinned toolchain's Cargo unless the user or the project chose
a toolchain.

## Decision

`sail verify` selects the toolchain its Cargo runs with in one place
(`managed_cli::cargo`, through `sail_verus::managed::cargo_toolchain`). Jobs
(whose workers run `sail verify`), `watch` and MCP select through it too.

1. **The caller's or project's choice wins.** Sail keeps the caller's Cargo
   when any of these holds: `RUSTUP_TOOLCHAIN` is set; `CARGO` names a Cargo;
   `RUSTC` names a compiler; the workspace or an ancestor has
   `rust-toolchain` or `rust-toolchain.toml`; rustup's `settings.toml` has a
   directory override for the workspace or an ancestor (settings that exist
   but cannot be read count as an override); or the Cargo Sail would run is not
   a rustup proxy (the same file, by symbolic or hard link, as `rustup` on
   `PATH`), including when rustup is not on `PATH`.
2. **Otherwise the pinned toolchain, if it is installed.** It is installed
   when rustup's home has `toolchains/<toolchain>-<host>` (or exactly
   `<toolchain>`) with `bin/cargo` and `rust-src` in `lib/rustlib/components`.
   Sail then sets `RUSTUP_TOOLCHAIN=<toolchain>` on every process it starts for
   the run: the `rustc --print sysroot` probe, `cargo metadata`, `cargo clean`
   and Cargo-Verus, whose Cargo (`SAIL_MANAGED_CARGO`, run by Sail's adapter)
   and compilers inherit it. Sail never sets its own environment.
3. **Nothing is installed.** The check reads only local files; verify, jobs
   and MCP never touch the network. A pinned toolchain without `rust-src`, or
   not installed at all, keeps the caller's Cargo. A release that ships a vstd
   proof then records the result note `SAIL3137`, naming the one command that
   enables the shipped proof:
   `rustup toolchain install <toolchain> --profile minimal --component rust-src`.
   Like other notes it is in the JSON result (`--output json`, jobs, MCP), not
   in the plain output, which stays Cargo-Verus's own.
4. **The selection is a proof input.** The effective `RUSTUP_TOOLCHAIN` joins
   the proof context's environment, as if the caller had set it, and the probed
   Cargo sysroot is the pinned one. Caller and pinned selections therefore
   have different contexts, receipts and dependency-store families.
5. **Plain fallbacks select the same way.** The setups `sail verify` runs as
   plain Cargo-Verus does (SAIL3112 setups and partial verification without
   `--output json`, ADR 0046) use the same selection. The explicit fragility
   measurement and seed portfolio keep the caller's Cargo.

### The parity contract

Default-mode `sail verify` is compared with
`RUSTUP_TOOLCHAIN=<effective toolchain> cargo verus verify`, where the
effective toolchain is the one Sail selected: the caller's `RUSTUP_TOOLCHAIN`
when set, otherwise the pinned toolchain when Sail selected it. Comparisons
stay byte for byte. `ci/stock-toolchain.py` derives the effective toolchain
for the e2e phases and refuses to run when the pinned toolchain lacks
`rust-src`, so CI always exercises the pinned selection. The conformance
harness already runs both arms with the pinned toolchain
(`sail_verus::run`), so its contract is unchanged. Without the variable, plain
`cargo verus verify` remains a valid oracle wherever the caller's Cargo is
the pinned one or the caller made a choice that Sail keeps.

## Consequences

- A rustup user with the pinned toolchain and `rust-src` installed (the
  README's install step) gets the shipped vstd proof with no extra setting.
- A project's `rust-toolchain.toml`, an override or an explicit variable keeps
  working exactly as before, and keeps missing the shipped proof unless it
  selects the pinned toolchain.
- Cargo-level output (for example a lockfile Cargo rewrites, or a warning only
  one Cargo version prints) is now the pinned Cargo's. Users who want to
  reproduce a run with stock Cargo-Verus set the same `RUSTUP_TOOLCHAIN`.
- Changing the pin changes the selected Cargo, and so every context, exactly
  as a re-pinned compiler already does.
- Revisit if Sail gains non-rustup installations as a supported setup (a
  toolchain bundled with Sail), or if the bundle factory records for more than
  one Cargo version.
