# VI-0030: An empty impl renames later methods and untouched clients' SMT

- **Status:** actual accepted-runtime Vest reproduction; a narrow fork source
  prototype is implemented but uncompiled, unqualified and unselected. Reduced
  fixture and new compiler controls remain unrun. No upstream filing.
- **Found:** 2026-10-18 while diagnosing issue #118's remaining incidental edits.
- **Runtime:** accepted fork `e5365c05d0a9f8c8cb655edc035ce9a2f6dde482`, normal
  Linux archive SHA-256
  `689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928`.
- **Corpus:** Vest `11af01ddaef76cceded03517cd48764adb9d35b1` with the retained
  common-source preparation from Sail `83128ad0d5327f00bfea822d608ebd99dd9d8112`.
- **Reduced candidate:** [verus-impl-renumbering](../../corpus/repros/verus-impl-renumbering/),
  not yet executed. It includes a closure under the later unchanged impl.

Inserting exactly an empty `impl TagFmt { }` after the unique `verus! {` opener
in `vest_lib/src/asn1/mod.rs` shifts the names of later impl methods in that
module. `rust_to_vir_base::def_path_to_vir_path` turns rustc's per-parent
`DefPathData::Impl` disambiguator directly into `impl&%N`. The original rule is
in accepted source blob `d792ca10bf8da30ba5f61433312bfd4a8823166d`.

Two independent fresh native c7i.4xlarge Cargo-Verus invocations used the
original `verify --locked -- --output-json --time --log smt-transcript` flags,
separate targets and log directories, and a 120-second child cap each. Both
exited successfully. Each retained both raw JSON documents and all 4,998
function records (1,871 vstd, 3,127 Vest), with equal module/friendly-name/mode/
passing-verdict multiplicities and equal proof counts (2,060 vstd, 3,237 Vest).
Neither produced verifier diagnostics. Twenty-three full Vest function paths
changed from `impl&%N` to `impl&%(N+1)`; no vstd function path changed.

Each arm retained all 725 SMT transcripts, including 94 empty files, and
26,485 batches. Exact pipe-input reconstruction retains each query byte and
its original echo terminator; no comment, whitespace or identifier is
normalized. Of the 725 matching session paths, 695 have identical input and
30 differ. Both literal-input multisets retain 569 distinct hashes and 156
duplicate occurrences. Raw transcripts additionally contain responses and
statistics: 184 complete raw bodies differ, which is not a solver-input count.

The change reaches untouched `cbor::chunk` and `cbor::format`. Their identical
source files receive this changed declaration, trigger and quantifier name:

```smt2
; before
(declare-fun ens%vest_lib!asn1.impl&%49.clone. (Poly Poly) Bool)
; after
(declare-fun ens%vest_lib!asn1.impl&%50.clone. (Poly Poly) Bool)
```

The two literal input sizes stay 1,528,429 and 6,125,973 bytes, with 35 and 125
batches respectively. This is direct namespace churn in untouched referencing
modules caused by the isolated empty-impl insertion. It is a different-program
A/B diagnosis, not same-source nondeterminism, conformance or a measured gain.

Before/final payload, source, runtime, corpus, compiler-toolchain and restored
common-body guards passed. Complete indexed capture verified every body and
mode. The fixed frame was 30 minutes work, 45 minutes outer and 60 minutes TTL,
with 1,740 seconds of work caps; no compiler/runtime/helper build or profiler
ran. The host and all exact owned AWS resources were independently verified
absent after capture.

Raw evidence remains outside git: complete archive SHA-256
`c32a761ebccad6c2947c4f3bc907e3bbe04920591fd73f0dc96ed6eeeaf1efbb`
(473,388,357 raw bytes, 36,082,186 compressed); diagnosis summary SHA-256
`1c4cffbe0a7abd71e903a814261bd5fcd3e63d4e02fd64445ad7c94c2fc208c5`;
complete CBOR exact-pipe line diffs SHA-256
`c19ef8f8014614f9fc5436b4ebb8fa067480e8ca1637dec93ecbb97fc6812d62`;
raw/literal body hash multiplicities SHA-256
`58224c0499d30083379f905e9e250b49f6843ed4b0a11b075e36625661bf10c3`.

A fork-side fix must preserve distinct impl identities, including separate
blocks with the same self type and trait/generic headers. Method path rendering
alone is insufficient: [VI-0019](VI-0019-crate-wide-closure-numbering.md)'s
closure `DefPathHash` also includes the enclosing impl disambiguator. The narrow prototype and its additional renaming policy are recorded in
proposed ADR 0056; compiler execution and selection remain pending. Exact Sail
cache keys and solver bytes remain unchanged by Sail. Full correctness, determinism and matched edit-loop gains remain required
before any new runtime is selected.

The isolated accepted-e536 prototype02 follows proposed
[ADR 0056](../decisions/0056-stable-anonymous-impl-identities.md). Only positive,
non-marker concrete trait references with zero-argument nominal Self and no
other arguments receive full typed crate/module/Self/trait keys after coherence.
Inherent impls remain legacy. Members and complete peer sets no longer enter
the key; closures use the same complete eligible parent identity.

Prototype01's sealed uncompiled source failed independent review: native ghost
erasure can remove private/restricted members, invalidating its asserted
producer/consumer membership parity. That static failure is retained; it never
qualified a runtime. Prototype02 removes that membership dependency, but its
arbitrary cfg/header eligibility boundary remains a source-review obligation
before building. No import mismatch error is accepted as a default-mode repair.

The actual 23 changed paths comprise 21 Clone methods and two proof methods;
the latter still need header classification. The unchanged-CBOR witness changes
a Clone contract name. Future matching calls must classify all 30 changed exact
sessions and retain the original full scope; no gain follows from this source.
New producer/native/consumer controls cover ghost/restricted members, ghost-only
and duplicate inherent blocks, unused lifetimes/predicates, exact method lookup
and over-limit legacy policy. All new controls and the standalone reduced corpus
fixture remain uncompiled and unrun. The independent paired filename repair also
retains spinoff counters for Sail attribution; no SCC prototype is included.
Full-key size, registry work and longer SMT names may erase a gain.
