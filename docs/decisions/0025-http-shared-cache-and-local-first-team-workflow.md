# 0025: HTTP shared cache and local-first team workflow

- **Status:** accepted
- **Date:** 2026-10-09

## Context

The signed directory tier works across shared filesystems, but teams need a
self-hosted network endpoint and authenticated publication. The owner chose a
deployable Sail server and required single-developer CLI use to remain lightweight.

## Decision

Add an optional HTTP(S) transport in `sail-daemon`, using pooled blocking reqwest
with rustls/ring. A dedicated optional hyper HTTP/1 server transports the existing
format-v2 signed entries and session certificates. Networking does not enter the
shim, key or cache layers. No internal dependency edge changes for this feature.
No HTTP client, server runtime, credential or configuration is required for local
verification. The existing daemon protocol and shared-entry format remain intact.

Reader trust remains configured ed25519 public keys. Separate bearer reader/writer
roles restrict storage access. Upload-supplied public keys validate signature
integrity, but never grant reader trust. Default-mode entries only; existing
solver/platform binding and the 50% unknown-serving threshold are preserved.
Validated publications are immutable: atomic first publication wins, identical
bytes are idempotent, other publications report conflict.

Persist private reader configuration under the local results root. Running daemons
reload endpoint and credentials within five seconds. Explicit directory stores
retain their existing behavior. HTTPS and hostname verification are mandatory
except for literal loopback HTTP; redirects, automatic content decoding and URL
credentials are refused. A private reverse-proxy deployment terminates TLS; the
server refuses non-loopback binding without explicit proxy acknowledgement.

Bound connections, headers, body deadlines, entry bytes and daemon lookup time.
Coalesce concurrent same-key reads. Any network/authentication/signature failure
is a cache miss and leaves the pinned solver fallback in control. Publication
eligibility uses the existing signed-directory exporter rather than a second policy.

## Consequences

Solo use remains automatic; teams add a server, signing trust and private token
files only when desired. The runtime container needs neither Verus nor Z3. The
server is not a proof authority. Authorized writers can cause storage misses or
consume disk, but cannot grant their keys reader trust. Existing local imported
entries survive disconnection/key removal according to ADR 0017's trust boundary.

Full transport, trust, deployment, resource bounds and acceptance tests are
documented in `docs/http-shared-cache.md`. New external dependencies retain the
workspace MSRV and license checks; narrowly scoped license exceptions are recorded
separately in ADR 0026.

Development and test profiles optimize only `curve25519-dalek` arithmetic at
level 3, matching the existing production release optimization. They retain
debug assertions and overflow checks. Every entry still runs the same
`ed25519-dalek::verify_strict` implementation and all binding, trust, hash, mode
and shape checks; no signature trust is cached. This targets measured unoptimized
arithmetic CPU cost within the unchanged serving deadline. Cargo's
[package profile overrides](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides)
change compiler optimization, without changing the dependency or algorithm.
The original failed HTTP deadline and fixed authentic/forged CPU control are
retained in `docs/validation/m7-shared-signature-cpu-diagnosis.md`; they do not
establish why the entire original wall-time gap occurred.
