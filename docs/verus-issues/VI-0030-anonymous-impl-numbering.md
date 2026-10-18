# VI-0030: An empty impl renames later methods and untouched clients' SMT

- **Status:** actual accepted-runtime Vest reproduction; a narrow fork source
  prototype is native-built and measured, but remains unqualified and unselected.
  The standalone reduced fixture remains unrun. No upstream filing.
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

Offline classification of all 30 original literal mismatches identifies only
five sessions whose changed lines are solely Clone symbols; 25 also change raw
trait-impl quantifier labels, deep_view fuel/definitions or other symbols.
The 23-path timing delta covers verified functions and omits spec declarations;
it is not a complete declaration inventory. Classification does not alter any
query byte or establish candidate coverage. The retained classification report
has SHA-256
`531e336b8e52a994a18b5e47f3a1ac9e4ec4981b5124cf30d180d5d1582e9ffd`.

The candidate correction preserves all accepted Rust/VIR identities,
imports and exported payloads. It selects injective SMT spellings for eligible
concrete trait functions after reading the complete merged pre-prune VIR, with
all duplicate candidate labels retaining legacy spelling. Original method paths
and accepted closure IDs are deliberately not renamed. Proposed
[ADR 0056](../decisions/0056-stable-smt-function-spellings.md) records the policy.

Two previous uncompiled source packets failed independent review: prototype01
used an associated-member inventory that native ghost erasure changes;
prototype02 inferred unique ground impls from coherence, but trivial bounds,
const specialization and cfg marker changes invalidate that assumption. Neither
was built or selected. Their static failures remain part of this gate's record.

Prototype03's immutable function-only AIR spelling map needs neither assumption.
It leaves original Path/Fun/ClosureId equality, lookup, sorting, diagnostics and
import/export unchanged. It uses full typed crate/module/Self/trait/member keys,
not body/span/index-derived IDs or truncated hashes. Unsupported or duplicate
candidates retain legacy names; inline AIR disables aliases for the invocation.

The actual 23 changed paths comprise 21 Clone methods and two inherent spec
methods. Supported concrete trait functions are the first target, including Clone and
spec methods such as deep_view where their actual typed headers fit. Full
header/session classification remains required; there is no Clone-name filter. Inherent leaf_inv/nonmal_leaf_inv, closures,
synthesized methods and raw impl quantifier labels retain accepted spelling and
may still cause edit-loop misses. All 30 changed exact sessions remain the
measurement scope; no selected subset or source result establishes gain.

Five pure VIR controls and eight actual compiler fixtures now pass on native
Linux, including original paths, raw AIR, complete negative diagnostics and real
producer/native/consumer behavior. The initial five invalid fixture premises stay
recorded as failures. Impossible duplicate declarations are strict original
frontend refusals; duplicate alias fallback is exercised by the pure VIR unit.
The separate standalone reduced fixture remains unrun.

The matched 27-call Vest screen reduces the empty-impl edit median from 45.089 s
to 33.955 s across three rotated rounds (24.693%); solver starts fall from 30 to
24 in every round. Literal edited input churn falls from 30 to 24 sessions. This
is measured candidate evidence, not whole-project acceptance or a selected pin.
Full corpus, source suite, controlled crate/root variation, deterministic output
and macOS qualification remain pending in the
[qualification record](../validation/smt-function-spellings.md). Sail continues
binding exact input bytes; no query normalization is introduced.
