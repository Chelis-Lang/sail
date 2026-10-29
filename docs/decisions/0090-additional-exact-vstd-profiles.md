# 0090: Additional exact shipped-vstd profiles

- **Status:** proposed; owner sign-off required before adoption
- **Date:** 2026-10-29
- **Amends:** [ADR 0048](0048-shipped-dependency-build.md),
  [ADR 0060](0060-shipped-vstd-under-a-raised-rlimit.md)
- **Tracks:** [#563](https://github.com/Chelis-Lang/sail/issues/563)

## Context

The v0.2.0 plan calls for more exact compiler-boundary profiles before
considering another verifier key. The aligned-Vest experiment in
[rlimit qualification](../validation/rlimit-subsumption.md) demonstrates the
benefit of serving its vstd proof. Editing a user's lockfile or features
does not implement that plan.

The 2026-10-29 Mac diagnostic uses staged Rust source `1dc5f1a5`, fork
`47fb3be6`, a release-layout bundle and registry vstd. It is not paired
cold acceptance:

| Actual invocation | Observation | Consequence |
|---|---|---|
| Vest, `--rlimit 100` | Default vstd features off; equivalent 1.0.2, indexmap 2.14.1 and unicode-ident 1.0.24 differ from the default policy | Record this exact closure and feature set |
| CapybaraKV, `--no-default-features` | The vstd closure is aligned; vstd itself has alloc/default/std; lookup refuses missing successful native producer provenance | Another vstd feature profile alone does not explain or fix this refusal |
| Anvil, `--lib -- --verify-module vdeployment_controller` | Runner explicitly does not offer shipped proofs for focused verification; zero bundled lookups | Preserve scope; address cold overhead under #232 |

Vest and CapybaraKV each record 20,197 bundled lookups, all hits; ADR 0062
already saves their matching solver work. Diagnostic archive
`v020-profiles-mac.tgz` remains outside Git. CapybaraKV's lookup-directory
provenance needs a narrowed reproduction and remains unselected here.
Archive SHA-256:
`c8f688f2a13df8e6ee4922fd7f2e76f58b50b233e51c6ee0d84a635512efdb0d`.

## Decision

Select route A from #563: additional **exact** compiler-boundary profiles.
Initially add Vest's observed closure with default vstd features disabled,
including its Mac `--rlimit 100` recording. Preserve the current default
and raised-limit Mac profiles. Route B, a post-frontend verifier-input key,
is not selected. No fork change is required.

Embed a finite set of closed source policies. Each binds exact versions,
features, archives, source files and compiler-read checksums. Resolve the
caller's actual metadata and features before offering reuse. Ambiguity or
any unsupported input refuses the offer. A profile never edits the caller's
manifest, lock, features, arguments, toolchain or source selection.

Every proof unit continues to bind the complete compiler-boundary tuple:
policy digest, arguments, relevant environment, successful native producer
provenance, dependency bytes, runtime, sysroot, platform and solver. Do not
project, discard or normalize a key field. Closure matching selects a
policy, not proof authority. All existing compiler-read, native graph,
artifact, transcript and establishing-run checks remain.

Bump lookup schema 12 to 13. Bind each unit in the release manifest and
index to its own policy and establishing run, with at most eight units per
bundle and the existing 16,384-member index ceiling. The factory uses an
explicit qualified-recording list; increasing the ceiling does not select
profiles automatically. Old keys cannot collide with schema 13. Unsupported
or damaged bundles fall back to real verification.

The factory records the actual supported fixture with its exact lock and
feature selection. Validate every unit and indexed member; compare output
with genuine pinned verification; refuse missing or inconsistent policy,
provenance, transcript, artifact or establishing-run relations. Retain the
recording source and platform/runtime identities.

Before adoption require exact-hit and single-field refusal controls for
every bound input, independent corruption of policy/unit/index/provenance
relations, the eight-unit boundary and refusal of a ninth. Preserve all
existing default and raised-limit controls. Compare served results with
fresh pinned-Z3 verification over the whole selected corpus. Run fast,
standard, deep, deep-rvt with and without cache, determinism, and six-round
four-project performance and memory qualification on both platforms.

## Consequences

Vest can use its own verified profile without source or configuration
edits. Measure the increase in archive size and factory work. A schema or
candidate build grants no performance acceptance.

CapybaraKV needs its provenance refusal diagnosed before selecting a
profile for it. Anvil's focused cold row remains a managed-path optimization
with the same partial scope. Adding Vest's profile fixes neither. Further
profiles retain these exact policies and qualification requirements; a new
reuse boundary needs another decision and soundness argument.
