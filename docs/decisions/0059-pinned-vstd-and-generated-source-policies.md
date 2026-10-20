# 0059: Pin the published vstd and generate the source policies

- **Status:** proposed
- **Date:** 2026-10-19

## Context

Each Verus release publishes a matching `vstd` to crates.io
(`source/vstd/Cargo.toml`, for example `0.0.0-2026-10-18-0147` for
0.2026.10.18.7d82994), together with new `verus_builtin`, macro and
state-machine crates. Sail's shipped vstd proof bundle (ADR 0048) and retained
proofs (ADR 0050) admit only the exact published packages and their exact
sources. Until now that identity was spread over code literals
(`managed/shipped.rs`, `managed/retained/eligibility.rs`,
`scripts/dependency-store-check.py`) and four hand-built source policies
(`engine/verus/*-inputs.json`), with no generator.

A stale literal or policy cannot produce a wrong answer: it makes the shipped
bundle or retained reuse decline, and verification runs as before. That is why
tests did not catch it, and why the Verus 0.2026.10.18 upgrade would have
silently lost both performance paths unless every literal was found by hand.

## Decision

1. **The published vstd is a pin.** `corpus/toolchain.toml` `[verus]` has
   `vstd = "<version>"`, read through `Pins::embedded()` like `commit` and
   `rust_commit`. It is optional for secondary pins, which run stock Verus.
   Production code reads it where it previously held a literal: the shipped
   boundary's vstd invocation check and the retained profile's graph
   selection. Without the pin, both decline.
2. **The shipped factory follows the pin.** `scripts/shipped-deps/Cargo.toml`
   requires `=<vstd>` and its lock names that version; a unit test checks
   both against the pin.
3. **The source policies are generated.** `scripts/source-policies.py`
   derives all four policies from the pins, Cargo's resolution under the
   pinned toolchain, crates.io archives checked against the lock's SHA-256,
   Vest at its corpus pin and the fork's Git tree. Before first use it
   reproduced the committed policies at the 426d8b0 pins byte for byte,
   except two recorded differences: `metadata_sha256` hashes `cargo metadata`
   output that names absolute paths, and the committed official Vest policy's
   `verus_builtin_macros` `source_bytes` disagreed with its own file sizes.
   `--check` compares a fresh generation with the committed files.

No dependency is added: the script carries a reference BLAKE3 for hosts
without the `blake3` Python module.

## Consequences

- A Verus upgrade changes `vstd` in the pins, re-locks `scripts/shipped-deps`
  and re-runs the generator. Nothing else holds the version.
- The native compiler shapes (`managed/shipped/native_shapes.json`) remain
  observed data keyed by package version. They follow Cargo's actual
  arguments and must be re-observed, not generated, when Cargo or a package
  changes.
- `metadata_sha256` stays informational: no consumer reads it, and it
  identifies the generating run's metadata.
- The layering is unchanged: the pins are already read by `sail-verus`.
