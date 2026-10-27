# Cache-origin admission choices

Status: source audit and implementation choices for
[#372](https://github.com/Chelis-Lang/sail/issues/372). Neither architecture is
selected or qualified. The six obligations in
[the boundary design](acceptance-boundary-refinements.md) remain open.

The query component now retains actual input delivery, complete stdout,
original command checkpoints and same-owner response positions. Those local
observations do not survive the public cache publication boundary as authority.
The missing fact is concrete: before serving a record, which actual observed
solver execution established its exact key and payload?

## Current paths

| Path | Actual source | Evidence supplied |
|---|---|---|
| Shim publication | `cached.rs::Entry::into_put` and `batch_observed` | The checked local observations become ordinary key, metadata and response bytes on the wire |
| Protocol PUT | `server.rs::State::put` | HELLO binds solver/mode; reply-shape checks do not establish a prior solver execution |
| Pending memory | `writer.rs::Writer::submit`, `Memory::insert_pending` | A pending entry is already serveable; persistence is not its first acceptance point |
| Disk and recovery | `sail-cache::DiskCache::{put_scoped,get_stamped}`, `persist::write_record` | Key/hash/stamp checks and exact writes preserve candidate data; they do not reconstruct a truthful publication ledger |
| Shared and bundled imports | `server.rs::{shared_get,keep}`, `sail-cache::{shared,pack}` | Signatures authenticate a publisher's claims; the truth of those claims needs a separate explicit contract |
| Runtime disk writer | `sail-runtime/src/store.rs` | The distinct `runtime-smt-v1` namespace has its own authority contract and cannot silently become a default shim origin |

These are source findings, not a new executable false-verdict reproduction.
The [query model](../../proofs/acceptance/query_association.rs) requires origins
to correspond to observed executions. Its `Publish` transition extends
`recorded`; recovery requires that prior membership. Arbitrary initial disk
and shared/bundled claims do not supply that membership.

The architecture-independent [candidate-write refinement plan](cache-write-candidate-refinement.md)
traces the actual job, disk arguments and same-original-Arc settlement. Its
stock-library probe rejects the actual Arc identity operation; its replacement
generation transitions are source-only and unqualified. Candidate fidelity
cannot establish solver origin or supply the missing admission capability.

## Two concrete architectures

**A. The daemon owns the observing solver path.** The daemon launches the
actual bound solver and owns its input, stdout, stderr and wait. A checked
transaction derives keys and entries from those operations and hands a private
admitted-batch capability directly to the writer. Public clients request work;
they cannot supply the output observation that authorizes a hit. A private
descriptor or a same-process worker can preserve this ownership; peer UID,
PID/image names and a capability returned by public HELLO cannot replace it.

This can keep one solver execution by moving the current foreground solver
path behind a streaming service interface. It is a larger change to launch,
streaming and failure recovery. The shim must preserve exact forwarding and
direct pinned-solver fallback, including already forwarded response accounting.
The actual immutable-image and endpoint-ownership contracts must be implemented
and qualified. One execution is an architectural count, not a latency result.

**B. The daemon independently validates candidate sessions.** Foreground shim
forwarding stays in place. A bounded candidate batch carries the exact raw
session once, command endpoints, response bindings and explicit terminal-silence
information. The daemon runs its own bound pinned solver and derives admission
from that run's actual complete write, pipe EOFs, quietness and normal wait.
Neither caller completion flags nor a plausible response can mint authority.

This is the smaller admission change, but adds another solver execution per
newly validated session, plus retained input and IPC. Wall time, CPU, memory
and availability cost are unmeasured. The current key/metadata/reply protocol
and stored entries cannot reconstruct the raw session, so an opaque digest
alone is insufficient for validation or legacy migration.

Re-execution can disagree in statistics or resource-limited outcomes. Such a
mismatch must refuse the proposed bytes. Alternatively, the validator may
publish its own newly observed entries with their own provenance; that is a
different observed run and must be represented as one. No query or reply
normalization, hidden timeout increase or comparator exception is authorized.
The existing warm reuse and diagnostic assertions remain qualification gates.

## Common acceptance and persistence core

Both architectures need the same concrete admission discipline:

1. A private admitted-batch constructor consumes the actual observed run,
   exact input/key correspondence and complete response association. It also
   establishes terminal quiet completion before issuing a silence certificate;
   the synthetic silence sentinel is not solver stdout.
2. Pending-memory insertion, disk publication and export require that capability.
   Closing only disk writes leaves the earlier pending-memory acceptance gap.
3. Persistent origin authentication binds the exact payload, key, solver, mode,
   metadata and validation revision. Its trust anchor must be protected outside
   replaceable entry data. A checksum or a public key stored beside an arbitrary
   entry cannot reconstruct the model's prior publication fact.
4. A narrow cryptographic assumption relates a valid authenticator to a prior
   signing operation. The source refinement must separately establish that every
   such operation consumes an admitted batch. Cryptography supplies no solver
   truth by itself. Native key protection and actual executable/endpoint identity
   remain explicit implementation obligations.
5. Legacy and imported tokenless records are candidates until validated under
   the chosen architecture. Existing unauthoritative files must not permanently
   block an admitted replacement through an ordinary `contains()` shortcut.

The charter's explicit trusted-publisher contracts can support a conditional
publisher-authenticity theorem for shared/bundled imports. They do not supply
the stronger observed-publication fact in the current model automatically.
Either the importer establishes that fact or the model/design must state and
justify the distinct publisher contract explicitly; it cannot be silently
treated as the same theorem.

## Decision and qualification

The owner choice concerns implementation and cost, not an exception to
correctness. A avoids a second solve through a larger owned-worker redesign;
B preserves the foreground path through additional independent work. Neither
choice closes compiler input, launch, original-request or service-composition
obligations by itself.

Before selection, require a real nonempty pinned-solver publication and hit,
ordinary controls for every admission boundary, restart/corruption/foreign-key
and imported-record cases, preserved full-scope conformance and quiet native
cold/warm/edit measurements. Retain every sample and every failed/interrupted
attempt. A source-only prototype, signature test or scripted read witness is
not real-project acceptance or the complete cache-origin theorem.
