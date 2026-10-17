# 0050: Content-bound retained proofs for an undone edit

- **Status:** proposed successor to the unmerged retained-input-events candidate (#163)
- **Date:** 2026-10-17
- **Extends:** [ADR 0023](0023-managed-incremental-workspace-verification.md),
  [ADR 0036](0036-managed-compiler-artifact-families.md) and
  [ADR 0041](0041-content-and-stat-currentness.md)

The source prototype contains shared bounded dependency snapshots, exact provider
source/feature authority, distinct native and GHOST/EXEC compiler-document
consumers, and archive/checked-placement modules. An explicit research request
can stage the workspace driver's source-read witness against its published
owned graph. The default-off `SAIL_RETAINED_CONTENT_RESEARCH=1` path now joins
source-qualified native producers, archive publication and checked workspace
restoration. Same-workspace native certificates retain their original run, PID
and compiler-read document; they never become synthetic current-run evidence.
Actual runtime, real Vest and ABA qualification remain pending. Local source checks and owned-child wire
fixtures are not real-project acceptance.

## Problem

An A → B → A edit replaces the latest proof receipt and its compiler outputs.
The reverted A then pays for compilation and verification again. Keeping a
small number of proven content revisions could restore A instead.

The earlier unmerged implementation kept bounded content-addressed output
blobs and restored them in Sail's driver, after Cargo decided to rebuild the
unit. Its recording guard depended on stamps and filesystem events proving
an unchanged compiler epoch. Actual coarse-filesystem and mapped-write ABA
counterexamples leave that claim blocked. Reading a file after compilation,
including a dep-info checksum produced by the final erased pass, does not
prove what the verification pass read.

ADR 0041 explicitly accepts some undetected changes for the Current label.
That limitation cannot authorize a new persisted verdict-reuse decision.

## Proposed boundary

1. Retain the existing candidate's bounded archive, checked output copies,
   exact target/compiler identity and establishing-run provenance. Do not port
   its event observer or treat timestamps as content evidence.
2. Recording requires the existing research fork's compiler-read observation:
   one complete document bound to the actual successful compiler child, run,
   compiler identity and request. It must contain exactly one GHOST
   verification phase and one EXEC compilation phase. Each phase records the
   checksum and unnormalized size from rustc's actual loaded SourceMap files;
   a later reopened path never substitutes for that record.
3. Fully hash the current selected source/dependency closure, generated Rust
   sources and configuration, including directory membership, file absence
   and lexical symlink targets. Every loaded source in both phases must be
   covered and match those literal bytes and sizes. GHOST B / EXEC A / disk A
   refuses recording. Missing or duplicate phases, extra tracked files,
   ambiguous paths or incomplete coverage refuse recording. Initial eligibility
   also refuses an unknown procedural-macro input closure; it does not infer
   arbitrary native macro I/O from dep-info.
4. Publish only after the ordinary whole-target successful document, Current
   result, dependency/owned-output guards and committed receipt pass. Native
   prerequisite units still run normally. The retained workspace unit binds
   its full actual compiler arguments, native dependency bytes, environment,
   working directory, runtime and both compiler-read phase records.
5. At restoration, fully rehash that complete source/inventory/dependency
   basis. A match selects only an already established proof of those bytes.
   Copy checked retained outputs into Sail's owned target, then repeat the
   input and dependency checks before replaying the stored diagnostics and
   publishing the ordinary staged receipt. A mismatch removes placed outputs
   and runs the real verifier. A failed removal is an explicit fatal cleanup
   result: invalidate the receipt and refuse the attempt or force a genuinely
   fresh compilation; never hide it as a cache miss. There is no intermediate
   compiler read to
   certify during restoration; final exact content is what selects the proof.
6. Keep four proofs per logical target, with exact compiler variants remaining
   distinct, and preserve the establishing run. A restored proof is reported
   as reused. Corrupt blobs, unsupported contexts, forced/focused/loader or
   engine runs take the ordinary path. No runtime pin changes are implied.

## First bounded gate

- Establish A with both real compiler phases, verify B, then restore A;
  require the same verdict/diagnostics and original establishing run, with
  no new verifier invocation on the final A.
- During a fresh run, make GHOST consume B, restore A before EXEC and preserve
  timestamps. The completed source document must prevent publishing any
  reusable A proof. Retain actual phase records and all raw outcomes.
- A one-byte C change, newly appearing module, changed symlink resolution,
  declared environment or native dependency must miss; damaged output blobs
  must never leave a partial restoration.
- Run the same gate with mapped writes and coarse timestamps. These controls
  challenge content binding, not filesystem-event delivery.

The current accepted runtime lacks the required two-phase document, so it must
miss safely. A research run can use the already built source-observer runtime
explicitly; default integration waits for its independent qualification.

## Compiler dependency bytes

This workspace boundary needs a separate dependency-directory snapshot. The
vstd experiment's three independent native copies are deliberately limited to
its closed builtin and macro graph; they do not close a workspace unit's Rust
metadata and Verus import graph.

Before launching the workspace verifier, admit its actual Cargo-owned
`-Ldependency` directories and explicit `--extern` paths. Copy the complete
relevant basename inventory (`.rmeta`, `.rlib`, platform macro libraries and
`.vir`) into fresh private directories, preserving names and independently
checking every copied byte. Rewrite every admitted dependency search argument
and explicit extern to those copies. Reject foreign search directories,
custom imports, missing files, aliases and unsupported native or macro
providers; the pinned sysroot remains independently bound. This changes
compiler input paths, never stored artifact bytes or source paths.

Keep copied graphs at stable content addresses through the owning compiler
artifact generation and retained entry. Verus marks imported `.vir` paths in
dep-info, so destroying a graph when its driver exits could invalidate ordinary
Cargo freshness. Do not rewrite dep-info to claim that mutable originals were
consumed. Reusing a graph requires full new reads of the original dependency
inventory and all private bytes. Existing bounded entry collection also keeps
explicit references from every still-live compiler artifact generation before
reclaiming unreferenced graphs.

The inventory must distinguish inputs from the current invocation's exact
owned output family. Blindly keying the previous A/B outputs in the same
`deps` directory would prevent the original A key from matching after B.
Exclude that family only after showing it cannot be an explicit or transitive
input; any ambiguous self-dependency refuses. Other artifact basenames and
their absence remain bound, including files used only for transitive lookup.

The fork resolves direct Verus imports beside the explicit extern. Transitive
imports use the actual Rust-selected crate source and its adjacent `.vir`.
Keeping the complete copied directory preserves both lookups. In the Cargo
route, Verus rebuilds its libraries instead of injecting the runtime's default
`vstd.vir`; non-Cargo invocation is outside this initial boundary.

Retained successful real Vest reports identify a workspace `vest_lib` unit
with `libvstd-….rmeta` and the matching `.vir`, and a vstd invocation with the
three official native inputs. They retain dependency hashes and the compiler
argument digest, but no complete dependency-directory size inventory. Thus
the actual copy size, full argv admission and native import-lookup equivalence
remain unmeasured. No small no-vstd example substitutes for the real Vest gate.

Before recording a retained proof, require the actual copied-input map to
remain intact through both compiler phases and publication. A controlled
original dependency A → B → A substitution must leave the compiler consuming
only private A; damaging a private input must prevent recording or replay.
The source-read B / disk A gate remains independently required.

## Expected value and remaining review

The optimization targets the roughly 30-second fresh frontend/verification
path after an undo. Full relevant-content hashing is paid instead of assuming
mtime freshness; its actual latency must be measured. The old file-event
attempts and their failed controls remain evidence against that earlier
approach. The connected source prototype establishes no gain or acceptance.

Before native qualification, review the initial eligible macro/native-input closure
and the exact shared observer consumer with the vstd-boundary work. The same
observer should be reused, not independently reimplemented from dep-info.

## Narrow implementation order

1. Reuse the existing two-phase document consumer without path normalization:
   this is same-workspace history. Bind its fresh private request, actual child
   PID and successful complete driver result. Use the compiler's actual
   checksum algorithm; full current file hashes must use that same algorithm,
   rather than compare different digests. Requesting SHA-256 explicitly is a
   possible way to reuse Sail's existing SHA-256 implementation, subject to
   actual two-phase argv/output checks.
2. Port the existing bounded archive and checked restoration helpers into the
   current managed driver/receipt layout. Start with the exact pinned Verus
   macro closure; unknown macro or opaque tracked-file input is a miss. Keep
   ordinary native prerequisite compilation and existing target locking.
3. Exercise the actual shared admission function with a genuine two-phase
   compiler document that records B, followed by literal disk A. Keep this
   distinct from the ordinary real Cargo A → B → A integration. Neither
   before/after stat equality nor a fixture's successful Current label may
   replace the source-read mismatch assertion.
4. Measure one fixed real-project revert only after those correctness gates.
   Record all hash/copy time and verifier/solver work. An archive match alone
   is not a latency result.

Rust 1.98 keeps the optional freshness checksum separate from the ordinary
source-map hash in
[`SourceMapInputs`](https://github.com/rust-lang/rust/blob/1.98.0/compiler/rustc_span/src/source_map.rs).
Its [dep-info writer](https://github.com/rust-lang/rust/blob/1.98.0/compiler/rustc_interface/src/passes.rs)
also reopens explicitly tracked extra files to hash them. Consequently the
two-phase observer's loaded-source records are required, and an extra-file
dep-info checksum alone remains insufficient.

## Initial provider scope

`engine/verus/retained-vest-inputs.json` is the complete selected normal/build
closure from the retained real Vest metadata: fifteen exact official providers,
718 ordinary source files and their literal features. Six provider subtrees
come from the primary pinned Verus source; nine registry packages bind their
real crate archives. The full file inventory and SHA-256 checks remain the
source authority. Independently derived BLAKE3 checksums and unnormalized sizes
let the native consumer check actual loaded `SourceMap` records without changing
native compiler arguments. README and other non-Rust source files are included.
Custom providers, unknown features, extra files and symlinks refuse admission.
This policy does not attest a native compilation by itself.

The native observer's document is distinct from the verification/compilation
pair. Its actual successful child, request binding, PID, complete loaded-source
checksums, tracked environment and private extra inputs must qualify separately.
Reopening a generated file or a mutable source path after the native compiler
returns cannot substitute for a consumed-input observation.

## Namespace and initial source scope

The initial policy closes 15 pinned provider packages and 718 literal files.
Unknown providers, features, compiler options and opaque macro inputs miss.
Actual native SourceMap checksums cover loaded sources, including literal
`include_str!` files; an extra tracked path is admitted only when the same
qualified SourceMap entry or a private immutable graph copy covers it.

Loaded source checksums alone do not prove negative module lookup. The
[VI-0025 reproduction](../verus-issues/VI-0025-module-candidate-absence.md)
therefore accompanies a conservative namespace guard: ambiguous `foo.rs` /
`foo/mod.rs` candidates and unsupported path/include syntax outside the fixed
provider closure refuse retention. This is an eligibility restriction, not a
claim that endpoint hashes certify arbitrary compiler reads.

See the [source checkpoint record](../validation/instant-revert.md) for the
executed checks and pending real-project gates.
