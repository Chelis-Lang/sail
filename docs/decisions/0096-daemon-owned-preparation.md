# 0096: Daemon-owned optional preparation

- **Status:** owner-approved implementation plan, 2026-11-01; implementation and qualification in progress
- **Date:** 2026-11-01
- **Amends:** ADR 0082's call ownership and ADR 0091's exclusion of background recording

## Context

The first unchanged Cargo call currently performs the full managed path and
records the next replay. A synchronous extra pass after verification merely
moves that cost to the preceding verification. Sail already owns a daemon,
bounded workers, workspace locks and cancellable process groups.

The owner selected automatic, opt-out background preparation and the expanded
implementation package in `docs/design/release-expansion.md`. This is not
approval to relax input, settlement, transcript or proof-admission checks.

## Decision

The job service owns one bounded preparation worker, at most 64 queued requests
and 32 MiB of request data. Newer requests replace queued work for the same
workspace. Ordinary jobs and connection-owned foreground CLI leases preempt
preparation. Cancellation and shutdown kill and reap the owned worker group;
unfinished work does not resume after a daemon restart.

Successful Current ordinary Cargo verification may enqueue its exact invocation,
output decision, environment, context and complete closing input revision.
The closing revision includes generated inputs first discovered during that
successful run; published proof revisions and establishing provenance stay
unchanged. Preparation protocol v2 also carries the canonical origin workspace
and its public read-back revision. Before entering managed verification, the
worker requires a fresh complete Current read-back of that origin; an edit
already made while queued declines without consuming build-script observations.
The worker still independently reobserves and exactly matches the closing
revision after its run. The public read-back revision can legitimately advance
to the complete closing revision during that run; it is a pre-execution guard,
not a replacement for any original closing-origin equality check. After settlement can
be established by a **new** observation, a private worker performs a genuine
unchanged managed call. Only the existing replay recording predicates can
publish its actual transcript, report and events. A changed origin, started
verifier, incomplete observation or transient output declines publication.
No transcript is synthesized and no inherited unsettled read ages into trust.
The existing exact-scope replay predicate may also admit a focused invocation.
Its original partial target markers and argument-bound key remain unchanged; it
never establishes a whole-target proof. Interactive-engine focus is ineligible.

The caller returns independently of preparation. An immediate follow-up takes
foreground priority and the ordinary verification path if preparation is not
ready. `SAIL_BACKGROUND_PREPARE=0` disables new preparation. Unavailable or
incompatible services preserve ordinary behavior. The separate preparation
protocol explicitly negotiates version two through its status response.

Project creation can start the daemon with one initial ordinary verification
request, without waiting for daemon startup or verification. This can schedule
one later unchanged capture; preparation never creates an unbounded chain.

Opt-in project byte budgeting uses idle maintenance and the existing atomic
whole-workspace prune operation. It skips locked or newly used workspaces,
protects the newest workspace, and reports a soft-cap overage rather than
deleting active data. No eviction occurs without an explicit policy.

## Consequences

Foreground result authority and required receipts/readback remain synchronous.
Only optional acceleration is prepared later. The worker's counters establish
no proof. Native qualification must account for preparation CPU, memory and
interference as well as caller latency, including immediate and delayed calls.
This decision selects no new digest-memo default or focused-vstd reuse boundary.

## Amendment (2026-11-10): an origin preparation of the caller's workspace may finish

- **Status:** proposed; needs owner approval before it merges (#595, PR #708)

A preparation is an ordinary managed run. It consumes its compiler target's
build-script export when it begins (ADR 0046/0047) and republishes it only
when it finishes. Killing it partway leaves the export missing, so the next run
of that target, which is the preempting call itself, cleans every build-script
package (vstd included), rebuilds the dependency graph and cannot retain what
it compiles. Loaded native Vest loops traced every instant-revert miss to
this (#595).

Preemption therefore distinguishes one case. When an ordinary job or a
foreground CLI lease arrives for the same workspace key as a running
preparation that has an origin (it re-runs a verification that just
succeeded), the preparation is preempted rather than killed. It may finish
until its own deadline, 60 seconds from its start, or a fixed grace of 60
seconds from preemption, whichever is earlier, and is then killed and reaped
as before. The caller is admitted at once and waits for the preparation only on
the target lock. Its result is still independent: it verifies normally and
uses no prepared result. A preempted preparation counts as superseded unless it
finishes ready.

Every other case keeps the original rule: queued work is dropped; a running
preparation for another workspace key, and a new project's initial preparation
(no origin, 600 second deadline), are killed at once; cancellation, a newer
request for the same workspace and shutdown kill and reap the worker group.

The wait is bounded by what killing would cost the same caller: an origin
preparation re-runs a verification that just succeeded on the same target,
while killing it makes the caller rebuild every script package and its
dependents. A target with no build-script packages loses nothing when its
preparation is killed, so for such a target the wait is pure cost; this is a
known limitation. A preparation that ends without a Current result can still
leave the export unpublished; that residual is unchanged by this amendment
and is tracked in `docs/known-issues.md`. Native measurements of revert
misses and edit, fix and first-call latency against main and stock, quiet and
loaded, are in `docs/validation/instant-revert.md`.
