# 0090: Additional exact shipped-vstd profiles

- **Status:** accepted by the owner on 2026-10-29; implemented with Linux/macOS factories qualified; release qualification open
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

## Amendment, 2026-11-10: members-only native lookup (#563)

**Status:** decided under the v0.3.0 engineering delegation (ADR 0109);
no verdict, diagnostic or check changes. Release qualification remains open
with the rest of this ADR.

**Cause.** The v0.3.0 cold-attribution spike reproduced CapybaraKV's
refusal on a native x86-64 host. ADR 0061 gives each closure unit a private
copy of every artifact in Cargo's `-L dependency=` directory, and completion
requires this run's producer record for each copy. CapybaraKV's `deps/` also
holds crates outside vstd's closure (`rand_core`, `libc`, `bindgen`,
`regex` and others), which no observation ever records, so every closure
unit declined silently and vstd missed with
`native dependencies lack this run's successful compiler provenance`.

**Decision.** A closure unit's private lookup directory holds only the
files a lookup of the unit's crate names can consider. rustc resolves a
crate by name among the search-directory files whose names start with
`lib` and that name, then reads each candidate's metadata. A file is
copied when it has this run's producer record or running mark; a file
whose name starts with any *lookup name* and lacks that provenance refuses
the observation exactly as before; every other file is left out of the copy
and of the graph inventory. The lookup names are the admitted closure's
crate names (each package name with `-` as `_`, and any manifest `[lib]
name`), the crate stem of every produced artifact, and the sysroot crates
rustc resolves by bare name (`std`, `core`, `alloc`, `proc_macro`, `test`,
`compiler_builtins`, `panic_unwind`, `panic_abort`, `profiler_builtins`).
An explicit `--extern` outside the membership refuses. This is the default;
there is no switch.

**Argument.**

1. *The compiler sees only the private directory.* The rewritten arguments
   name the private copies alone (ADR 0061), so a left-out file cannot be
   read, loaded or certified.
2. *Membership only narrows.* A left-out file's name does not start with
   any lookup name, so no crate lookup this unit makes can consider it: the
   closure's own crates are found by name and hash, and the sysroot names
   cover every bare-name lookup. The candidates a lookup sees, and so the
   crate it loads or the error it reports, are those of the full directory.
   Prefix matching keeps over-approximating: `syn` keeps `synstructure`,
   `core` keeps `core_foundation`, and such an unrecorded candidate refuses.
3. *No check changes.* Every copied input still needs an established
   producer record of this run, its bytes are reread at completion, and the
   source-read, compiler, extern and rebinding checks are unchanged. The
   unit key and the vstd invocation's own captured externs are unchanged.
4. *Fail safe.* Any refusal runs the real compiler with the original
   arguments and records nothing. A compiler that fails under the private
   directory removes its running marks and writes no record (only a
   successful child completes), so dependents refuse and vstd takes the
   ordinary unserved path; the failure's diagnostics are the same compile
   over the same candidates.

**Surfacing.** Every refusal or completion decline now has a stable code
(for example `lookup-unproduced`, `graph-unestablished`) in the
`SAIL_SHIPPED_TRACE` events. A closure unit's decline is also recorded for
the run, and when no more specific reason applies the `SAIL3138` note names
the first such unit and code instead of the generic lookup mismatch.

**Evidence.** The spike's opt-in prototype (`7449105b`) served CapybaraKV
the existing default unit, the same key IronKV uses, in 6 of 6 cold rounds
(stock 62.16 s, 0.2.0 57.82 s, prototype 36.36 s mean). The implementation
(`7b3b9746`) on c7i.8xlarge, us-west-2, Ubuntu 24.04, six rotated cold
rounds per arm, all verdicts identical (CapybaraKV 2060 + 732 verified,
IronKV 2060 + 313):

| Project | Stock | 0.2.0 | `main` | This change | vstd |
|---|---:|---:|---:|---:|---|
| CapybaraKV | 54.10 s | 50.86 s | 50.26 s | 31.71 s | served 6/6 (main 0/6) |
| IronKV | 37.84 s | 17.31 s | 15.90 s | 15.91 s | served 6/6 (unchanged) |

An earlier build (`1a7bd036`) served 5 of 6 rounds: it chose the left-out
files before copying, so a foreign crate written by a concurrent compiler in
between was copied and then had no record. Each file is now decided while
it is copied. Evidence archive SHA-256:
`0aad9491704dddb83c978529b2b339adbae14e4987aee31c64b1c580336bc9b7`
(earlier build:
`3ba61b7b3770b5367e203546abeb90d581ca47397e21fb62536095e3336b6f27`).
A rebuilt `sail`
cannot use a release bundle, because the release index binds the published
binary, so those rows use the alternate-bundle research mode; measuring
the staged candidate is a release-qualification item, as is the macOS run.
