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
output decision, environment, context and input revision. After settlement can
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
protocol explicitly negotiates version one through its status response.

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
