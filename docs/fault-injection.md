# Fault injection

The charter's fault-injection tier: kill solvers mid-query, corrupt cache
entries, drop the daemon, race concurrent clients. Each must end in either a
correct answer or a safe fallback, never a wrong answer (rules 3 and 10).

The suite is `crates/sail-shim/tests/faults.rs`, with helpers in
`crates/sail-shim/tests/support/`. It runs against the pinned Z3 and is ignored
without it:

```sh
SAIL_TEST_Z3=<pinned z3> cargo nextest run -p sail-shim --run-ignored all
```

Earlier host measurements completed this suite in about nine seconds. Current
runtime depends on the platform and workload. The latest full solver suite
passes; an earlier timeout remains unexplained and retained below.

## Retained unresolved timeout

The integrated Linux run at source `62ad095` passed 36 of 37 solver checks, but
`unverifiable_shared_entries_never_change_an_answer` exceeded the unchanged
180-second CI limit and was terminated after 190.016 seconds. Its cause is not
yet established; the earlier successful scenario outcomes below do not make
that frozen run a pass.

Read-only inspection of the preserved scratch state showed completed forged,
untrusted-signer and other-solver stores with local entries. The platform store
had been rewritten, but its local cache held only the trust key; the truncation
case had not started. This narrows the unfinished phase without identifying a
production cause. No bound was raised and no check was removed. The targeted
diagnostic records case/round boundaries and, if the existing 60-second session
bound expires, the exact owned child tree, thread wait states and output size.
It does not read credentials or process environments.

A subsequent isolated run of this exact case, with diagnostic logging and all
original checks intact, passed in 11.340 seconds. All five cases and both
sessions per case passed; the platform-mismatch case recorded one rejection,
zero shared hits and zero shared timeouts. This did not reproduce or explain
the original integrated timeout. The
[portable diagnostic record](../bench/validation/shared-signature-fault-diagnostic-linux-emulated-2026-10-10.json)
retains both outcomes, the diagnostic invocation's initial solver-path setup
failure, and the executed source, binary and log hashes. No production fix or
deadline change is claimed.

The later complete `d12d4af` Linux solver suite passed all 37 cases in 114.446
seconds, including this shared-entry case in 9.652 seconds. It uses the original
assertions and deadlines with the phase diagnostics above. This is a complete
suite pass for those frozen artifacts, not an identified cause or causal fix
for the earlier timeout. The normal workspace gate on the same source failed
HTTP-cache concurrency tests and remains a separate blocker. See the
[complete solver record](../bench/validation/m7-linux-solver-d12-emulated-2026-10-10.json)
and [failed gate](../bench/validation/m7-linux-gate-d12-failure-emulated-2026-10-10.json).

## How faults are injected

- **The daemon** is the real `sail_daemon::serve`, in-process. Daemon faults
  come from a proxy on its own socket in front of it. The proxy relays
  protocol frames and counts requests by operation across all connections.
  On the *n*th `HELLO`, `GET` or `PUT` it can:
  - *die*: close the connection without replying, close every later one, and
    remove its socket;
  - *die mid-reply*: send half of the reply frame, then die;
  - *hang*: accept and read, but never reply again;
  - *delay*: send that one reply after the client has timed out.
- **The solver** is the pinned Z3 behind a wrapper that logs its pid, so a
  test can `kill -9` it while a query runs. A second wrapper replays a
  recorded output stream, stops partway through a reply, and kills itself.
- **The cache** is edited on disk between sessions.
- **Correctness** is judged against the pinned Z3 run directly. Statistics
  replies are compared by `rlimit-count`, the only figure plain Verus uses;
  every other reply must be byte-identical. The *replay check* recomputes the
  v2 key of every command in every session the test ran. Every entry the cache
  would serve (that is, every entry that passes `DiskCache::get`'s integrity
  checks) must be the direct-Z3 reply for its key.

## Scenarios

The client timeout is set to 300 ms with `SAIL_DAEMON_TIMEOUT_MS` in the hung
and slow daemon tests.

