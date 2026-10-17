# VI-0023: Spec formal spelling changes untouched callers' solver inputs

- **Status:** reproduced in historical captures; candidate normal runtime/vstd and eight
  focused native controls pass. A targeted Capy screen observes a 24.61% rename
  saving; the Capy matrix passes. Both final combined platform runtimes are built; native
  broader qualification remains pending. No upstream issue filed.
- **Found:** 2026-10-17 while assessing the residual #118 incidental-edit scope.
- **Original runtime:** fork `13a1176c`, native c7i.4xlarge / 16 vCPU,
  us-east-1; this is not a current cc7 measurement.
- **Reproduction:** `corpus/repros/verus-spec-parameter-renaming/` (new reduced
  sources executed on genuine cc7); fork regression `sail_vi0023_spec_parameter_names`.
- **Decision:** [ADR 0054](../decisions/0054-stable-spec-parameter-names.md), proposed.

A spec formal rename changes binder declarations and references in its defining
axiom, even when the importing caller is unchanged. CapybaraKV's
`common.subrange_v.seqs_match_except_in_range` changes `s1!` to `s1_renamed!`;
Anvil's `trusted.liveness_theorem.valid_owned_vrs` changes `vrs!` to
`vrs_renamed!`. This is source-edit sensitivity, not demonstrated same-source
nondeterminism.

The corrected historical captures are distinct from earlier `spec-wide` traces
that also inserted `sail_perf_true`. The latter do not isolate formal renaming.
The corrected CapybaraKV table has 111 logs / 732 checks, three unchanged repeats
with zero differences, and 26 changed buckets / 208 checks after divergence.
The corrected Anvil table has 45 logs / 144 checks, three unchanged repeats with
zero differences, and 39 changed buckets / 135 checks after divergence. These
figures characterize byte divergence; they do not establish misses, solver work
or current-runtime wall savings. The historical report's coarse timing is not
attributed to this isolated edit without matching source provenance.

The retained outer archive is SHA-256
`0e697b2e07001a7b4f572386267aab94b3dbb3db1facc1f0ed48097e0bfe5450`.
Its corrected tables/diffs are under
`l40b/capybarakv-det-fork-spec-wide-fail-fix/det/` and
`l40/anvil-det-fork-spec-wide/det/`. Full solver streams were not retained in
those historical records; the new native screen must retain them.

The [current native screen](../validation/spec-parameter-names.md) retains all
18 Capy timing rows and the enclosing matrix setup failure separately.

The candidate changes only spec formal lowering, preserving source names/spans
and the existing scope-aware rewrite. It must first prove literal caller-input
stability and unchanged native verdicts/diagnostics, then demonstrate a useful
paired gain before any wider qualification. No Sail workaround or normalization
is selected. The reduced positive, shadowing, recursive and generic-default
fixtures are not claimed to pass until executed with the actual compiler.

| Date/source | Failed or interrupted gate | Raw SHA-256 |
|---|---|---|
| 2026-10-17, fd26 / Sail4b32 | Native baseline preflight FAIL: the first reduced source lacks module-local imports and reports six E0425 errors; candidate build, eight new controls and timing rows UNRUN. Final external-input inventory also fails on numeric permission-mode parsing. Prospective fixture-import and typed-mode corrections preserve every assertion; no result is reclassified. | `2d0db249de21ecab81ce765f0a22df570ad8a95ef31bcb323122ae1e9dc75c6d` |
| 2026-10-17, 60ead1ae / Sail3e928d93 | Native baseline recursive positive-2-before FAIL: the unchanged assertion needs explicit reveal fuel. Candidate build and timing rows UNRUN. The prospective actual-Rust fixture adds reveal_with_fuel(value,2); all ten derived sources pass expected outcomes on genuine cc7 macOS, with original runtime bytes/modes unchanged. This is fixture correctness only. | `20c18c522a35ba04aef985a355694b79cfebabfacdfe23a865c3c5a3def7cf16` |
| 2026-10-17, 0a691d04 / Sail1d3af06d | Native baseline ten-source preflight and Sail release build PASS; checker-example build FAIL because its package is sail-cli, not sail-conformance. Candidate Verus build, eight native regressions and timings UNRUN. Prospective package-only command correction preserves source, assertions and caps. | `cd3439cdc3bd8b8bc18b454501505660106cd4999caa770627f22cd290c00240` |
| 2026-10-17, 0a691d04 / Sail1d3af06d, corrected checker command | Ten native baseline sources and Sail/checker builds PASS; genuine candidate vstd compilation FAIL with `unknown variable: VarIdent("n", VirParam)` at `poly.rs:466`. No candidate runtime, eight native regression controls or timings qualified. The source audit identifies the spec/decreases-proof shared parameter scope; the prospective fix and reduced via fixture still require candidate-native execution. | `74765136ff9b5ff67ba1ef9ce6917060b5af21691365df4f1d0ccef880944030` |

| 2026-10-17, aead21d2 / Sail 4ce1f00e | Genuine normal candidate runtime/vstd and all three VIR plus five new RVT controls PASS, including the via regression. First Capy timing harness FAILS before any sample: its intentional selector cleanup exposes an absent default stock install (SAIL3003). Source/runtime final guards pass. A prospective remaining screen reuses this actual packed runtime and original test proof, installs stock at the real default cache path before the unchanged harness, and preserves its corpus across containers. No gain or whole-corpus acceptance. | `1b9518aa334eb558f2cb7e510d5f2f7318807a0e56d6e8a4ba5e21d951f24816` |

| 2026-10-17, aead21d2 / Sail 4ce1f00e, retained runtime | Stock-default setup and fresh Sail build PASS. Capy baseline cold67.57s and spec-wide56.97s exit0; observer FAILS before noopend because it expects the appended comment in the rename file rather than the committed recipe’s separate noop file. No candidate timings or gain. Runtime/source/Sail final guards PASS; prospective observer correction preserves every command and is exercised against both complete committed project edit/reset sequences. | `b0e6972da39dcec22e4277c5eab933bc001f213205b456f3a21f299b047446d5` |

| 2026-10-17, aead21d2 / Sail 4ce1f00e, exact recipe observations | All 18 Capy timing rows PASS; rename median 51.990→39.194s (24.61%), solver processes 25→0. Enclosing plain matrix FAILS before verification because it omits the exact timed `--no-default-features` and enters unavailable libclang/PMEM setup. Anvil UNRUN; source/runtime/Sail final guards PASS. No correctness or selection claim. | `00b4e545740d053f37cc3a1a7b0619444938c4a46e099104081b023efcc9192d` |

The compiler panic exposes an internal pairing requirement: vstd's
`EndianNat::to_big(n)` uses `via Self::to_big_decreases`, whose special proof
body is lowered in the spec function's parameter scope. Both must retain the
same positional identities. These `#[via_fn]` helpers cannot be called as ordinary
proof functions. The prospective correction also positions their corresponding
formals; ordinary proof/exec formals remain unchanged. The reduced
[`termination-via.rs`](../../corpus/repros/verus-spec-parameter-renaming/termination-via.rs)
passes on genuine cc7 macOS, including its explicit fuel and termination proof;
the corrected candidate also passes the native via regression. All ten derived
baseline sources pass expected outcomes on genuine cc7 macOS, with unchanged
runtime bytes/modes. This is fixture correctness, not candidate qualification.
