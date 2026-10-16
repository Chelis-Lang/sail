# 0050: Retained proofs: reverting to verified content restores its proof

- **Status:** implemented candidate; independent review and qualification pending (#163)
- **Date:** 2026-10-16
- **Extends:** [ADR 0023](0023-managed-incremental-workspace-verification.md)
  (receipts), [ADR 0036](0036-managed-compiler-artifact-families.md) (one
  Cargo target per compiler family) and
  [ADR 0041](0041-content-and-stat-currentness.md) (what Current means)
- **Roadmap:** item B4 of the
  [2026-10-13 roadmap review](../reports/2026-10-13-roadmap-review.md)

## Context

- **The agent loop reverts.** An agent tries an edit, sees it fail and undoes
  it; a developer runs `git checkout` back or switches branches. Plain
  `cargo verus verify` re-verifies the reverted content, because Cargo
  compares modification times and the reverted files are newer than its last
  build. Sail did the same.
- **What blocked reuse after A → B → A** (managed runner at `f0c4fa8b`):
  1. *One receipt per target.* A proof context holds one receipt per Cargo
     target (`receipts/<package, target, kind>.json`), and the driver retires
     it before it verifies the target again (`receipts::retire`). B's run
     replaced A's receipt; nothing of A's proof survived.
  2. *Cargo rebuilt the unit and overwrote A's artifacts.* B's verification
     rewrote the unit's `.rmeta`, `.vir` and dep-info in Sail's target, so even
     a kept receipt would not have matched the bytes it binds.
  3. *Cargo rebuilds after the revert anyway.* The reverted file is newer than
     B's build, so Cargo invokes the compiler for the unit again; and the
     runner's dirty-receipt scan cleans the package because B's receipt
     describes other content.
- **What plain prints for the reverted run.** Cargo's status line for the
  unit (`Checking demo v0.1.0 (...)`), Verus's diagnostics and its
  `verification results:: N verified, 0 errors` line, then `Finished`. Any
  reuse must print exactly that (charter rule 1, ADR 0046 decision 2).

## Decision

1. **Retain proofs, not just receipts.** After a Current run publishes a
   workspace target's receipt, the runner also keeps a *retained proof* of it
   in `<context>/retained/`: the receipt; every file rustc wrote for the unit
   (`<out-dir>/{lib,}<crate><extra-filename>.*`: `.rmeta`, `.vir`, the
   dep-info) as content-addressed blobs; the verifier's captured stdout and
   stderr; and what the driver observed when it verified the unit: the
   SHA-256 of every file the unit's dep-info names, the environment the
   driver received from Cargo (every proof-context variable, which includes
   Cargo's per-unit `CARGO_*` and the Verus driver's `__VERUS_*`, plus
   `OUT_DIR` and every variable the dep-info declares; Cargo's jobserver
   handle excluded), the working directory and the complete source inventory
   described below. Only workspace members that
   declare `package.metadata.verus.verify = true` and are in the selection's
   dependency closure are retained; registry dependencies have the
   [dependency store](0045-dependency-store.md).
2. **Restore in the driver, after Cargo decides to rebuild.** Nothing changes
   before Cargo runs: Cargo decides that the reverted unit must be rebuilt,
   prints its status line and invokes Sail's driver, as for any edit. The
   driver then looks among the retained proofs of exactly this target
   identity (the manifest, crate name and type and the digest of the complete
   compiler arguments, which include the unit hash, features, cfgs, the
   output directory and every `--extern` path) for one whose recorded inputs
   all match now:
   - the entry's checksum and its receipt's seal are intact, the receipt is a
     complete, successful whole-target receipt of this proof context (which
     binds the toolchain, Sail, Z3, the arguments, the bound environment and
     the Cargo configuration), and its artifacts are among the stored files;
   - the compiled dependencies (`--extern` files and their `.vir`) have
     exactly the bytes the establishing verification read;
   - every file the dep-info names has the recorded content (rehashed in
     full; nothing is inferred from timestamps), and the environment and
     working directory are the same;
   - a complete managed source snapshot has the recorded revision, including
     directory inventory and symlink targets. It covers the package's whole
     dependency closure, workspace manifests, Cargo configuration, known
     external source directories and the current `OUT_DIR`. Every dep-info
     input must be present with the same bytes in that snapshot; unknown
     inputs refuse retention and restoration.
   Managed snapshots and mutation stamps bracket fresh verification and
   restoration. Changed bytes, new files, rewritten inputs and retargeted
   symlinks refuse the optimization. On a match the driver writes the
   stored files back to their paths (each through a temporary file, with its
   digest checked on the bytes written), writes the recorded stderr and
   Verus's stdout as plain Verus prints it (rebuilt from the recorded document
   exactly as for a verification), and records the receipt's report as the
   target's observation, marked with the run that verified it. Cargo then
   records its own fingerprint from the restored dep-info, as after a
   compilation. Anything else is an ordinary miss and the driver verifies.
