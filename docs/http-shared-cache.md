# Optional team cache over HTTP

Local `sail verify` remains automatic. A single developer needs no server,
Docker installation, signing key, account, credentials or remote configuration.
This feature shares signed SMT entries between machines; it does not share Cargo
artifacts or frontend verification receipts.

## Local trial

Start the server in one terminal:

```sh
sail cache serve --store /tmp/sail-team-store
```

It prints its allocated loopback endpoint and the paths of two private credential
files. It never prints their contents. The reader token allows GET; the writer
token allows GET and PUT. Ctrl-C stops the server. Existing credential files are
reused on restart. To rotate server credentials, replace the private files and
restart the server.

On a writer machine, verify normally to populate its local cache, then sign and
publish that cache:

```sh
sail verify
sail cache keygen --out team.key
sail cache publish --to http://127.0.0.1:PORT \
    --key team.key --token-file /tmp/sail-team-store/.sail-server/write.token
```

Distribute `team.key.pub` and the reader credential through your normal private
configuration mechanism. Keep the signing secret and writer credential with the
trusted publisher. On each reader:

```sh
sail cache connect http://127.0.0.1:PORT \
    --trust team.key.pub --token-file /tmp/sail-team-store/.sail-server/read.token
sail verify
sail status
sail cache disconnect
```

Replace PORT with the printed port. Readers on other machines use the HTTPS
endpoint described below. `--cache DIR` selects a particular local cache for
connect, disconnect and publication; by default these commands use the same
results directory as verification. Credential paths are made absolute and only
references are saved. Credential files must be regular, owner-only files with
at least 16 token characters. Connections and disconnects affect new daemon
lookups within five seconds, without restarting verification jobs.

An unchanged loopback HTTP configuration retains its connection pool after fresh
endpoint, private-token and CA validation. Rotated credentials, changed CA bytes
or a changed request timeout replace the transport; invalid configuration disables
remote lookup. HTTPS refreshes reconstruct the client so platform certificate-root
behavior remains unchanged. Signing trust and entry verification still run
independently on every accepted remote reply.

Disconnect removes the remote configuration. It preserves local imported entries
and trusted public keys. Removing a signing key stops future imports after trust
reload; previously imported local entries remain until local maintenance removes
them. This matches the existing signed-directory tier's trust policy.

## Private team deployment

Use HTTPS for remote access. Plain HTTP is accepted only at literal loopback IP
addresses; `localhost` is deliberately not an HTTP exception. URLs cannot contain
credentials, query strings or fragments. TLS certificates and hostnames are
verified. `--ca-file FILE.pem` on connect or publication adds a private CA.
There is no insecure TLS switch, automatic redirect or proxy environment fallback.
An explicit private CA file must contain at least one PEM certificate; empty or
non-certificate content is a configuration error.

The server is an HTTP backend. A non-loopback bind requires the explicit
`--behind-tls-proxy` option. Keep that backend private and expose a TLS reverse
proxy. `deploy/cache/nginx.conf` is a complete server block for an existing nginx
installation; supply your deployment's certificate and DNS name.

For a container deployment, build Sail's Linux x86-64 release binary with the
pinned toolchain and use the runtime-only image:

```sh
cp target/release/sail deploy/cache/sail
docker build --platform linux/amd64 -t sail-team-cache deploy/cache
docker network create sail-team-private
docker run --platform linux/amd64 --name sail-cache --network sail-team-private \
    --mount source=sail-team-store,target=/store -d sail-team-cache
```

Run your TLS proxy on that private network, exposing only its HTTPS port. No
backend host port is published above. The image runs as UID 10001 and requires
no Verus, Z3, Rust installation or corpus checkout. A bind-mounted store must be
writable by that UID. Credentials live under the store's `.sail-server` directory;
copy the reader token privately to readers and use the writer token only when
publishing. The store and credentials survive container replacement.

## Integrity, failure behavior and limits

GET and PUT use the unchanged
`v2/<platform>/<solver-sha256>/<key-prefix>/<key>` layout. Publications carry the
public key used to verify their signature. That key proves the upload's integrity;
it does not add the key to any reader's trust configuration. Readers independently
check their configured signing keys, signature, key, solver and platform identity,
reply hash, command shape, mode and unknown-serving policy. Only default-mode
entries are shared. Whole-session silence certificates are ordinary signed
entries and are published alongside replies, allowing a fresh reader to avoid
starting Z3 when the complete session is available.

