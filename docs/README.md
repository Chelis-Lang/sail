# Documentation

Start with the repository [README](../README.md) for installation and ordinary
verification. Guides describe behavior on the branch that contains them;
experimental features and validation limits are identified explicitly.

## Using Sail

| Task | Guide |
|---|---|
| Install an explicit version or local archive and check prerequisites | [Installation](install.md) |
| Submit neutral Rust source to Sail-owned verification jobs | [Neutral source jobs](neutral-contract-jobs.md), [service specification](spec/neutral-contract-jobs.md) |
| Run and retain bounded neutral body-mutation campaigns | [Neutral mutation](neutral-mutation.md), [qualification limits](validation/neutral-mutation.md) |
| Find independently evaluated scalar literal-contract counterexamples | [Scalar witnesses](literal-witnesses.md), [qualification](validation/scalar-literal-witnesses.md), [decision](decisions/0077-scalar-literal-contract-witnesses.md) |
| Inspect requirement IDs, proof tiers and qualifiers from retained neutral jobs | [Requirement assurance](requirement-assurance.md) |
| Check scalar contracts on ordinary Rust functions with explicit requirement IDs | [Neutral contracts](neutral-contracts.md), [source specification](spec/neutral-contracts.md) |
| Verify, reuse complete reports, watch edits, inspect and cancel jobs | [Workspace verification](workspace-verification.md) |
| Restore a previously verified revision after undoing an edit | [Instant revert](instant-revert.md), with qualified platform/profile and cost limits |
| Check one repair, follow proof events, explain failures and compare saved baselines | [Walkthrough](proof-repair-walkthrough.md), [repair contract](proof-repair.md) |
| Integrate an agent through stdio MCP | [MCP tools](mcp.md) |
| Execute raw SMT transcripts through the pinned runtime, shared daemon or MCP | [Runtime](runtime.md), [version-one protocol](spec/runtime.md) |
| Read job Status and Results from Rust | [Result client](result-client.md) |
| Run isolated native model checks, simulation and independent trace validation from Rust | [Temporal jobs](temporal-jobs.md) |
| Consume structured reports and provenance | [Output schema](output-schema.md) |
| Persist Sail's caches between CI runs, with measured gains and a GitHub Actions example | [CI cache](ci-cache.md) |
| Share signed solver replies over HTTP(S) | [HTTP shared cache](http-shared-cache.md) |
| Interpret Current, Historical and Unknown evidence | [Freshness](freshness.md) |
| Understand trust boundaries and opt-in verdict differences | [Trust](trust.md), [divergences](divergences.md) |
| See what Sail speeds up and what it does not, with reproducible runs | [Performance](performance.md) |
| Check supported verifier versions and workload benchmarks | [Compatibility](compatibility.md), [generated benchmarks](benchmarks.md) |
| Find limitations and unresolved failures | [Known issues](known-issues.md) |
| Look up an error code | [Error codes](errors.md) |

## Developing and validating Sail

[AGENTS.md](../AGENTS.md) defines the workflow, Rust conventions, documentation
requirements and gates. [Testing](testing.md) lists the parallel end-to-end
phases and their local entry points. The [charter](CHARTER.md) and its corrections define
the correctness contract. [ADRs](decisions/README.md) explain implementation
decisions; proposed candidates are distinguished from accepted decisions.

[Private dependencies](private-dependencies.md) describes the temporal
extension workspace, the only build that needs the private D.I.V.E. source,
and its ephemeral read-only authentication.
[Native worker validation](validation/dive-runtime-worker.md) records the
focused checking, replay and process-isolation evidence and its limits.
[Combined production integration](validation/integration-production.md) records the
current cumulative source, native gates, source and temporal controls, unchanged
corpus-component qualification and explicit mutation limits.
[macOS integration acceptance](validation/integration-macos.md) records the
native host gate and installed temporal/source controls, including the
unestablished positive mutation-kill boundary.

