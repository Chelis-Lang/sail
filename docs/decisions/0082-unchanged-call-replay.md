# 0082: Replay unchanged calls from a complete stat-stamp vector

## Amendment: empty human Cargo output (2026-10-27)

A Current Cargo no-op can contain no observed targets when its preceding proof
was Historical and supplied no reusable receipt (#476). Such an observation
cannot satisfy the independently requested nonempty whole-proof inventory.
Whole and exact-partial proof replay keep their request and producer checks.

Human CLI calls have a separate byte-only record, `human-empty-v1-<key>.json`,
with a distinct kind/version envelope. Recording retains the original request
and observation vector, requires no verifier invocation or reused target, and
checks the actual tee output before writing. Replay rechecks the ordinary
workspace/family locks and complete recorded vector, the original result header,
independently resolved current request, raw native path binding and typed guarded
revision readback. Its shared parser admits empty stdout and exactly one plain
standard `dev`/`release` Cargo footer with a fully parsed numeric duration.
Warnings, extra lines, escapes and other profile renderings take the ordinary path.

The process-local admission exposes only those footer bytes. The actual human
consumer is selected before result-producing execution; it creates no refreshed
ProjectResult, verified label, receipt, run result or whole/partial proof token.
JSON, jobs, MCP, engine and independent-request-capture consumers cannot select
this path. Persistent protection, complete physical observations and real launch
origin retain their existing assumptions and open refinement obligations.

Implementation and direct-stock/native qualification are tracked separately in
the [owning record](../validation/acceptance-boundary-refinements.md).

- **Status:** accepted (owner approval, 2026-10-26)
- **Date:** 2026-10-26

## Context

An unchanged `sail verify` starts no verifier. It still costs 1.5 s on Anvil
(`--lib -- --verify-module vdeployment_controller`) and 0.76 s on Vest, where
stock `cargo verus verify` takes 0.26 s and 0.20 s (native c7i.8xlarge).
The breakdown shows Cargo running four times: Sail's own metadata, the build
environment query, the invocation and the post-run metadata. Around them,
Sail re-observes every input: about 234,000 `lstat` calls and 24,900 content
reads per Anvil call. Content hashing is not the bottleneck: a prototype that
remembered every digest across runs saved nothing measurable.

Until now Sail trusted stat stamps only within one run, between the stamps
bracketing that run's content reads. Cargo itself decides freshness from file
mtimes across runs.

## Decision

`sail verify` (human or `--output json`, managed, without the interactive
engine) may replay the previous identical call instead of running Cargo.
This extends stat trust across runs, as follows.

**Key.** The record is keyed by the invocation: the working directory, the
cache, the selected Sail, Cargo, Cargo-Verus, solver and shim paths, the
Cargo toolchain selection, whether a daemon is available, the arguments, the
output mode, whether stdout and stderr are terminals, and the value of every
variable the proof context binds, every `SAIL_*` variable, and `PATH`, `HOME`,
`TERM`, `PWD` and `CARGO_TARGET_DIR`. Job workers, observers (`SAIL_JOB_EVENTS`,
`SAIL_PROOF_EVENTS`), `SAIL_MANAGED_FORCE`, research modes and
`SAIL_MANAGED_NOOP=0` never replay.

**Recording.** A record is written only at the end of a run that ended
Current, started no verifier, did not retry, was not forced, reused proof
receipts, and placed nothing from the dependency store. It holds:

- the full `lstat` stamp (device, inode, size, mode, mtime and ctime with
  nanoseconds) of every path of the run's final source observation, exactly
  the set its mutation stamp covers: dependency-closure files and every
  directory walked (a new `src/bin` file changes its directory), workspace
  manifests and lockfile, every Cargo configuration and toolchain-file
  location (absent ones as absent), dep-info and build-script inputs, linked
  ancestors. These stamps must fold to the run's own post-run mutation stamp;
- the stamp of every entry of the owned Cargo target (artifacts,
  fingerprints, build-script outputs, directories);
- the stamp, following links and not, of every installed file the proof
  context identifies (Sail, Cargo, Cargo-Verus, the Verus engine, the solver,
  rustc, rustup, sysroot libraries), and of rustup's `settings.toml`;
- the content digest of every record in the proof context (except its run
  directories), the compiler family, sibling contexts' receipts and the
  runtime identities, with the complete set of entries in each;
- the value of every variable a unit declares (`env-dep`,
  `rerun-if-env-changed`);
- the run's report, its human stdout and its stderr.

A stamp is recorded only if its ctime is at least 2 s older than the run's
start. A user program cannot set ctime, and any change to a file advances it,
so a change after that point cannot share the recorded stamp even on a
coarse clock. A file changed during or just before the run is not recorded;
the next unchanged run records it.

**Replay.** Under the same workspace and family locks a full run takes, every
recorded observation is compared again. If all match, the call prints the
recorded stdout and stderr, with only the elapsed time in Cargo's `Finished`
line regenerated, and reports the recorded report under a new run identity
and wall time. It writes that result into the context's run directory, as a
full run does. Anything mismatched, missing or unreadable takes the ordinary
path, unchanged. A call that did not end replayable removes its key's record.

**Scope.** Replay applies only to calls whose full path started no verifier.
Stock re-verifies a failing project on every unchanged rerun, because Cargo
does not cache a failed build. Sail's full path then starts the verifier too,
so failing projects are never recorded and always take the full path. An
exact partial-scope JSON call (`--verify-module`) replays only to the same
arguments. Its report stays partial, and a whole-scope call has a different
key.

## Consequences

The trust is the same class of evidence as Cargo's own mtime fingerprints,
strengthened by inode, size, mode and ctime. What it cannot detect: a
filesystem whose ctime does not change on write, or a clock set backwards
past the recorded ctime, combined with an edit that keeps size, inode and
mtime. Content-preserving tools (for example `touch -r`) change ctime and so
miss. Writes Sail cannot see, such as an undeclared read by a build script,
are equally invisible to Cargo.

Measured on native c7i.8xlarge (us-east-1), nine alternating rounds with
`cargo verus verify` and the previous main: unchanged Anvil takes 26 ms
(main 1.503 s, stock 0.258 s) and unchanged Vest 26 ms (main 0.751 s, stock
0.201 s). The recording call costs 93 ms (Anvil) and 39 ms (Vest) over a
plain full-path call. The first unchanged call after a verification still
takes the full path and records. `SAIL_MANAGED_NOOP=0` disables replay;
`SAIL_MANAGED_NOOP_TRACE=FILE` logs each hit, miss reason and recording.
When an ordinary run cannot record, the trace also names its eligibility,
original-request admission or original-producer refusal. This optional file
does not change stdout, stderr, the recorded result or replay eligibility.
Revisit if Sail adds per-run side effects that consumers rely on (job events,
watch state) to the managed path. Replay is excluded wherever those are
requested.

## Amendment: solver identity (2026-10-26)

**Context.** Every shim handshake (one per Verus solver session) made the
daemon launch `z3 --version` and read the whole Z3 executable, because equal
path, length and mtime do not establish equal bytes (#213: a one-byte
replacement at fixed path, length and nanosecond mtime kept the old digest).
That cost 2 to 3.3 s per fresh project and 0.5 to 0.9 s per Vest edit.

**Decision.** The daemon reuses a solver's identity (its version output and
executable SHA-256) under the same rule as replay. It keys the identity by
the absolute solver path and the complete stamp of the file it resolves to
(device, inode, size, mode, mtime and ctime with nanoseconds; the shared
`sail_api::stamp::Stamp`). An identity is recorded only after a full fresh
identification whose stamps before and after are equal, whose ctime is at
least 2 s older than the start of that identification, and whose executable
is not an interpreter script (`#!`, whose output can depend on files the
stamp does not cover). A later handshake whose stamp is exactly the recorded
one skips the version launch and the read. Any other stamp, an unreadable
stamp or a relative path identifies afresh as before, and a launch, version
or read failure still refuses the handshake, so the shim falls back. The memo
lives only in the daemon's memory and holds at most eight paths.