| Scenario | Safe outcome | Observed outcome |
|---|---|---|
| Daemon socket missing, or stale (nobody listening) | Pass-through | Pass-through; exact replies |
| Daemon dies at `HELLO` (closed, or mid-reply) | Pass-through | Pass-through; exact replies |
| Daemon dies while serving from cache (at `GET` 1, 4 or 9; mid-reply at `GET` 3 or 7) | The command is a miss; the solver replays the prefix | Exact replies, exit 0; the session finishes on the solver without recording |
| Daemon dies while recording after a miss (at `PUT` 1, 5 or 20; mid-reply at `PUT` 2 or 12) | Forwarding is unaffected; recording stops | Exact replies, exit 0; entries stored before the death pass the replay check |
| Entry corrupted: byte flipped in the reply or header, truncated to half or to nothing, overwritten with garbage, replaced by a directory | Miss | Exact replies over two sessions; corrupt files are removed when read and re-recorded |
| Two entries' files swapped; an entry replaced by another key's valid entry | Miss (the header names a different key) | Exact replies |
| An entry re-headered to claim another key, with a valid hash: an `echo` reply under a `check-sat` key and the reverse | Not served | The shape check rejects both; exact replies. The forged files stay on disk, because the daemon never overwrites an entry |
| Garbage at the key of a `check-sat` that answers `unknown`; stray `.tmp-*`, non-key and foreign-directory files | Miss; strays ignored | Exact replies |
| Solver killed mid-query, pass-through | Exit with the solver's status; output so far is a prefix | Exit 137 within the deadline; output is every reply before the killed query |
| Solver killed mid-query, cached mode after a miss (stdin still open, as with Verus) | As pass-through; nothing stored for the killed query | Exit 137; output is every reply before the killed query; no entry for its key |
| Solver dies partway through a reply (fake solver) | The partial reply may be forwarded, as pass-through would; it is never stored | Exit 137; output is a prefix; zero entries are published (ADR 0022) |
| 16 concurrent sessions sharing prefixes (cuts of the fixture, and variants that diverge after a cut), cold and then warm, and then 32 at once on an empty cache | Every reply exact; every entry correct | Exact replies in every session; every entry passes the replay check |
| Daemon hangs at `HELLO` | Pass-through after the timeout | Exact replies within the deadline |
| Daemon hangs at `GET` 1 or 6, or answers `GET` 3 or `HELLO` late | Miss after the timeout; the connection is not reused | Exact replies within the deadline |
| Daemon hangs at `PUT` 1 or 10, or answers `PUT` 4 late | Forwarding is unaffected; exit waits for at most one timeout | Exact replies, exit 0, within the deadline |
| Memory tier warm, then every stored entry changed under the running daemon: removed, a byte flipped in place, swapped with another entry, or replaced by a re-headered forgery (ADR 0015) | Every `GET` returns exactly what the disk tier holds now | Every `GET` matches the disk; later sessions are exact; the forgeries are not served (shape check) |
| `gc` to half the cache, `verify --remove` and `clear`, in a loop, during 16 concurrent sessions, cold then warm | Only misses | Exact replies in every session; every entry passes the replay check |
| Shared store published from another cache, read by a daemon on an empty cache (ADR 0017) | Served from the shared tier, copied locally with provenance `shared:<key id>` | Exact replies; every reply up to the one unpublished `unknown` came from the store; local copies pass the replay check |
| Shared store with every `check-sat` answer flipped and its hash fixed (a forgery with no valid signature) | Miss | Exact replies over two sessions; no forged answer reaches the local tier |
| Shared store with wrong answers signed by an untrusted key; or correctly signed by the trusted key for another solver binary, or for another platform class | Miss; foreign solver and platform counted | Exact replies; `shared_other_solver` and `shared_other_platform` count them; nothing wrong stored locally |
| Shared store with every entry truncated to half | Miss | Exact replies |
| Shared store hung: every entry a FIFO, so opening it blocks | One timeout, then the store is skipped for the back-off | Exact replies in two sessions; one timeout counted |