The [roadmap](ROADMAP.md) tracks implementation and acceptance separately.
The [combined release matrix](validation/release-matrix.md) states current
Linux/Mac measurements and remaining gates; [mutation qualification](validation/mutation-suite.md)
covers the broader edit loops and soundness checks.
The [erasure index record](validation/shadow-index.md) tracks the current
runtime improvement and its installed consumer checks.
The [M7 acceptance guide](bench/m7-repair-acceptance.md) specifies frozen real
projects, unchanged deadlines, source-byte checks and performance gates.
[Fault injection](fault-injection.md) covers solver, cache and daemon failures.
[The proposed evidence-acceptance core](design/evidence-acceptance-core.md)
defines a conditional safety theorem for query association, input identity,
requested scope and result authority. It is a design proposal, not an
implemented formal guarantee.
[Direct stock-Verus checking](design/acceptance-proof-checker.md) documents the
component lifecycles, composed admission, complete proof inventory and deliberately broken
controls. [Production refinements](design/acceptance-production-refinements.md)
connect source-shared operations and typed factories to native callers; their
[qualification record](validation/acceptance-production-refinements.md) states
the remaining integration and service-wide proof obligations.
[Acceptance boundary refinements](design/acceptance-boundary-refinements.md)
tracks the follow-up work on raw transcripts, evidence providers, independently
retained requests and concrete service composition.
The [cache-origin admission choices](design/cache-origin-admission.md) map the
remaining writer/recovery gap to owned execution or independent validation.
The [acceptance proof WIP handoff](acceptance-proof-handoff.md) identifies the
published work branches, existing evidence and unfinished integration work.
The [compiler-input observation proposal](design/compiler-input-observation.md)
separates the next loader/decoder implementation slice from unsupported opaque
I/O and the native observation/launch architecture decisions.
The [source lexer qualification plan](design/acceptance-source-lexer.md) covers
exact Git-blob differential checks and separate native preflight measurements.
The [owned compiler pipe candidate](design/acceptance-compiler-pipes.md) connects
actual endpoint reads and EOF to completion, with its remaining qualification
and physical-origin limits.
The [solver identity record](validation/solver-identity.md) tracks the stale
executable-hash reproduction, handshake repair and remaining qualification.
The [cold first-run record](validation/cold-first-run.md) compares first runs
with stock, explains where the gap went and states what still exceeds the
release bar.
The [miss-replies record](validation/vstd-miss-replies.md) measures the
release's shipped vstd solver replies on a shipped-proof miss and breaks down
what a miss costs.
The [shipped-miss hashing record](validation/shipped-miss-hashing.md) measures
refusing the shipped vstd proof on its sysroot shape before hashing.
[Output failure evidence](e2e-output-evidence.md) describes bounded original
E2E streams and producer records retained for diagnosis.
[Managed output ordering](validation/managed-output-order.md) tracks the
acknowledged-driver candidate and its remaining correctness and cost checks.
[Socket fixture retirement](validation/mcp-fixture-retirement.md) records the
MCP test-server cancellation regression and its remaining validation scope.
[Solver-continuation findings](reports/2026-10-16-solver-snapshots.md) and their
[qualification record](validation/solver-snapshots.md) preserve the blocked
exactness experiment; no continuation implementation is selected.
The [rlimit record](validation/rlimit-subsumption.md) tracks the validation of
serving the shipped vstd proof under a raised `--rlimit` (ADR 0060).
The [function-spelling record](validation/smt-function-spellings.md) tracks the
unselected native Vest edit candidate and its remaining qualification.
The [source-suite integrity record](validation/source-suite-integrity.md) tracks
executable digests, legacy-manifest migration and native rebuild controls.

[Deep Verus checks](deep-verus-tests.md) explains the fork and stock source
variants, exact shim comparisons and the independent stock test-program oracle.
[Releasing](releasing.md) gives the release procedure, the archive names and
layout, and how `sail` finds the bundled engine.
[Managed runs](managed-cold-start.md) explains what the first managed run costs
and when receipts are reused.
The [reader acceptance guide](reader-acceptance.md) gives the native reader
latency gate and how to run it.
[Sail's Verus](../engine/verus/README.md) describes the fork runtime every
verification runs ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)),
its interactive engine, the prebuilt artifact and the runtime identity. Its checks are in
the [M7 validation record](validation/m7.md) (Gate 1). Long corpus, source-build and benchmark work
runs locally, and required CI stays short. Reference performance numbers come
from a native x86-64 Linux host (`scripts/perf-host.sh`).
The [CI image startup record](validation/ci-image-startup.md) describes the
shared core/MCP packaging targets and their measured acceptance status.

