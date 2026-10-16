# 0045: A machine-local store of verified registry dependencies

- **Status:** accepted (owner direction 2026-10-15; implemented)
- **Date:** 2026-10-15
- **Extends:** [ADR 0023](0023-managed-incremental-workspace-verification.md)
  (receipts) and [ADR 0036](0036-managed-compiler-artifact-families.md)
  (one Cargo target per compiler family) beyond one workspace
- **Roadmap:** item B1 of the
  [2026-10-13 roadmap review](../reports/2026-10-13-roadmap-review.md)

## Context

- **Owner direction (2026-10-15):** "if sail ships with its own prevalidated
  fork of verus, then sail shouldnt be doing rework on every first call."
  Every new project, worktree or agent sandbox verified vstd before anything
  else: 21–30 s on a native host, 37–60 s on a loaded Apple M3.
- **Where the time goes.** On the M3, vstd's own `--time` report gave 38.8 s
  wall: 9.9 s in the Rust frontend (types, checks, compile) and 27.9 s in
  verification (4.3 s VIR, the rest lowering and SMT on 7 threads,
  153 s CPU). The whole vstd crate is the work; a partial skip is no use.
- **What plain `cargo verus verify` does.** With a crates.io `vstd` (the
  README's First verification, ADR 0043 decision 8), Cargo compiles vstd and
  its dependency closure (`verus_builtin`, the two macro crates, `syn`,
  `quote`, `proc-macro2` and others: 15 packages with vstd) into the
  project's target directory, and the verifier verifies vstd: stdout gets
  `verification results:: 2060 verified, 0 errors` before the crate's own
  line. A later run in the same target finds all of it fresh and prints
  nothing for it.
- **A prebuilt vstd cannot be shipped.** The owner asked to ship vstd
  pre-built and pre-verified. Two facts rule out an artifact built elsewhere:
  - *Diagnostics name vstd's source by absolute path.* A failed vstd
    precondition renders
    `::: $CARGO_HOME/registry/src/index.crates.io-1949cf8c6b5b557f/vstd-0.0.0-2026-10-04-0306/std_specs/vec.rs:55:9`,
    taken from the compiled metadata. A vstd compiled under another Cargo
    home would print another path: a diagnostic plain Verus never prints
    (charter rule 1).
  - *Cargo would not accept it.* Cargo's fingerprint of a registry unit
    includes its source path, which lies under the Cargo home, so a unit
    built under another Cargo home is stale and Cargo rebuilds and
    re-verifies it anyway.
  A unit compiled on the user's own machine, under the user's own Cargo home,
  has neither problem.
- **Cargo accepts a copied target.** Copying a project's target directory to
  another path and verifying a second project with the same `Cargo.lock`
  there rebuilt only the second project's crate (2.7 s instead of 37 s on the
  M3): Cargo's fingerprints are independent of the target directory's path
  and of the workspace for registry packages.
- **Scope (owner, 2026-10-15).** Managed verification (CLI `--incremental`,
  MCP, jobs, `watch`, `repair-check`) is becoming what `sail verify` runs; the
  ordinary non-managed path is going away, so it gets no seeding.

## Decision

1. **A store, filled by the first managed run on the machine.** A Current
   managed run that holds a valid receipt of its own for a store candidate
   (below) copies the candidate's compiled Cargo units, its receipt, its
   build-script contracts and the verifier's recorded output for it into
   `<cache>/dependency-store/v1/<key>/`. A later managed run whose compiler
   family target does not exist yet (a new project, worktree, sandbox, or one
   after `sail cache projects prune`) copies the units of every candidate the
   store holds into that new target before Cargo starts. Cargo's own
   fingerprints then decide what is fresh, exactly as for a target plain
   Cargo-Verus had built itself; anything Cargo does not accept it rebuilds,
   and the verifier verifies it as usual. Release archives carry nothing new:
   no prebuilt vstd can match the user's machine (Context).
2. **Candidates.** A package declaring `package.metadata.verus.verify = true`
   that is in the selected targets' dependency closure, is not a workspace
   member or a selected target, and whose entire dependency closure comes from
   a registry or git source. Path dependencies are excluded: their bytes are
   per checkout, and Cargo shows their lints, which seeding would hide.
