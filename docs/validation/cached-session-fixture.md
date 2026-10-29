# Cached-session fixture readiness

Status: fixture correction passes native gates and repeated controls; hosted
checks remain required before merge.
Tracks #616. Production shim, daemon, cache, protocol and goldens are unchanged.

Hosted source `de8961b8` fails one of 51 solver tests: the cached-statistics
control prints the genuine Z3 statistics instead of its deliberately rewritten
entry. Exit0 and stderr agree. Its test/production solver paths are unchanged
by the metadata candidate. The native preceding solver51 checks pass. The
cause of this particular failure remains unknown.

The fixture did contain a readiness gap: it spawned `serve` asynchronously
and waited only for the socket path to exist. A Unix socket name can appear
before `listen` finishes, and daemon initialization follows binding. The
fixture now binds and initializes synchronously before spawning `Server::run`,
as the daemon's existing integration fixtures do. Binding failures are explicit.
All cache-hit, corruption, fallback, stdout, stderr and exit assertions remain.
No sleep, deadline or retry allowance is added.

Raw failed hosted job log SHA-256:
`39e56589ad2e2162683edf734400d75284d21dd0a6cda9b7793b4e0287fd7e1b`.
A passing rerun alone will not establish the cause of the original failure.

On 2026-10-29, native c7i.8xlarge/us-east-1, source `3f09465d` passes the
complete image gate (1,242 ordinary tests), solver51 and 100 separate invocations
of the exact failing test. An unchanged-fixture control at `f3f7b9ee` also
passes all 100 invocations. Thus this experiment does not reproduce or explain
the original hosted miss. No failed repetition is discarded.

Corrected native log SHA-256:
`aefc365d3bf0b9e8ffb17944c4df30108ab13324d4df15d525425b66d1ee6823`.
Indexed attempt: the original repeated-control wrapper completes all 100 calls,
then its identity-report command refuses the container mount's Git ownership.
The corrected report explicitly reads that known checkout; no source check or
test assertion changes. Native Mac host qualification is recorded separately.

After rebasing onto current main, `1c2d1f30` passes the native complete image
gate (1,247 ordinary tests) and solver51 again. The Mac qualification at
`3f09465d` passes strict Clippy and 1,231 host tests on
mac-m4.metal/us-east-1b/macOS 27.0. That fixture/helper and its solver paths
are byte-identical after rebase; main's added metadata path is independently
qualified and is not called by cached-session controls.

| Final log | SHA-256 |
|---|---|
| Native current-main gate and solver | `70336d47e10ebbbc87da9b771ba8c7cb985a4448fae41f3ef70014707ad7123f` |
| Mac host gate | `ac960cedfc5e4161831e306b05b11fe0d13be255eb7fcf395b1ac888ac80e0dc` |
