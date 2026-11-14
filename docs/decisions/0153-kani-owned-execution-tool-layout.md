# 0153: Stage an original Kani release with one exact Rust toolchain route

- **Status:** implemented development candidate; native qualification pending
- **Date:** 2026-11-14
- **Extends:** [ADR0152](0152-kani-runtime-tool-payload-identity.md)
- **Tracking:** #712 G

## Context

The pinned release driver resolves Cargo through its own toolchain/bin/cargo.
Original release installation creates that route after unpacking. The original
bundle authenticator correctly refuses added links and must retain that strict
original-payload boundary. A caller must not inject an arbitrary Rust toolchain.

Primary source: the [pinned driver](https://raw.githubusercontent.com/model-checking/kani/kani-0.68.0/kani-driver/src/session.rs),
retained externally by SHA-256
`38cdd12b2ef8de089204f0629f16bd6475179ecde2b10c3eeb01ec4f78e10c21`.
Both release Cargo-selection functions use the fixed toolchain/bin/cargo route.

## Decision

Create a new directory under a private parent, copy every original bundle file
with exact bytes and executable bits, retain notices and the original empty
scripts directory, and add exactly one root toolchain link whose absolute
target is the authenticated Rust prefix. Leave both original prefixes unchanged.
Original authentication still refuses links. Staged authentication accepts only
that fixed namespace/target pair and verifies all other original payloads exactly.
Missing, redirected, regular-file or additional links refuse. Recheck both
original prefixes, copied files and the route before later use.

Copy through exclusive output creation and bounded no-follow input reads;
require original size, executable bit and digest. Existing destinations and
nonprivate parents refuse. Caller-owned immutable prefixes and private parents
remain preconditions; no general adversarial filesystem snapshot is claimed.
Partial stages remain diagnostic material, without readiness or proof authority.
No dependency graph, source fork or default solver path changes.

## Consequences

This supplies the original driver's required layout without changing compiler
bytes, trusting Rustup bookkeeping or enabling a backend. It executes no harness,
creates no source/currentness/proof/cache authority, and introduces no CLI
backend. Native original-tool staging, route/copy/refusal controls, independent
review, the eventual executor/source-environment closure and Mac qualification
remain separate. Consumer skill instructions stay unchanged because backend
availability is unchanged.

Existing stages can be inspected only against the original authenticated
prefixes, with the same private root/parent, complete file and exact route checks.
This is tool-file inspection, not a retained verification or proof receipt.

Route equality compares exact native OsStr bytes. Path component equality is
insufficient because it accepts redundant separators/current-directory segments;
the original native counterexamples remain retained. Contributor inspection
arguments and JSON path representation refuse unsupported text without panicking.
