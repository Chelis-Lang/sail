# Exact DeferredLocator proposal

Status: unselected implementation for independent review under proposed
[ADR 0033](../../../../docs/decisions/0033-deferred-import-diagnostic-locators.md).
The [candidate patch](../deferred-import-locator.patch) has passed its private
ordinary source build with fresh vstd verification (2060 verified, 0 errors).
Full source-control and original qualification gates remain pending. Its helper
controls pass against pinned rustc APIs;
[identity](identity.json) and [evidence](helper-controls.json) distinguish those
controls from the pending actual source and original fourteen-stage gates. The
`50667954` candidate and its failed qualification remain immutable and
unselected. This candidate introduces no eager source loading or resolver
mutation. The existing full standard conformance attempt is independent.

The independently reviewed committed bytes have a closed private ordinary
build. [Launch evidence](build-launch.json) records the
exact builder/archive/patch/features, ordinary-priority two-job actor, private
target ownership, compiler-only seed inventory and corrected pre-compiler
archive-transfer setup failure. That historical launch record remains unchanged.
[Build evidence](ordinary-build.json) records the successful 1720.396-second
build, exact thirteen-artifact manifest, source/lock identities and closed owner
before source tests. Actual full library controls and strict Clippy are running
in the private target; no source or product qualification is claimed, and no
selected runtime changed.

[Inactive compiler cleanup](preceding-compiler-cleanup.json) retired only the
preceding b46 compiler intermediates after the new builder closed. All thirteen
b46 runtime bytes, its source and raw failure evidence remain unchanged; three
probe/test executables were separately retained. The active candidate target is
protected. Allocated retirement is recorded separately from concurrent host
backing-space changes.

The [actual first refusal](../loaded-foreign-authority/delayed-role-review.md)
is an imported raw-unit span for `vstd/function.rs:151:20: 151:65 (#0)`.
Constructor and pre-focus states are both Delayed. A LoadedForeign-only snapshot
correctly misses it. This proposal records the **exact diagnostic recipe** of
that Delayed descriptor; it never attests loaded content or promotes the state.

## Proposed owned schema

Extend the existing private mapping's state enum with `DeferredLocator`. Keep
the existing original crate/range/expected hash fields in `Record`, rather than
duplicating them. A DeferredLocator owns these additional fields:

| Field | Authority and proposed representation |
|---|---|
| `loader_path` | Exact UTF-8 bytes of the actual `ExternSourceInfo::Delayed.filename`, copied only if absolute, NUL-free and at most 4096 bytes. Decline other path encodings. Do not canonicalize, resolve symlinks, trim, join a guessed base or infer a display filename. |
| `foreign_identity` | Actual unique current `ExternalSource::Foreign` candidate: stable imported crate, typed `SourceFileHash`, `StableSourceFileId`, complete bounded FileName encoding, normalized and unnormalized lengths. This is imported metadata identity, **not** the hash algorithm/content of the current file loader. |
| `current_validation_range` | Actual current index/start/end, used only for same-callback revalidation and ambiguity checks. Never restore these coordinates or include them as cross-callback equality fields. |
| `initial_role` | Exact tag `DeferredLocator`; later stock Loaded/None transitions never change this immutable tag. |

The owning Record retains original stable crate, original start/end and complete
expected raw hash bytes/length from the borrowed original metadata table. The
descriptor's hash must equal those bytes. The original table has no hash-kind
tag: never manufacture one. The current Foreign candidate's actual typed kind
is independently bound; candidates with equal raw bytes but ambiguous content,
crate, range or typed identity decline capture. In particular, neither matching
display text nor a current Foreign file alone proves the descriptor loaded it.

## Capture and handoff

1. Only the existing optional preparation selector enables capture. During
   `SpanContextX::new`, original metadata remains borrowed. Read the **actual**
   descriptor with `try_lock`; Loaded uses the existing policy, Delayed is
   considered for the new role, and None/Busy/poison remains unavailable. Never
   invoke `resolve`, `load_file`, `Span::span` or SpanData Debug.
2. Require one original range entry in the selected crate and one current
   Foreign candidate matching the expected raw hash and stable crate. Require
   nonoverlapping original/current ranges, matching normalized length and exact
   typed/name/stable identity. Ambiguous original hash-table collapse, duplicate
   content, overflow, absent Foreign or missing original metadata declines the
   role. This deliberately excludes metadata-only delayed files without a
   uniquely associated current Foreign owner.
3. Before freezing Sources immediately ahead of focus, move the **same** owned
   family through revalidation. Require the actual descriptor still Delayed with
   exact same path/hash/original range. Independently check the complete current
   Foreign identity/table ambiguity and current range. Constructor Delayed that
   is now Loaded/None/Busy misses; do not reconstruct its discarded descriptor,
   reset current state, or allocate a second seed.
4. Freeze the immutable recipe for focus/full workers. No compiler borrow,
   SourceMap/SourceFile Arc, resolver mutex, source text or old raw span escapes.
   Stock reporting may later move the **current** descriptor Delayed→Loaded or
   Delayed→None. The snapshot remains Deferred and is not reclassified. It never
   writes any value back into the current resolver.

The absolute path restriction avoids an unbound changing cwd. It does not claim
filesystem immutability or fresh source contents. No extra file is read by this
feature, so there is no added loader/SourceMap allocation or eager-load input
boundary. The current untouched stock reporter owns ordinary on-demand loading,
hash checking, offset translation, failure and rustc emission. Any custom-loader
scope not covered by the complete current invocation/settings identity must
decline this role rather than infer a loader algorithm from Foreign metadata.
The pinned main ghost compiler uses `VerifierCallbacksEraseMacro`; its setup has
no custom FileLoader assignment. The custom `TCFileLoader` in `driver.rs` belongs
to the separate fresh trait-check compiler, not this handoff. Confirm that exact
closed call boundary in the candidate; do not turn it into a general guarantee
for arbitrary rustc clients.