Uploads spool to private temporary files. Signed entry headers are limited to
16 KiB, and signature validation borrows the reply buffer instead of copying it. A complete, validated upload is installed
atomically without replacing an existing publication. Identical bytes return
success; another valid publication at the same path returns a conflict. Publication
reports retain conflicts separately and continue with other entries. Because
publication time is signed, a later signing of the same local entry may conflict
with the earlier publication; the earlier valid entry remains available.

Publication uses four bounded workers. The server allows eight connections, at
most 64 headers and a 16 KiB header buffer; header reads expire after five seconds,
and upload bodies after 30 seconds. Every connection, including response writes to
a stalled reader, expires after 40 seconds. A filesystem operation already in progress keeps its
capacity reservation until it finishes, so timed-out requests cannot accumulate
unbounded replacement I/O.
Entries are limited to 64 MiB regardless of
Content-Length or chunked framing. Uploads stream to disk; download and signature
validation buffers are bounded by the same entry limit. At most eight server
entry operations are active, so their worst-case entry buffers total 512 MiB.
Use a local store filesystem; this server does not promise to interrupt a stalled
kernel filesystem operation. Graceful interruption removes partial upload files;
a process killed abruptly may leave private `upload-*` files that an operator can
remove while the server is stopped.

Configuration discovery, signing-key directory reads, token/CA loading and
client construction run on owned helpers, including startup and refresh. Signing
trust keeps its original five-second reload cadence. A pending refresh does not
serve entries under an expired key snapshot. No filesystem discovery or key read
runs on the lookup/status caller.

The daemon's entire shared lookup uses its existing 250 ms default fallback
deadline. This is not a hard real-time return guarantee: thread scheduling and
condition-variable mutex reacquisition can delay a caller. Results that reach
final admission after the deadline are misses. A cold connection may therefore
produce no hit. Client construction, transport scheduling, signature validation and request
I/O can consume that budget; a timeout safely falls back to the local solver.
Concurrent same-key requests share one in-flight lookup and receive the helper's
validated result without waiting for the initiating caller to resume. A helper
that finishes late cannot overwrite a published timeout. Optional transport
initialization starts asynchronously when a daemon binds with an HTTP store
configured. Setup sends no HTTP request and performs no entry or trust validation;
all serving checks remain on the lookup path. A single owned setup constructor
shares the existing constructor gate with lookup helpers. Setup never waits for
client construction. Implicit configuration discovery uses one metadata helper
and waits at most 100 ms (or the configured lookup timeout, if shorter); explicit
directory configuration needs no helper. Missing or busy discovery creates no
setup owner, and an unconfigured local daemon constructs no HTTP client. A stalled or
newly configured connection can still cost a miss while initialization is in
progress. `sail status` reads cached transport metrics without constructing or
retiring an HTTP runtime. One bounded presence observation covers its entire
optional shared-cache section. A busy discovery helper can omit that section;
busy snapshot locks can omit remote metrics or the last transport status.
Trusted-key IDs describe only a complete unexpired trust snapshot. The list is
empty when that snapshot is unavailable or trust refresh is pending, and is
never used to authorize an entry.

Startup no longer reads or lists malformed trusted-key files in the daemon log.
Inspect the configured trust directory explicitly with
`sail cache --cache DIR trust list`; ignored files appear as `ignored:` lines,
or in the `problems` array with `--json`. Invalid key files still authorize
nothing. An empty optional status key list can also mean a pending refresh;
it does not diagnose a malformed key.

Shutdown marks an unfinished setup owner abandoned before returning; that owner
cannot publish its completed client afterward. Completed owners are joined; an
unfinished blocking constructor is not joined on the bounded serving or shutdown
path. There is at most one setup owner per daemon and no overlapping constructor
retries. Failed construction retains the existing five-second reload cadence.
The [whole-session replay investigation](validation/m7-http-session-replay-777.md)
records the cold first-fetch failure that motivated this change.
Credential reload still invalidates old backoff and negative-cache state before
using the new configuration. Missing entries are
negative-cached for 60 seconds. Authentication errors, throttling, server errors,
transport failures and timeouts cause misses and 30-second backoff; rejected signed
entries remain misses. A remote failure never changes a verdict or prevents the
pinned solver fallback. Existing shared counters and HTTP request, authentication-refusal, unavailable,
coalesced-request and last-status fields appear in `sail status`. Authentication
refusals are reported separately from deadline timeouts. Error messages omit credentials,
response bodies and raw network errors.

## Validation

Fast tests cover writer/reader roles, untrusted signing keys, tampered and misfiled
entries, unknown refusal, interrupted and oversized uploads, path traversal,
concurrent immutable publication, chunked responses, redirects, content encoding,
stalled responses, CA trust, hostname mismatch and same-key coalescing.

