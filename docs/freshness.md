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

- the content revision of its inputs, hashed after its guards are in place and
  again after verification, is unchanged;
- the device, inode, size, mode, modification time and change time of every
  input file and directory are unchanged across the run. The change time cannot
  be set by user programs, so an edit restored during the run is still caught;
- on Linux, kernel read leases on every input in the selected closure stayed
  unbroken for the whole run, and the mount table generation did not change
  (below);
- on Linux, no absent configuration path was created, even briefly;
- every unit Cargo compiled belongs to the selected closure, and build-script
  and compiler declarations were complete.

The run-time guard covers the packages Cargo can compile for the selection:
the dependency closure (every resolved dependency kind) of the selected targets,
plus the workspace manifest, lockfile and every member manifest. A workspace
can resolve far more than that. Vest's members resolve registry crates such as
`wasm-bindgen`, which are never built for `-p vest_lib`. Watching all of them
exceeded the guard's 8,192-watch bound, so every run was Unknown. Build-script
coverage likewise does not wait for scripts outside an explicit `-p` selection's
closure, because they never run for it. After every invocation, each
`compiler-artifact` and `build-script-executed` record Cargo emitted must name a
package inside both scopes. Otherwise the guard fails with
`native-observer-scope-exceeded` and the result is Unknown. Without an exact
selection or a complete, unambiguous resolve graph, every resolved package stays
in scope, as before. The whole-workspace input revision and stamps are unchanged,
so an edit to any package during a run still makes it Historical.

### Why the run keeps kernel leases and mount checks

Content digests and stamps catch every ordinary edit. Two rare cases slip past
them, and the run-time guard exists to catch exactly these:

- **A write through a shared memory mapping.** A process that keeps a source file
  mapped writable can change its bytes without updating the file's timestamps
  (while the page is already dirty) and change them back before the run ends.
  The verifier then checked bytes that neither digest nor stamp shows. A kernel
  read lease cannot coexist with a writable mapping, and any write breaks it.
  See `corpus/repros/linux-inotify-mmap/`.
- **A bind mount placed over an input and removed during the run.** Paths,
  bytes and the mount table look identical afterwards. The guard watches the
  kernel's mount generation, which any mount or unmount changes.

The absent-path observer catches a configuration file created and deleted
during the run (Cargo may have read it), without being confused by Cargo's own
journal writes in its home directory. Kernel leases on the toolchain sysroots
also let a run reuse sysroot digests at its final check instead of rehashing the
whole toolchain.

These guards apply only while a run is in progress. Reading a result later uses
only the content check above.

### The lease helper

Kernel leases are held by an embedded Python 3 standard-library helper launched
with isolated module loading, structured arguments and private bounded socket
I/O; Rust cannot handle the lease-break signal without unsafe code. Sail resolves
an absolute interpreter once per run and binds its loaded standard-library
files and mapped ELF libraries into the run's context; the helper checks its
actual loaded closure against that list before and after acquiring leases. It
owns at most 4,096 file descriptors and exits immediately on `SIGIO`, so an
editor's write proceeds at once and the run becomes not Current. The helper never
acquires or renews a lease after readiness, and barriers inspect kernel lease
state directly, with both complete inventories bounded by 2 MiB (see
[ADR 0029](decisions/0029-kernel-lease-work-bounds-and-freshness.md)).

Missing Python, denied lease ownership, unsupported filesystems (only local
ext2/3/4, xfs, btrfs, tmpfs, overlay and zfs are trusted), an existing writable
mapping or helper loss makes the run Unknown; verification still runs.
Unprivileged leases require matching file ownership. A root container may lack
`CAP_LEASE`; see [native product setup](native-product-setup.md). macOS has no
leases and relies on the content revision and stamps.

The precise observer that supports the guard indexes canonical watch membership.
A canonical path already in the index skips reparsing the owner's immutable
mount snapshot; it passed that same check when it was admitted. Canonicalization,
native masks and the 8,192-watch bound are otherwise unchanged.

## Reader latency

The M7 reader gate (four clients, 200 Status and Result reads, p95 below
200 ms, every sample Current while another verification runs) is measured
with `scripts/reader-gate.sh`; see
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).

## First root-binding mismatch diagnosis

When the run-time precise observer refuses a root binding, the completed job's
`SAIL3114` note can include one fixed first-mismatch row: `root-ordinal`,
`path-sha256`, and old/new `present(device,inode)`, `absent` or
`error(errno=...)` states. The ordinal starts at zero in the exact supplied root
order, and SHA-256 binds the path's native bytes. The digest is an identifier,
not a secrecy guarantee or freshness authority. No path string, environment value
or error message is recorded.
