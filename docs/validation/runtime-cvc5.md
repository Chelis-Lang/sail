# Raw SMT solver backends

Status: Linux x86-64 development candidate qualified at source `c3ecf532`.
The full local gate, solver tests, complete local end-to-end run, PR conformance,
fast/standard corpus and scoped independent installer review pass. Mac
qualification, real-project cvc5 acceptance and advertisement remain pending.
ADR0134 records policy and authoritative option sources.

The default config remains Z3-only. An explicit cvc5 config uses its own pinned
identity, option/reply dialect and cache policy. The Linux contributor fixture
installer copies only the binary and its original notices after both SHA-256
checks. No Mac binary, released availability or speed gain is claimed.

Required evidence includes exact legacy Z3 argv/key bytes and capability output,
per-solver native direct comparisons, models/cores/multiple checks, unknown and
resource negatives, cache isolation/corruption, process failures/cancellation,
full gates, independent review and Mac before advertisement. Consumer skill
and released user workflows stay unchanged while this config is unadvertised.

Documentation changed: ADR0134, this validation record, errors and changelog.

Initial native source8f73ab5e stops before compilation: Docker's explicit
build-context allowlist omits the new fetch helper. The original image-build
failure is retained. The corrected source includes the helper and binds the
allowlist itself into image tags; native gates must run again.

Local Python validation of the original official archive agrees with both pins
and all eight curated executable/notice files byte-for-byte. This performs no
solver execution and is not native backend acceptance. Four synthetic helper
controls,27resource copies, shell syntax and book source/build/links also pass.

Independent review reproduces two fixture-installer defects at1b215e63:
a mode-setting error after publication leaves a destination that prevents retry,
and distinct ZIP spellings can collide after path parsing. The successor stages
on the destination filesystem, sets modes before its final rename, rejects
noncanonical/colliding output paths and retains all digest/size/type checks.
Faulted mode publication and alias controls are added; native qualification and
fresh successor review were pending at that point. Raw runtime code is unchanged by this
installer correction. API comments now describe backend-specific resource units
and capability-based availability.

Native x86-64 EC2 validation at `1b215e63` uses a c7i.4xlarge in us-east-2
and image `fd944133a95f5d6b-core`: workspace gate 1,402 passed / 57 skipped,
solver 53 passed, complete local end-to-end including Extra, and all four PR
conformance arms pass (33 same, 311 compared functions each; zero differences,
unstable units or infrastructure errors). Seven resource-drift units remain
reported. Independent controls compare actual main/candidate Z3 capabilities,
argv, cache bytes and disk reuse; both solvers' direct/cold/warm transcripts,
models, cores, incremental checks, resource exhaustion, backend isolation,
corruption, process failures and cancellation pass. This is bounded SMT
runtime evidence, not real-project cvc5 acceptance, Mac readiness or performance
evidence. The original failed packaging attempt remains recorded above.

The independent native archive has SHA-256
`b52f62d5abc4de9acae88b6018132819e4d0fe408dbf298e69ef958a2d704fa8`;
its report has SHA-256
`91259cc896147919e60ab11783729aa4d39ea25eb2e854d99217b88645001b76`.
All 80 indexed native members were checked independently by the reviewer.
The review rejects the predecessor installer; no successor approval is inferred.


## Corrected candidate qualification

The corrected source `c3ecf53265b4f414c152236a5c5f8e158200cd50` passes on
native c7i.4xlarge EC2 hosts in us-east-2, image `6c19f5c8d397b61c-core`:
workspace gate 1,402 passed / 57 skipped, solver tests 53 passed, the complete
local end-to-end suite including Extra, and all four PR conformance arms
(33 same / 311 functions each, zero differences, unstable units or infrastructure
errors; seven resource-drift units retained). The complete gate archive is
`6bc397f04c9d9cd704dfbbd5c844e44d57f1aa6465d54ef2b5a2e8863dc43544`;
all 877 indexed payloads were independently rehashed and retained externally.

A separate native run passes all four fast arms (147 same / 3,451 functions
each; 25 resource-drift units) and all four standard arms (159 same / 13,166
functions each; 32 resource-drift units), including Anvil. Every arm has zero
differences, unstable units, infrastructure errors and skipped projects.
The catalogue and 18-mutant HumanEval sweep, including cold, revert and forced
revert comparisons, pass. These runs compare the unchanged default Verus/Z3
path against stock; they do not establish that Verus supports cvc5 or that cvc5
has passed a real-project benchmark. The original archive is
`4a0d0f9f2131c31c43e81669b8d97c2891975d3aba607b1c7a19d0dc6ed03b13`
(31,817 indexed payloads). No performance conclusion is drawn from these
regression runs.

The fresh successor review approves the exact corrected source. Its 42
independent installer groups and six repository installer tests pass both in
the local Python harness and the native production image. All eleven runtime
files are byte-identical to the independently exercised predecessor, whose
per-solver controls are explicitly reused; no predecessor installer approval
is inferred. The native successor archive is
`cbf33c7b2e867b64c3c61274a7a5ad47315224a4e2f3ca7f64f50720e2faf0ac`;
the review report is
`878ab5286d7e428c458b9e424b8cc12f3fede4c0576fd2d8599aa10a84940783`.
All 16 native and 44 complete-review payloads were independently checked and
retained externally.

This change affects runtime solver selection, option admission, replies and
cache policy, so ADR0087's standard tier applies and was run above. It changes
no proved model, refinement body or temporal interface. Runtime Rust uses
portable existing process/filesystem facilities; the new pinned installer is a
Linux CI fixture and refuses other hosts. Mac execution remains required before
advertising cvc5 availability, and no Mac result is claimed here.

Prior attempts: `8f73ab5e`, 2026-11-13, image-context failure (new helper absent);
`1b215e63`, 2026-11-13, native gates pass but independent installer review rejects
post-publication chmod and ZIP path aliases. Both original records and raw
failures remain retained. The successor fixes both without weakening checks.
