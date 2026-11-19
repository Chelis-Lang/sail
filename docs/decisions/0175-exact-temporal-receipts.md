# 0175: Exact completed temporal check receipts

- **Status:** accepted for implementation, #774
- **Date:** 2026-11-19
- **Amends:** the original no-cache slice of [ADR 0071](0071-shared-temporal-runtime.md)

## Context

Every native job launches a worker and repeats the full exploration, including
checks of a spec the caller has just restored. The runtime already authenticates
the installed worker, freezes the complete requested module set and qualifies
both exhaustive success and independently validated counterexamples.

## Decision

Allow a backend to opt in with a producer-specific, versioned checking policy
and an independent request/response validator. Store complete native evidence
in a separate content-addressed namespace under the trusted local runtime cache.
Only exact check requests with Holds or Refuted are eligible. Bind the full
request, wall-time limit, authenticated worker, backend, platform, Sail version
and policy. Validate at publication and recovery. The runtime uses the
disk cache's versioned envelope and integrity checks. Its Unix writer opens
private directories and creates a mode-0600 temporary file before atomically
renaming it into place. The existing workspace `rustix` library supplies
descriptor-relative publication and bounded, nonblocking, no-follow reads.

An embedded call reuses matching evidence by default. A socket client must
negotiate replay support and recompute the matched key. Attributed results
identify the current caller and retain the producer's original evidence.

## Consequences

Undoing an edit restores the earlier check. Damage, failed admission or
installation drift cannot promote a receipt to a logical verdict. Local disk
integrity follows the [trusted-writer model](../trust.md). A shared archive
requires a trusted writer and independently verified provenance.

Under changed inputs, a trace can remain valid even if fresh exploration reaches
a resource stop before the violating state. Exact replay therefore requires
the complete original inputs. Checking edited properties over a retained graph
requires a qualified graph interface from D.I.V.E.
