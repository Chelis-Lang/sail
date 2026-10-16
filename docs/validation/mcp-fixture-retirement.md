# MCP socket retirement and job fixture setup

| Field | Value |
|---|---|
| Status | Candidate: focused macOS checks and full host gate pass; combined-fixture native Linux gate and PR checks remain. Both fixture failures from [#103](https://github.com/Chelis-Lang/sail/issues/103) now have reproduced causes and candidate corrections. Native qualification remains pending. |
| Scope | Test-only owned Unix socket server. Production MCP, job cancellation, transport and verification behavior are unchanged. The timed job-tool fixture also uses an installed interpreter for fresh command text. |
| Environment | Shared Apple M3 developer machine, macOS 26.5, Rust 1.99.0; correctness reproduction and stress checks, not performance measurements. |
| Original source | `0a7c1d0d`, unchanged `crates/sail-mcp/src/test_io.rs`, compiled directly as a Rust test binary. |
| Original failure | One stalled run in 100 invocations and another in 300, four invocations at a time. A live sample shows fixture retirement waiting in `join` while its worker remains in `recvfrom`, beyond the configured two-second read timeout. |
| Candidate | Accepted sockets stay nonblocking. Each fixture read/write checks an owned stop flag and its existing deadline. Retirement sets that flag before joining. A retired read returns EOF; a retired write fails. No thread is detached and no retry or relaxed assertion authorizes a pass. |
| Checks | 61 MCP library tests pass, including cancellation/framing controls and the new silent-peer timeout check. All 1,200 independent fixture stress invocations pass, four at a time. Pedantic release clippy passes. The complete combined macOS host gate passes: 614 tests, 35 skipped. The earlier socket-only candidate passed 613 tests; the combined run also covers the interpreted job fixture. |
| Documentation | This record and the developer documentation index. No user-guide change: the implementation is compiled only for tests. |

The original one-second retirement assertions, protocol checks and the
independent job-tool test's two-second limit remain unchanged. The fixture's
read/write limit is still two seconds by default; explicitly shorter read
limits remain effective. The new check leaves a peer open and silent, confirms
the requested timeout, and confirms that retired I/O returns while the peer
is still open.

The captured stacks identify the unbounded blocking operation in this fixture;
they do not establish an underlying kernel defect or qualify production job
cancellation. The separate aggregate job-tool timing failure is reproduced and addressed
in the candidate fixture setup described below; it is not a production
CLI startup optimization.

## Evidence and attempts

Raw records remain outside the repository:

| Date | Source/scope | Result | SHA-256 |
|---|---|---|---|
| 2026-10-16 | Original fixture, 300 standalone invocations | One stalled worker; owner waiting in `join` | Run summary `ec109d09a1ab9c508cc1ab7b5af599c2d8087104708f7567883cee43e00a4ac9`; live stack `e8a2802f69a05c236c0c0a5588055185def5971536b9ff601dbe80f58ca67030` |
| 2026-10-16 | Nonblocking fixture, 1,200 invocations | All pass | `6f6394208025fb917d3c9d61d66fc11aae96e8bdd3c8365c1e3e45b8bb9027be` |
| 2026-10-16 | MCP library tests | 61 pass | `ecdfcd057fe6cc5443b7d3b42538df32a07244efdd1abe70ec0999513c1a6a33` |
| 2026-10-16 | Combined macOS host gate, both fixture corrections | 614 pass, 35 skipped; all-target clippy passes | `1808eff28819456dc519b7cf7c1c7387b8c2fe4de7127e629410c0037b066407` |

No corpus entry, runtime, solver key, verdict or diagnostic is changed.
Conformance and performance claims are unaffected; required PR checks still
run in full.

## Timed job-tool fixture

The unchanged two-second job-tool check fails in 8 of 200 independent
invocations on this shared Mac, four at a time. The blocked worker remains
running. The first `start_verification` operation takes 1.85–2.11 seconds in
representative failing runs; later operations are tens of milliseconds.
The original run summary SHA-256 is
`ae44977aa90d04c8e1b47788b78e3f64172adebc5270ad8c71355d53430562bf`.

A temporary diagnostic around the unmodified child-process runner separates
spawn from exit/reap. Spawn returns in about a millisecond, while first
execution of the newly created script can take over a second; later
executions are much shorter. This identifies the fixture's first-execution
phase, not an underlying operating-system cause. The temporary instrumentation
was restored byte-for-byte and is absent from the implementation. Its source
SHA-256 is `4d774fcaed5ebae0cd263b6c8278cd3486437cbc657418ac5458c8491b16d1ce`.

Only this timed fixture now calls the installed shell with fresh owned
`verify` and `jobs` command files. The command text reconstructs the original
argument list before dispatch and reads the same fresh JSON responses from
the same working directory. Every child launch and command/response read
still occurs inside the original timer. There is no executable warmup, retry,
pre-executed tool call or increased deadline. The other fixtures retain their
private executable because some tests deliberately replace it; the installed
interpreter is never written or removed.

All original lifecycle, historical-provenance, model, counterexample, missing
obligation, blocked-worker and two-second assertions remain unchanged. The
candidate passes all 200 independent invocations at the same concurrency;
summary SHA-256
`f23d421264a16e649916a5211280fe442042afbf39325b984d0ea4035ed57230`.
These are fixture correctness checks; they do not establish a guarantee about
real Sail CLI first-launch latency.
