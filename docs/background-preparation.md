# Background preparation

Available in published `0.2.0` and the earlier registry prerelease `0.2.0-rc.1`. Installed feature controls pass on native Linux and macOS;
performance limits remain below. Published 0.1.0 does not provide this behavior.

After a successful Current Cargo verification, Sail can prepare the next
unchanged invocation in its daemon while your command has already returned.
`sail new` can also start ordinary project verification after creating the project,
without waiting for Cargo or verification to finish. This uses the same pinned
runtime and proof checks as explicit verification. Exact focused Cargo invocations
can prepare only that same argument-bound scope; their target remains incomplete,
and a later whole-target call still requires independent full verification.
Interactive-engine focus is not prepared.

Ordinary verification, watch, repair and jobs take priority across workspaces.
A call never uses a prepared result it did not wait for, and it verifies
normally. It drops queued preparation and kills running preparation for any
other workspace, and a new project's initial preparation, at once. One case
waits: a running preparation that re-runs a successful verification of the same
workspace and arguments may finish, for at most 60 seconds from its own start,
while the call waits for it on the target lock. Killing it partway would leave
the compiler target without its build-script records, so the call itself would
rebuild every dependency, vstd included, and instant revert could not apply to
what that call compiled
([#595](https://github.com/Chelis-Lang/sail/issues/595)). A preparation that
started just before your edit verifies the edited source, and the call then
reuses that work. A preparation that ends without a Current result can still
leave the target without those records, so the next call rebuilds the same way
(see [known issues](known-issues.md)). Later reuse still checks current inputs and keeps the original
proof's establishing run. The handoff binds the successful run's complete closing input revision,
including generated inputs first discovered by a cold build. Before entering managed
verification, the worker freshly checks the origin's Current input record and declines
a source edit already made while it was queued. It requires the exact closing
revision again before recording; ordinary result revisions and
establishing proof provenance are preserved. Edits invalidate old preparation. Background progress
is never itself a verified result.

Preparation is automatic when the matching daemon supports it. To disable it:

```sh
SAIL_BACKGROUND_PREPARE=0 sail verify
SAIL_BACKGROUND_PREPARE=0 sail new demo
```

Inspect `sail status --json`'s `preparation` field for queued/running work and
completed-record, superseded and failure counts. These are lifetime activity
counters, not evidence that your current source is proved. The regular command's
diagnostics, exit status and JSON report remain authoritative.

If no preparation is offered, inspect `sail verify --output json` for Currentness
and `no_reuse`. An unqualified Cargo setup verifies normally without managed
proof reuse or preparation. The primary Verus pin uses Rust/Cargo 1.99.1; when
compatible with your project, select it explicitly with
`RUSTUP_TOOLCHAIN=1.99.1 sail verify`. Sail continues to honor project toolchain
choices rather than silently replacing them.

The optional connection attempt and its individual reads/writes use a 25 ms
timeout. A full listener backlog declines preparation through the ordinary
verification path.

Preparation uses at most one additional worker. Requests are bounded and newer
workspace requests replace older ones. Each captured output stream is limited to
16 MiB; exceeding that limit drains the output and declines replay publication. Stopping the daemon cancels its work;
unfinished work is not resumed on restart. An unavailable or older daemon simply
leaves ordinary verification available. Explicitly selected unavailable services
are not replaced by project creation.

This uses idle CPU and disk after the command returns. Whether it reduces an
immediate call, and its total CPU/memory cost, are separate measurements. It does
not enable unselected digest optimizations or change focused verification scope.

An opt-in budget maintenance pass that already started may finish. Its cap is
soft, and pruning rechecks workspace locks before removal; an already-started
removal can finish before a new call acquires that workspace.
