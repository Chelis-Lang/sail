# Independent preparation namespace review

Static review passes for the unselected `c47dee2` candidate. The
[exact record](m7-preparation-namespace-independent-review.json) binds the full
patch, narrow delta, domain and checked identities. Complete-source compilation,
strict checks, ordinary runtime and the original qualification remain pending.
No compiler, proof, endpoint or runtime actor was launched by this review.

The earlier migration failure verified two functions with empty stderr but
published no new preparation. The parallel worker's stock diagnostic map was
`None`; preparation incorrectly required that same optional argument. This
establishes missing publication, not an accepted old-domain collision or an
imported-role failure. The earlier [runtime closure review](m7-late-import-handoff-runtime-independent-review.md)
remains a separate scope.

The correction keeps the actual SourceMap on the compiler thread. A separate
fixed-slot service holds its borrowed file guard while the worker performs the
existing complete preparation operation. Only imported identity words cross the
bridge, and the service invokes the unchanged current-namespace check. It never
renders diagnostics, loads files or consumes nested reporter messages under that
guard. Stock diagnostic `None`, worker concurrency and FIFO handling remain
unchanged. Disconnection and unwind release the service without waiting for an
acknowledgement; no peer retains the control mutex while executing validation.

The fixed control reservation is 16 KiB. A permanent 64 KiB reserve covers
marginal reporter-queue blocks and retiring control shells. This explicitly
accounts for quota refund preceding outer Arc deallocation. The proposed phase
maximum is 500 MiB plus 80 KiB, below the unchanged 512 MiB ceiling. The exact
pinned standard-library sources, native ceiling controls and paused-after-refund
control support the static argument. Actual full-source enum, block and shell
layout assertions still must pass; ordinary diagnostic queue ownership is not
reclassified as a bounded preparation allocation.

The review found no remaining static authority, deadlock, Send-ownership or
allocation blocker. It authorizes the already approved private complete-source
controls, not source selection or a qualification claim. All original source
assertions, diagnostic and wire comparisons, hit requirements and 600-second
bounds remain required.
