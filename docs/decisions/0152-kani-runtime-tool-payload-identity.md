# 0152: Authenticate Kani's actual compiler and Rust payloads in Rust

- **Status:** implemented development candidate; native qualification pending
- **Date:** 2026-11-14
- **Extends:** [ADR0137](0137-pinned-kani-release-bundle.md), [ADR0149](0149-original-kani-rust-payload.md)
- **Tracking:** #712 G

## Context

The original-bundle installer and original Rust archive checker are contributor
fixtures. An executor cannot replace authentication of the tools it will use
with printed version labels or Rustup-generated bookkeeping. Every original
file has already been independently compared against the exact official inputs.

## Decision

Add a nonexecuting Unix library tool authenticator, enabled for the pinned
native Linux x86-64 payload only. Reuse the original Kani bundle's embedded file
pin, and authenticate the complete Rust file inventory through a compact
SHA-256 commitment rather than committing a raw 7,017-row JSON dump. Its encoding
is sorted ASCII relative paths, compact UTF-8 JSON, and records with fields
bytes, executable, sha256 in that order. Original archive and independently
installed inventories agree on 7,017 files / 1,586,050,167 bytes and commitment
`f2fdf9b82a0521dd8c88fc10cef8e217cf42d855252786492564d82fa7cc4dbe`.
The original official channel and six archive pins remain authoritative.

Tree inspection refuses symlinks, special/unreadable files and non-ASCII paths,
and bounds directory depth, path size, entries and aggregate read bytes.
Final opens use no-follow/nonblocking flags and streaming fixed-size hashing.
The exact existing ten Rustup bookkeeping names may be nonexecuting bounded
files; their contents grant no identity or execution authority. Empty-directory
layout is bounded but not authenticated. Prefixes must remain caller-owned and
immutable throughout inspection and later use. This is not an adversarial
filesystem snapshot or atomicity guarantee. Recheck repeats complete inspection.

Add the existing workspace sha2 and rustix dependencies to sail-kani; these
supply streaming identity hashing and safe no-follow file opens. No internal
layering edge, daemon/cache/shim dependency or backend fork changes.

## Consequences

This is necessary tool-origin groundwork for the executor. It does not execute
Kani, establish input/source/environment closure, prove bounded properties,
create receipts, enable cache reuse or expose a CLI backend. Native original-
payload and negative controls, source/lock binding review and the applicable
gates remain pending. Mac pins and execution remain separate.

Independent exact lock-binding review at1662c2dd approves only the new
Cargo.lock digest `e9182e0cfa37214ad952fc2ffc41f94bc2d611612825a4333e281559e55ced66`.
All256 package identities/records remain exact except sail-kani's two new edges;
all9 selected dependency closures remain exact. All78 Python controls and39
focused refusal controls pass. Every316 generated driver and complete compiled-
input map is byte-identical against the restored original lock. Advance only
PARSER_NATIVE_SHA256's Cargo.lock value; preserve the original strict refusal.
The independent report is externally retained as sha256
`93bed717fd2e38486a681e7cfa6799293c38a3d0dd2b2bf2bfd44bf6c9868c0c`;
all934 indexed payloads are independently lead-rehashed and externally retained.
The metadata-only finalizer's original uname inventory assertion failure remains
visible; its correction changes no audit or acceptance result. No native proof,
tool-authenticator or backend qualification follows from this source binding.
