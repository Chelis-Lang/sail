# Source-suite executable integrity (#202)

**Candidate; native qualification pending.** Source-suite manifests bind every
named test executable's SHA-256, alongside their existing runtime-artifact
hashes. Older manifests remain readable but cannot qualify a suite. A
non-engine rebuild clears both fixed release-output trees before recording
new hashes; engine-runtime installation and release pins are unchanged.

| Gate | Current result |
|---|---|
| Manifest mutation, legacy, duplicate-entry and writer fixtures | Implemented; native execution pending |
| Actual stock suite: timestamp-restored executable mutation and legacy refusal | Native control prepared; not executed |
| Actual rebuild: retained Cargo intermediates, both output roots, foreign build-directory override | Native control prepared; not executed |
| Workspace, solver, seven end-to-end phases plus extra, PR conformance | Pending final-source native execution |
| Full fast and standard conformance, every arm | Pending final-source native execution |

The native control uses a private source build of the active stock pin, first
built with `--keep-intermediates`. It checks its complete declared runtime and
test inventory and successful test listing, changes one real test executable
while restoring its modification time, then requires `SAIL3040` before any test
is listed. Runtime bytes must remain unchanged during those mutations. It
also removes the test-digest fields to exercise legacy refusal.
The selected executable is retained compressed with its hash, size, mode and
native metadata; a 64 MiB raw bound refuses oversized backups before mutation.
A second original is saved only if the no-run observation changes its bytes.

A direct Vargo no-run invocation records whether Cargo actually reports the
altered executable as fresh; the record does not assume that outcome. The
subsequent xtask rebuild must remove markers from both fixed output roots,
leave the foreign `CARGO_BUILD_BUILD_DIR` sentinel untouched, produce a real
selected-test `compiler-artifact` with `fresh: false`, restore a complete valid
manifest and list the same tests successfully. The built xtask is invoked
directly, so an outer Cargo invocation cannot consume the hostile environment
variable first.

The test-only Cargo observer forwards stdout bytes, stdin, stderr and the
child status. Its per-invocation evidence is capped at 16 MiB and 64 calls;
missing, incomplete or capped records refuse the control. It changes no
ordinary Cargo invocation or Sail default. Small observer fidelity controls
cover binary bytes, stdin, nonzero exit, signals, the Cargo proxy's literal
executable path and both observation bounds. Those controls do not qualify
Cargo, Verus or a source runtime.

No native attempt or performance result is claimed here. The genuine combined
fork runtime and its separate source-suite qualification are outside this
change. See [ADR 0019](../decisions/0019-rust-verify-test-conformance.md) for
migration and trust limits, and [releasing](../releasing.md) for commands.
