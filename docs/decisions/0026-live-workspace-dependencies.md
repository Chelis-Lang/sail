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

## Validation

Run the complete workspace gate, including cargo-deny, on every feature PR.
Measure CLI size, daemon memory and idle CPU with networking disabled, and
keep every required CI job below its existing ten-minute bound. Fast tests
use bounded local fixtures; corpus acceptance and performance measurements
stay local-only.
