# Decisions

Accepted decisions supplement the charter and its corrections appendix.
Use [the template](0000-template.md) for a new decision.

- [0001: Workspace, toolchain, lints, layering and CI tiers](0001-workspace-toolchain-and-ci.md)
- [0002: AI-authorship guard and workflow mode](0002-authorship-guard-and-workflow-mode.md)
- [0003: Integration point: a Z3-compatible shim named by `VERUS_Z3_PATH`](0003-integration-point.md)
- [0004: Session-state model and query keys](0004-session-state-and-query-keys.md)
- [0005: Shim pass-through, observation tap and fail-safe rules](0005-shim-pass-through-and-fail-safe.md)
- [0006: Conformance harness and corpus manifest](0006-conformance-harness-and-corpus-format.md)
- [0007: Benchmark methodology (cold workload and baseline)](0007-benchmark-methodology.md)
- [0008: Default-mode keys bind the whole process prefix](0008-default-mode-keys-bind-the-process-prefix.md)
- [0009: Solver nondeterminism and unstable results](0009-solver-nondeterminism-and-unstable-results.md)
- [0010: Owner decisions on the bootstrap open questions](0010-owner-decisions-2026-10-08.md)
- [0011: Local cache, daemon protocol and lazy solver sessions](0011-local-cache-and-lazy-solver-sessions.md)
- [0012: Warm and edit workloads](0012-warm-and-edit-workloads.md)
- [0013: One container image for CI and local testing](0013-ci-image.md)
- [0014: Function attribution, the ledger, and `--output json` schema v1](0014-attribution-ledger-and-output-schema.md)
- [0015: Daemon operations, the in-memory tier and write-behind](0015-daemon-operations-and-memory-tier.md)
- [0016: The MCP server (`sail mcp`)](0016-mcp-server.md)
- [0017: The shared tier: signed entries, verified before use](0017-shared-tier-with-signed-entries.md)
- [0018: Fragility measurement](0018-fragility-measurement.md)
- [0019: `rust_verify_test` conformance from a Verus source build](0019-rust-verify-test-conformance.md)
- [0020: Complete-run seed portfolio](0020-complete-run-seed-portfolio.md)
- [0021: Optional local Singular source validation](0021-optional-singular-source-validation.md)
- [0022: Solver stderr and cache publication](0022-solver-stderr-and-cache-publication.md)
- [0023: Managed incremental workspace verification](0023-managed-incremental-workspace-verification.md)
- [0024: Persistent workspace jobs and live clients](0024-persistent-workspace-jobs-and-live-clients.md)
- [0025: HTTP shared cache and local-first team workflow](0025-http-shared-cache-and-local-first-team-workflow.md)
- [0026: Live workspace dependencies](0026-live-workspace-dependencies.md)
- [0027: Interactive proof engine and repair workflow](0027-interactive-proof-engine-and-repair-workflow.md)
- [0028: Exact prepared graph-read dependencies (proposed candidate)](0028-exact-prepared-graph-read-dependencies.md)
- [0029: Kernel lease work bounds and freshness](0029-kernel-lease-work-bounds-and-freshness.md) — superseded by 0041
- [0030: Loaded imported span authority (proposed candidate)](0030-imported-span-authority.md)
- [0031: Guarded sysroot digest handoff (proposed)](0031-guarded-sysroot-digest-handoff.md) — superseded by 0041
- [0032: Sticky kernel mount-generation coverage](0032-kernel-mount-generation-coverage.md) — superseded by 0041
- [0033: Exact deferred imported diagnostic locators (proposed candidate)](0033-deferred-import-diagnostic-locators.md)
- [0034: MCP response encoding ownership](0034-mcp-response-encoding-ownership.md)
- [0035: Guarded retained output digests (rejected: removed, saved at most 1.6 ms per read)](0035-guarded-retained-output-digests.md)
- [0036: Managed compiler artifact families (implemented; product validation pending)](0036-managed-compiler-artifact-families.md)
- [0037: Owned async shared HTTP transport (rejected after comparison)](0037-owned-async-shared-http-transport.md)
- [0038: M7 rescue: native reference hosts, evidence policy and engine conditions](0038-m7-rescue.md)
- [0039: The engine is built from a private Verus fork](0039-private-verus-fork.md)
- [0040: Warm solver sessions for edit-loop misses (accepted: do not build now)](0040-warm-solver-sessions.md)
- [0041: Content and stat checks replace kernel leases for currentness](0041-content-and-stat-currentness.md)
- [0042: Diagnostics are compared as a set of complete messages](0042-diagnostics-compared-as-a-set.md)
- [0043: Default mode runs Sail's Verus, built from the fork](0043-default-mode-runs-the-fork.md)
- [0044: Output schema v2: establishment and evidence tier](0044-output-schema-v2.md)
- [0045: A machine-local store of verified registry dependencies](0045-dependency-store.md)
- [0046: Managed verification is the only `sail verify`](0046-managed-verification-by-default.md)
- [0047: Attribution logs on request, and cheaper managed checks](0047-attribution-logs-on-request.md)
- [0048: Reuse vstd at its compiler boundary (qualified explicit research; release/default pending)](0048-shipped-dependency-build.md)
- [0049: The fork names closure types by definition path (proposed)](0049-stable-closure-names.md)
- [0050: Content-bound retained proofs for an undone edit (scoped native qualification passes; delivery pending)](0050-retained-proofs-instant-revert.md)
- [0051: Solver continuation for changed-check misses (proposed, blocked and unselected)](0051-exact-solver-snapshots.md)
- [0052: Complete frontend inputs and current-source diagnostic binding (proposed)](0052-warm-frontend-reuse.md)
- [0053: Disable unqualified native-artifact seeding (proposed, unselected)](0053-disable-unqualified-native-store.md)

- [0054: Stable solver names for spec formal parameters (proposed)](0054-stable-spec-parameter-names.md)
- [0055: Acknowledge visible managed-driver output (accepted)](0055-acknowledged-managed-output.md)

- [0056: Stable SMT spellings for concrete trait functions (proposed, measured candidate)](0056-stable-smt-function-spellings.md)
- [0058: Managed builds use the engine's pinned Cargo (accepted)](0058-managed-builds-use-the-pinned-cargo.md)
- [0059: Pin the published vstd and generate the source policies (proposed)](0059-pinned-vstd-and-generated-source-policies.md)
- [0060: Serve the shipped vstd proof under a raised `--rlimit` (accepted)](0060-shipped-vstd-under-a-raised-rlimit.md)
- [0061: Certify native units after their compile (accepted)](0061-pipelined-native-admission.md)
- [0062: Ship the pinned vstd's solver replies in the release archive (accepted)](0062-shipped-vstd-solver-replies.md)
- [0063: A seeded mutation suite as soundness probe and edit-loop benchmark (accepted)](0063-seeded-mutation-suite.md)

- [0064: Accept verification-progress note ordering (accepted)](0064-verification-progress-note-order.md)
- [0065: One verification runtime for Oyster (Oyster parts superseded by 0109)](0065-oyster-runtime.md)
- [0066: Shared neutral contract syntax and checked runtime (runtime implemented; backend qualification separate)](0066-neutral-contract-runtime.md) — superseded by 0109
- [0067: Isolated native D.I.V.E. runtime worker (proposed)](0067-dive-runtime-worker.md)
- [0068: Shared runtime transport and agent tools](0068-runtime-daemon.md)
- [0069: Exact-scope report reuse](0069-exact-scope-report-reuse.md)
- [0070: Owned neutral source verification jobs](0070-neutral-contract-jobs.md) — superseded by 0109
- [0071: Native temporal jobs through the shared runtime](0071-shared-temporal-runtime.md)
- [0072: Derive requirement assurance from authenticated neutral evidence](0072-requirement-assurance.md) — superseded by 0109
- [0073: Preserve compound-field definedness and qualify retained lowering](0073-compound-field-definedness.md) — superseded by 0109

- [0074: Keep bounded neutral-source mutation evidence separate from proof](0074-bounded-neutral-mutation.md) — superseded by 0109
- [0075: Preserve floating-point identity in retained JSON evidence](0075-retained-json-roundtrip.md)


- [0076: Explicit flat Rust source profile two](0076-neutral-source-profile-two.md) — superseded by 0109
- [0077: Separate scalar literal-contract counterexamples from Verus mutation verdicts](0077-scalar-literal-contract-witnesses.md) — superseded by 0109
- [0078: Captured acceptance models checked by stock Verus (accepted)](0078-stock-acceptance-models.md)
- [0079: Source-shared executable acceptance refinements (accepted)](0079-source-shared-acceptance-refinements.md)
- [0080: Optional native temporal runtime (superseded by 0083)](0080-optional-temporal-runtime.md)
- [0081: The cache checks response framing with the SMT-LIB splitter (accepted)](0081-cache-response-framing.md)
- [0082: Replay unchanged calls from a complete stat-stamp vector (accepted; amended for solver identity)](0082-unchanged-call-replay.md)
- [0083: The temporal backend is an extension workspace (accepted)](0083-temporal-extension-workspace.md)
- [0084: Build-script receipts for a qualified range of Cargo releases (proposed)](0084-qualified-cargo-range.md)
- [0087: Scope long conformance gates to runtime behavior (accepted)](0087-scoped-merge-conformance.md)
- [0088: Accept named compiler-visible integration settings (accepted)](0088-compiler-visible-integration-settings.md)

- [0089: Exact input-prefix and solver-delivery operations](0089-checked-input-prefix-delivery.md) (proposed)

- [0090: Additional exact shipped-vstd profiles](0090-additional-exact-vstd-profiles.md) (accepted; implemented, factories qualified, release qualification open)
- [0091: Settled source and owned-target digest observations](0091-settled-source-and-target-digests.md) (accepted; implementation qualified, Linux target default selected, release qualification open)

- [0092: Per-file owned-target digest observations](0092-per-file-owned-target-digests.md) (owner-approved bounded prototype; implemented, off by default and unselected)
- [0093: Named order exceptions in stock source-suite debug snapshots](0093-stock-source-suite-debug-order.md) (owner-approved; implemented and qualified against the pinned stock suite)
- [0094: Isolated specification termination checks](0094-isolated-spec-termination-checks.md) (accepted bounded correction; qualified and selected on Linux and macOS)

- [0095: Standalone Verus files](0095-standalone-verus-files.md) (implemented and qualified on native Linux and macOS)

- [0096: Daemon-owned optional preparation](0096-daemon-owned-preparation.md) — owner-approved; feature merged, native functional controls pass; performance acceptance open

- [0097: Consumed resource statistics in determinism](0097-consumed-resource-statistics-in-determinism.md) (accepted; strict default retained, explicit consumed-counter classification)

- [0098: Publish Sail as sail-solve on crates.io](0098-crates-io-distribution.md) (owner-authorized; native package and installed-command qualification complete, initial registry publication pending)

- [0099: Mutation explanation-note cardinality](0099-mutation-explanation-note-cardinality.md) (accepted; existing per-code uniqueness corrected, native Linux/Mac witness and negative controls qualified)

- [0100: Mutation-first roadmap and design-first Oyster hold (accepted; Oyster parts superseded and backend ordering amended by 0109)](0100-mutation-first-roadmap.md)

- [0101: D.I.V.E. behind a default-off build flag](0101-dive-behind-a-default-off-flag.md) (accepted; extension workspace is the flag, main-gate lock and publish guards, flip queued)

- [0103: Scoped lifting of the neutral-widening hold](0103-scoped-neutral-widening.md) (accepted by owner; named items only, under explicit profile three) — superseded by 0109

- [0104: Submitter requirement attribution for ordinary Verus jobs](0104-ordinary-job-requirement-attribution.md) (accepted; separate negotiated view, never neutral assurance)

- [0105: Place neutral shadows away from inherited Cargo configuration](0105-neutral-shadow-placement.md) (accepted; private per-user placement, unchanged refusal with named cause) — superseded by 0109

- [0106: Decode source-named inputs from failed-proof models](0106-decoded-counterexample-inputs.md) (accepted; parameters and named struct fields, raw model retained, never a validated refutation)

- [0107: Freeze neutral profile three before its first release](0107-freeze-neutral-profile-three.md) (accepted; retained version-four manifests replay byte for byte, later widening takes a new identity) — superseded by 0109
- [0108: The SMT runtime is a supported command](0108-supported-smt-runtime.md) (accepted; `sail runtime` and the MCP runtime tools are listed as supported)
- [0109: Oyster retired; v0.3.0 scope](0109-oyster-retired-v030-scope.md) (accepted by owner; Oyster abandoned, v0.3.0 pillars, experimental Kani then cvc5 inside the release, neutral-contract and contract-crate removal after stated preconditions, no second Verus pin)
- [0110: Interrupted first observations are discarded, not trusted](0110-interrupted-first-observation.md) (accepted, conservative rule only; per-predicate `SAIL3114` reasons, continuous absent-configuration drain, unsourced checks never reused)
- [0111: A Verus rejection counts as a mutation detection](0111-mutation-evidence.md) (accepted by owner; attributed, stable rejections with a baseline twin that has headroom score as detected, with a `verifier-rejected`/`counterexample-confirmed` breakdown; campaign views only, solver answers verbatim)
- [0112: Preserve stock builtin source paths in the runtime](0112-stock-builtin-source-paths.md) (owner-approved scoped exception; implementation and runtime qualification pending)

- [0114: Owned Verus mutation campaigns and their evidence](0114-verus-campaign-execution.md) (accepted for implementation; independent evidence model, bounded owned copies and full-unit confirmation; qualification pending)

- [0113: Compare complete compiler invocations in Cargo conformance](0113-conformance-invocation-capture.md) (accepted; complete exact streams, invocation identity, probes and interleaving checks)

- [0116: Recorded proof health without inferred stability](0116-recorded-proof-health.md) (accepted for implementation; bounded retained measurements and shared interfaces; qualification pending)

- [0128: Separate bounded Kani observations from ordinary proofs](0128-kani-bounded-observations.md)