## Provenance and exact current raw binding

Use a new explicit preparation domain, proposed
`sail-prepared/graph-ledger/import-locator-authority/v6`, with separate imported
tags for LoadedForeign and DeferredLocator. Keep Portable version3 only if its
representation remains unchanged. Hash the Deferred tag, exact two packed
original words, original authority and complete immutable recipe above; check
current coordinates but do not hash relocation coordinates. Every other typed
graph/lowering/settings/effect/span dependency remains unchanged. Existing local
raw lo/hi/hygiene/parent provenance is untouched.

Eligibility additionally requires raw owner **exactly `()`**, two words, checked
lo≤hi within the unique original range and the existing exact
`(AstId,data,as_string)` current binding. Decoding restores that current raw Arc,
not an old SpanData or a synthesized location. Unknown raw Any, missing or
ambiguous bindings and changed spelling remain misses. Grouped sorting must
continue to see `from_raw_span(unit) == None`.

The private provenance memo is scoped to this single immutable Sources instance.
Its imported key continues to include both exact words. New domain tests must
reject old v5 artifacts and in-memory old entries even under an identical caller
context; no relabelling, resealing or copied qualification receipt is permitted.

## Consumer boundary to close before implementation

Pinned `spans.rs::from_air_span` returns local raw spans directly; imported unit
spans use `unpack_span` and the current resolver. `Reporter::report_as` resolves
primary spans and labels with its current SourceMap. Fresh trait conflict/check
and erasure paths must remain freshly executed; some call `from_air_span` without
a SourceMap and therefore observe only the current stock state.

B's independent closed scan finds no raw-span downcast or SpanContext resolver
access in VIR lower/poly/interpreter/SST/AIR. Their span uses are serialization,
identity, as_string logs and message construction. Before any hit, verify this
closed set again against the exact new patch and preserve routed guard coverage.
Cached messages must preserve report/report_as/report_now/report_as_now and group
semantics and go through the current Reporter. FocusReporter and oracle shadow
Collectors must not resolve descriptors or change the primary state. A shadow
wire/effect comparison alone cannot prove resolver side-effect equivalence: add
actual independent Reporter/SpanContext controls as well.

## Physical ownership

Reuse the **one** process-wide 8 MiB admission, including Loaded and Deferred
records together; do not add a second 8 MiB family. Count at most 4096 records.
Before `try_reserve_exact`, check count×`size_of::<Record>()` plus header/control
margin; validate actual capacity afterward. For each record, pre-admit complete
bounded name/path bytes plus old/new growth and formatting scratch before any
copy. Store only one copy of each needed name/path, with checked capacities.
Borrowed late comparisons do not allocate names or clone PathBuf/hash tables.

Admission-time PEAK publication must cover successful and failed captures.
Retained telemetry charges the entire 8 MiB while any family owner is live.
Physical records/names/paths must be disposed before the final admission release,
including errors, poisoned locks and Arc clones. Existing formatter audit applies
only to its exact pinned FileName chain. Preserve its scratch accounting and add
the new path-copy accounting explicitly. Prospective phase maximum remains
500 MiB (492+8); this is not yet a qualified physical bound or total compiler RSS.

## Required adversarial and executable controls

- Actual pinned Delayed descriptor + present equal-hash Foreign: capture causes
  no load/state transition; current raw unit remains unit. Fresh and restored
  ordered primary/multispan/grouped/immediate diagnostics match, including current
  resolver success, missing file, hash failure and repeated emission. Include
  duplicate StableSourceFileId reuse: stock may read a path then return its prior
  SourceFile. The recipe must follow that current stock behavior, not claim a
  freshly read loaded owner.
- Alter each exact path/hash/original crate/range/typed kind/stable ID/name/length;
  add equal-content or overlapping owners; remove metadata/current owner. All
  refuse. Change only current relocation coordinates with a complete unique
  current authority and require current rendering, never old coordinates.
- Constructor→pre-focus Delayed→Loaded/None, Busy/poison, changed descriptor,
  added ambiguity and relative/non-UTF8/overlong path refuse. After the immutable
  handoff, exercise untouched stock success/failure transitions and prove no reset
  or state promotion by preparation code.
- Keep imported-word/raw-kind/as_string/missing-binding and all local hygiene,
  parent, changed-content/range and clone-lineage negatives. Old-domain controls
  must reject actual old intact artifacts and preserve their bytes.
- Prove bounds with real oversized names/paths, failed admission, capacity limits,
  final-owner races and early-selection/frontend failure telemetry. No unbounded
  source load is introduced or exempted as compiler working IR.
- Run actual VIR/driver/strict checks, ordinary fresh-vstd build, original14 source
  pipeline, valid-HIT same-input ordered wire/effects/diagnostics and injected HIT
  faults, cancel/late-EXEC/normal-termination controls and original 600-second
  previous-revision FOCUS gate. Preserve strict progress-note failures; neither
  a safe miss nor this design can qualify the source or satisfy HE/Vest 2×.

Implementation is reviewable; the new full source build remains pending explicit
review. See the [implementation and control scope](implementation.md). Retain all
previous runtime, failure and selected-state evidence; no selected artifact
changes are proposed.
