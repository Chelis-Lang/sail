# Freshness of retained workspace proof reports

## What you get

Every managed result (`sail verify --incremental`, `sail repair-check`, and the
same jobs through MCP) carries a Currentness label:

- **Current**: the result matches your code as it is now. No source file,
  manifest, lockfile, build-script input or Cargo configuration that Cargo
  compiled or read for the selected targets has changed since the run.
- **Historical**: one of those inputs has changed since the run. The result is
  still the true record of what was verified then, but it no longer describes
  your code. Re-run to get a current result.
- **Unknown**: Sail could not establish either, for example because a file
  could not be read or the run itself could not watch its inputs. Re-run.

These labels describe your inputs, not the solver's verdict. A Current result
can contain a failed proof. Only a Current, successful result can be reused as a
proof receipt or accept a repair. CLI and MCP clients receive the same label.

Any doubt makes a result Historical or Unknown, never Current. The cost of a
wrong "not current" is one re-run; a wrong "Current" is never acceptable.

## Reading a result later

When a run ends Current, Sail records the SHA-256 content digest of every file
in the selected targets' dependency closure, as Cargo resolved it: each closure
package's sources and build inputs, compiler-declared includes, the workspace
`Cargo.toml` and `Cargo.lock`, and every member manifest. It also records every
Cargo configuration path (`.cargo/config.toml` and `rust-toolchain` files in the
invoking directory's ancestors, and the Cargo home configuration), including the
ones that do not exist. The record is `read-check.json` in the run's proof
context under the Sail cache.

Each later read of the result (job Status or Result, `sail jobs`, MCP reads,
watch) rereads every recorded file completely, in parallel, and compares
digests. It also checks that the absent configuration paths are still absent.

- Every digest matches and nothing new appeared: **Current**.
- Any file changed, disappeared or appeared: **Historical**.
- A file cannot be read, or the record is missing or malformed: **Unknown**.

Nothing is inferred from timestamps, so an edit whose modification time was
restored still reads as changed. Files of packages that the selection never
builds, such as other workspace members' dependencies, are not part of the
record; editing them does not change the label. The toolchain itself (the Verus,
Cargo, Sail and Z3 binaries) is identified by the run's proof context rather
than reread on each read; a different toolchain gives a different context on
the next run.

Job submission uses the same check: a new request joins a running or completed
job only when the recorded inputs are unchanged. Without a record (no Current
run of that context yet), requests never coalesce.

## How a run decides it is Current

A run is Current only when all of the following hold:

- the content revision of its inputs (every source, manifest, lockfile,
  build-script input and compiler-declared include of the resolved packages,
  and the Cargo configuration) is the same after verification as before;
- the device, inode, size, mode, modification time and change time of every
  input file and directory, and of every installed toolchain file, are
  unchanged across the run. The change time cannot be set by user programs,
  so an edit that was restored during the run, even with its modification time
  put back, is still caught;
- on Linux, no absent Cargo configuration file was created during the run, even
  briefly (an inotify watch on those paths tells such a file apart from Cargo's
  own journal writes in its home directory);
- every unit Cargo compiled belongs to the selected targets' dependency closure,
  so the read-time record covers everything that was compiled;
- the build-script and compiler declarations were complete.

Otherwise the run is Historical (an input changed) or Unknown (one of the
checks could not be made), with the note "inputs could not be confirmed
unchanged; re-run for a current result". Verification itself still runs and
its report is kept as the record of that run.

Build-script coverage does not wait for scripts outside an explicit `-p`
selection's dependency closure, because they never run for it. Without an exact
selection or a complete, unambiguous resolve graph, every resolved package is in
scope.

### What these checks do not catch

Two rare cases can change the bytes a run verified without changing any
content digest or stamp, if the change is undone before the run ends:

- a process writing through a writable shared memory mapping of a source file
  while that page is already dirty, which does not update the file's
  timestamps;
- a bind mount placed over an input and removed again, which needs root.

Earlier versions held kernel read leases and watched the mount generation to
catch these, at a cost of about a quarter of a second on every run and a large
amount of code. Ordinary editors, formatters and agents write files, which
these checks do catch. If you run a tool that edits sources through shared
mappings during verification, re-run after it finishes.

## Reader latency

The M7 reader gate (four clients, 200 Status and Result reads, p95 below
200 ms, every sample Current while another verification runs) is measured
with `scripts/reader-gate.sh`; see
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).
