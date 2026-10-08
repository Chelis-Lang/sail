# 0015: Daemon operations, the in-memory tier and write-behind

- **Status:** accepted
- **Date:** 2026-10-08

## Context

ADR 0011 gave sail a daemon that serves a disk cache. Four things were
missing:
- **Operations.** The charter names `sail status` and `sail cache`. Neither
  existed, and nothing could inspect, check, shrink or empty a cache.
- **The in-memory tier.** The charter puts an in-memory tier in front of
  the disk tier.
- **Lifecycle.** The daemon `sail verify` starts on demand ran until killed,
  printed to stderr, and left its socket file behind.
- **Fill cost on macOS.** Filling the cache cost 1.66 times plain Verus on
  vstd on macOS (31.5 s against 18.9 s; +3% on Linux). Each shim's storer
  waits at exit for its `PUT`s, and the daemon wrote about 20,000 entry
  files synchronously, one per `PUT`, on a slow filesystem.

Constraints:
- Charter rules 3 and 10, and AGENTS.md's fail-safe rule: no failure may
  produce a wrong reply.
- The disk tier's integrity rules (ADR 0011): atomic writes, verified reads.
- sail must not change what Verus prints.
- The layering table.

## Decision

### Protocol

One operation joins the protocol: `INFO` (op 5, empty payload). It returns
the daemon's configuration and state as JSON: protocol and sail versions,
pid, uptime, socket, cache root, log path, idle timeout, the memory tier's
budget, entries, bytes and pending writes, and the write queue's budget,
entries and bytes.

`STATS` gains two counters, both with serde defaults so either side can be
older:
- `memory_hits`: hits served from memory, included in `hits`;
- `dropped_puts`: `PUT`s dropped because the write queue stayed full.

`STATS` and `INFO` are read-only and additive, so `PROTOCOL_VERSION` stays
at 1, as ADR 0012 did for `STATS`:
- an older client never sends `INFO` and ignores the new `STATS` fields;
- a newer client asking an older daemon for `INFO` sees the connection
  close. `sail status` then shows only the counters, and `sail cache` falls
  back to `--cache` or the default root.

No operation that changes the cache was added. Maintenance works on the
cache directory directly (below), and the memory tier is built to follow
the disk.

### The in-memory tier

`sail_cache::memory::MemoryTier` is a byte-bounded LRU map from key to
reply and effort. Its budget is 256 MiB by default (`--memory-bytes N`,
`SAIL_DAEMON_MEMORY_BYTES`, 0 to turn it off). Each slot is charged its
reply length plus 128 bytes. A reply larger than a quarter of the budget is
not held.

A slot holds one of two kinds of bytes:
- **On disk.** The bytes of an entry file, with that file's stamp: device,
  inode, length and modification time.
  - Such a slot is only filled from a disk read that passed the integrity
    checks, or from a write-behind write that completed.
  - Before serving it, the daemon `stat`s the entry file. It serves the slot
    only if the stamp is unchanged; otherwise it drops the slot and reads
    the disk.
  - Entries are only ever created by renaming a new file into place, so a
    replaced entry has a new inode. An in-place rewrite changes the
    modification time.
  - So a memory hit is always byte-identical to the entry the disk tier
    holds at that moment. An entry removed by `gc`, `verify --remove` or
    `clear`, from any process, is a miss at once, as is an entry corrupted
    or forged on disk.
- **Pending.** A `PUT` whose disk write is queued (below). No entry file
  exists for the key yet.
  - A `PUT` is not taken if an entry file already exists for its key, or if
    a write for that key is already queued. So a pending slot never shadows
    different bytes on disk. This matters: statistics replies for one key
    differ in their timing figures, and the disk keeps the first one stored.
  - When the write completes, the slot becomes on-disk with the new file's
    stamp. When the write fails or finds an entry already there, the slot
    is dropped.

The tier lives in `sail-cache`, the charter's home for every tier. The
daemon owns one instance and applies the rules above.

### Write-behind

The daemon acknowledges a `PUT` as soon as the entry is held in memory as
pending and queued for disk:
- One writer thread persists queued entries in order, with the disk tier's
  existing atomic write (temporary file, then rename) and its rules.
