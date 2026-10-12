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

### Configured transport preparation at daemon bind

The complete `777b86a` solver suite exposed a cold first-fetch miss: the fresh
reader returned the exact direct solver output but started Z3 despite published
signed replies and a session-silence certificate. A separate phase diagnostic
measured helper dispatch at 106.857 ms, transport initialization completion at
270.456 ms, body completion at 314.578 ms and final validation at 315.341 ms.
The foreground had timed out at 301.815 ms. HTTP succeeded; no signature, trust,
identity or authentication rejection was recorded. See the
[retained failure](../validation/m7.md).

A configured daemon now launches one optional HTTP constructor asynchronously
at bind, overlapping construction with ordinary solver identity setup. This
sends no network request, adds no readiness wait and caches no signature or
trust decision. Local unconfigured daemons create no HTTP client. Each actual
lookup retains the original 250 ms fallback deadline and all entry validation.
Configuration installed after bind can still incur a cold miss.

The setup owner shares the lookup constructor gate. It cannot overlap a second
constructor, and failed construction retains the existing five-second reload
cadence. Shutdown marks the owner abandoned under the publication lock, so its
late result cannot be installed. A finished owner is joined; a constructor still
blocked in a third-party builder may finish after shutdown, but is not joined on
that bounded path. Only one such setup owner exists per daemon. This improves
cold-cache availability without promising that every cold lookup succeeds.

### Revalidate unchanged loopback transport without reconstructing it

Repeated complete workspace gates retain a setup-publication timeout at the
original 250 ms limit. Their exact complete causes remain unestablished. A
separate owner observation records material runtime CPU, and the package-only
optimization control does not show a benefit; its profile stays unselected.
See [the retained observation](../validation/m7.md).

Configuration refresh previously constructed a blocking HTTP runtime before
comparing it with the current client. Reuse now follows fresh endpoint policy,
owner-only token read, bounded CA read and PEM validation, and requires exact
normalized URL, token, CA-byte hash and timeout equality. A missing or invalid
configuration publishes no client and invalidates prior suppression as before.
The same initializer and cancellation locks own publication and wakeups; no
extra constructor, request, signature cache or foreground wait is introduced.

Reuse applies only to permitted literal-loopback HTTP. HTTPS reconstructs on
refresh because platform trust-root reload is not bound by this configuration
identity. The existing setup-contention, timeout, credential rotation, forged
reply and whole-session controls retain their original assertions and limits.
Focused identity controls and complete gates are required before this change
can establish a validated scope; it does not reclassify any preceding failure.

### Bounded trust and configuration discovery

The complete lookup clock now starts before helper dispatch. Discovery and trust
refresh run inside owned helpers, not before the deadline. Setup performs only
bounded optional presence discovery on the caller, then reads configuration,
credentials and CA files on its existing constructor owner. The sole presence
helper retains no client, credentials or trust model; an older/busy answer is not
reused as fresh authority. Status inspects complete snapshots with try-locks and
may omit unavailable optional fields. This corrects issue 76 independently of
unresolved emulated HTTP timing failures. No deadline, trust TTL, signature,
dynamic-connect policy or transport ownership rule changes.