The real-Z3 test fills a writer cache, publishes replies and the session certificate,
then uses an empty reader cache. Replies match direct Z3 byte for byte and the
reader starts no additional solver process. It runs in the existing solver gate:

```sh
SAIL_TEST_Z3=<pinned-z3> cargo nextest run -p sail-shim \
    --run-ignored only -E 'test(fresh_reader_replays_whole_session_without_starting_solver)'
```

Long corpus validation remains local-only. Test CA and private-key fixtures under
`crates/sail-daemon/tests/fixtures/` are exclusively for loopback tests; renew their
leaf certificate before October 2027. They must never be used in a deployment.

## Developer serving tests

The concurrent-reader, paused-initiator, status-refresh and disconnect regressions
establish their real HTTP transport and signing trust on an owned setup helper
before testing serving. Setup has its own two-second bound, records its actual
duration, and must send zero requests. Serving still uses the original 250 ms
lookup deadline, the server's 75 ms response delay, and the original exact
Hit/Rejected, one-request, signature and initiator-state assertions. Credential
rotation and invalid-credential rejection still run against the real transport.

Cold construction is tested independently by pausing the actual constructor.
Followers and status must remain bounded, no request may start while it is paused,
and a constructor completing after published fallback must skip that obsolete
request. A timeout is a safe miss; it never authorizes an unverified entry.

This separation follows the retained integrated run at source `d12d4af`, where
five HTTP fixtures failed before or during their first cold lookup. For example,
client construction consumed 152 ms before a paused-initiator test's network
request, and the validated result was published after 398 ms. The 250 ms serving
assertion failed as recorded. That run remains failed; establishing future
fixtures does not reinterpret it as a pass or change a production deadline.

Dev/test builds compile `curve25519-dalek` arithmetic with the production release
optimization level, while retaining debug assertions and overflow checks. The
same strict signature verification and all negative trust/binding checks run on
every entry. This removes measured unoptimized arithmetic work from the serving
budget; the 250 ms deadline and 75 ms delayed-server assertions remain unchanged.
CPU and wall-time evidence, including the original failed established-transport
test, is retained in
`validation/m7-shared-signature-cpu-diagnosis.md`. It is a development-build
improvement, not a production-release speedup or an attribution of every earlier
wall-time delay to cryptography.

The stalled-response integration test measures cold transport construction, the
original 50 ms request against a 300 ms server delay, and destruction of the
owned blocking client inside its unchanged 250 ms total bound. Its phase output
separates those costs without moving initialization or the runtime join outside
the assertion. HTTP lookup phase diagnostics retain the original signed-entry
checks and 75 ms response fixture; isolated CPU and TCP controls are diagnostic
evidence, not substitutes for the complete concurrent lookup checks.

[The retained HTTP follow-up](validation/m7-http-7dc-followup.json) records a
negative small-response TCP control and a fixed strict-signature CPU control.
Neither establishes the cause of the preserved full-gate failures.

## Current integration evidence

Local HTTP controls and signature regressions retain the same 250 ms lookup
bound and every trust check. The latest `749255e` full workspace attempt failed
two HTTP elapsed/serving cases; separate passing frozen controls do not qualify
the complete gate. [The phase record](validation/m7-http-749-followup.json)
separates those facts and retains the unresolved cause. An unavailable or slow
shared store still falls back to local verification; no timeout, unsigned reply
or diagnostic timing authorizes a cached proof.

### Rejected transport adapter experiment

Production retains the pinned reqwest blocking transport. An owned async-client
candidate passed 39 affected security and lifecycle controls, including the
unchanged eight-reader, one-GET fixture. Its once-only complete-operation
comparison was slower: combined helper and I/O CPU rose from 66.264 ms to
84.160 ms, and lookup-to-publication wall time rose from 185.680 ms to
222.422 ms. The smaller body-adapter cost did not justify selection.

The unselected patch and exact limits are retained in
[ADR 0037](decisions/0037-owned-async-shared-http-transport.md) and the
[comparison record](validation/m7-http-async-comparison.json). Both measured
fixtures passed the original 250 ms deadline and 75 ms response delay. Those
separate passes do not erase the failed full gate or qualify live readers.
No trust, signature, TLS, payload, timeout or fallback contract changed.

An additional unselected source candidate reads an exact small HTTP body within
the existing 8 KiB application buffer, avoiding the extra read used only to
observe EOF. Larger and unknown bodies keep the capped streaming path. Its
component controls and original eight-reader deadline still require execution;
it does not establish a cause or improvement for the retained failures. See the
[small-body plan](validation/m7-http-small-known-body-plan.md).
