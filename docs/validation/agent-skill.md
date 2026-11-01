# Agent skill validation

Status: local package and task checks pass; required PR CI remains the merge gate.
Date: 1 November 2026.

## Scope

The canonical `sail-verification` package, its documentation and a lightweight
package-relative-link check are new. No verifier, solver, cache or result
semantics change. This is documentation/diagnostic tooling under ADR 0087;
standard/deep corpus runs and new performance measurements are not required.

## Package and cold readings

The package validates with the skill frontmatter validator and is discovered
and copied by `skills` CLI 1.7.4 into a clean temporary project. Its references
and MIT license travel inside the package. Package snapshot SHA-256:
`6058172373ce53cebe4bf9acc4e75c8fb90c94b5cb466cf0016b356e51b5e5a8`.

Independent fresh Bedrock technical and devrel readings assessed commands,
release capabilities, currentness, solver products and first-use instructions.
The completed technical reading found no contradictions. Accepted findings
clarified service overrides, Verus-only currentness and capability-based
standalone selection. Suggestions to add repeated maturity warnings or treat
skill version 1.0.0 as the Sail binary version were rejected. Skill and runtime
versions are separate. One truncated technical call was excluded; its completed
replacement supplies the technical reading. Raw requests, responses and
accepted/rejected dispositions stay outside the repository.

## Bounded task comparison

Eight fresh response-generation calls compared the prior website skill with
the new package on four prompts. The model generated artifacts and commands;
it did not execute tools. Relevant artifacts were then run against actual Sail
on a native Linux c7i.4xlarge host in us-east-1. Runtime source: a939b802;
workspace version 0.1.0, pinned fork 0ad405eb and Z3 4.16.0. The CI image's
validated prebuilt engine was explicitly selected for the source-build tests.

| Task | Prior website skill | New package |
| --- | --- | --- |
| Fix a Verus implementation while preserving its contract | Corrected function verified, complete requested target, Current | Same result |
| Interpret old, selected-function and Current-but-failed results | Refused to claim the whole target verified | Same, with explicit scope/verdict/currentness separation |
| Produce one model query and one named unsat-core query | Returned a wrapper explicitly described as a placeholder; actual Sail rejected it with SAIL6101/exit 2 | Actual Sail returned sat/model and unsat/core, exit 0; both second-call cache_hit values were true |
| Existing standalone file on a main build reporting 0.1.0 | Incorrectly required creating a Cargo project | Inspected help and selected direct file verification; actual call passed Current/complete |

This is a small qualitative comparison, not an agent success-rate or speed
benchmark. The raw SMT schema improvement and standalone distinction are
observed in these examples only. Native output manifest SHA-256:
`7b8df6404f3e9ace95dbbde1f94ccf345e294976be62ef6a8d293f7248f20ef3`.

## Gates

Two native `ci/local.sh gate` runs passed during preparation. The final run
included the package-relative-link regression test, fmt, strict Clippy, 1,303
nextest tests, doctests, rustdoc, layering, dependency policy and generated
benchmark-page freshness. The documented commands and model-generated artifacts
were exercised separately as described above. No new network/model/browser
integration tests enter CI.

## Failed or incomplete attempts

- a939b802, 1 November: one final technical model call reached its token limit;
  excluded from completed-review evidence and replaced by a fresh bounded call.
- a939b802, 1 November: a source-build setup probe lacked SAIL_VERUS_BUILD and
  correctly reported its engine missing; subsequent actual task runs explicitly
  selected the CI image's validated engine and passed.
