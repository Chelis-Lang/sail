# Workspace hygiene

Retire a worktree after its changes are integrated or its experiment is closed.
Keep the active team roster, container mounts and running compiler paths in the
protected set. Recheck ownership immediately before each deletion: a worktree
created after the initial inventory is active until its owner says otherwise.
Use one continuing Cargo target per build environment where possible.

Before retiring a checkout, preserve its Git commit under a recovery reference,
its branch, staged and unstaged binary patches, and an archive of the actual
working files, including untracked source. Verify the archive hash and recovery
reference before removing the checkout. Keep validation output outside disposable
compiler directories. Preserve original failed outcomes, full raw invocation
logs, JSON reports and receipts, source/patch/lock identities, and the selected
required product and runtime. Keep compact evidence outside temporary storage.

Immediately retire finished run scratch and inactive compiler targets after
verified evidence transport and complete closure/holder checks. Do not archive
whole working caches or multi-gigabyte targets. Historical executables and
libraries may be retired with their hashes and source records retained; mark
them nonreexecutable without rebuilding. Preserve selected required runtime
binaries, rather than every historical test image. These retirements do not
change original validation outcomes or recorded artifact hashes. Do not clean a
target used by an active compiler, even if its source appears unchanged.

## Completed local runs

[scripts/retire-completed-run.py](../scripts/retire-completed-run.py) retires one
explicit, owned native `sail-*` directory directly under temporary storage. It
requires a terminal PASS or FAIL with complete closure evidence, then performs
fresh bounded holder checks in both Sail containers and an idle-process check
in the target container. Live processes, unavailable observations, mounted
storage, protected-path overlap, special files or exceeded limits refuse
cleanup. It sends no signals and performs no retries.

The helper inventories at most 20,000 entries and 8 GiB of regular-file bytes.
It archives complete text, JSON, source and receipt files, capped at 128 MiB,
with full byte hashes and mode checks. Compiler target trees and historical
binaries are excluded from the archive. Symlinks are recorded and unlinked
without traversal; hardlinked evidence gets full content backing, and remaining
outside links survive. Each native capture, holder or retirement phase shares
one 300-second cooperative deadline across its operations; metadata, holder
reads and transport also have separate finite bounds.
These are operational limits, not a memory or hard filesystem-latency guarantee.

Only after the host archive is fully verified does the helper recheck membership,
births and holders, then unlink exact original bindings with checks on every
ancestor. Its journal records a completed prefix count, last removed name and
pending operation on failure, bound to the exact inventory and deletion order.
It does not repeatedly rewrite an ever-growing full removal list. The terminal
record includes the full removed set only after completion.
An uncertain or interrupted cleanup stays uncertain; inspect its retained record
and use a separate ownership audit before further action.

`scripts/local-combined-gates.py` now performs this retirement after original
source/product checks and complete phase-log retention. The native gate result
stays unchanged; `controller.json.retirement` reports cleanup independently.
The host controller exits nonzero if retirement is unavailable or incomplete.
Durable evidence defaults to the Sail cache's `evidence/completed-runs` directory;
`--retirement-output` selects a new host directory outside temporary storage.
An explicit external `--source` and the shared compiler target remain untouched.
The default source inside the run root is retired after retaining its exact
source archive; a subsequent build must stage that archive into a fresh path.

For the existing reader and benchmark controllers, first retain the complete
terminal records and raw logs and complete their original closure checks. They
do not yet automatically hand off retirement. Use the helper afterwards, with
the original terminal record, exact native root, required full logs/receipts and
external protected paths from that run's retained inventory:

```sh
python3 -B scripts/retire-completed-run.py \
  --container "$RUN_CONTAINER" --root "$RUN_ROOT" \
  --output "$DURABLE_EVIDENCE_DIR" --terminal-record "$TERMINAL_RECORD" \
  --protected "$SELECTED_PRODUCT" --protected "$SELECTED_RUNTIME" \
  --protected "$SOURCE_ROOT" --protected "$SHARED_TARGET" \
  --required "$NATIVE_TERMINAL_RELATIVE" --required "$FULL_LOG_RELATIVE" \
  --host-evidence "$RETAINED_SOURCE_ARCHIVE"
```

Each variable must come from the actual run record; paths to required native
files are relative to the fixed root. Repeat `--required` for all indispensable
streams and receipts. A benchmark host `controller.json` has a bounded
`closure` census; reader retention needs an equally complete closure record.
Missing closure is a deferred retirement, not permission to manufacture a
successful record. Preserve corpus pins, selected source and runtime outside
the disposable root. An overlapping protected path must be relocated and
resealed through the normal setup before whole-root retirement.

Inspect stopped containers and dangling images individually. Remove containers
without deleting their volumes. Preserve active containers and data volumes;
avoid a global prune while other projects are building.

The [M7 cleanup record](validation/m7-workspace-cleanup.json) records retired
checkouts, compiler files, preservation checks and observed free space. Recovery
archives live under the local Sail cache in
`retired-worktrees-2026-10-10/<worktree>/`. Each directory contains
`source.tar.gz`, `metadata.json` and staged/unstaged patches, plus `target/` when
raw artifacts were retained. Its recorded `refs/cleanup/worktrees/<worktree>`
reference restores the commit; unpack the source archive into that checkout to
restore the actual working files. The patches also retain staging information.

One newly active diagnostic checkout was mistakenly included in that cleanup.
Its archived edits and commit survived; the agent recovered them into a protected
checkout and restarted from committed native Linux source. The interrupted
compile remains a setup failure. No tests ran in that interrupted attempt.
