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
their own environment by both stock and Sail, so they see the same paths. The
shipped vstd proof bundle is the exception: it is recorded once and served
elsewhere, and its recorder requires `rust-src`. The recorder therefore remaps
the `rust-src` root to `/rustc/<rustc commit>` for the vstd unit, so the
bundle's VIR names the standard library as the release's `vstd.vir` does
([compiler-boundary record](../validation/vstd-compile-boundary.md#verus-7d82994-bundle-and-standard-library-paths-vi-0031)).

Serving the bundle restores local names. A vstd compiled on the user's machine
names standard library spans by the toolchain's `rust-src` copy when it is
installed, and its own sources by the user's registry directory; the bundle
recorded `/rustc/<rustc commit>` and the virtual rustc-dev base. Whenever Sail
sets `SAIL_RUSTC_DEV_SOURCE_BASE`, it also sets `SAIL_VIR_SPAN_REMAP`, which
Sail's Verus applies to the span strings of imported VIR: one `<from>=<to>`
pair per line, the first matching prefix wins, both prefixes absolute, and a
malformed line selects nothing. The pairs are
`/rustc/<rustc commit>/=<sysroot>/lib/rustlib/src/rust/` (only when the
verifier toolchain has `rust-src`) and `<virtual base>/=<registry>/`. The
presence of `rust-src` joins the proof context. The variable needs a runtime
built from fork branch `fix/vir-span-remap`; earlier runtimes ignore it.

These strings appear in `--log impl-names` and the VIR logs, and Verus prints
one in place of a label whose imported span it cannot resolve. No ordinary
diagnostic that prints one has been found: cycle errors through standard
library implementations report a shorter cycle without them, and `--profile`
labels resolve vstd spans normally.