- `GET`s are served from memory meanwhile.
- The queue is bounded at 64 MiB of replies. A `PUT` that finds it full
  waits up to 1 second for room, then is dropped and counted in
  `dropped_puts`. A dropped `PUT` only costs a future miss.
- Shutdown, from the idle timer or a signal, drains the queue before
  exiting.
- If the daemon is killed, queued entries are lost. A write interrupted by
  the kill leaves at most a temporary file, which is never read as an entry.
  So the kill costs misses, never a corrupt or wrong entry.

This changes one guarantee of ADR 0011: a `PUT` acknowledgement no longer
means the entry is on disk. Nothing relied on it. The shim treats every
store as best effort.

### Access times and eviction

Every hit, from memory or disk, refreshes the entry file's access time if
it is more than 60 seconds old. Only the access time is set, so the stamp
does not change. Setting it explicitly also works on `noatime` mounts.

`gc` takes an entry's last use as the later of its access and modification
times, and evicts the oldest entries first. No index is kept, so nothing can
drift out of step with the files.

### `sail cache`

`sail cache [--cache DIR | --socket PATH] [--json] <stats|verify|gc|clear>`
works on the daemon's cache root, found with `INFO`, or on `DIR`. With no
daemon and no `--cache`, it uses the default root `sail serve` would use,
and says so.
- **`stats`:** entries and bytes, by command; `unknown` answers with their
  `reason-unknown` counts, how many used at most half their limit, and the
  spread of used/limit; counts of invalid and temporary files.
- **`verify [--remove]`:** checks every file under `<root>/v2`:
  - the header parses and has this format version;
  - the key in the header matches the file name;
  - the reply hash matches;
  - the reply has the shape its command requires. The header records the
    command's name only, so this is a name-level form of the shim's shape
    check.

  Stray files, key-named directories and temporary files older than an hour
  are also bad. Exit 5 (`SAIL0501`) if bad files remain. `--remove` deletes
  them.
- **`gc --max-bytes N`:** evicts least recently used entries until entry
  files fit in `N` bytes (`K`, `M`, `G` and `T` suffixes), and removes
  temporary files older than an hour.
- **`clear --yes`:** renames `<root>/v2` aside, then deletes it, so every
  entry disappears at once. Without `--yes` it refuses (`SAIL0502`, exit 2).

Every operation is safe while a daemon serves and writes the same cache:
- files are only ever removed whole;
- a removed entry is a miss;
- the memory tier follows the disk (above);
- young temporary files may belong to a write in progress, so they are never
  touched;
- writes still queued when `clear` or `gc` runs land afterwards, and are
  served from memory until then. They are correct entries, so this only
  means a cleared cache can hold a few entries again at once.

`sail-cli` now depends on `sail-cache` directly, for these operations: a
new edge in the layering table. The CLI already reached `sail-cache`
through `sail-daemon`.

### `sail status`

`sail status [--socket PATH] [--json]` asks the default socket (or
`$SAIL_DAEMON_SOCKET`) for `STATS` and `INFO`, counts the disk tier's entries
and bytes, and prints:
- the protocol and sail versions, pid and uptime;
- the idle timeout, cache root and log;
- the disk and memory tiers and the write queue;
- the counters: sessions (`HELLO`s), gets, hits (and memory hits), puts and
  dropped puts.

If no daemon answers, it prints `SAIL0401` and exits 4. `--json` then
prints `{"running": false, ...}`.

### Lifecycle

`sail serve` takes these settings:
- **Idle timeout.** It stops after 30 minutes with no open connections
  (`--idle-timeout SECS`, `SAIL_DAEMON_IDLE_TIMEOUT`, 0 for never). The
  `sail serve` that `sail verify` starts therefore goes away by itself. A
  later `sail verify` starts a new one, and the disk tier persists.
- **Log.** It logs to `<cache root>/daemon.log`: start, configuration, stop
  reason, final counters and write errors. Its only stderr output is one
  line at start, which `sail verify` discards. A log larger than 1 MiB at
  start is moved to `daemon.log.1`.
