# Common-target solver process observer

[Observer](../../scripts/m7-faults-live-observer.py) is a separate read-only Linux
process. It starts no test, solver, daemon or compiler and sends no signals.
The original suite, 60-second session deadlines, 180-second test timeout,
termination grace, full-byte assertions and result classification stay unchanged.
Preparation has 15 [pure/fake-proc controls](../../scripts/test-m7-faults-live-observer.py)
passing; live suite observation has not yet run.

The caller supplies the finished common-target faults ELF's exact pathname,
SHA-256 and frozen source commit. Only a regular, caller-owned ELF named
`faults-<hex>` directly under the common debug dependency directory qualifies.
Aliases, globs and private build targets refuse. Startup streams the digest
under a 128 MiB size bound. Each snapshot checks the frozen executable's inode,
size, owner and change timestamps before and after collection. A process root
must expose that exact path and inode through its executable link; unsupported
guest executable reporting produces missing/partial coverage, without guessing.
The source commit is the caller's provenance label, rather than inferred source
ownership from a process.

Discovery reads only bounded kernel birth/parent/state/CPU metadata and
executable links. Detailed thread/FD observation applies only to qualified roots
and birth-bound descendants. Every new edge requires a live parent and child
birth no earlier than its parent's birth. The complete recorded chain is checked
again around collection. Recycled process or thread identities discard their
data. A previously admitted descendant can remain observable after its parent
exits, using its retained birth token; an unobserved orphan is never guessed.
No process environment, command line, arbitrary file content or FD target is
opened. Thread records retain TID, birth, name and wait channel. FD records
retain the link target and a fixed fdinfo whitelist: position, flags, mount ID,
inode and kernel-supplied eventfd count/pipe size. Replaced targets or changed
inode/mount/flags during the read refuse that FD record.

These are concurrent snapshots, not an atomic ownership census. Reuse of one FD
number for the same target can be indistinguishable. Position is not queued pipe
bytes; the observer does not open pipes or issue queue-length operations. Wait
channels do not establish a userspace stack, mutex owner or historical cause.
Ordinary descendant disappearance, permission failures and truncation remain
explicit unavailable records. Empty records do not certify actor absence.

Fixed limits are 4,096 discovered PIDs, 128 selected processes, 1,024 retained
birth identities, 256 threads and FDs per process, 1,024 threads and 2,048 FDs
per cycle, 4 MiB of metadata reads per cycle, a 1-second collection work budget,
4 MiB per JSON record and 32 MiB total output. The default interval is five
seconds, with at most 180 polls. The work clock and read bounds are checked
between operations; kernel filesystem calls and output scheduling are not
interruptible deadlines. JSON encoding stops at the record bound. A reserved
terminal marker reports controller stop, storage cap, poll cap or observer error.
The output is new-only and mode 0600. A cap remains partial evidence rather than
changing or interrupting the test.

Use a fresh native-owned diagnostic directory outside the test scratch and
common target. Copy the reviewed script there and verify its digest before
startup. Supply `SAIL_OBSERVER_ELF`, `SAIL_OBSERVER_ELF_SHA256`,
`SAIL_OBSERVER_SOURCE` and `SAIL_OBSERVER_DIR` from the finished root build.
Run in the same container as the original suite:

```sh
printf 'workspace\n' > "$SAIL_OBSERVER_DIR/phase"
python3 "$SAIL_OBSERVER_DIR/m7-faults-live-observer.py" \
  --elf "$SAIL_OBSERVER_ELF" --elf-sha256 "$SAIL_OBSERVER_ELF_SHA256" \
  --source "$SAIL_OBSERVER_SOURCE" --out "$SAIL_OBSERVER_DIR/observations.jsonl" \
  --stop-file "$SAIL_OBSERVER_DIR/stop" --phase-file "$SAIL_OBSERVER_DIR/phase" \
  > "$SAIL_OBSERVER_DIR/observer.stdout" 2> "$SAIL_OBSERVER_DIR/observer.stderr" &
SAIL_OBSERVER_PID=$!
```

At the actual solver step, the controller may write `solver` to the phase file.
The fixed label is supplied by the controller; process observations never infer
test names or suite phase. Missing or invalid labels remain unattributed or
unavailable. Workspace observations must be distinguished from ignored solver
observations. After the suite's original terminal cleanup, stop and reap only
the observer through its cooperative stop marker:

```sh
: > "$SAIL_OBSERVER_DIR/stop"
wait "$SAIL_OBSERVER_PID"
```

Retain JSONL and observer stdout/stderr alongside the original complete suite
log and ELF/source seals. If the observer fails or caps, keep that failure;
the suite's verdict and timeout remain authoritative. The earlier original GC
timeout and separately passing isolated diagnostic stay separate. This tool
adds no acceptance, performance, cleanup or causal credit.