**#213's scenario.** A same-path replacement by rename gives the path a new
inode and a fresh ctime. An in-place rewrite keeps the inode but advances
ctime, and restoring mtime with `utimes` advances ctime again. Either way
the stamp differs and the handshake re-identifies. The new file is then too
recent to record until 2 s after its last change. The settle rule covers
coarse clocks: a change after recording carries a ctime at or after the
recording's start, which is past the recorded ctime.

**Consequences.** The limits are those of replay: a filesystem whose ctime
does not advance on write, or a clock set backwards past the recorded ctime,
combined with a rewrite that keeps size, inode, mode and mtime. Version
output is assumed to be a function of the native executable; a script
wrapper is never memoized, so a wrapper whose target changes keeps the fresh
version check. The managed runner's once-per-run solver digest is unchanged.
Measured on native c7i.8xlarge (us-east-1), six alternating pairs against
main: a fresh project's first `sail verify` takes 30.46 s (main 31.13 s,
faster in every pair), with 366 sessions in both arms. A Vest edit's wall
time overlaps (22.55 s against 22.69 s), and the daemon's CPU per edit falls
from 3.40 to 1.04 s ([record](../validation/solver-identity.md)).

## Amendment: Cargo's copied build-script uplift (2026-10-26)

**Context.** On macOS no unchanged call was ever recorded (#437). Cargo
uplifts each compiled build script, `build/<package>-<hash>/build_script_<stem>-<hash>`,
to `build-script-<stem>` in the same directory. On Linux this is a hard link
that Cargo leaves alone when it is already the same file. On macOS Cargo
copies instead (rust-lang/cargo#10060), so on every run, fresh units included,
it removes the copy and copies it again. The copy and its directory then
carry a ctime from inside the run, and the 2 s rule refused the record.
Unchanged Anvil took 1.6 s and Vest 0.6 s, where Linux replays in about
26 ms.

**Decision.** Such a copy is bound by content instead of by stamp. A target
entry whose stamp is too recent is treated as an uplifted copy only when its
name is `build-script-<stem>` without an extension, its directory is
`build/<package>-<hash>/`, the directory holds `build_script_<stem>-<hash>`
with the same hash, both are regular files, and their bytes are equal. The
record then holds:

- the original's full stamp, under the ordinary 2 s rule;
- the copy's size and mode, with replay requiring a regular file of that size
  and mode whose bytes equal the original's;
- for the directory, its device, inode and mode and its complete sorted
  entry names in place of its stamp. Every other entry keeps its own stamp.

Anything else that is too recent, including a copy whose original is
missing, named for another hash, unreadable or different, still refuses the
record. Replay compares the copy with the original byte for byte instead of
recording a digest: reading both from the page cache and comparing them
costs less than hashing one (`same_content`), and the result is the same
because the original's stamp is bound. On Linux the hard links are settled,
so no path changes there.

**Soundness.** No file a call read can change undetected. The original is
bound as before. The copy's bytes are compared on every replay, so any change
to the copy (a rewrite, a replacement, a link in its place, another mode or
size) misses whatever its ctime. In the directory, any added, removed or
renamed entry changes the name set. A replaced directory changes the inode,
and a changed entry changes its own stamp. What the directory's stamp
recorded beyond that (its size and times) changes only through those events.
The comparison runs under the family lock, so no Sail run or prune changes
the target meanwhile, as for every other replay observation.

**Consequences.** Vest and Anvil unchanged calls replay on macOS. Measured
on EC2 mac-m4.metal (us-east-1, macOS 27.0, load average 9 to 14 from other
work), four unchanged calls 5 s apart after a cold run, with `cargo verus`
and the main stage:

| Project | Branch replay | Main | Stock |
|---|---|---|---|
| Vest (`-- --rlimit 100`) | 0.093 s | 0.638 s | 0.260 s |
| Anvil (`--lib -- --verify-module vdeployment_controller`) | 0.060 s | 1.643 s | 0.246 s |

Medians over the replayed calls; main never records, so its median is over
calls 2 to 4. The first unchanged call still takes the full path and records
(Vest 4 copies, Anvil 41). Anvil's copies hold 30.9 MB; on the same host,
reading and comparing them with their originals takes 4.4 ms single-threaded
against 10.9 ms for hashing the copies alone, and replay spreads the
comparisons over worker threads. CI runs on Linux, where the copies are hard
links, so the unit tests in `noop.rs` carry the copy cases.
