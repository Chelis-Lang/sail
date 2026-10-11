# D20 active repair: sysroot coverage cause audit

The [terminal workflow record](m7-owned-reader-d20.json) retains a completed
active repair with complete successful obligations and Unknown input freshness.
The selected HumanEval target verified 331 functions; the complete 1871-function
vstd report was reused. Accepted was false, so the original workflow stopped
before either reader collector or retained CLI/MCP integration. This is an audit
of original artifacts and exact `d20d034` source, not a proof retry or a new
observation owner.

## What the original evidence establishes

The primary worker note is `sysroot-namespace-invalidated`. In
[Baseline::clean](../../crates/sail-verus/src/managed/sysroot.rs), that label
collapses failure of either the namespace observer or the explicit-files
observer. The namespace observer runs first; if it fails, short-circuiting
prevents the files and lease checks. Therefore this note does not establish a
lease syscall refusal, helper death, a changed file, a mount event, or a specific
writer. The later generic public validation note is a separate daemon projection
note and does not explain the original worker failure.

Original per-context runtime identities retain 418 ordered rows. Of those, 327
belong to the discovered Rust 1.98.1 x86_64 Linux sysroot. The exact identity JSON
is 79,091 bytes and its SHA-256 is
`71e1eccfbde7036fd46eb767a1ae6577f9314cf7af0e1707aff1200fd159801c`.
The private derived sysroot inventory is independently sealed in the terminal
record. Matching final runtime/input seals do not rule out transient events or
restored mutations during the proof.

The original setup timers record 12.686 seconds acquiring sysroot coverage,
99.321 ms for the optional handoff, 37.358 seconds in local sysroot hashing,
42.208 ms at the initial sysroot barrier and 3.763 ms issuing the natural fill
hint. The hint phase is reached only after the original local barrier succeeds,
with a socket/scope and no imported snapshot. Thus the baseline was initially
complete and used original local hashes; this run does not demonstrate a warm
handoff. The later worker note records lost coverage before receipt admission.
None of these timers identifies the loss event or attributes its cause.

## Actual observer coverage and conservative refusal paths

For each discovered sysroot, the namespace observer explicitly tracks `lib`,
`lib/rustlib`, `bin/rustc` and `bin/cargo`, before enumeration. It recursively
watches existing descendants and watches every existing ancestor needed to
protect namespace bindings. The second observer tracks every enumerated file
and its canonical/symlink bindings. Both retain their independent mount guards.
The library namespace walk is broader than the 327 content-hashed files: the
installed rustlib tree also includes installer manifests, debugger helpers and
`rustc-src`, besides target library directories. A tracked event there is a real
namespace invalidation under this current contract, even if that particular file
was not one of the 327 hashed rows. No original artifact records such an event
here.

[Observer::clean](../../crates/sail-verus/src/managed/precise.rs) separately
records reasons for mount-generation/root identity changes, changed root
bindings, native watch loss/overflow, an unbound watch, tracked events, read
errors, event-count exhaustion and nonempty-drain budget exhaustion. Ancestor
watches can receive unrelated sibling events. The existing related-path predicate
filters their mutation classification, but all events still consume the 32,768
event bound and the nonempty drain's 20 ms work budget. An unrelated queued event
can therefore cause a conservative refusal if that drain cannot finish within
its bound. This is a code-level possibility, not a diagnosis of this historical
run; the original report discarded the narrower reason and no raw watch event
was persisted.

Mount guards intentionally latch changes anywhere in the actual mount namespace,
including restoration, rather than relying on equal final mountinfo bytes.
Existing shared namespace activity is another possible refusal path, with no
recorded evidence selecting it here. The retained logs show no narrower observer
reason. Retrospective file metadata or current holders cannot recover the lost
inotify queue or identify the writer that produced an earlier event.

## Next diagnostic seam

Preserve the existing observer role and `failure_reason` at the first failing
barrier, retaining current short-circuit order and all refusal semantics. That
diagnostic-only reporting change is implemented; strict checks and all seven
Linux sysroot controls [pass](m7-sysroot-explicit-detail-control.md). The
[isolated diagnostic plan](m7-sysroot-proof-diagnostic-plan.md) retains the
original proof and exchange bounds. Its new evidence can distinguish namespace
versus explicit files and the exact existing reason without guessing from phase
duration. If event attribution is needed,
record a bounded opt-in failure-only mask and root-relative path for the actual
first tracked event; do not scan unrelated processes or print environment/input
contents. No exclusion, watch reduction, deadline change or coverage relaxation
is justified by this coarse historical result.

See [freshness](../freshness.md), [native product setup](../native-product-setup.md)
and [ADR 0031](../decisions/0031-guarded-sysroot-digest-handoff.md) for authority and
fallback rules. This audit makes no live-reader, source-budget or performance
acceptance claim.
