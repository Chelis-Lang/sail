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

The server is an HTTP backend. A non-loopback bind requires the explicit
`--behind-tls-proxy` option. Keep that backend private and expose a TLS reverse
proxy. `deploy/cache/nginx.conf` is a complete server block for an existing nginx
installation; supply your deployment's certificate and DNS name.

For a container deployment, build Sail's Linux x86-64 release binary with the
pinned toolchain and use the runtime-only image:

```sh
cp target/release/sail deploy/cache/sail
docker build -t sail-team-cache deploy/cache
docker network create sail-team-private
docker run --name sail-cache --network sail-team-private \
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

Uploads spool to private temporary files. A complete, validated upload is installed
atomically without replacing an existing publication. Identical bytes return
success; another valid publication at the same path returns a conflict. Publication
reports retain conflicts separately and continue with other entries. Because
publication time is signed, a later signing of the same local entry may conflict
with the earlier publication; the earlier valid entry remains available.

Publication uses four bounded workers. The server allows eight connections, at
most 64 headers and a 16 KiB header buffer; header reads expire after five seconds,
and upload bodies after 30 seconds. Entries are limited to 64 MiB regardless of
Content-Length or chunked framing. Uploads stream to disk; download and signature
validation buffers are bounded by the same entry limit. At most eight server
entry operations are active, so their worst-case entry buffers total 512 MiB.
Use a local store filesystem; this server does not promise to interrupt a stalled
kernel filesystem operation. Graceful interruption removes partial upload files;
a process killed abruptly may leave private `upload-*` files that an operator can
remove while the server is stopped.

The daemon's entire shared lookup remains bounded by its existing 250 ms default.
Concurrent same-key requests share one in-flight lookup. Missing entries are
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
