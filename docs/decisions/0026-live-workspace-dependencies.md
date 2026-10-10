# 0026: Live workspace and optional HTTP dependencies

- Status: accepted
- Date: 2026-10-09

## Decision

Use notify 8.2 for workspace filesystem events, with content validation and
polling fallback where events are unavailable. Its published license is
CC0-1.0. Permit that license only for notify 8.2.0 in cargo-deny.

Use reqwest's blocking client and Rustls with the ring provider for the optional
remote shared tier. Use Hyper, hyper-util, http-body-util, bytes and Tokio for
its bounded HTTP/1 server. Disable reqwest and Rustls default features so the
service does not introduce an additional cryptographic provider. The native
platform verifier includes webpki-root-certs 1.0.9, licensed
CDLA-Permissive-2.0; permit that specific package and version only. Dependency
updates must revisit these explicit exceptions. The repository's general
license allowance, advisory check and source restrictions remain enforced.

All dependencies are declared centrally and pinned by Cargo.lock. Networking
remains in sail-daemon; the shim, key and protocol foundations do not acquire
HTTP or filesystem-watcher dependencies. No HTTP runtime or listener starts
when the remote feature is unconfigured. The existing local CLI installation
remains sail plus sail-z3; managed Cargo/driver aliases are created privately
by Sail when needed.

Enable sha2's `asm` feature so its existing CPU dispatch also uses arm64 SHA
instructions. Managed verification hashes the compiler runtime and artifacts
by content on every request; the software implementation made that validation
cost exceed fresh verification on a small crate. This changes the dependency's
implementation, not the digest algorithm, key format or content-binding rule.
Sail itself continues to forbid unsafe code. Existing key, signature and
transcript tests must pass with the feature enabled.

## Validation

Managed verification also uses the safe inotify 0.11.5 API on Linux for
precise observation of absent configuration files.
Cargo creates and removes usage-database journals in its home directory;
stamping that entire directory would incorrectly invalidate every proof.
The observer must distinguish those writes from a transient configuration
file and synchronously drain the ordered kernel queue before publishing receipts.
Existing content and mutation stamps still protect present inputs.
Unavailable, incomplete or overflowing observation prevents currentness.
macOS retains conservative directory mutation stamps for missing configuration
paths. FSEvents notifications, cookies and synchronous flush alone cannot prove
delivery of every prior change. Unrelated Cargo journal or ancestor-directory
activity can therefore make a macOS result historical and prevent reuse.
Workspace code remains safe Rust.

Run the complete workspace gate, including cargo-deny, on every feature PR.
Measure CLI size, daemon memory and idle CPU with networking disabled, and
keep every required CI job below its existing ten-minute bound. Fast tests
use bounded local fixtures; corpus acceptance and performance measurements
stay local-only.
