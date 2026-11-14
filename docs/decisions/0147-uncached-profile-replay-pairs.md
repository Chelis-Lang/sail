# 0147: Compare separate plain and traced diagnostic prefix executions

- **Status:** implemented; bounded native Linux component qualification passes, unselected
- **Date:** 2026-11-14
- **Tracking:** #712 B; extends ADR0138/0144/0146

## Decision

Add an unselected `sail-portfolio` executor for the exact original prefix
through a selected process check. Run the caller-authenticated pinned Z3 in
two fresh uncached processes, first plain and then with diagnostic tracing.
Send identical original bytes to both; retain the prefix, original streams,
completion records and trace in a new directory under the caller's private
parent. Existing destinations refuse. No shim, cache, daemon or proof receipt
is involved. No dependency edge changes.

Require complete paired responses and exact answers, follow-ups, non-statistics
stdout bytes, stderr and successful exits. Resource-limit counters and all
non-observational statistics entries remain exact. Separately preserve original
allocation, memory, maximum-memory and clock observations as detailed below.
Require a complete supported trace; report its internal begin-check events
independently from the original SMT check count. Counts aggregate the entire
prefix, including earlier functions/checks, never exclusive function cost.
Malformed or differing non-observational evidence is unavailable measurement.

Input remains bounded by ADR0138; analysis admits at most 100,000 commands. Each output read is bounded to 4 MiB and
trace reads to the counter's 64 MiB. Poll produced file sizes every 5 ms and
stop/reap a process at a limit or error. This is bounded capture reading and
best-effort producer termination, not a hard filesystem quota; files can
overshoot between polls. Each process has a positive wall limit at most 300 s.
Only the trusted pinned solver, which does not spawn descendants, is supported.
No process-tree containment is claimed.

## Limits

The caller must separately authenticate solver bytes, retained input, original
invocation, mode, target/function/check binding and historical status. A caller
supplying fabricated input cannot obtain proof or currentness authority here.
Original options that redirect or disable tracing are unsupported; absent or
inconsistent traces refuse. A successful pair says only that these separate
executions agree; even identical `unknown` answers provide no proof evidence.
Only the explicitly named diagnostic observations below may differ. No default
diagnostic normalization or conformance exception is introduced.

This is a replay component, not the full profile CLI/MCP, automatic job lookup,
idle stability worker, real-project acceptance or release availability.

## Original strict comparison and successor

Fresh native review of355539cf passes the conservative execution/refusal
contract but does not qualify real-profile availability: all20prefixes across
seven original Verus streams refuse exact stdout. Independent pairing of132
statistics replies shows only num-allocs (132), memory (10) and max-memory
(10) differ. An additional actual recorded toy prefix has23statistics replies;
num-allocs differs in all23 and time appears only in traced output as0.00 once.
Resource-limit counters, all other statistic values, every non-statistics
reply, stdout bytes outside statistics, stderr and exits remain exact.
Original strict refusals and failed harness/collector attempts are retained.

For diagnostic replay comparison only, parse complete statistics replies into
ordered exact key/value entries. Preserve rlimit-count and every other
non-observational entry exactly, including relative order and inventory.
Separately record original num-allocs, memory, max-memory and time values;
only time may be absent on one side. Values must be bounded finite nonnegative
numbers; duplicate/malformed keys and incomplete replies refuse. Changes are
bounded to4096records. Capture files are never rewritten, and all stdout
bytes outside statistics plus stderr/answers/follow-ups remain byte-exact.
This changes no default-mode query, cache, verdict or conformance comparison.
The four values describe instrumentation observations, not accepted proof cost.

Also distinguish actual original SMT checks from internal trace begin-check
events. Native pinned Z3 returns sat for a trivial valid check with zero such
events. Report both counts independently; require complete matched responses
for the actual original checks and a complete supported trace, without
inventing a one-to-one event theorem. Counts still describe the whole prefix,
never the exclusive selected function. Cap prefix analysis at100000commands
before starting either child. Successor bounded native qualification and fresh review pass at `b81ff00f`;
its exact scope, original observations and failed attempts are recorded in
the validation record. Original 355 passes are not relabelled.
