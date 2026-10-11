# Setup publication within the existing shared lookup budget

A deterministic control establishes a race in configured HTTP setup. While the
single setup owner constructs a client, the lookup helper observes Initializing.
The old path immediately publishes Skipped, even when construction finishes
within the original lookup budget. This can force a solver fallback unnecessarily.
Removing the new wait reproduces that failure; the regression and timeout
negative are documented in [the exact record](m7-http-setup-contention.json).

Only the existing helper waits. A condition-variable predicate shares the
publication mutex; direct resolution, daemon startup and status remain
nonblocking. The absolute deadline is established before helper spawn, and the
foreground's original 250 ms timeout remains unchanged. There is no new constructor
or retry. Publication clears prior configuration suppression before waking
readers. An elapsed deadline or an already published fallback prevents starting
a new request. Abandoned setup still cannot install a client, and a lookup waiting
for it ends at its existing bound. Signature, solver, query and trust checks stay
on every accepted reply.

The positive control holds a refresh after establishing fixture transport so it
can expose the contention deterministically. This is a supplementary regression,
not prewarming any original acceptance fixture. The timeout negative requires
fallback and backoff, exactly one constructor, and zero late network requests.
Host daemon controls and strict Clippy pass; the integrated Linux gate and original
solver suite are pending.

The [native d333 run](../../bench/validation/m7-linux-ci-d333-native-failure-2026-10-11.json)
failed the unchanged whole-session replay control, with equal replies but an
extra solver start. Workspace, all three end-to-end phases and 33-unit conformance
rounds passed. Its phase trace was not retained, so that particular failure is
not attributed to this proven contention race. The aggregate remains failed.