3. **Source inventory after Cargo's scripts, before the verifier.** The
   runner's per-package revision is computed before Cargo runs, so for a
   crate that includes a generated file it describes the previous generated
   file (or none, after targeted cleaning). The driver instead snapshots the
   complete source view after Cargo finishes the package's script and adds
   its current `OUT_DIR`. A shared path-resolution guard observes existing
   roots, observed directories and every symlink from the same topology pass,
   including aliases whose canonical targets were already visited. Optional
   absent configuration paths keep their existing absence checks; their nearest
   existing resolution parents are observed. After native activation a second
   complete snapshot, topology revision and mutation stamp must match the
   preceding inventory before the driver may compile or restore. They must
   match again after verification or restoration, with the native guard checked
   on both sides of each check. Scripts can regenerate earlier content
   while newly appearing inputs remain bound. Positive dep-info paths alone
   are insufficient: adding `leaf/mod.rs` beside a previously verified
   `leaf.rs` makes `mod leaf;` ambiguous without changing any old input bytes.
   The first prototype incorrectly restored success in that case; the
   [validation record](../validation/retained-proofs.md) preserves the repro.
   Rename-and-restore of an ordinary ancestor or an intermediate symlink may
   leave every observed final byte unchanged. Synchronous inode and global
   mount-generation events must therefore remain clean throughout the guarded
   epoch; an error or event permanently refuses retention/restoration.
   Unsupported observation and more than 4,096 distinct resolution components
   or 40 symlink expansions per root are ordinary misses. Earlier performance
   results do not qualify these additional checks.

   This adds the macOS-only direct `sail-verus -> kqueue` dependency at exact
   version 1.2.1 (already transitive in the lockfile, MIT) for its safe synchronous
   vnode and global filesystem-event API. Linux uses the existing `inotify`
   and `rustix` dependencies, including sticky procfs mount-generation polling
   ([ADR 0032](0032-kernel-mount-generation-coverage.md)). Opened inode checks
   and final path checks bracket registration; input reads are trusted only
   after observation activates. No unsafe Rust exception or asynchronous
   notification thread is introduced. Borrowed file descriptors stay open until
   the queue is dropped; no unsupported or overflowed queue authorizes reuse.
4. **Provenance (charter rule 7).** A restored target is `reused` and its
   `establishing_run` is the run that verified it; the run adds
   `SAIL3132: <target> has the content that run <id> verified; its retained
   proof was restored instead of verifying it again`. Its staged receipt
   carries the establishing run (`restored`), so later reuse of that receipt
   still names it. A restored receipt is published like any other, after the
   run's own Currentness checks; it is not retained again (its proof is).
5. **When retention applies.** Only runs that may reuse proofs: not with a
   process-loader variable (ADR 0046), not forced (`--force`,
   `SAIL_MANAGED_FORCE`), not focused, not with unobserved inputs, and never
   for an interactive-engine run. A run whose result is not Current retains
   nothing. Only a successful complete verification is retained; failures
   never are.
