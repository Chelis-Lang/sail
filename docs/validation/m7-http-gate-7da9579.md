# Original HTTP gate failures and one send/body diagnostic

The complete original workspace gate on `7da9579` ran all 522 tests and failed
two HTTP tests: eight readers coalescing one lookup, and a lookup waiting for
setup publication. The original 250 ms deadline, 75 ms server delay, signatures,
fixture code and internal concurrency remained unchanged. The
[portable failed gate record](m7-http-gate-7da9579.json) retains phase data and
raw log hashes. The independent 39-case solver pass is a separate scope.

In the eight-reader failure, one request and seven coalesced readers were
recorded. The server read a complete 285-byte request. Transport returned
252.417 ms after the first lookup started, after that reader's original budget;
signature classification took a further 11.298 ms. The trace includes 57.506 ms
from request construction to server acceptance, 50.046 ms from acceptance to
request read, and 54.260 ms from response send to the blocking client's return.
The setup-publication failure retained no complete phase trace. These facts
do not establish a complete scheduling, network or CPU cause.

One separately frozen diagnostic ran the same eight-reader fixture once. Only
bounded own-thread CPU observations and wall timers were added around the
blocking send and body read; diagnostic output was deferred until the fixture
released its transport. The exact fixture function remained byte-identical.
All original assertions passed, with publication 240.262 ms after the first
lookup. Send took 155.722 ms and body read 2.427 ms; both recorded zero helper
CPU ticks at 100 ticks per second. These coarse ticks omit the HTTP runtime
thread and server, and the observations themselves perturb timing. The
[diagnostic record](m7-http-sendbody-7da9579.json) seals source, dependency,
binary, patch and log identities. This single positive result does not replace
the failed complete gate.

Inspection of pinned reqwest 0.13.5 found its blocking client forwards a request
to a separate current-thread Tokio runtime and waits on a response channel.
Its blocking wait helper also constructs and enters a shell Tokio runtime
under debug assertions; release omits that check. That is a concrete additional
development operation, but the observations do not show it dominates this
request or caused either original failure. No profile, transport, signature
validation, fixture or deadline change was made. Further product work needs
causal evidence; successful isolated controls do not qualify the full gate.

Shared-store failures continue to fall back to the original solver. See
[http-shared-cache.md](../http-shared-cache.md) and
[ADR 0025](../decisions/0025-http-shared-cache-and-local-first-team-workflow.md) for the serving and trust
contract.
