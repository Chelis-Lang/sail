# 0187: Sail runs a Kani fork with deterministic GOTO output

- **Status:** accepted (owner approved the fork and its public home on 2026-11-28; the lead records its scope under the 2026-11-12 delegation)
- **Date:** 2026-11-28
- **Applies:** [ADR 0039](0039-private-verus-fork.md)'s fork rules to Kani, as [ADR 0180](0180-kani-phase-scope.md) decision 4 permits
- **Extends:** [ADR 0137](0137-pinned-kani-release-bundle.md)

## Context

The CBMC-boundary reuse layer (W6 in [the Kani phase design](../design/kani-phase.md))
keys on the exact bytes CBMC reads. Stock Kani 0.68.0 reproduces 0 of 49
harness inputs across ten rebuilds of the same source
([S3](../validation/kani-spike-goto-determinism.md)), so that key would almost
never hit. S3 found five causes and a 146-line patch that removes them: 49 of
49 identical across ten rebuilds at one checkout path, with verdicts and
checks equal to stock.

The gain is measured by [S4](../validation/kani-spike-history-replay.md): with
the patch, the history replay projects 42.0× on the median s2n-quic commit and
15.4× on the median Firecracker commit over `cargo kani -j8`. Without it the
key hits on no rebuild, so the projected gain is lost. ADR 0039 requires a
measured gain before a fork ships; this is it.

## Decision

**Repository.** The fork is the public `Chelis-Lang/kani`, a GitHub fork of
`model-checking/kani`. Branch `sail/<version>` is upstream tag
`kani-<version>`, then one commit with the determinism patch, then one commit
with the fork note and the bundle workflow. For 0.68.0 that is
`sail/0.68.0` at `ecf42b98`, with the patch at `320178f6`. Upstream's
workflows are disabled on the fork. Nothing is pushed, filed or commented
upstream; whether to offer the patch upstream is the owner's decision.

**The patch.** It changes what Kani writes, never what it checks:

1. Constant allocations are named after rustc's stable hash of the global
   allocation instead of its creation-order `AllocId` (Kani #4821), with a
   `::dup` suffix when the name is already taken by a different allocation.
2. The "Correct CBMC vtable size" sanity-check description prints the MIR
   type with `Display` instead of `Debug` of its kind, which contains rustc
   table indices.
3. The goto symbol table is written sorted by name instead of in
   string-interning order.
4. The `goto-cc` link step runs in the directory of `kani_lib.c` when every
   path it is given is absolute.
5. `goto-instrument` runs in that directory too.

Items 3 to 5 choose one output stock can already produce. Item 1 renames
internal symbols one to one, the kind of change ADR 0049 admits on
conformance rather than construction; the names never reach Kani's output.
Item 2 is the one user-visible text difference: stock's description is not
reproducible run to run (two stock runs of the arrayvec pilot agree on it for
2 of 9 harnesses), and the fork prints the same type in readable form. The
lead admits it as a diagnostic-only difference: the stock-versus-fork
comparison masks exactly descriptions starting "Correct CBMC vtable size" and
nothing else, and any status difference on those checks remains a failure.

**Runtime and oracle.** Sail's Kani runtime is the fork's Linux x86-64
release bundle, built by the fork's `sail-bundle.yml` the way upstream's
release workflow builds it (`cargo bundle` on ubuntu-22.04 after upstream's
setup action) and published as a prerelease asset with a SHA256 file. Release
tags are `sail-kani-<version>-<n>`; `n` counts rebuilds on one upstream
version. `engine/kani/fork-bundle-linux.json` pins the release, fork commit,
archive and every file, and `sail engine install --backend kani` and
`ci/kani-fetch.py` install it by default. Stock Kani 0.68.0 stays pinned in
`engine/kani/bundle-linux.json` as the conformance oracle, exactly as stock
Verus is for the Verus fork (ADR 0043). The rust toolchain pin is shared.

**Re-basing.** When Sail moves to a new Kani release: fetch tag
`kani-<new>` into the fork, branch `sail/<new>` from it, cherry-pick the
determinism commit and the workflow commit, then rerun S3's rule (ten
rebuilds of the arrayvec pilot and s2n-quic-core from one checkout path, at
least 99% of harnesses byte-identical at the CBMC input) and stock-versus-fork
per-check equality on the fast corpus. Dispatch the workflow with
`sail-kani-<new>-1`, regenerate the fork pin from the released archive, and
move the stock pin and the Rust toolchain pin in the same change. A rebuild
of the same version takes the next `n`.

**Drop condition.** The fork is retired when a stock Kani release that Sail
pins meets S3's rule on its own. Upstream fixing #4821 alone is not enough:
S3 showed four more causes. If only some causes are fixed upstream, the
fork shrinks to the rest.

## Consequences

- The fork bundle differs from stock in three of its 120 files:
  `kani-compiler` and `kani-driver`, which carry the patch, and `kissat`,
  which upstream's setup builds from source and which differs only in its
  embedded build date and host. CBMC, `goto-cc`, `goto-instrument` and all
  Rust libraries are byte-identical to stock.
- Conformance (W3) compares Sail's fork-backed results with direct stock Kani
  and names the vtable description as the one masked field.
- Every Kani upgrade costs a cherry-pick, two measurements and a bundle
  build. The bundle workflow ran in under six minutes on a hosted runner.
- Exact-bytes keys still miss across checkout paths when a project compiles
  its own path into the program (bolero's `CARGO_MANIFEST_DIR`, S3), and on
  every release version bump, because the crate version feeds rustc's crate
  hash in every symbol. [The fork record](../validation/kani-fork.md) shows a
  version-free crate identity would remove the second (40 of 40
  s2n-quic-core inputs identical across a bump, simulated) but would print
  hashes stock never prints in property names and check descriptions. It is
  not part of this decision and would need its own ADR.
- macOS bundles are not built yet; the Mac spike (S9) decides whether they
  are needed.
