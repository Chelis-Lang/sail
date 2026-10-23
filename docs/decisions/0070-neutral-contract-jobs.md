# 0070: Owned neutral source verification jobs

- **Status:** accepted
- **Date:** 2026-10-21

## Context

The qualified neutral helper admits source, produces a deterministic owned Verus
shadow and returns conservative actual proof evidence. Oyster imports only the
neutral Sail API and uses the Sail daemon. Calling the backend library or
constructing a shadow, tool path or verifier process in Oyster would cross that
boundary. Existing project jobs provide durable lifecycle, process supervision,
logs and baselines, but their ordinary submission accepts caller Cargo paths,
arguments and environment and their retained result lacks contract metadata.

## Decision

Add an explicitly negotiated source-only submission to the existing owned job
service. Move pure manifest, proof and trust types into `sail-api`, reexport them
from the backend helper, and retain unchanged project reports alongside optional
typed contract evidence. Sail alone chooses the shadow, tools and controls.
Reuse the existing worker process, adding explicit neutral deadline and stream
bounds. The CLI acceptance example depends on the runtime contract library only to compile a separate body-edit sample witness; the API request never carries an executable or verifier path. Refuse older endpoints without fallback. Preserve contract source and
metadata with sealed worker stdout and stderr evidence in independently retained baselines.

The job's live-source currentness is unknown: the client must compare its actual
source with the returned original hash. A proof under requires is neither
requirement entailment nor a tier or mutation-gate decision. Generic legacy
comparison/explanation remains unavailable for this family until its semantics
are contract-aware. Existing ordinary Verus jobs retain their behavior.

## Consequences

Oyster can request actual contract verification through its existing Sail
service dependency. Actual service acceptance establishes this route separately
from helper qualification. The [version-one specification](../spec/neutral-contract-jobs.md)
defines request bounds, retained evidence and acceptance. Requirement entailment,
tier persistence and mutation establishment remain later independent slices.
