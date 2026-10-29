# Source-suite executable integrity (#202)

**Qualified on native Linux.** Source-suite manifests bind every
named test executable's SHA-256, alongside their existing runtime-artifact
hashes. Older manifests remain readable but cannot qualify a suite. A
non-engine rebuild clears both fixed release-output trees before recording
new hashes; engine-runtime installation and release pins are unchanged.

| Gate | Current result |
|---|---|
| Manifest mutation, legacy, duplicate-entry and writer fixtures | Pass in the native workspace gate: 642 tests, 35 skipped; two doctests |
| Actual stock suite: timestamp-restored executable mutation and legacy refusal | Pass: both refuse listing with `SAIL3040`; runtime bytes unchanged |
| Actual rebuild: retained Cargo intermediates, both output roots, foreign build-directory override | Pass: exact selected test rebuilt, both markers removed, foreign sentinel unchanged |
| Solver, seven end-to-end phases plus extra, observer fidelity controls | Pass: 35 solver tests, every phase and five observer controls |
| PR / full fast, each of four modes | 33/33 units and 311 functions / 147/147 and 3,450; zero disagreements or measured drift |
| Full standard, each of four modes | 159/159 units and 12,726 functions; zero Different, Unstable, infrastructure errors or skipped projects |
| Final source and image checks | Pass: all 529 tracked files, index and untracked-file checks; both image identities unchanged |

The 2026-10-16 run qualifies Sail source
`6ba306369ca08f65d356108b51a8fc789aed8e70` on Ubuntu 24.04, x86-64
EC2 c7i.4xlarge (16 vCPU), us-east-1. Sail uses Rust 1.99.0; the actual
stock source build uses Rust 1.98.1 and Verus
`426d8b01e7ffb910a36c15e1f870ff985bb61eef`, with pinned Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
The engine arm uses the existing pinned runtime, manifest SHA-256
`4f0d0d3f4315a3b0c2c4d2a977a712db8db5ba2c3f7f72c39899cf9fd50fccdd`.

Standard cache-fill and engine each report one resource-limit drift for
`verified-node-replication`'s
`lib::spec::cyclicbuffer::log_entry_alive_value_wrap_around`: 51,948 plain
versus 51,945 Sail, with matching verdict and diagnostics. Pass-through and
cache-warm report none. Drift remains reported under the existing harness
criterion; no comparison or baseline was relaxed.

The native control uses a private source build of the active stock pin, first
built with `--keep-intermediates`. It checks its complete declared runtime and
test inventory and successful test listing, changes one real test executable
while restoring its modification time, then requires `SAIL3040` before any test
is listed. Runtime bytes must remain unchanged during those mutations. It
also removes the test-digest fields to exercise legacy refusal.
The selected executable is retained compressed with its hash, size, mode and
native metadata; a 64 MiB raw bound refuses oversized backups before mutation.
A second original is saved only if the no-run observation changes its bytes.

A direct Vargo no-run invocation actually reports this altered executable as
`fresh: true`, and its changed bytes remain. The selected `compile` executable
is 539,944 bytes; its first byte is changed and its original mtime restored.
The subsequent xtask rebuild removes markers from both fixed output roots,
leaves the foreign `CARGO_BUILD_BUILD_DIR` sentinel untouched, produces a real
selected-test `compiler-artifact` with `fresh: false`, restores a complete valid
manifest and lists the same tests successfully. The built xtask is invoked
directly, so an outer Cargo invocation cannot consume the hostile environment
variable first.

The test-only Cargo observer forwards stdout bytes, stdin, stderr and the
child status. Its per-invocation evidence is capped at 16 MiB and 64 calls;
missing, incomplete or capped records refuse the control. It changes no
ordinary Cargo invocation or Sail default. Small observer fidelity controls
cover binary bytes, stdin, nonzero exit, signals, the Cargo proxy's literal
executable path and both observation bounds. Those controls do not qualify
Cargo, Verus or a source runtime.

Raw archive SHA-256:

- Actual stock integrity/rebuild control:
  `f6dca4f73f538f60df5709b464d5878b788636a0824a00829c6294199753da50`.
- Workspace gate:
  `47c86e25a5d2400d9d7f0faac947330bd1fadb49525196abe2a9427dd3cc6330`.
- Full fast / standard:
  `0432a49a8f5b459fcc5119061200e5c39d234d26cb109041c5262dc8075aaeae` /
  `d9ae9f6785186bd7b454711535e38712694350d265f142e093e252d329f2b0bb`.
- Complete final evidence and source/image checks:
  `223e6f335a8f963a43c5eb4752915c8c7f248ea5670a20c60ec07d761bc69ff4`.

Earlier attempt: 2026-10-16, source `6ba30636`, interrupted after the stock
build checkpoint; integrity and final outcomes were not recovered. A separate
native root-owned-file control demonstrated an archive permission failure
mechanism. The successful retry verifies privileged full archival before the
image builds. The earlier lost outcome remains unknown.

The #202 PR head `7c2d26d1`, rebased onto main `82d46529`, retained the two
Rust implementation files and three integrity-control scripts byte-identically
to qualified source `6ba30636`. This does not claim a native run of that rebased
head or any later harness integration.

This checks suite integrity and rebuild/listing behavior, not execution of the
complete 4,695-case source suite. The genuine combined fork runtime and its
separate source-suite qualification are outside this change. No macOS host
gate or performance result is claimed for this run. See
[ADR 0019](../decisions/0019-rust-verify-test-conformance.md) for migration and
trust limits, and [releasing](../releasing.md) for commands.
