# Runtime protocol 1: exact SMT execution

This specification defines the first implemented verification-runtime slice.
It leaves the existing Verus command, cache protocol and project-job protocol
unchanged. S.A.I.L. owns execution; clients supply requests and consume evidence.

## Interfaces

`sail-api::runtime` defines `Request`, `Response`, `Query`, `Limits`,
`Outcome`, `Answer`, `Check`, `Product`, `SolverIdentity`, `Capabilities`
and `Error`. Every wire request and response carries `version: 1`.
`Request` contains `input: Input::Smt { queries: Vec<Query> }`; `Response`
contains `output: Output::Smt { results: Vec<QueryResult> }`. Both payloads
use an explicit `kind: "smt"` discriminator and preserve query order. A query
contains a nonempty `obligation_id`, strictly sorted unique `requirement_ids`,
`backend` (currently `z3`), the exact UTF-8 `script`, and `limits`.

`sail-runtime` exposes `Runtime::new(Config) -> Result<Runtime, Error>`,
`Runtime::capabilities()`, and
`Runtime::execute(&Request, &Cancellation) -> Result<Response, Error>`.
`Config` contains the installed bundled Z3 path, an optional disk cache directory,
and a positive worker bound. It performs no downloads or installation. The
executable must match this build's host-specific SHA-256 and version in the
existing `corpus/toolchain.toml` pin. Identity is checked before each execution
or cache lookup. A missing or changed executable is explicitly unavailable.
`Cancellation` is a clonable thread-safe cancellation token.

Malformed version, identity metadata, command grammar, or limits fail admission
for the entire request before any backend process or cache access. At most 64
queries and 16 MiB total script bytes are admitted. Per-query defaults are
2,000,000 Z3 resource units, a 30,000 ms safety deadline, 16 MiB input bytes,
and 16 MiB retained bytes for each output stream. Limits are positive and
bounded by service maxima. Query IDs and requirement IDs are copied exactly.
Each identity is at most 1024 bytes, with at most 1024 requirement IDs per query.
The sum of three times each query's output bound must fit a 128 MiB batch
budget, reserving stdout, stderr and any duplicated product expression bytes
before dispatch. Large batches must explicitly choose smaller output bounds.
No input text is normalized, rewritten or augmented. Z3 resource limits and
seeds are passed as explicit process arguments and included in the cache key.

## Commands and transcript semantics

A query is one complete, possibly incremental SMT-LIB transcript containing at
least one check. The admitted grammar is deliberately closed: declarations,
definitions, assertions, set-logic, selected safe set-options, push, pop,
check-sat, check-sat-assuming, get-model, get-value, get-unsat-core,
get-info :reason-unknown, and a final optional exit. Full lexical and structural
parsing precedes execution. Echo, tactic checks, reset, set-info, unknown
commands, output-redirection options and print-success true are rejected.
Print-success false, model/core production flags, zero random seeds, and an
in-script resource limit exactly equal to the effective request limit are
admitted. Conflicting limits, timeout options and nonzero seeds are rejected.
Push/pop depth is bounded and underflow is rejected. No command follows exit.

The service derives an ordered response plan from admitted commands. Every
check must produce exactly one bare `sat`, `unsat` or `unknown`; each requested
product must produce its corresponding syntactically valid response. Missing,
extra, malformed or error responses make the entire transcript an
infrastructure failure, never a decided check. Products attach to the most
recent check and are rejected if no check precedes them. State-changing
commands invalidate the current check for subsequent products. A model or
value requires SAT; a core requires UNSAT; reason-unknown requires UNKNOWN.
Checks and product command indices refer to the original command order.

`Answer::{Sat, Unsat, Unknown}` describes raw satisfiability, not a proof of a
caller-selected proposition. `Outcome::{Completed, Unavailable, Timeout,
Cancelled, OutputLimit, Infrastructure}` is separate. Only a completed
transcript has checks. Exact bounded stdout and stderr bytes, solver identity,
script SHA-256, effective limits and cache provenance remain in every result.
Unknown has no successful proof interpretation. Raw products are retained;
there is no invented semantic decoding of unsupported model values.

## Execution and caching

Fresh solver processes run the exact admitted script with deterministic zero
SMT/SAT seeds, explicit resource units and `-smt2 -in`. Input is written while
both output streams are drained. Deadline, cancellation and output overflow
kill and reap the actual process group, including descendants, and join all
started I/O workers. Output is retained only up to the configured per-stream
bound; truncated output never supplies checks. Nonzero exit, stderr diagnostics,
invalid response encoding and incomplete writes are infrastructure failures.
The deadline covers execution, including I/O cleanup. Missing backends do not
fall back to a different backend.

Batch work uses at most the configured worker count. All admitted queries
receive an ordered result; cancellation stops active processes and marks
undispatched work cancelled. Every started worker is joined before return.

The existing `sail-cache::DiskCache` stores runtime records under the separate
`runtime-smt-v1` namespace. Keys bind exact script bytes, actual solver family,
version, executable SHA-256, platform, seed policy and every effective limit.
Caller obligation and requirement IDs are attribution outside the solver
record. Cache hits restore only the exact content-bound solver record and
attach the current caller's IDs, recording `cache_hit: true` without claiming
a new execution. All stored evidence is integrity-checked and the full response
plan is revalidated on every hit. Corrupt entries are misses. Only complete
transcripts with no unknown checks are stored. Cancellation, deadlines,
overflow, unavailable and infrastructure failures never become cache successes.

## Scope and compatibility

Capabilities advertise only Z3 complete transcripts and bounded batches.
Incrementality is push/pop within one complete transcript, not retained remote
session handles. cvc5, certificates, Rust contract lowering, temporal execution,
the requirement ledger and higher assurance tiers are not advertised by this
slice. Temporal execution will use D.I.V.E. through S.A.I.L.; no TLC backend is
introduced. The original Verus path and all existing public schemas retain
their meanings.

### Execution environment

The solver receives a cleared environment with `LC_ALL=C`. Ambient loader,
locale and solver settings cannot change the authenticated executable's behavior.
This fixed environment policy is part of the cache key; entries created under
an earlier environment policy cannot be reused.
