# 0071: Native temporal jobs through the shared runtime

- **Status:** implemented; local gate passed, native qualification pending
- **Date:** 2026-10-23

## Decision

Extend the existing version-one runtime's tagged input and output families with
one native temporal job. Preserve the SMT family unchanged. Capabilities gain
an optional, defaulted temporal field, so old clients still read new service
capabilities and new clients explicitly refuse old services for temporal work.
The private transport, cancellation, disconnect monitoring and idle guard are
shared. No new server or alternative checker is introduced.

Public payload and evidence types live in `sail-api::temporal`; the API does not
link D.I.V.E. The native adapter reexports these types and retains independent
property-role interpretation and trace validation. The native source revision
and worker protocol remain pinned. Source closures, caller identities and
whole-worker deadlines are content-bound. Execution outcomes remain separate
from native logical outcomes and typed completion evidence.

The runtime receives a trusted worker installation choice from Sail, never
from caller wire data. Release staging installs the worker beside Sail with a
versioned manifest covered by the existing release inventory. Its final bytes
are hashed after stripping. Source developers can use the same staging helper.
The parent authenticates the installed worker and runs a private frozen copy.
Missing, modified or incompatible installations cannot establish a verdict.

Native evidence remains exact, including operational paths and elapsed values;
these are evidence, not deterministic semantic model data. The shared slice
introduces no temporal cache, ledger tier or assurance claim. Simulation and
validated individual traces never establish exhaustive properties. Existing
Verus verification and raw SMT cache behavior remain unchanged.

## Qualification

Actual shared service controls cover projected Step plus Within requirements,
real action refutation and independent replay, finite simulation, stopped
exploration, rejected host modules, deadlines and installation drift. Existing
worker process controls remain in force. Mixed old/new capability controls and
SMT regression tests cover compatibility. Native Linux and macOS acceptance,
protected CI and independent review remain required before merge.

The [source-scoped validation record](../validation/shared-temporal-runtime.md)
separates the passing local gate from remaining platform and service qualification.
