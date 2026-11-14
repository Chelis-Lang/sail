# Uncached diagnostic replay pairs

Status: bounded native Linux successor qualification and fresh independent
review pass at `b81ff00fceb5a1878ac380750d8d103bb5b0f1cb`. This unselected
library authenticates no input, solver, invocation, currentness or proof. Full
profiler/worker/CLI/MCP, Mac and release acceptance remain open.

The candidate composes the exact prefix selector with separate plain/traced
solver executions and the existing bounded trace counter. It retains original
captures, records narrowly named observational statistics separately, and
refuses different proof responses/counters, incomplete, erroneous or unsuccessful
runs.
Counts describe the entire prefix. Caller-authenticated source/session/solver
bindings and private output-directory ownership are explicit preconditions.

Source tests cover response completeness, exact differences and rejection
before output creation. An ignored native solver control covers two sequential
checks, MBQI, datatypes, existing-directory refusal and a real solver error.
The successor runs these controls natively; its scope is recorded below. Output polling can overshoot file limits; no hard
quota or descendant cleanup is claimed.

Documentation changes: ADR0147, this record, the crate README and changelog.
Consumer commands and released availability are unchanged, so no new consumer
skill command is added. Full worker/profile/CLI/MCP integration remains planned.

## Original355qualification and unavailable real prefixes

Native c7i.4xlarge/us-east-2 full gate passes1433workspace tests. Independent
review passes26process/protocol/bounds cases,25actual-Z3 cases, focused tests
and retained rerun of the ignored native solver control. Original ephemeral
ignored-test captures are deleted; the later mounted rerun retains all pair
files. Timeout/error/size refusals reap their direct child. No descendant or
hard-quota guarantee is claimed.

All20original Verus prefixes and one recorded toy prefix strictly refuse.
Independent byte-frame pairing finds only num-allocs/memory/max-memory/time
observations differ, with exact rlimit/allotherstats/nonstats/stderr/exits.
ADR0147 records the narrow successor scope and separates internal trace events
from actual SMT checks. No blanket stdout or conformance waiver is introduced.

The lead independently verifies all1344archive entries (1162files,178dirs,
four symlink metadata entries) and externally retains original owner/modes,
all raw controls, strict failures, malformed first harness, invalid trivial
positive expectation and failed unprivileged collector. Archive SHA256:
`d6213cded806b8cc2a4b96e48b7e1121199dfbc18cbb2c63992700432525dfe2`;
index `05dd4b7d4a25b3668c649bc32414e92cd6a1dd3472650ff69d5c8526405b1e01`.
The successful collector reads with privilege without modifying original
ownership/modes. Full profiler/worker/product/Mac/performance acceptance remains
open. The successor has not inherited a passed gate.

| Source / attempt | Actual result |
|---|---|
| cf676b36 / observations1 | Affected Clippy refuses constant chunks_exact and an empty-vector assertion style. Successor uses fixed array chunks and a typed empty-vector equality; all inputs and assertions remain. Tests are not reached. |

## B81 successor qualification

On native c7i.4xlarge/us-east-2, fresh review independently runs the complete
workspace gate (1,442 tests), nine profile/prefix tests, the retained ignored
pinned-Z3 control, 65 synthetic/process cases, 29 actual-Z3 cases and six
additional boundary probes. Final external-driver dependency versions and
checksums match the candidate lock; earlier unlocked/offline attempts and the
invalid follow-on dependency audit stay excluded and retained.

All 20 original Verus prefixes plus the original recorded toy prefix succeed.
An independent byte-frame audit checks 155 statistics replies: num-allocs
changes in 155, memory and max-memory each in ten, and time appears on one
side once. Ordered non-observational entries, all rlimit values, every
non-statistics stdout byte, stderr and successful exits stay exact. Both
original observation values remain visible; captures are not rewritten.
Valid trivial/BV checks can have zero internal begin-check events, recorded
separately from actual matched SMT checks. Counts cover the entire prefix.

The caller precondition on original trace-control options remains material.
A deliberately unsupported late trace-false input is accepted, but a
quantified control retains the same complete trace (two internal checks,
eight quantifier and 16 theory instances). This establishes no truncation
defect under the pin; it does not qualify arbitrary original tracing options
or remove the caller's compatibility responsibility. No hostile executable,
concurrent mutation, descendant containment or hard filesystem quota is
claimed. The component is unselected diagnostic tooling under ADR0087: no
affected corpus entries, full corpus/model/refinement/product/performance or
Mac pass is inferred. Required hosted checks remain separate.

The lead independently verifies all 3,687 archive entries: 3,273 original
files, 410 directories and four symlinks as metadata, including original
bytes, modes/owners, nanosecond mtimes and link targets. Archive SHA256:
`4aead53e585f4db57dbd5d155f91cc754726dfa0d801c2e287597af61c5cf44a`;
index `db4212f7bc6789061d305f03f9463a5b242a212aaa1a7aad6c7236375db4f32d`.
Original 355 strict failures remain retained separately. Documentation and
consumer command availability remain as stated above; no full profiler is
claimed.
