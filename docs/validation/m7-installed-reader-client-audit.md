# Installed complete-result client audit

The original `4472279` workflow fails its first, unchanged Python census. The
retained 200 samples all pass Current admission and keep the same live verifier
across each RPC. Its 100 Result replies are 4,312,245 bytes each, with p95
394.713 ms; 100 Status replies are 452 bytes, with p95 148.430 ms. Equal lengths
do not prove byte-identical frames. The [audit record](m7-installed-reader-client-audit.json)
binds the raw samples and inspected source hashes. No endpoint, proof, retry or
build was launched for this review.

The measured collector opens a new Unix socket, sends its request, receives the
complete bounded frame and runs Python `json.loads`. Its clock includes all
these stages but does not separate them. Process census and full evidence
validation follow that clock. It does not use the Rust jobs Client or its
result-decoding cache. The original fail-fast rule leaves the Rust census and
retained CLI/MCP workflow unattempted. An SDK improvement therefore cannot be
attributed to, or presented as a fix for, these measured Python samples.

The Rust collector separately keeps one shared `Client::new` across four
readers. That constructor stages retention: first eligible reply keeps validated
body bytes, a second identical fresh body can promote one decoded graph, and
later identical replies return complete independent Response clones. Each call
captures its candidate generation before connecting; cache contention or changed
bytes parses normally. Status replies do not replace the eligible Result entry.
The 128 MiB family bounds retained/in-flight cache entries, not returned graphs
or process RSS. Its timer includes full client read/decode; the collector's
additional complete Value-to-ProjectResult validation is outside that timer.
This path was not measured in the latest original workflow.

Installed MCP durable reads use the explicit immediate-promotion family across
at most eight endpoints. Every read still gets a complete fresh service reply,
validates job ID/version/variant and preserves joined cancellation. A warm
cache avoids parser work, but static downstream costs remain: conversion of the
Response to Value, cloning structuredContent while encoding the complete result
as text, another clone into the JSON-RPC envelope, and final full-line encoding.
`server::send` currently holds the shared output mutex during that final
encoding, so another reply or ping may wait behind large-result serialization.
This is a concrete lock/copy boundary, not a measured cause of the Python
failure or evidence that installed MCP fails a latency bound.

A narrower future MCP optimization could consume owned Values or use borrowed
serialization to eliminate redundant graph clones, and finish full-line
encoding before taking the output mutex for one complete newline-delimited
write. It must retain both exact structured and text payloads, no late cancelled
response, original output/error behavior, complete EOF and all public fields.
No such implementation is proposed as qualified here; actual large-result and
concurrent ping/status controls would be required before claiming a gain.

The earlier [offline components](m7-result-client-components.json) and
[staged-client replay](m7-staged-result-client.json) remain separate evidence;
their timings cannot be subtracted from live Python timings to assign a cause.
The [client guide](../result-client.md) describes current ownership/transport
contracts. Original four-reader/200-sample/200 ms p95 acceptance and 120/30/5
workflow bounds are unchanged. Full responsive-result product acceptance remains
open; this review does not authorize replacing either collector.
