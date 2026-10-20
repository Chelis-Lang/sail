# 0050: Content-bound retained proofs for an undone edit

- **Status:** implemented; final-source native qualification passes for the scoped Linux/public profile; delivery PR/merge pending (#163)
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
Final source `83128ad0` passes real Vest ordinary and genuinely bundled ABA,
all 72 matched correctness/cost rows and complete native default gates. The
public alloc/std profile alone is eligible ordinarily; explicit research keeps
other closed profiles separate. Cold/edit/unchanged and cache costs remain
visible in the [validation record](../validation/instant-revert.md). Local
source checks and owned-child wire fixtures are not real-project acceptance.

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
executed checks and later scoped real-project qualification.

## Aggregate retained-arena admission

The arena has prospective limits of 8 GiB of regular-file data and 65,536
filesystem entries, including directories, failed writes and lifetime markers.
A bounded scan refuses symlinks, special files, excess depth and incomplete
inventory. One short filesystem lock covers each graph-plus-marker,
certificate-plus-producer or archive publication. Admission reserves the entire
additional write peak, including replacements and JSON temporaries, before
writing; children compile without that lock. Concurrent publishers cannot
independently spend the same remaining capacity. Contention or fullness takes
the genuine compiler path; no old proof is accepted to escape the limit.

Lifetime markers are deduplicated by graph content rather than native run key.
They still preserve every potentially live compiler generation. No unknown
live graph is deleted and no dep-info is rewritten to reclaim space. Full graph
size is reserved even for an identical existing graph, so a near-full arena can
conservatively refuse an otherwise reusable proof.

Native compiler request scratch and initial private copies belong to the
existing managed run's sidecars; only published graphs, checked certificates,
producer records and archived proof bodies enter this arena. These limits do
not cap all ordinary managed-run logs or other Sail caches. Existing managed
project pruning continues to own those lifecycles.

The source controls exercise actual graph publication and marker deduplication,
concurrent admission, failed/partial writes, count/byte/overflow limits and real
symlink/FIFO refusal. Those source controls alone do not establish native
acceptance. The later final-source `83128ad0` qualification includes complete
cycle and aggregate storage costs for its stated Linux/profile scope.

## Pinned Cargo routing

The actual Cargo-Verus driver decoder includes internal routing tokens. The
closed option policy admits exactly one pinned dependency-compilation mode,
checked primary/dependency identity, the official vstd mode only for that
admitted unit, and the exact declared verified imports. Cargo also emits a
self-lib if-present import for packages with auxiliary targets; explicit self
externs remain forbidden. Foreign, duplicated or malformed modes/imports and
Rust tails refuse. Literal argument/environment bytes remain bound in keys.

## Automatic-selection candidate

An isolated source successor selects a retained plan without a user flag only
on x86-64 Linux and Apple-silicon macOS, each with one exact normal-runtime
manifest that must equal that platform's pin `manifest_sha256` (a unit test
per platform enforces it). The macOS authority is
`67589de3c3f9681fb50dcd96313311e1a3d2776241d121f6c0e433be54cf7f0c` (fork
`f8589ed4`), qualified for ordinary public alloc/std reuse
([record](../validation/instant-revert.md#macos-qualification-on-runtime-f8589ed4));
the previous macOS authority `4392062a…` (fork `64baccbb`) was qualified too
([record](../validation/instant-revert.md#macos-qualification-on-runtime-64baccbb)). The `83128ad0` gates
below used manifest `a845cd2f…` (fork `e5365c05`). The authority is now
`576558fc76eecfdd91908fb85e30916e9d78b0153a144041f5b03937e4bcde26` (fork
`f8589ed4` on Verus `7d82994`), requalified for ordinary Linux public
alloc/std reuse ([record](../validation/instant-revert.md#runtime-f8589ed4-requalification)).
The previous authorities `cd78756b…` (fork `64baccbb`,
[record](../validation/instant-revert.md#runtime-64baccbb-requalification)) and
`f4568e29…` (fork `9c0ca7e7`,
[record](../validation/instant-revert.md#verus-7d82994-runtime-requalification))
were requalified too. A valid engine with
another manifest compiles normally and notes `SAIL3136`. It calls existing `Installed::at` with fresh complete artifact reads and binds
the actual selected launcher and Z3. Config currently has no held fresh
`Installed` authority to reuse, so this validation adds an invocation cost; no
metadata memo substitutes for it. The same 718-file provider/native/source
policy, observer schemas, keys and final Current checks remain in force.

The existing serialized runtime plan reaches both native observation and
verified restoration without adding an environment switch. The outer caller's
absence semantics remain unchanged. Explicit vstd-boundary mode suppresses
automatic retention, both explicit requests still refuse, and force-fresh
remains fresh. macOS and unknown runtime/source contexts compile normally.
Those gates pass at final source `83128ad0` for the public alloc/std Linux
profile; current delivery PR checks and merge remain pending. Earlier failed
cohorts are not retroactively accepted.

## Separate public-provider authorities

The automatic successor retains the existing 718-file path policy byte for byte
and adds two closed public registry profiles. Each binds 15 genuine `.crate`
archive SHA-256 values, every archive member and compiler checksum/size (662
files), exact package versions, source identities and features. One matches
ordinary current-pin new libraries; the other matches current-pin Vest's locked
versions and alloc/std features. They are not a Cartesian allowance of versions
or features. Unknown graphs still compile fresh. Producer certificates bind
the selected policy digest; no certificate from the path profile or other
public profile can establish a different authority. The actual GHOST/EXEC and
native-read checks, private graph, namespace exclusions and Current checks remain
mandatory. No observer protocol or query bytes change.

Current-pin Vest changes only its vstd Cargo version and resulting lock entries
to Sail's current pin. It does not use path-provider patches or change proof
source. It is a separate scenario from the historical unmodified Vest pin.
Actual offline Cargo metadata and all archive/compiler checksums pass locally;
those local source-admission controls alone are not native automatic reuse
qualification. The later final-source ordinary cohort is separately qualified
in the validation record.

## Amendment 2026-10-20: retention must pay for itself (#156)

Retention is optional restoration work, and on a cheap target it costs more
than it can save. On the current Linux runtime the ordinary new-library
profile admits the one-function `sail new` demo. Its retention checks read
the private dependency graph (vstd's 43 MB `.vir` and the macro libraries)
before and after the compiler, 1.5 s per edit and 0.7 s per revert, against
a 0.8 s compile. Restoring that crate was slower than compiling it.

Decision: each verifying compile records its compiler wall time and, when
retention was engaged, the time spent in the driver's retention admission,
restore attempt and finish. These records sit in `retention-costs/` beside
the proof context's receipts. A run prepares no retention plan when every
selected target has a record whose compile time is below 4× its retention
time. A target with no record, an unreadable or other-version record, or no
measured retention keeps retention. So a project's first run, including cold
Vest A, still captures its proof, and the A → B → A return stays instant.
The ratio rule follows from the costs: one retention costs about one restore
check, so a target whose compile is under 4× that saves little even on a
revert and pays the full cost on every edit. Measured examples: the demo
compiles in 0.75 s against 1.4 s of retention and skips retention after its
first run. Vest compiles in 26–29 s against 1.4–1.7 s and keeps it.

The records authorize nothing and bind no proof. A skipped plan compiles
normally, so verdicts, diagnostics, receipts, cache keys and every check
compare the same bytes. Timing makes only the cost decision vary from run to
run. The threshold is the constant `RATIO` in `managed/retained/cost.rs`.

## Amendment 2026-10-20: the source snapshot reads the selected closure (#278)

**Status:** accepted by the owner 2026-10-20 (#289).

Every retained source snapshot read every package in Cargo's metadata. Vest's
metadata lists the whole workspace and every resolved registry package, about
11,600 files, although `-p vest_lib` compiles only `vest_lib`, vstd and the 15
closed providers. A cold run takes that snapshot five times: before and after
vstd's compile, before and after the target's, and at publication. Each took
200–265 ms on macOS and about 175 ms on Linux, almost all on the critical
path.

Decision: the snapshot reads the packages in the selected targets' dependency
closure (every resolved dependency kind, as `runner::dependency_closure`
computes it), plus every workspace member's manifest, the workspace manifest,
lockfile, toolchain file and Cargo configuration it already read. Receipts
already scope their inputs by the same closure (`input::closure_digests`). A
package without an id is never dropped. Nothing else changes: full content
reads, the closed provider policy, namespace admission over the compile
roots, the compiler-read coverage of every loaded source in both phases and
the private dependency graph.

Why this is sound: a file of a package outside the closure cannot reach the
proof unnoticed. A source the compiler loads must be covered by the snapshot
with matching bytes, or recording refuses. The compile roots, whose namespace
facts are bound, are the closed providers' and the selected manifests'
directories, all inside the closure. Binary dependencies are bound by the
private graph inventory and their producer certificates. Member manifests stay
bound because Cargo reads them to resolve the workspace.

Effects: retained keys change, so entries recorded before this change miss
once and are aged out normally. An edit to a package outside the closure, for
example another workspace member, no longer invalidates a retained proof,
which matches the receipts. Each snapshot takes 25–50 ms on Vest instead of
200–265 ms.

## Amendment 2026-10-20: native units are certified after their compile

Accepted by the owner. Native units no longer wait for their dependencies'
certificates or for other units' scratch directories before compiling; they
are certified after their compile by the same checks. See
[ADR 0061](0061-pipelined-native-admission.md).
