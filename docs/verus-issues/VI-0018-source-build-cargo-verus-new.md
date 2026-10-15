# VI-0018: A source-built `cargo verus new` knows no vstd for its own version

- **Status:** worked around in Sail's build (`cargo xtask build-verus-engine`);
  not a fork source change
- **Verus:** fork commit `13a1176c` (upstream `426d8b0`), runtime built by
  `cargo xtask build-verus-engine` (Linux and macOS)
- **Found:** 2026-10-15, L41, while making the fork runtime Sail's default
  Verus (ADR 0043)
- **Impact on sail:** with the pinned runtime, `cargo verus new` cannot create
  a project, so the runtime alone cannot serve the README's first steps.
  Verification of existing projects is unaffected.

## Reproduction

With the pinned runtime's directory `R`
(`<cache>/verus-src-build/<variant>/verus/source/target-verus/release`):

```sh
R/cargo-verus verus new --lib demo
# Error: no known toolchain manifest for Verus version 0.2026.10.13.13a1176;
# cannot determine the vstd dependency
```

`cargo-verus` finds the `vstd` dependency to write in a table compiled from
`source/cargo-verus/toolchain-manifests/*.toml`
(`cargo-verus/src/toolchains.rs`, `infer_vstd_dependency`). A source build's
version is `0.<commit date>.<short sha>` (`cargo-verus-toolchains/src/versions.rs`),
for which no committed manifest exists. Verus's release workflow writes one
for the build's own version before building
(`.github/workflows/build.yml`, step "create temporary toolchain manifest":
`cargo run -p cargo-verus-toolchains --bin create-manifest -- --write-to-dir
cargo-verus/toolchain-manifests`); Sail's source build skipped that step.
The stock release's `cargo verus new` writes `vstd = "=0.0.0-2026-10-04-0306"`.

## Workaround

`cargo xtask build-verus-engine` now runs the same `create-manifest` step
before building, so a runtime built from this change writes the vstd version
of `source/vstd/Cargo.toml` (unchanged by the fork). The manifest file is
untracked, so the build is not marked dirty, and no verifier byte changes.
The runtime pinned before this change still lacks it until the next re-pin.
Until then, the README creates a project with `cargo new` and the two
`Cargo.toml` additions `cargo verus new` would make.

Upstream description: building Verus from source the documented way leaves
`cargo verus new` unable to create projects, because the toolchain manifest
for the build's version exists only in release builds.