6. **Bound.** Each logical target (manifest, crate name and crate type) keeps
   its four most recently used proofs across compiler-argument and Cargo-unit
   variants;
   `SAIL_RETAINED_PROOFS` sets another number, and `0` turns retention off
   (restoring and recording). The variable never reaches a proof and is
   observation-only, so it changes no proof context. Retaining the same
   inputs and compiler identity again replaces the older proof of them.
   Compiler identity still matches exactly during restoration; another
   variant is a miss rather than a damaged entry. Blobs that no remaining
   proof refers to are deleted when a proof is retained; copies left by a
   killed run are deleted after an hour. `sail cache projects prune` removes
   retained proofs with the rest of the workspace's directory.
7. **Failure is a normal verification.** A missing, mismatched, unreadable or
   damaged entry (a checksum, seal or blob digest that fails, a recorded
   output missing) is skipped silently; a damaged one is deleted. If writing
   a stored file fails, the files already placed are removed, so the unit's
   files are either all the retained ones or Cargo's own, and the driver
   verifies. Failed optional dependency reads and incomplete source inventories
   also run the verifier. A failure to retain never changes the run's result (an event in
   `SAIL_JOB_EVENTS` records it).

## Conformance obligations

- **Unit tests** (`sail-verus` `managed::history`): an undone edit restores
  exactly the retained files, receipt and recorded output, and the
  latest content restores its own proof; a one-byte difference in the
  source or another dep-info input, changed dependency bytes, a changed or
  newly set declared variable, another working directory, another proof
  context and a run without retention all verify, and place nothing; Cargo's
  jobserver handle and unbound variables do not matter; a tampered blob,
  entry or missing recorded output is ignored, removed and leaves none of
  its files in the target; eviction keeps the most recently used proofs and
  only their blobs; outputs rewritten after the driver observed them are
  never retained; dep-info with escaped spaces and declared variables is
  read exactly. New source candidates, generated directory additions,
  symlink retargeting, same-byte writes during observation and uncovered
  compiler inputs refuse restoration. Compiler-identity variants share one
  target's bound and mismatches never delete usable variants.
- **CI, End to end (output)** (`ci/test-output.sh loop`): an edit and its
  undo print exactly what plain stock `cargo verus verify` prints on the same
  content (stdout, stderr with Cargo's elapsed time blanked, exit code), the
  undo restores the retained proof, and a one-byte change verifies again and
  is identical too. Adding an ambiguous module candidate produces stock's
  E0761 and exit code, with no retained restoration.
- **CI, End to end (managed)** (`ci/test-managed.py`): returning a
  build script's declared variable to an earlier value regenerates the
  included file and restores that content's proof (no verifier, Current, the
  same report and establishing run, `SAIL3132`), and the next run reuses it
  as a receipt; a source edit undone restores; a one-byte difference
  verifies; damaged blobs verify with the same verdicts. The missing-receipt
  checks run with retention off, so they still exercise fresh verification.

## Measurements

Native performance qualification remains pending for the source-inventory
implementation. Measurements of the earlier dep-info-only prototype do not
qualify this candidate. Scope and remaining gates are in
[retained-proof validation](../validation/retained-proofs.md).

## Consequences

- An undone edit, a checkout back or a branch switch to content verified in
  the same proof context costs Cargo's own work and a copy of the unit's
  files, instead of the verifier: the same output as plain, much sooner.
- **Disk.** Each retained proof stores the unit's files: about 20 KB for a
  small crate and about 30 MB for vest's `vest_lib` (its `.vir`), so up to
  four times that per target and proof context; unchanged files are stored
  once ([disk use](../workspace-hygiene.md#retained-proofs)).
- **Same trust boundary as receipts** (ADR 0023): checksums detect
  accidental corruption, not a local writer who fabricates both an entry and
  its blobs.
- **Determinism.** Reuse assumes, as receipts and the solver cache already
  do, that the verifier gives the same result for the same inputs. Plain
  re-verification of a proof that is unstable under the same inputs could
  differ from the restored result; a proof that times out or fails is never
  retained.
- **Not covered.** Content verified in another proof context (other
  arguments, toolchain or environment) or another workspace is not restored,
  and neither are registry dependencies (ADR 0045 covers new targets).