[M7 validation](validation/m7.md) gives each M7 acceptance gate's status, best
evidence and indexed failed or interrupted attempts, and the
[deep-tier record](validation/deep-tier.md) gives the release-time deep
conformance, `rust_verify_test` and Singular runs, the
[HTTP shared-cache record](validation/http-shared-cache.md) gives that
feature's tests and integration evidence, and the
[macOS engine record](validation/macos-engine.md) qualifies the engine runtime
bundled in the macOS release archive, and the
[clean macOS record](validation/clean-macos.md) checks the release candidate
on a fresh Mac, the [fork-default record](validation/fork-default.md)
compares Sail on its Verus with plain stock Verus, and the
[determinism record](validation/determinism.md) measures the fork's Verus for
run-to-run differences, and the
[dependency store record](validation/dependency-store.md) checks new projects
that start from a stored vstd against plain stock Verus. The
[fallback assessment](validation/dependency-store-fail-closed.md) records the
native-input mismatch, temporary v1 quarantine, its cost and qualification.
The [compiler-reuse integration](validation/proof-reuse-integration.md) binds
current default gates and the distinct retained-proof and explicit
[vstd-boundary](validation/vstd-compile-boundary.md) qualifications; shipped
fresh-machine first use remains pending. The
[incidental-edit record](validation/incidental-edits.md) tracks the frozen
three-project edit catalogue and the combined runtime's remaining gates.
The [mutation-suite record](validation/mutation-suite.md) gives the seeded
mutation suite's soundness results and edit-loop timing against stock on four
real projects ([ADR 0063](decisions/0063-seeded-mutation-suite.md); how to run
it: [testing](testing.md#mutation-suite)).
The [full closure-identity record](validation/closure-full-identity.md) tracks the
subsequent runtime, its matched edit measurements and completed qualification
with platform/scope limits.
The [partial benchmark entrypoint record](validation/perf-real-partial.md)
describes focused Anvil commands, public JSON output and unavailable stage timings.
Raw logs stay outside the
repository and are cited by SHA-256. Earlier per-run records and unselected
engine candidates are preserved in the `archive/m7-contracts-f7103bf` tag.
Each record applies to its stated source, runtime and environment. A later pass
does not erase an earlier failure, and a candidate build or static review does
not qualify a runtime. Reports summarize completed milestones:
[bootstrap](reports/2026-10-08-bootstrap.md),
[initial product](reports/2026-10-09-initial-product.md), and
[live workspace](reports/2026-10-10-live-workspace.md). ADR 0038 records the
M7 rescue's decisions; the rescue review and lane reports are kept in the tag
`archive/rescue-m7-98f6292`. The
[new-user walkthrough](reports/2026-10-13-new-user-walkthrough.md) and
[roadmap review](reports/2026-10-13-roadmap-review.md) are the current inputs
to planning. The [warm frontend profile](reports/2026-10-16-warm-frontend.md)
records the current floor, failed Anvil scope and bounded native compiler
comparison; its [qualification record](validation/warm-frontend.md)
distinguishes the research checks from pending runtime acceptance.
The [verification runtime scope](scope/verification-runtime.md)
records the owner's direction for releases after 0.1.0.

[Verus issues](verus-issues/README.md) contain local reproductions and workarounds.
[Survey documents](survey/README.md) retain Verus/Z3 protocol, solver behavior,
corpus and prior-art research. Upstream reports are a human decision; agents
record discoveries in this repository.

[Dependency-store stdout validation](validation/dependency-store-output.md) records the rc.5 output defect and its correction.

[Native source-sync validation](validation/host-source-sync.md) records source
content and timestamp controls for the EC2 host helpers. The
[Linux teardown record](validation/perf-host-teardown.md) covers retained
recovery state and termination-before-cleanup controls.

The [Cargo target-context record](validation/cargo-target-context.md) covers
owned metadata selection, source-directory coverage, and declared environment
reads (#166 and #189), with native full fast/standard and separate joined
main checks.

- [Managed Cargo selection qualification](validation/managed-selection-family.md):
  candidate dependency-artifact sharing, observations and pending native gates.

The [spec formal naming qualification](validation/spec-parameter-names.md) records
the qualified binder fix, both platform artifacts, completed native gates and
preserved earlier failures.

[Unscheduled-script validation](validation/unscheduled-script-cleaning.md) records
the qualified #174/#215 implementation, its nine-sample targeted gain, complete
native/Mac gates, unchanged Vest costs and preserved historical failures.

[Neutral source profile two qualification](validation/neutral-source-profile-two.md) records explicit flat Rust admission, retained legacy identities and the bounded UNKNOWN diagnostic.