3. **Key.** SHA-256 over: the compiler-family digest (ADR 0036: the exact
   arguments, the stat identities of Sail, the shim, Cargo, rustc, rustup,
   the verifier and its sysroots, the engine identity, every relevant
   environment variable and the bytes of every Cargo configuration file), the
   candidate's package id, every resolve node of its closure (package ids,
   enabled features and dependency edges), and the workspace manifest's
   `[profile]` table. Any difference is another key: an ordinary miss.
   Cargo-Verus derives a dependency's verifier arguments only from its own
   arguments (in the family digest) and the `package.metadata.verus` of the
   closure's packages (registry bytes, bound by the content revision below);
   it reads no workspace metadata (`cargo-verus`
   `make_cargo_plan`, verus@426d8b0), so nothing else needs to be in the key.
4. **What an entry holds.** Every file of every Cargo unit of the closure
   (`.fingerprint/`, `build/` and `deps/` entries named by the unit hashes in
   Cargo's own artifact records of the run, `.vir` files included) with its
   SHA-256, size, permission and modification time; the receipt; the closure
   packages' build-script contracts; the content revision of the closure's
   source files; and the verifier's captured stdout and stderr for the
   candidate. A manifest checksum binds all of it. Entries are written to a
   staging directory and renamed into place, so readers see whole entries
   only; concurrent publishers of one key keep the first and discard the
   rest.
5. **Using an entry** requires, before anything is copied: an intact manifest
   for exactly this key and package; a sealed, complete, successful receipt;
   an unchanged closure content revision; unchanged declared environment
   variables; and current build-script contracts. Each stored path must be
   relative and plain (no `..`). Files are copied, with their digests checked
   on the bytes written and their recorded modification times (Cargo compares
   unit times), into a hidden sibling of the new target, which is renamed to
   the target only once every placeable entry is complete. A run killed while
   seeding therefore leaves no target, only a hidden leftover that the next
   seed removes. The receipt is then moved to the target's paths and its
   artifact and dependency digests are checked against the bytes there.
6. **Reuse is still a receipt check.** The seeded target is reported reused
   only if Cargo reports it fresh and the moved receipt matches the artifact
   names and bytes Cargo reports, the compiled dependencies' bytes and the
   declared environment, as for a sibling receipt (ADR 0036, L23). When the
   run ends Current, the receipt is adopted into the run's own proof context,
   bound to that context and its input revision by its own commit marker, so
   later runs reuse it through the ordinary receipt path. If a seeded run is
   not Current, a later run without a receipt for the candidate checks the
   store's receipt against the bytes already in its target, without copying.
7. **Provenance (charter rule 7).** A reused target names the run, in another
   workspace, that verified it (`establishing_run`). The run that placed it
   also sets `dependency_store` to the entry's key on that target and adds a
   note: `SAIL3130: <package> was verified by run <id> in another workspace
   and reused from Sail's dependency store`.
8. **Output (charter rule 1).** Seeding changes no solver query, verdict or
   diagnostic. For the run that seeds, plain Cargo-Verus would have compiled
   and verified the candidate and printed its results line. The managed
   report carries that target's complete report, the one its establishing
   run recorded, and `dependency_store` marks the targets plain would have
   verified in this invocation, so a renderer that reproduces plain output
   prints their recorded results there; the entry also keeps the verifier's
   raw output. Two rules keep the rest identical: Cargo caps a registry
   package's lints, so plain prints no diagnostics for it; and a candidate
   whose verifier printed anything but its JSON documents, Cargo artifact
   notices and Verus's long-running timing notes is never stored. A timing
   note (`<check> has been running for N seconds`, with its second line, or
   `<check> finished in N seconds`: a note with no code and no children) is
   printed by a one-thread verifier, Verus's default on a two-CPU host, for
   any query that takes more than two seconds of wall time, so plain prints
   it on one run and not the next; a seeded run that prints none is the
   plain run whose queries all finished in time. (Until 2026-10-15 these
   notes also refused storage, so on a slow two-CPU CI runner no run stored
   vstd: later runs reuse that run's verification and its captured output.)
   Cargo's `Compiling` progress lines for seeded
   units are not printed; their order already varies between plain runs.
9. **Failure is a normal verification.** A missing entry, another key, an
   entry evicted or deleted while it is being copied, or a changed input (an
   edited registry source, a declared variable or a build-script input) is a
   silent miss, as plain Cargo prints nothing when it rebuilds. Only a damaged
   entry whose key matched (its manifest checksum, receipt seal or a stored
   file's digest or size fails, or a stored path leaves the target) prints one
   `warning[SAIL3131]: ignoring the dependency store entry for <package>:
   <reason>; verifying it normally`. Either way nothing of the entry reaches
   the target, and Cargo builds and the verifier verifies the dependency. A
   failed publication or receipt adoption leaves the store and the run's
   result unchanged; staging and eviction directories left by a killed run
   are removed once they are an hour old. With any
   dynamic-loader variable set (`LD_*`, `DYLD_*`), the store is neither read
   nor written: the key binds their values, but equal values cannot certify
   what the compiler driver and solver loaded.
10. **Size.** An entry for vstd is about 120 MB (dev profile). The store keeps
    the four most recently used entries; `SAIL_DEPENDENCY_STORE_ENTRIES`
    sets another limit, and `0` disables the store. This variable never
    reaches a proof, so it is not part of any context. Eviction renames an
    entry away before deleting it, so a run copying from it misses cleanly
    (decision 9).

## Conformance obligations

- **Unit tests** (`sail-verus` `managed::store`): exactly the closure is
  stored and placed with its times; a workspace unit is never stored; a
  tampered or truncated file or manifest warns and leaves the new target
  absent; a seed killed midway leaves no target and its leftover is removed;
  an entry evicted during a seed is a silent miss; stored directories cannot
  escape the target; stale staging and eviction leftovers are reclaimed;
  loader variables turn the store off; another
  feature set, compiler family or profile is another key; an edited
  dependency source is a silent miss; path closures and noisy verifiers are never
  stored, while a verifier that printed only timing notes is stored and seeded; concurrent publishers leave one entry and concurrent seeders all
  succeed; eviction keeps the most recently used entries.
- **CI, End to end (managed)** (`ci/test-managed.py`): the first Current run
  that verifies vstd stores it; after a prune, the
  rebuilt project's vstd comes from the store; two new projects seeded at
  once each verify only their own crate, report vstd with the stored
  establishing run and exactly the stored report, the same crate verdicts
  and the SAIL3130 note; the next run of a seeded project starts no verifier.
- **Local, against plain stock Verus**
  (`scripts/dependency-store-check.py`): for a seeded first run, a mismatched
  vstd and a corrupt entry, Sail's exit code, its stderr without Cargo
  progress lines (and the SAIL3131 warning) and the `verification results::`
  lines rendered from its reports (vstd's, then the crate's) equal plain
  stock `cargo verus verify` on the same project in a new target, byte for
  byte, with diagnostics that point into vstd's source. Results:
  [dependency store validation](../validation/dependency-store.md).

## Consequences

- A new project, worktree or sandbox that resolves the same vstd closure as
  an earlier one on the machine pays for its own crates only. The first
  project on a machine (and the first after a Sail, toolchain or Verus
  upgrade, which changes the key) still verifies vstd once.
- Measured on a native c7i.4xlarge (medians of three rounds): a new
  one-function project's first managed run 27.5 s → 1.3 s (MCP `verify`
  1.5 s), a new vest worktree's 52.3 s → 25.7 s; the first run on a machine
  is unchanged ([record](../validation/dependency-store.md)).
- Every bound environment difference is another key, as for any proof
  context. For example, Python 3 adds `LC_CTYPE=C.UTF-8` to a process it
  starts under the `C` locale, so an MCP server launched that way does not
  share entries with a shell's CLI runs; it stores its own after one
  verification.
- Projects with a different `Cargo.lock` resolution of vstd's closure (a
  different `syn`, say) get their own entry after their first run.
- The store costs disk (about 120 MB per entry, four by default) and one
  copy of the closure per new target (a fraction of a second).
- The ordinary non-managed `sail verify` is unchanged; it is being retired.
- A Cargo home whose registry sources are re-extracted after an entry was
  stored makes Cargo rebuild the closure (its build scripts declare source
  timestamps): a verification, never a wrong reuse.
- Same trust boundary as receipts (ADR 0023): the checksums detect accidental
  corruption, not a local writer who fabricates both the manifest and its
  files. The store lives in the user's own cache directory.

## Amendment (2026-10-16, #177 and rc.5 finding 15): selection-free key and receipt binding

**Key.** Decision 3 bound the exact arguments. The `-p a` run of the #121
workspace therefore missed the entry a single-package project had stored,
although the resolve nodes of vstd's closure were identical in both
(15 packages) and the same workspace run from the member's directory without
`-p` was seeded. The key now uses the compiler family digest without the
arguments that only select workspace packages and targets (`-p`/`--package`,
`--workspace`/`--all`, `--exclude`, `--lib`, `--bin`, `--bins`; arguments
after `--` are kept). Cargo-Verus gives a dependency the same verifier
arguments whichever members are selected; a selection that changes how the
dependency compiles (feature unification) changes Cargo's unit hashes, so
Cargo rebuilds the seeded units and the stored receipt's artifact names do
not match (decision 6): a miss, never a wrong reuse.

**Receipt binding.** A receipt's input revision bound the bytes of the
workspace `Cargo.toml` and `Cargo.lock` for every target, so adding any
dependency cleaned vstd and verified it again (22 s on a c7i.8xlarge; plain
Cargo-Verus keeps vstd fresh). For a target whose whole closure comes from a
registry or git source (the candidates of decision 2), the revision now binds
the closure's resolve nodes and the workspace's `[profile]` table, the same
inputs decision 3 argues are complete, together with the closure's file
contents, compiler-declared inputs, the `rust-toolchain` files and the Cargo
configuration, in place of those two files' bytes. Other targets keep the
full binding, and run currentness and read checks still compare every
manifest and lockfile (ADR 0041). Cargo's own freshness and the receipt's
artifact and dependency digests are unchanged.

**Removed script units.** A build-script record whose unit directory no longer
exists (Sail's targeted `cargo clean -p`, after which a changed profile gives
the rebuilt unit another directory) is dropped after the run. Kept, it could
never match its output again, and the next run cleaned the package and its
dependents: after a profile change, vstd was verified a second time.

## Displayed stdout when seeding (#184)

The entry carries `verus.shown-stdout`: the exact bytes the driver rendered
with the target's effective output flags. Its digest and size, and those of
stderr, are checked before any artifact is placed. Older entries without
that stream miss; damaged or missing stored bytes warn and verify normally.
The existing narrow stderr policy still rejects diagnostics; only artifact
notices and recognized wall-time progress notes are omitted (decision 8).
On the first run of a newly seeded target, a matching fresh Cargo artifact
event restores displayed stdout once, before a dependent target's output.
Existing-target attachments and unchanged receipt reuse print nothing.

An entry records one verified root's report and output. A root whose closure
contains another verified root is not stored; the leaf proof may still be
stored, while its dependents verify normally. This prevents copying another
proof's artifacts without restoring its evidence and diagnostics.


## Compiler-boundary correction (#187, candidate)

Copying a native dependency's output is insufficient to establish that its
current build would succeed: a changed linker at an unchanged PATH location
can fail while Cargo accepts the copied fingerprint. The [reproduction and
candidate gate](../validation/dependency-driver-replay.md) records this
confirmed defect. This amendment replaces decisions 1, 4 and 5's whole-target
placement for the candidate; it is not yet qualified or accepted.

Cargo must actually compile native dependencies. Reuse happens only when the
verified library's metadata-only compiler invocation arrives. The v2 entry
binds exact native extern bytes, complete source/generated-input topology and
bytes, actual compiler/sysroot inputs, exact effective flags and declared
environment values. Pre/post mutation observations bracket recording and
placement. A matching entry restores only `.rmeta`, `.vir`, dep-info and
validated output; no native artifact or Cargo fingerprint is restored.
Unsupported input closures compile normally. Corruption must never produce a
partial successful materialization, and a replay's `establishing_run` remains
the original proof run rather than the new materialization run.

The existing Cargo tracked-read environment contract remains: declared reads
are exact, and the known compiler/Verus environment is bound. No new claim is
made about arbitrary untracked I/O by build scripts or proc macros. Native
processes still execute on every fresh target. Exact proc-macro byte identity
can decline cross-target reuse on macOS; this is a measured acceptance limit,
not permission to normalize or omit those bytes.


### Path-resolution mutation guard (candidate)

Positive file hashes do not cover replacement of a directory above an input
root. Renaming an ancestor away, reading a different tree at that name, then
restoring the original leaves the root's own inode and ctime unchanged.
A symlink target may itself traverse another symlink outside the literal
root's ancestors. Both cases reproduced against the first candidate helper.

The candidate resolves every component of each original root and every
expanded symlink target, retaining full local stamps before observation starts.
It opens each ordinary component and checks that descriptor's inode identity
before attaching its watch to that held descriptor. Mac watches activate once;
Linux inotify follows the held `/proc/self/fd` descriptor instead of resolving
the original name again. Final path identities and symlink stamps are checked
after activation, before inventory reads. The guarded epoch starts there: an
ancestor replacement entirely before activation cannot affect any later input
read. Requiring ordinary ancestor ctime stability before activation instead
incorrectly refused unrelated concurrent output under shared ancestors.

It synchronously observes ordinary component inodes for rename, deletion,
revocation and attribute changes. Event drains bracket every later validation,
and any failure stays latched. Symlinks additionally retain their own inode/ctime
stamps. Unrelated sibling file or directory output does not invalidate an
observed ordinary ancestor.

The crate graph adds a direct macOS-only `sail-verus -> kqueue` edge at exact
version 1.2.1, already present transitively in the lockfile (MIT). Its safe
`Watcher` API supports synchronous vnode observation without an unsafe Rust
exception or an asynchronously delivered watcher callback. The helper holds
all borrowed file descriptors until the queue is dropped, registers once,
and treats error, revoke and unexpected events as refusal. Linux uses the
existing direct `inotify` dependency: rename/delete/attribute events, unmount,
queue overflow and ignored watches refuse. Mounting another filesystem over
an ancestor and unmounting it can evade vnode-only observation. Mac therefore
also registers the synchronous global `EVFILT_FS` mount/unmount filter; Linux
polls a held procfs mount-table descriptor for mount-namespace generation
changes. Any mount event is a miss, even if unrelated. Native setup/registration
errors or missing mount observation refuse before any inventory bytes are
read; stamp-only observation cannot authorize replay.

Resolution is bounded to 4,096 distinct component paths and 40 symlink
expansions per root. Inventory capture independently bounds the complete tree
to 100,000 entries and two GiB of streamed bytes with a 64-KiB read buffer.
Exceeding a bound refuses reuse. These checks do not yet establish native Linux
or real-project acceptance; the [candidate gate](../validation/dependency-driver-replay.md)
records that qualification separately.


Input opens use safe `rustix` with `RDONLY | NONBLOCK | NOFOLLOW | CLOEXEC`.
The held descriptor must be a regular file or directory before any hash or
watch registration. This also refuses a FIFO substituted after the caller's
stat without waiting for another process. Resolution checks the descriptor's
expected inode identity before watching it; hashing checks its full mutation
stamp and size. The recursive inventory admits each directory and streamed
file, including pre-existing nested mounts, through the same descriptor check.

Filesystem admission is deliberately limited: local APFS on macOS, and
ext2/3/4, XFS, Btrfs, tmpfs and overlayfs on Linux. Network, pseudo, FUSE and
unknown filesystems refuse replay, because registration success alone does
not establish that native events cover external changes. The inotify
[documented limitations](https://man7.org/linux/man-pages/man7/inotify.7.html)
explicitly exclude remote filesystem events and several pseudo-filesystems.
HFS is refused: an actual HFS+ image accepted a source write/read/restore
with the original mtime restored and an identical full stamp. A timestamp
is not a general mutation counter on that filesystem. Admission predicates
are tested; APFS and Linux overlay have actual rapid-ABA controls. Other
admitted Linux types still need their applicable native gates;
this is not a claim that every mount arrangement is qualified.

### Regular executable access events

On macOS, executing a watched regular Mach-O file reports `NOTE_ATTRIB`
when its access time advances, despite unchanged compiler bytes and guarded
metadata. Treating that event as an input mutation prevented live publication.
Regular endpoints therefore bind the full native stamp (identity, permissions,
link count, ownership, size, mtime and ctime) in registration, declared-path
coverage and both mutation barriers. Access time is excluded as before.
Native rename/delete/revoke events still latch refusal for every watched
regular file; directories retain native attribute events because transient
ancestor permissions can affect path resolution. Symlink, mount and complete
inventory guards are unchanged. Content/permission changes followed by restore
must still refuse through ctime, and replacement ABA must still refuse.
The change remains a candidate until live store and both-platform gates pass.

### HFS refusal

The regular-executable correction's full-stamp argument does not hold on HFS.
An actual mounted HFS+ image with the exact `c053bb13` helper accepted a
same-length source write, actual bad read, restored bytes and original mtime:
all stamped metadata was identical. Replay therefore refuses every HFS input
through held-descriptor filesystem admission before hashing or registration.
Ordinary verification remains available. APFS/Linux rapid-ABA stress passes
are scoped evidence, not proof of complete event coverage for every admitted
filesystem; structural write-event coverage remains under review before this
candidate can be accepted.

### Complete native input events

Inventory capture first enumerates every name, type and native stamp without
reading compiler-input file bytes. One queue then observes every inventory
file/directory, every resolution ancestor and every symlink parent before
hashing begins. The complete topology and identities must still match the
metadata-only pass after activation. Held descriptors and this same queue
remain active through compilation or restoration.

Regular files latch native writes/extension/link changes (Linux `MODIFY` and
`ATTRIB`); complete inventory directories latch membership changes. Symlink
parents also latch membership changes, so coarse timestamps cannot hide an
alias moved aside and restored. Ancestor-only directories permit unrelated
sibling writes. Linux duplicate-inode watches union their masks; overflow,
registration failure, unknown filesystems and admission-cap exhaustion refuse.
The combined native path cap is 100,000; the streamed-byte cap stays 2 GiB.
The measured vstd input plan contains about 9,002 existing source/compiler
paths, before extern/generated inputs and resolution ancestors. macOS keeps
one descriptor per watched non-symlink path; insufficient process/native-watch
limits decline replay rather than raising resource limits implicitly.

### Acceptance blocked by memory-mapped writes

The complete-event candidate is not accepted. Independent actual-source
APFS controls wrote different bytes through writable memory mappings, read
those bytes, restored the original contents, and kept the mappings open at
the guard barrier. All six regular-root/recursive-child cases retained equal
full stamps and were incorrectly accepted by the native-event observation.
Neither fine timestamps nor complete subscribed write events establish the
required compile bracket. The [validation record](../validation/dependency-driver-replay.md)
retains the exact source and failed controls; no input boundary is waived.

When an admitted guard does detect a mutation, its invalidation also suppresses
ordinary receipts and makes project currentness Unknown. Unsupported preflight
continues ordinary verification, and store publication I/O alone leaves a sound
compiler observation available. Focused adapter/runner tests establish this
publication ordering, not completeness of the still-blocked input guard.

### Unimplemented conservative fallback after the blocked guard

One immediate correctness proposal is to withhold v1 native-artifact seeding
and publication, retain normal Cargo/native compilation, and accelerate only
the actual verifier's exact Z3 queries through the existing shim/daemon.
`managed::runner::invoke` already selects this path; ADR 0008's default key
binds the solver identity, platform, mode and complete solver-process input
prefix. No source-derived proof key, new filesystem observer, native-artifact
copy or history-tolerant query key would be introduced.

This is not implemented or accepted. Historical first-project evidence puts
the remaining fresh vstd path near 25–30 seconds on the original host class,
against the old store's 1.3-second total; the [gate record](../validation/dependency-driver-replay.md)
states scope and raw identities. The existing strict store-hit assertions
would still fail, so this cannot be called delivery of first-run proof reuse
or used to relax those checks. The current v1 changed-linker defect remains
open pending an explicit correctness/test/performance decision.

A longer-term alternative needs a complete compiler-derived proof identity
or private immutable compiler inputs that preserve native compilation and
diagnostics. Exported `.vir` alone omits proof bodies and is insufficient.
Those alternatives are unimplemented designs, not established safe boundaries.
