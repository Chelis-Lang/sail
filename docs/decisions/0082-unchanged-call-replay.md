# 0082: Replay unchanged calls from a complete stat-stamp vector

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
