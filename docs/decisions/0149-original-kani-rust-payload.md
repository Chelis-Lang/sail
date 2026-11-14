# 0149: Bind the original Linux Rust payload for Kani groundwork

- **Status:** implemented; native Linux contributor qualification passes
- **Date:** 2026-11-14

## Context

ADR0137 authenticates the Kani release bundle, whose external nightly Rust
installation remains outside that identity. Package G requires complete backend
identity before execution or reuse. The selected release uses nightly 2026-08-21
on x86_64-unknown-linux-gnu, including rustc-dev, llvm-tools and rust-src.
The original [Rust channel manifest](https://static.rust-lang.org/dist/2026-08-21/channel-rust-nightly.toml)
binds the six original compressed component archives and their versions.

## Decision

Pin that original channel file's SHA256 and each selected archive's digest,
size, exact component name, version and complete installed-payload bounds in
`engine/kani/toolchain-linux.json`. The contributor-only, read-only
`ci/kani-toolchain.py` checks original archives without running installer code.
It may additionally compare a caller-owned immutable installed prefix against
every original payload byte and executable bit. Missing or extra payloads,
links, duplicate members, noncanonical paths, digest mismatches and excessive
inventories refuse with `SAIL6302`.

A small explicit set of bounded nonexecutable Rustup bookkeeping files may be
present. Their original hashes are reported as untrusted metadata; they cannot
select or authenticate an executable or change the required payload. The checker
does not install or execute Rust/Kani, authorize an environment, establish source
closure or cache authority, or produce a bounded verification result. Concurrent
mutation of supplied archives or the installed prefix is outside its contract;
callers must retain immutable owned inputs. No default backend changes.

## Consequences

The original external compiler payload now has a reproducible identity separate
from Rustup's generated metadata and the Kani bundle. Actual native installation,
versions and refusal controls pass with independent review; see the
[qualification record](../validation/kani-rust-payload.md). Future execution
must separately bind all runtime and source inputs, environment, requested bounds
and outputs; a matching payload alone is insufficient. The library and CLI
backend, direct arrayvec qualification, macOS and release acceptance remain
separate gates. No new dependency or crate edge is introduced.
