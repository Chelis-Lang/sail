# Inactive validation compiler scratch cleanup

The [retained cleanup record](m7-inactive-incremental-cleanup.json) covers eight
inactive Linux validation `target/debug/incremental` directories. They released
2,246,709,248 allocated bytes (about 2.09 GiB). Only those exact directories were
removed after owner authorization; the record lists their root labels and full
context-relative names. The root-qualified local inventory and complete seals
remain in the ignored raw-record directory recorded there.

Before deletion, complete live command, working-directory and file-descriptor
scans found no ownership of either completed validation root. The same checks
ran immediately before each removal, together with exact scratch directory
device/inode validation. The preservation seal matched before and after:
17,743 entries, including 17,247 regular files totaling 4,834,850,037 bytes.
Path, kind, mode and file SHA-256 were compared; no symlinks occurred in this
protected inventory.

Every context binary, dependency, Cargo fingerprint, receipt, sidecar and raw
transcript was preserved. The active standard conformance cache, common Cargo
target, source-agent bootstrap/seed/build targets and every sealed runtime were
outside the deletion scope. No container or data volume was removed.

The record also retains an earlier cleanup of 7,216 closed object files in the
MCP worktree's private `target/debug/deps`: 245,407,744 allocated bytes, with all
ten retained executable/loadable-library hashes verified afterward. The combined
recorded allocation was 2,492,116,992 bytes. Concurrent builds and cleanup make
filesystem free-space changes unsuitable for attributing an additional saving.

These are storage and preservation checks. They establish no new product,
source-engine, proof, conformance or performance acceptance result. Existing
failed validation records remain intact.
