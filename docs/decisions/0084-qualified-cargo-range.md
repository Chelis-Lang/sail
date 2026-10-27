# 0084: Build-script receipts for a qualified range of Cargo releases

- **Status:** proposed
- **Date:** 2026-10-26

## Context

Proof receipts for a project whose selected dependency closure contains build
scripts (every Verus project: vstd's closure includes proc-macro2's) need the values of the
Cargo-generated variables each script declares with `rerun-if-env-changed`.
Sail does not observe a script's environment. It derives those values from
the run's own Cargo evidence (#299, `managed::build_environment` and
`managed::build_inputs` package environment):

- `cargo build --unit-graph -Z unstable-options` (unit graph version 1): each
  `run-custom-build` unit's `pkg_id`, `platform`, `features` and
  `profile.debug_assertions`, plus roots, targets and dependency indices;
- the target-info `rustc --print=cfg` query Cargo issues through
  `RUSTC_WRAPPER`, re-run with `--print=cfg` alone;
- Cargo's derivation in `custom_build.rs` `build_work`: `CARGO_FEATURE_<envify>`
  per unit feature; `CARGO_CFG_*` from the rustc cfg minus `proc_macro`
  (target-info filter) and minus rustc's `debug_assertions`, plus the unit
  profile's `debug_assertions`, plus `feature`; values joined with commas;
- `OUT_DIR`, `CARGO_MANIFEST_DIR`, `CARGO_MANIFEST_LINKS` and `CARGO_PKG_*`
  from `cargo metadata` and the `build-script-executed` message's `out_dir`,
  whose sibling `output` holds the `cargo:`/`cargo::` `rerun-if-*` lines.

Dep-info, `cargo metadata`'s schema, the JSON messages, the default target
layout and the script output syntax are stable Cargo interfaces that every
managed run already depends on, with or without scripts. The unit graph is
unstable and the derivation is Cargo-internal, so #364 accepted only the
pinned Cargo's source commit (1.99.1, `5f94df47`); any other Cargo fell back
(`SAIL3139`) to plain Cargo freshness with no receipts. Real projects pin
their own Rust: Anvil's `rust-toolchain.toml` selects 1.98.0, so it got no
receipts and no unchanged-call replay.

ADR 0058 already keeps a project's own toolchain choice. Running the pinned
Cargo instead, with the project's rustc, is not sound for this purpose: the
parity oracle is `cargo verus verify` under the project's toolchain file,
which runs the project's Cargo. Another Cargo's output (lockfile rewrites,
warnings, resolver behaviour, progress lines) and behaviour (manifest
features, MSRV-aware resolution, `[lints]` handling, as 1.99.1 changed) are
not the project's. A toolchain file pins Cargo as much as rustc.

## Decision

1. `QUALIFIED_CARGO` lists the Cargo source commits whose protocol regions
   are qualified, one per release commit: 1.93.0 through 1.99.1. Patch
   releases that share a commit share an entry:

   | Cargo | Commit |
   |---|---|
   | 1.93.0, 1.93.1 | `083ac5135f967fd9dc906ab057a2315861c7a80d` |
   | 1.94.0 | `85eff7c80277b57f78b11e28d14154ab12fcf643` |
   | 1.94.1 | `29ea6fb6a5db279426f4cc4e17aa385f05a0cfbc` |
   | 1.95.0 | `f2d3ce0bd7f24a49f8f72d9000448f8838c4e850` |
   | 1.96.0 | `30a34c6821b57de0aaec83a901aca39f88f6778c` |
   | 1.96.1 | `356927216a2d746168cf76e5e88cc3f4b58e029d` |
   | 1.97.0, 1.97.1 | `c980f4866141969fab6254a680546a277789d6f0` |
   | 1.98.0, 1.98.1 | `797e8a9bca276c1c9f9f738d2a20f484fa4eea9d` |
   | 1.99.0, 1.99.1 | `5f94df4789f005f9a352888e8355ffc645b7ed0e` |
 Identity is
   the `commit-hash:` line of `cargo --version --verbose`, recorded by every
   query as before. Any other commit, including a malformed or missing
   observation, keeps exactly the existing behaviour (`SAIL3139`, or fail
   closed).
2. A commit is qualified only with both kinds of evidence:
   - **Source audit.** `scripts/cargo-protocol-audit.py` compares each
     protocol region against an already qualified commit. For 1.96.0 through
     1.99.1 every region is identical after the 1.99 module move. The accepted
     differences are: before 1.96 the unit graph lacks the nightly-only
     `nounused` dependency field, which Sail never reads; before 1.95
     `CompileKind` serializes `CompileTarget::name` and the target query adds
     `--target` inline, and `name` equals `rustc_target()` and the argument
     list is identical for every target.
   - **Real-Cargo self-test** (`ci/run.sh cargo-range`): a one-package
     project with hyphenated and non-ASCII features, a `links` key and a
     pre-release version is built by that release, with a wrapper recording
     Cargo's own cfg queries. In development and release profiles, with and
     without an explicit `--target`, every `CARGO_FEATURE_*` and `CARGO_CFG_*`
     value and every package value the build script receives must equal
     Sail's derivation. The step also requires a divergent control (1.87.0,
     whose unit-graph package IDs never match metadata) to be unqualified and
     to fail the self-test.
3. Cargo 1.88 through 1.92 pass the self-test but are not qualified: before
   1.93 Cargo never sets `CARGO_CFG_DEBUG_ASSERTIONS` and gives every
   run-custom-build unit a default profile, so Sail's derivation agrees only
   through a different code path. They can be added with an audit of that
   path. Cargo 1.87 and earlier refuse authority (package IDs) and stay on
   the fallback.
4. Qualifying a new release (for example 1.100) is a code change: run the
   audit against a qualified tree, run `ci/run.sh cargo-range` with it in
   `SAIL_CARGO_RANGE_TOOLCHAINS`, then add its commit. There is no runtime
   probe: it could only re-confirm an audited commit, and could never extend
   trust to an unaudited one, while adding a first-use build to the run.
5. Nothing else changes: the receipt digest still binds every derived value,
   each run derives them from its own query, the proof context still binds
   the exact Cargo identity, and the shipped vstd proof remains recorded with
   the pinned Cargo only (ADR 0058).

## Consequences

- Projects that pin Rust 1.93 through 1.99 (Anvil on 1.98.0) get proof
  receipts and unchanged-call replay. Instant revert's profile (ADR 0050)
  is unchanged and still excludes crates with build scripts. Their output stays byte for byte that of `cargo verus verify` under
  the same toolchain file (`ci/run.sh e2e cargo-range`).
- A wrong derivation would bind a stale receipt, so every table entry carries
  the audit and the self-test; the self-test is local-only because it installs
  toolchains.
- Each new stable Cargo release starts outside the range until it is
  qualified. Revisit if Cargo stabilizes a build-script environment or unit
  graph interface Sail can read directly.