- **Signals.** SIGINT, SIGTERM and SIGHUP stop it cleanly, through
  `signal-hook`, a new dependency of `sail-cli` only. It is used because
  sail's own code may not use `unsafe`, and the queue must be drained on
  an explicit stop.
- **Stopping.** On any clean stop it:
  1. stops accepting connections;
  2. waits up to 5 seconds for open ones;
  3. drains the write queue;
  4. removes its socket, if the file is still the one it bound.

The bench and the conformance step pass `--idle-timeout 0`, because they
manage their daemons themselves.

### Fault-injection knob

`SAIL_DAEMON_WRITE_DELAY_MS` slows every disk write by that many
milliseconds. It exists only so tests can stop or kill a daemon with
writes still queued. It never changes what is written.

## Consequences

**Tests in the fast gate** (no solver, about 2 seconds):
- `crates/sail-daemon/tests/tiers.rs`:
  - memory hits match disk after every kind of outside change;
  - a `PUT` is acknowledged before its write, and shutdown drains the queue;
  - a full queue drops `PUT`s without storing anything wrong;
  - `gc`, `clear` and `verify --remove` racing six clients only ever cost
    misses;
  - an idle daemon stops, removes its socket and logs.
- `crates/sail-cli/tests/daemon_ops.rs`, against a real `sail serve`
  process: `sail status`, every `sail cache` command, SIGTERM draining the
  queue, idle exit, and `kill -9` with writes queued (entries are lost,
  never wrong).

**Solver-backed tests** (`crates/sail-shim/tests/faults.rs`, part of
`ci/run.sh solver`, a few seconds each):
- the memory tier never serves what the disk does not hold, under real shim
  sessions;
- `gc` and `clear` during concurrent sessions only cost misses.

**Measurements.** These were taken locally on macOS arm64, on a machine
shared with other jobs (load average 20 to 35), so expect noise of tens of
percent. Benchmarks stay out of CI.

- **Daemon alone.** 20,000 synthetic `PUT`s, then three passes of `GET`s
  over every key, from one client:

  | Daemon | 20,000 `PUT`s acknowledged | On disk | `GET`, per call |
  |---|---:|---:|---:|
  | Before (synchronous writes) | 18.6 s, 27.0 s | same | 85 to 174 µs |
  | Write-behind, memory tier on | 0.9 s, 0.6 s | 7.4 s, 3.8 s | 25 to 88 µs |
  | Write-behind, memory tier off | 1.1 s, 1.1 s | 7.2 s, 5.8 s | 112 to 167 µs |

  A `PUT` used to cost about 1 ms of the shim's time. Now it costs about
  50 µs. A memory hit costs a `stat` instead of an open, read and hash.
- **vstd fill** (`times-ms.total`; three rounds, interleaved):

  | Round | Plain Verus | Fill, before | Fill, write-behind |
  |---|---:|---:|---:|
  | 1 | 54.4 s | 57.7 s | 56.5 s |
  | 2 | 92.1 s | 68.4 s | 51.1 s |
  | 3 | 55.8 s | 54.3 s | 48.4 s |

  Every fill stored all 19,876 entries. A stop with SIGTERM right after the
  run took 0.03 to 0.06 s, so the queue had already drained. The load
  swamps the 1.66× overhead measured earlier; the fill with write-behind
  was the fastest of the three in every round.
- **Warm workload.** `sail-bench warm` on the fast tier's guide examples:
  33 units, 1 fill round and 2 warm rounds each, run twice with the memory
  tier on and twice with it off. Warm wall time over plain was 0.82 and
  0.87 with memory, against 0.85 and 0.83 without. All 6,484 warm hits came
  from memory. There were 99 same and 0 different conformance results in
  every run. The memory tier has no measurable end-to-end effect here: a
  warm unit is dominated by Verus itself (ROADMAP, "warm solver pools would
  not pay off"), and these units make at most a few hundred `GET`s.

**Revisit:**
- a daemon killed by SIGKILL still leaves a stale socket. The next start
  replaces it, and clients treat it as no daemon;
- the 256 MiB memory and 64 MiB queue defaults, when larger corpora are
  measured;
- whether `gc` should run automatically against a configured budget.
