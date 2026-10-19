# VI-0031: exported VIR records the build host's rust-src paths

- Found: 2026-10-19, Linux x86-64, Verus 0.2026.10.18.7d82994 (Rust 1.99.1)
  while building fork runtime `9c0ca7e7`; 0.2026.10.04.426d8b0 (Rust 1.98.1)
  behaves the same.
- Status: Rust behaviour that Verus inherits, reproduced; Sail's runtime build
  avoids it. No upstream issue filed.
- Reproduction: [one external type specification](../../corpus/repros/rust-src-std-paths/README.md).

The standard library's source spans have rustc's virtual path
`/rustc/<commit>/library/...`. When the toolchain has the `rust-src`
component, rustc maps those spans to the local copy below the sysroot
(`lib/rustlib/src/rust/library/...`). Verus writes span strings into the VIR
it exports (`--export`), so an exported library names the build host's
toolchain directory instead of the virtual path. The repro's 18 standard
library spans change accordingly; verdicts do not.

vstd's VIR is exported the same way. The Verus release's `vstd.vir` names
`/rustc/<commit>` (10,199 occurrences on 7d82994), and so do Sail's earlier
runtimes. A first container build of the `9c0ca7e7` runtime, on a toolchain
with `rust-src` (as the CI image installs it), instead recorded the same
10,199 spans under `/opt/rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/lib/rustlib/src/rust`.
It was discarded unpublished. The prebuilt runtime would otherwise depend on
the build host and differ from the release's vstd; diagnostics through those
spans were not compared.

Sail's handling: `cargo xtask build-verus-engine` refuses to build when the
Verus toolchain has `rust-src` and checks the built `vstd.vir`, and
`ci/engine-artifact.py` refuses to pack or install a runtime whose `vstd.vir`
names a local `rust-src` path (`SAIL0944`). User projects are verified in
their own environment by both stock and Sail, so they see the same paths.
