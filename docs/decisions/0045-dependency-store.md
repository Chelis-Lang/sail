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
