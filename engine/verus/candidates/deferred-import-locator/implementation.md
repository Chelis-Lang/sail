# DeferredLocator implementation and control scope

This is an unselected source candidate under proposed [ADR
0033](../../../../docs/decisions/0033-deferred-import-diagnostic-locators.md).
[Identity](identity.json) freezes the full patch, its delta from `50667954`,
the seven touched or added source files, and the helper/control inputs. The
selected runtime, the b46 source and its strict qualification failure remain
unchanged. No new ordinary source build, vstd proof or qualification has run.

## Authority and consumers

`capture.rs` adds a separate DeferredLocator state to the existing owned
mapping. It owns the exact absolute bounded UTF-8 descriptor path and one
current Foreign metadata identity, alongside the Record's original stable
crate/range/expected raw hash. Capture reads the actual Delayed descriptor; it
does not call the resolver or FileLoader. The original raw bytes have no
algorithm tag. The Foreign identity retains its actual typed hash independently
and does not attest what a future private loader will return.

Pre-focus validation uses the same owner. It requires the actual descriptor
still Delayed with the exact path/hash and verifies the complete current
Foreign identity and namespace. Added ambiguity, overlapping current ranges,
changed descriptors and early Loaded/None/Busy states refuse the role. It never
promotes an unavailable state or writes a resolver value.

The engine uses separate Loaded and Deferred provenance tags and the new v6
domain. It binds original words/range/hash, exact descriptor path and current
typed hash/stable identity/name/lengths. Current cnum/index/start/end validate
the same callback but do not enter cross-callback equality. Imported raw owners
must be exactly unit; the existing exact `(id,data,as_string)` lookup restores
the current raw Arc. No stored SpanData or translated location is restored.
`current_range()` remains unavailable for DeferredLocator.

The [closed consumer audit](../decode-ownership/deferred-locator-consumer-audit.md)
continues to apply. Current stock Reporter/SpanContext methods are unchanged.
They alone resolve primary and label spans, load/check sources, emit diagnostics
and transition the current descriptor after handoff. The immutable capsule stays
Deferred, including when the current resolver becomes Loaded or None. Local
raw/hygiene/parent handling, semantic inputs, routed graph reads and effects are
unchanged. Existing old-domain rejection remains, with an added explicit v5
domain comparison in the actual VIR test.

## Physical admission

The two roles share one process-wide 8 MiB owner, not two families. The enlarged
`Record` is included in the checked count × `size_of::<Record>()` calculation
before reserve; capacity is checked afterward. At most 4096 records and 4096
current source files are supported. Header and control scratch reserve 1024
bytes. Each owned path/name is at most 4096 bytes. Before copying a path or
formatting a name, retained bytes plus two complete bounded strings and 1024
bytes must fit; actual capacity enters the retained sum. The pinned FileName
formatter's borrowed streaming chain is covered by the existing formatter
audit. No arbitrary custom formatting or source-text allocation is introduced.

Late path/hash/name comparisons borrow their inputs. The initial owner moves
into the existing Arc snapshot; no additional source table, compiler borrow,
SourceMap/SourceFile Arc or seed copy is created. Trace serialization borrows
the admitted path/name under the existing observer mutex and byte/record limits.
No loaded-content or translated-range claim appears in a Deferred trace.

Every failure path physically disposes its strings and records while admission
is still held. Revalidation can replace a role with Unavailable, dropping the
old payload under the same owner. Final drop disposes the record vector before
the fixed admission is released. Existing capture-time peak publication and
clear/terminal sampling include this family. The prospective simultaneous
maximum remains 492 + 8 = 500 MiB within the unchanged 512 MiB gate. This is an
allocation-owner argument pending executable qualification, not a compiler RSS
or observed peak claim.

## Executed and pending controls

The [helper record](helper-controls.json) preserves the initial failed compile
(missing pinned `Pos` trait import), the intermediate 19-control pass, and the
final 22-control pass. The final run took 3.294 seconds at ordinary priority,
with bounded 60-second compile and 30-second test processes. Cases run serially
because the actual process-wide family admits one owner; all internal race
controls remain exercised. The harness extracts the pinned resolver and uses
actual SourceMap/SourceFile/Span APIs. Its charge-publication shim is deliberately
empty, so it does not certify engine telemetry.

Thirteen unchanged Loaded controls and nine Deferred controls establish:

- Zero loader reads and no state changes during capture/revalidation, with no
  Deferred coordinate authority and no second family admission.
- Actual stock success, hash mismatch and missing-file behavior after handoff,
  repeated resolution, root hygiene/no parent, and duplicate StableSourceFileId
  reuse without a false fresh-content attestation.
- Exact descriptor changes, Busy/early state changes, range/hash/metadata and
  namespace ambiguity refusals, relative/non-UTF8/NUL/oversized path refusals.
- Current primary/label display resolution and ordering, plus physical disposal
  before the admission-release race and unsupported multi-file Loaded ranges.

Three additional controls in `source-controls.rs` are appended only to the
actual source test module. They are **uncompiled** by the reduced helper:

- Actual engine provenance is path-bound, raw-unit-only and independent of
  current coordinate relocation.
- The actual prepared Span codec restores the exact current raw Arc and refuses
  changed id, packed words or spelling.
- Actual stock Reporter plus admitted ReplayMessages preserve four emission
  modes, levels, order, primary/label diagnostics and grouped reporting for
  stock success/hash mismatch/missing cases. A test emitter records rustc
  diagnostics; this is not the full CLI stderr oracle.

The test emitter records `DiagInner` level/messages/primary spans/labels, not
the complete formatted diagnostic stream. Missing/hash-failed label fallback
`dbg!` stderr is outside that capture. After the four scalar replay calls,
both arms call actual Reporter `report_as_multi` directly: this checks current
stock group resolution, not a cached group operation or QueuedReporter sorting.
The untouched original pipeline remains responsible for exact full stderr and
its queue/order behavior. No coverage claim substitutes these field controls
for that oracle.

These source controls, source telemetry, actual VIR/driver strict checks, fresh
ordinary vstd/manifest and all original fourteen stages still must execute.
Original 600-second bounds, exact stock stderr, ordered same-input wire/effects,
old-domain migration, injected-hit/cancel/late-EXEC and genuine previous-revision
FOCUS assertions remain unchanged. Safe helper behavior is not a preparation
HIT or HE/Vest product/performance acceptance.
