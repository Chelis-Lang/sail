# Single-threaded SourceMap test owner correction

Status: unselected. The ordinary corrected build passed, but the corrected
actual driver controls failed to compile. [Identity](identity.json) binds the complete new
[candidate patch](../../deferred-import-locator-source-owner.patch) and its
[narrow delta](from-a61.patch).

The immutable a61/436dd candidate passed its ordinary fresh-vstd build (2060/0),
38 VIR and 55 driver library controls, plus strict VIR Clippy. Its
[driver strict failure](../source-gate-a61-failure.json) rejected
`Arc<SourceMap>` in the new actual Reporter test. Pinned SourceMap is
single-threaded. The correction changes only that test's map owner to `Rc`,
inside the existing rustc compiler thread. No production function, diagnostic
mode, order, expected level, assertion or lint policy changes.

All helper/codec/physical/domain controls and the original fourteen stages stay
mandatory. The original 600-second compiler bounds and strict stock diagnostics
stay unchanged. Prior successful tests do not substitute for execution against
these newly bound source bytes. Source qualification, genuine prior-revision
FOCUS reuse and the aggregate preparation gate are still pending; the selected
runtime remains unchanged and the feature remains experimental under proposed
ADR 0033.

The next private control-file retention uses a content-verified move across the
cache/evidence mount boundary for only the same two known empty Cargo control
files after their holders close. The thirteen sealed runtime files never move;
all exact hashes and direct inventory are checked afterward. The original EXDEV
setup failure remains separately recorded.

[Build launch](build-launch.json) records the newly authorized ordinary-priority
two-job private build and compiler-only seed from the closed prior candidate.
Its runtime files, source and failed strict record remain immutable. This is a
launch record, not a qualified runtime. [Host executable deduplication](host-executable-dedup.json)
preserves every archived runnable path while sharing exact duplicate bytes after
holder checks; selected/new/qualification runtime paths are excluded.

[Ordinary build evidence](ordinary-build.json) records this corrected variant's
fresh 2060/0 vstd proof, 1054.968-second closed build and thirteen-artifact seal.
The subsequent [source gate](source-gate-4a3-failure.json) passed 38 VIR controls
and failed to compile the driver controls with E0277: the pinned diagnostic
emitter requires rustc's `DynSend` marker, which `Rc<SourceMap>` does not
implement. Neither strict check ran on this correction; no source or product
qualification is claimed. The previous a61
strict failure remains separate, and original fourteen-stage qualification has
not launched.

The pinned compiler itself stores `Arc<SourceMap>` in `ParseSess` and accepts
it through its distinct `DynSend`/`DynSync` contract. Those markers are different
from standard Rust `Send`/`Sync`. The fixture needs its exact custom `FileLoader`
and hash inputs, which `ParseSess::new` does not accept. A small actual-type
ownership control is being used before another correction; no constructor is
hidden from Clippy and no unsafe marker implementation is added. The failed
Rc variant, all thirteen runtime bytes, and both known empty Cargo control-file
records remain retained. All original Reporter assertions and qualification
requirements remain mandatory.
