# General performance qualification

**Current status:** implementation candidate; broader native qualification in
progress. No new release performance acceptance or frontend reuse is claimed.

The [review](../design/general-performance.md) defines the unchanged authority
boundaries. The [evaluation guide](../performance-evaluation.md) defines the
expanded corpus, explicit runtime selection and separate diagnostic controls.
The existing [v0.2.0 matrix](v020-performance.md), observer-overhead failures and
strict stock source-suite failure retain their original scope and requirements.

| Gate | Current evidence | Limit |
|---|---|---|
| Broader selection | 18 reviewable workloads; four new repository pins; Verus-TLA baseline passes (141 functions compared); hekate-math baseline passes all 18 units (1,848 function records compared) | Baselines compare stock with bare stock/shim; selected-fork, managed and targeted-edit coverage remain separate |
| Run-scoped observations | Fresh-byte decode sharing, borrowed source projection basis, indexed dependency traversal and exact streamed JSON hashing implemented | Native whole-invocation benefit not yet measured |
| Linux functional controls | Graph/projection composition passes strict Clippy, 1,280 ordinary tests (56 skipped), doctests, documentation/layering/dependency gates and six evaluation controls on c7i.8xlarge / us-east-1 | Subsequent workspace-preparation fix needs its own completed result |
| Algorithmic scaling | Native single-chain lookup including index construction: 4,096 nodes, 83.888 ms legacy / 1.574 ms indexed; identical closure | One component diagnostic, not repeated whole-program acceptance. JSON timing benefit is small and variable; bounded intermediate allocation is the distinct change |
| macOS | No result for this candidate | Native host gate, paired performance and release memory gates remain |
| Compiler reuse | No persistent frontend reuse implemented or selected | ADR 0052 complete-input/output shadow qualification remains open |

Raw records stay outside git. Add their source/runtime/environment identities,
scope and SHA-256 here after completed captures; do not describe a build or
matching failed baseline as a passing real-project workload.

## Compatibility findings

The initial successful baselines use stock `0.2026.10.18.7d82994`, its pinned
Z3 and Rust 1.99.1 on c7i.8xlarge / us-east-1. Verus-TLA uses scratch source
dependency adjustment; hekate-math runs its original 18 standalone proof roots.
Both arms of every passing unit exit zero with zero verification errors;
hekate-math reports 3,050 verified occurrences per arm across its roots. These
are compatibility/comparator controls, not timing acceptance.

Initial intake/component archive SHA-256:
`1b28f575e3317810594778665e628f017ab8267ad0a6fcd94eb426ff7aeecc67`.
It retains both intake attempts, the original comparator reports, selected
binary digests and all component samples, including slower JSON samples.
Source is the recorded working tree based on `715c0f5e`, before the complete
workspace preparation fix; it is not a clean-head release acceptance record.

Semi-persistent exposes a harness preparation defect: patching only the
selected package leaves conflicting exact vstd pins in other Cargo workspace
members. The candidate now discovers the complete workspace with Cargo's
offline, dependency-free metadata and adjusts each scratch member consistently
on both sides. Its native rerun remains pending. Fetched sources and proof
bodies are unchanged.

The TLSF pin does not compile with the pinned vstd: `Set::new` returns an
`Option<Set<_>>`, and `Map::new` expects a set rather than its predicate argument.
Both arms report the same two E0308 errors and compare zero functions. This
is an incompatible candidate, not a successful allocator baseline. No proof
port or runtime substitution is included.

## Failed attempts

- 2026-10-31, `8f298701`: workspace preparation composition failed to compile
  because registry-adjustment logging lost its repository-root parameter
  during extraction. Restored that explicit parameter before native rerun.

- 2026-10-31, working tree based on `715c0f5e`: first native candidate gate
  failed Clippy's needless-borrow check in the benchmark CLI. Corrected in the
  subsequent native subset gate; no check or baseline changed.
- 2026-10-31, working tree based on `715c0f5e`: first intake refused all six
  selections because independently installed tools looked for a sibling shim.
  The runner now passes explicit executable paths; the rerun retains all units.
- 2026-10-31, working tree based on `715c0f5e`: semi-persistent's three package
  attempts refused Cargo resolution due to incomplete scratch workspace pin
  adjustment. TLSF reports matching compiler errors. Both remain explicit.
