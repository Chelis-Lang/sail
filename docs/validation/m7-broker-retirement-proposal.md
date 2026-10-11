# Preparation broker retirement: isolated proposal

Status: design and std-only protocol model. No candidate source, runtime,
qualification assertion or deadline changed. This proposal addresses availability
at broker admission; it does not explain or fix the three imported-namespace
refusals in [the failed original qualification](m7-preparation-namespace-qualification-independent-audit.md).

## Definite interleaving in the frozen broker

The existing preparation gate serializes worker operations. It does not serialize
compiler-side disposal:

1. A worker completes its preparation operation. `Lease::drop` sets
   `requester=false`, notifies the compiler service and releases its Shared Arc.
2. The worker returns from `with_namespace` and releases `PREPARATION_GATE`.
3. A second worker acquires that gate before the compiler thread consumes the
   notification and disposes its Request. The first Shared still owns the
   16 KiB active reservation.
4. The second worker's `Lease::pair` compare-exchange expects zero, sees the old
   reservation and refuses. This emits the separate outer `source-map` miss.

The compiler's sole reporter receiver prevents concurrent services. It does not
prevent step three: the second worker performs admission before enqueueing its
request. The permanent retirement reserve proves bounded physical overlap; it
does not make this admission succeed. This interleaving follows directly from
the frozen source. Its occurrence in the retained failed compiler run remains
unestablished until the separately reviewed failure observation executes.

## Narrow normal-path correction

Add one fixed Completion control to the already admitted 16 KiB broker family.
Its predicate means only that the previous compiler service has retired. It is
not source authority. Keep the same Shared quota owner and existing non-waiting
Lease Drop.

The compiler handler installs a completion RAII guard **before** borrowing
`map.files()`. It invokes the unchanged service/validator against that live
borrow, then drops the borrow and the service Request before signalling
completion. On unwind, lexical disposal releases the later files guard before
the completion guard signals. No queued stock diagnostic is processed during
this scope, and no source object crosses the bridge.

After Ready succeeded and the worker's operation returned, `with_namespace`
explicitly disconnects and waits for that completion predicate. It then drops
the Lease while `PREPARATION_GATE` is still held. With the compiler Request gone,
the worker is the last Shared owner: its fields, quota owner and Arc disposal
finish before the gate can admit the next operation. The operation result,
restored effects, counters and publication are unchanged. The acknowledgement
must also run for an ordinary inner refusal (`Some(None)`), rather than only a
successful cache publication.

Ready failure, queue failure and worker unwind retain the existing Drop path:
disconnect and notify, with no acknowledgement wait in Drop. Such paths may
still conservatively refuse a later admission while retirement finishes. An
acknowledgement failure must not turn an already restored operation into a second
decode, retry or rollback; it is an availability aid only.

Completion uses a predicate and notification under the same mutex, preventing a
lost wake. Spurious wakes inspect the predicate again. Its RAII notification may
recover a poisoned completion mutex only to record lifetime disposal; it cannot
grant source authority. The service is already woken by requester disconnection
and performs no external operation before returning. There is no new thread,
queue, filesystem work, polling loop or arbitrary timeout. Existing external
compiler supervision remains unchanged. This is a resource-bounded handshake,
not a hard real-time scheduling guarantee.

## Physical ownership and required controls

Admission must cover Shared, Completion, their Arc headers, one boxed request and
the fixed retirement token before either new allocation. The existing 16 KiB
active reservation is ample prospectively; actual pinned source sizes still need
an executable assertion. The final Shared owner refunds its active reservation
at the existing disposal point. Nothing refunds it merely because the requester
disconnects or the completion predicate becomes true.

The permanent 64 KiB reserve stays charged. A small old Completion shell can
still be returning from its notifier after the worker observes completion and
the next family is admitted. The single compiler receiver and gate bound that
overlap to one current and one retiring family. Conservative native accounting
includes eight initialization candidates for the new Shared and Completion
mutex/condition-variable pairs, plus two native objects in one retiring
Completion. Old Shared native fields have already disposed before its quota
refund. The proposed full-source 4 KiB shell/layout check must therefore use
**ten 256-byte native terms**, not eight. Three existing reporter blocks plus
this margin must still fit the unchanged 64 KiB reserve. The reporter enum remains
boxed and its size/alignment assertion must remain intact.

The phase ceiling remains **500 MiB + 80 KiB**, below the original 512 MiB. This
is a prospective retained-allocation argument, not RSS or qualified performance.
Neither the active reservation nor the permanent reserve increases.

The [isolated source](broker-retirement-proposal.rs) passed four model controls
on host Darwin arm64 with Rust 1.99.0, under eight-second external supervision:

- The old disconnect sequence deterministically refuses the next pair while
  the previous service and namespace stand-in remain held.
- The proposed normal finish cannot release the worker until service and guard
  retirement, then drops the last Shared owner before admitting the next pair.
- Controlled worker unwind completes without an acknowledgement wait in Drop.
- Fixed model sizes including ten conservative native terms fit 4 KiB.

The model namespace is an owned atomic stand-in, not an actual rustc SourceMap.
Its layout check is not the pinned std/libc or full reporter-enum assertion.
[The result](m7-broker-retirement-model.json), [stdout](m7-broker-retirement-model.stdout)
and [stderr](m7-broker-retirement-model.stderr) retain this limited scope. The
stderr panic is intentional and caught by the unwind control.

Before source integration, actual pinned controls must preserve all old broker
tests and add real compiler-thread guard retirement, service unwind, notification
before waiter registration, cancellation/drop, after-refund physical overlap and
the revised complete-layout assertions. B's current failure-only diagnostic is
independent and remains necessary to attribute the real compiler failure.
