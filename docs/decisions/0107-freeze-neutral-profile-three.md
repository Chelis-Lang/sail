# 0107: Freeze neutral profile three before its first release

- **Status:** superseded by [ADR 0109](0109-oyster-retired-v030-scope.md): the surface this decision describes was removed in v0.3.0. The last implementation is tagged `archive/neutral-contracts-final`. Previously: accepted
- **Date:** 2026-11-09
- **Amends:** the freezing rule in [ADR 0103](0103-scoped-neutral-widening.md)

## Context

ADR 0103 assembled neutral profile three (lowering version four, contract
version three) across several pull requests and froze it at its first release.
Profile three's emitted bytes changed between those pull requests:

- Predicate calls began to emit resolved `crate::` paths.
- Each inline module gained the Verus prelude.
- `module_file` attribution was corrected.
- The signed remainder helper took Verus's executable shape.

Each change was correct. But a version-four manifest retained from an earlier
`main` revision now fails `validate_manifest`. It is refused as drift and never
accepted, but the evidence it describes cannot be replayed.

Consumers no longer wait for a release. A consumer may depend on the S.A.I.L.
crates at a pinned git revision of `main` and run a locally built service. Any
version-four evidence such a consumer retains must keep replaying, just as
evidence from profiles one and two does.

The authorized forms that are not admitted yet are fixed arrays and indexing
([issue 687](https://github.com/Chelis-Lang/sail/issues/687)), function-level
termination measures ([issue 688](https://github.com/Chelis-Lang/sail/issues/688))
and unit enums inside modules
([issue 689](https://github.com/Chelis-Lang/sail/issues/689)). Each still needs a
design decision or an upstream fix.

## Decision

1. **Profile three is frozen now.** This applies from the commit that adds this
   ADR. Its admission, its emitted shadow bytes and its manifest shape are
   frozen as they stand, without waiting for a release.
2. **Retained manifests replay.** `neutral_profile3_frozen` holds version-four
   manifests for the division, remainder, body-call, loop and multi-file
   closure fixtures. Each one must validate, and fresh lowering must reproduce
   it byte for byte. Each manifest binds its shadow through `lowered_sha256`.
3. **Later widening gets a new identity.** A later change to what profile three
   admits or emits gets a new lowering version, and the deferred forms above are
   such changes. A change to the admitted syntax also gets a new explicit
   profile. Version-four manifests keep validating under their own version.
4. **Earlier revisions are not supported.** A manifest recorded from a `main`
   revision before this freeze is not supported. It is refused as drift, and its
   source must be lowered again.
5. **Bug fixes follow the earlier profiles.** A soundness defect is fixed with a
   new lowering version, as version two corrected version one. Retained
   version-four manifests are then checked against the corrected semantics.

## Consequences

- Consumers can pin any `main` revision at or after this freeze. Their profile-three
  evidence stays valid across later S.A.I.L. revisions.
- Arrays, indexing, function measures and module enums each cost a version bump.
  They stay explicit and selectable, with no silent change.
- The release that first carries profile three ships these exact bytes.