Disabling the signature check makes the forged-answer case fail with a
wrong `check-sat` reply, so the test does exercise the signature.

Daemon-side cases that need no solver run in the fast gate:
- **`crates/sail-daemon/tests/tiers.rs`:**
  - memory hits against outside changes to the disk;
  - write-behind: acknowledgement before the write, a full queue dropping
    `PUT`s, and draining on shutdown;
  - `gc`, `clear` and `verify --remove` racing six clients;
  - idle shutdown.
- **`crates/sail-daemon/tests/shared.rs`** (ADR 0017): a verified shared
  entry is served, copied locally with its provenance, and never published
  again. Each of these is a miss: forged (valid hash, wrong answer),
  untrusted signer, another solver, another platform, corrupt, truncated,
  empty, another key's entry, a near-limit `unknown`, the wrong shape, a
  directory. A rejected key is not looked up again for a while. Sessions
  without `HELLO` never consult the store. A hung store (a FIFO) costs one
  timeout, then is skipped until its back-off ends, and then serves again.
- **`crates/sail-cli/tests/shared_ops.rs`**: `keygen`, `trust`, `publish`
  and `sail serve --shared` as real processes.
- **`crates/sail-cli/tests/daemon_ops.rs`**, with a real `sail serve`
  process:
  - SIGTERM with writes queued: every entry is written, and the socket is
    removed;
  - `kill -9` with writes queued: some entries are lost, and every one left
    is valid and correct. `sail cache verify` passes, and a new daemon
    serves what is left exactly.

## Bugs found

The suite found three bugs, now fixed. ADR 0011 records them in its
2026-10-08 fault-injection update.

1. **Hang when the solver died in cached mode.** After a miss, the shim's
   main thread kept reading stdin and only waited for the solver once stdin
   closed. A client that waits for a reply before sending more (Verus does)
   waited forever. The test killed the solver mid-query, and the shim was
   still running 20 s later. Pass-through did not have this bug.
2. **A partial reply could be stored.** At the end of the solver's output,
   the last reply runs to the end of the stream. If the solver died partway
   through a reply, the partial bytes were added to the end of the reply
   before them. When that reply was a model or a `:version`, the result still
   passed the shape check, so it was stored and a later session would have
   served it. Example: a model followed by `<<DO`.
3. **Concurrent stores of one key shared a temporary file** in the disk
   tier. The renamed entry could mix two writers' bytes. That fails the
   integrity check, so the cost was a miss, never a wrong answer. A unit test
   in `sail-cache` reproduces it.

Hardening made alongside: after a daemon error, the shim drops the
connection for the rest of the session and does not reuse it for stores.
A late reply can therefore never be read as the answer to a later request.
The client timeout is configurable with `SAIL_DAEMON_TIMEOUT_MS`.

## Not covered yet

- Killing the shim itself (ADR 0005's known limitation: the solver runs on
  until it reads end of file).
- A real `sail serve` killed with a signal *during shim sessions*. Killing
  one with writes queued is covered without a solver (above). What a shim
  sees is what the proxy's closed-connection and removed-socket faults
  produce.
- A same-shape forgery in the *local* tier (an `unsat` stored under a key
  whose answer is `sat`). It passes every check. The local tier trusts its
  single writer, the daemon. The shared tier does not: there the same
  forgery is refused for want of a valid signature (above).
- A forgery signed by a *trusted* key. It is served: a trusted signer is
  trusted completely (`docs/trust.md`).

## Solver stderr (ADR 0022)

Real Z3 quantifier warnings match direct stdout, stderr and exit code on fill
and repeated runs. Late stderr and stderr larger than the socket buffer also
match and publish no entries. Quiet-session tests still require genuine hits.
Abnormal solver termination discards every staged reply, including complete
replies before the fault. Earlier prefix retention was a cache benefit, not a
correctness requirement.
