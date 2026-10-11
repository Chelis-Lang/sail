# Preparation broker retirement acknowledgement

Status: file-only candidate overlay, unselected. The pinned helper's 16 controls
and isolated strict check passed. Removing its finish wait failed the unchanged
lifetime assertion as expected, without a timeout. Complete-source adapter/layout
controls, strict checks, ordinary runtime and original qualification are pending. The selected runtime and product behavior are
unchanged. This is an availability correction, not imported-span authority or
evidence that the previous qualification's namespace refusals have been solved.

Apply `from-boundary.patch` to the exact corrected diagnostic candidate
`3892328a418109596f6a4a5f3649f5e7d370b19321419c29780d7a3961b44d61`.
[identity.json](identity.json) binds all eight resulting source files, unchanged
source lock/domain, three changed files and the same physical limits. The
diagnostic-only baseline continues independently; this behavior change must not
be substituted into its failure attribution run.

The exact old race and isolated model are retained in
[the design](../../../../docs/validation/m7-broker-retirement-proposal.md).
The worker can release its preparation gate while the compiler still owns the
previous Request; a subsequent pair then safely refuses admission. A sole
reporter receiver prevents concurrent services but does not order worker
admission after compiler disposal.

## Runtime-ready source change

`Lease::pair` reserves the existing 16 KiB before allocating Shared and one new
fixed Completion control. The completion predicate is lifetime-only. The
compiler installs its RAII Retirement token before borrowing `map.files()`.
After service and that borrow end, it drops the Sources reference and token.
Lexical unwind also drops the later namespace guard before the token signals.
No SourceMap, SourceFile, span or compiler context crosses the bridge.

After Ready and the entire operation return, the worker disconnects, waits for
that fixed predicate, then physically drops its Lease before returning to the
caller that holds `PREPARATION_GATE`. This applies to success and inner refusal.
No operation result, restoration, hit counter, key or publication is retried or
changed. Existing Lease and Request Drop bodies remain byte-exact, disconnecting
without an acknowledgement wait on early failure, peer loss or unwind.

The token can recover poisoned completion state solely to record disposal.
Worker completion-lock errors return through the existing conservative lifetime
path; they grant no source authority. No new thread, filesystem work, polling,
stock reporter routing or deadline is introduced. The original externally
supervised compiler bounds remain; the handshake is not a real-time guarantee.

## Physical proof and executable controls

The active reservation stays 16 KiB and the permanent queue/retirement reserve
stays 64 KiB. One retiring Completion can coexist with the next family after its
notification, so its storage and two native objects remain covered by that
permanent charge. The strengthened actual layout assertion includes two Shared
allocations, two Completion allocations, boxed NamespaceRequest bodies,
retirement handles, ten 256-byte native terms and the original fixed margin.
Original reporter size/alignment, old layout and three-block assertions remain.
The prospective maximum stays **500 MiB + 80 KiB**, below the original 512 MiB.
Nothing refunds active quota just because a worker disconnects or observes ACK.

The entire old nine-test broker module is byte-exact. Six appended pinned helper
controls exercise the old live-owner admission miss, actual SourceMap guard
retirement, notification before wait registration, validator unwind, a paused
old Completion after active refund, and complete fixed-layout bounds. A new
full-source Sources adapter test invokes the actual queue/handler on both success
and inner refusal. Its fixed test-only hook pauses after service while the real
files guard still lives. An observer releases that pause before assertions, then
checks that the worker could not return or refund the charge. The
[actual-call counterfactual](actual-finish-removed.patch) removes only the
`with_namespace` normal finish call; that full-source sensitivity remains
unexecuted. It must fail the same adapter assertion before claiming integrated
regression coverage. The std-only model and helper-level sensitivity are separate
evidence; [api-controls.json](api-controls.json) retains their actual scope.

`namespace.rs` is the exact helper from this overlay. `tests.rs` adds the pinned
native-type ceiling check without changing any original control. Complete
compiler source tests and strict checks must still run before an ordinary build.
The original stock diagnostics, exact same-input oracle, prior-revision hit
requirement and all 600-second qualification bounds remain mandatory. Default
proof repair and cache users must not treat this candidate as selected or
qualified, and no measured reader/proof latency gain is claimed.

The independent boundary diagnostic subsequently observed one `pair-busy`
refusal with 80 KiB retained, and three separate `UnknownCrate` validator
refusals. That is evidence of the admission race, not proof this uncompiled
adapter fixes every refusal or produces a persistent hit. The original failed
qualification and diagnostic remain separate.

Run the bounded helper reproduction with
`python3 engine/verus/candidates/preparation-retirement/test-api.py --work-dir <fresh-private-directory>`.
It compiles the pinned actual types without Cargo or a proof actor, preserves
raw output and the expected-negative result, and never changes the selected
runtime. The additional isolated Clippy invocation used the same positive
compiler arguments with `clippy-driver` and `-D warnings`.
