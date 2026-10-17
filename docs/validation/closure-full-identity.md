# Full closure-identity runtime qualification

**Qualified closure-identity runtime, originally selected by merged PR #221.**
The matching platform pins in the [spec-formal runtime change](spec-parameter-names.md)
retain this closure identity and add independently qualified changes. The
evidence below remains bound to its original source/runtime; it is not
relabeled as execution of that successor.
The pin binds fork
`cc7bbce4fdc8cf7e4256668b3ee8431343bdb6b9` to genuine normal Linux/macOS runtime
archives. It retains both halves of Rust's stable crate-qualified closure
identity for equality, hashing and SMT names. No natural collision or wrong
verdict was observed. The combined source also includes the earlier stable
closure naming, imported-closure ordering, proof-note ordering and Cargo-home
source mapping changes under [ADR 0039](../decisions/0039-private-verus-fork.md)
and [ADR 0049](../decisions/0049-stable-closure-names.md).

The measured/qualified Sail source is `56004d81`, with the solver identity
repair already present. The final main composition also contains the separately
qualified fresh-byte digest memo from PR #219
([record](solver-identity.md)). This pin change adds no Rust, Cargo or
conformance comparison changes. Neither qualification is relabeled as a
native run of the composed head. Fork publication and branch/tag updates are
separate lead actions.

| Gate | Actual result and limit |
|---|---|
| Focused native compiler | Pass: two VIR tests, 16 fresh imported-closure clients and ten tiny verification rows; complete raw identities retained |
| Normal Linux/macOS builds | Pass: original normal builder and packer, fresh vstd 2,060 verified / 0 errors; original 14/15 archive members checked |
| Complete fork source suite | Pass: 4,709 Same, 58 ignored, all 4,651 nonignored plain tests pass; original 154 executables and 1,479 compiler source files retained |
| Native workspace and solver | Pass: 657 tests, 35 skipped, two doctests; 35 solver tests |
| End-to-end and PR corpus | Pass: all nine phases plus extra; four PR arms each 33 Same / 311 functions |
| Full FAST | Pass: four arms each 147 Same / 3,450 functions; zero Different, Unstable, infrastructure errors, skips or rlimit drift |
| Full STANDARD | Pass: four arms each 159 Same / 12,726 functions; zero Different, Unstable, infrastructure errors or skips; rlimit drift 2/2/1/2, reported unchanged |
| Default DEEP | Pass: four arms each 159 Same / 12,726 functions; zero Different, Unstable or infrastructure errors, two expected optional Singular skips per arm; rlimit drift 2/1/1/2, reported unchanged. The normal archive does not provide Singular |
| Current FAST determinism | Pass: all 147 current inputs, three runs each; 577 query buckets, zero varying buckets or problems, unchanged committed oracle |
| Matched edit verdicts/diagnostics | Pass: both recipes, all vstd and Vest entries with exact multiplicities and all four unchanged native checker calls; body 1,871 + 3,127 entries, helper 1,871 + 3,128; no verifier diagnostics; literal SMT differences retained without normalization |
| Ordinary Linux delivery | Pass: actual staged bundle, empty-cache smoke and negative proof; distinct from warned runtime-override timing |
| Ordinary Mac delivery | Product components pass: normal install, stage, package, original new/edit/repair smoke and negative proof; enclosing inventory guard FAIL remains visible below |

## Matched native measurements

One quiet native `c7i.8xlarge` (32 vCPU), Ubuntu x86-64, `us-east-1`; three
rotated rounds, identical Sail implementation, default Cargo optimization and
literal source recipes. All 54 rows succeeded. The actors are plain stock,
accepted `73dea021` through Sail, and candidate `cc7bbce4` through Sail using
announced development runtime overrides. These are whole-command measurements,
not ordinary unchanged-receipt reruns or a default bundled-CLI benchmark.

| Recipe and phase | Plain stock median | Accepted 73 median | cc7 median | cc7 versus 73 |
|---|---:|---:|---:|---:|
| Body recipe cold | 64.381 s | 71.170 s | 71.841 s | 0.9% slower |
| Body edit | 35.074 s | 25.130 s | 25.437 s | 1.2% slower |
| Body restored + comment | 34.935 s | 24.820 s | 25.232 s | 1.7% slower |
| Helper recipe cold | 64.355 s | 71.351 s | 72.019 s | 0.9% slower |
| Helper insertion | 35.160 s | 34.682 s | 25.107 s | **27.6% lower time (1.381x)** |
| Helper restored + comment | 35.148 s | 24.953 s | 25.085 s | 0.5% slower |

Helper-edit accepted samples are 34.866/34.682/34.554 s; candidate samples are
25.079/25.297/25.107 s. Each paired round improves (1.390/1.371/1.376x).
The harness's `noopend` phase restores pristine source and appends its fixed
comment, forcing frontend work. It does not measure ordinary unchanged reuse.
The cold/body results do not establish a speedup. This is a narrow closure-edit
win, not completion of the entire three-project edit catalogue, a repair 2x
claim or an all-cache-hit claim. Earlier 8a results do not qualify cc7.

The original timing run remains enclosing **FAIL**: after all 54 rows passed,
the untimed matrix refused an unused deleted-temp-worktree assertion before
compilation. The prospective remainder consumes immutable captured cold/edited
sources and removes only that assertion. The first remaining-only run then reached all three body compilers (each
exit0), but its reader demanded the wrong managed-project schema from valid
flat reports. That enclosing FAIL also remains. Actual body raw outputs are
retained for comparison without compiler repetition. The subsequent scoped matrix passes on the actual flat reports, comparing
all original crates/functions/diagnostics with all stock documents. Its enclosing
run still FAILS at determinism startup before any sample because a legacy
retention namespace omitted a subprocess constant; no oracle check is waived. Runtime, source, graph, lock,
restoration, binary and literal raw-output observations remain retained.

## Scope and evidence

Both archives use upstream `426d8b01`, Rust `1.98.1` and source lock
`80c71fdd00ef5073cd93edf6b5e37a635f7b4114fba8f6553ba2125eecb1395e`.
Normal runtime manifests remain distinct from the complete source-suite
manifest; no missing test files or installable variants are synthesized.

Raw records live outside the repository, identified here by SHA-256:

- Focused compiler final: `18810f6898fc2108854d05430126bf47111b0ce9a3d3267b10aade6959839ed9`.
- Normal Linux archive: `717680b4cb126f48397b02f32dcbf54e0f2a320fa0c16c86a97237a8c3eee8b1`;
  manifest `8fd9ffb1ed9fe5bd82506d0b1b45571642c4886ad256b228b4bcd54b3f0db550`.
- Normal Mac archive: `65523848b990f49221035a087e685fe9666e65d34cb5b9d1b76f25f690f86d9b`;
  manifest `c35e88e0293bc6afbedddcd25d3750bcbac8202fd42d26108e6d9d3497cd445c`.
- Complete suite: `4f31e2dcff721eb2a274e21052f59c6f5689be3bb2f827ec323bee288f35c82a`;
  1,674 files / 643,946,925 uncompressed bytes, including all 154 executables.
- Full fork suite final: `aa558a7df2a0e0f248adc545b2f8ebb65b22c893a48221d7d459eafa0b3c2755`.
- Native gate/E2E/PR/FAST final: `040249dc37d61aa50afd872c6514f91f6a8df862cf011e3724e0cef5cf2bf97e`.
- STANDARD final: `0e2a869c74cc5bf95790c39c8fe8f2fad61e45daff1571721db6679ad73fae31`.
- Default DEEP final: `96d3755ec048cd55e326d46c143695bdaf73d8a12432c45024c778cbdffe430a`.
- Current FAST determinism final: `d6a79d0939d4fb1f8791434c1619270e634805d679c8d2279a9dc1fb0ae8a801`;
  All 441 literal stdout/stderr/status records are retained. Exact current FAST
  selection and final source/runtime/binary guards pass. Solver-input bytes
  match exactly; only the unchanged oracle's existing timing-field blanking
  applies to verifier-output comparisons.
- Passed two-recipe matrix: `c47e8fb8c00deee5c3c02fdae0d351d2c202a4bf297136b008de9a05cd26e774`;
  enclosing determinism-startup failure: `6ee105910f016dbfe710ed6f33857a24887dc3c111de853be9c4ad76d3e0e7fd`.
- All 54 rows and original failed matrix: `4dc553860d06d4d9744f29e31a9d56b1171fa45f269ec742dacf10e95765bb77`.

Mac ordinary delivery retains every original installed file unchanged but adds
57 crate-download archives (4,292,181 bytes) to the observed global cache.
The writer is unknown. Its enclosing inventory guard stays FAIL, despite the
passing build/install/stage/smoke/negative-proof components and exact source,
original runtime and toolchain inventories. Difference record:
`e56168e1ef7ab92fe1eb834be8f9f389c452d8c6c9ada1b398eeef7d0a2db613`.
There is no retry to obtain a quiet global cache.

Failed/interrupted enclosing attempts, all 2026-10-17, remain indexed:

| Source/runtime | Result and cause | Evidence SHA-256 |
|---|---|---|
| d422 / cc7 | Full suite startup FAIL: explicit stock-toolchain path omitted; normal build and suite capture passed | `27ef509df5dbbbf0f11c901b75e879528e923e97b7f362c073a8488d1ffef741` |
| d422 / cc7 | Retained restore FAIL: regular-file archive omitted Git's required empty refs directory | `69bebeca910d0bd2b4d5ad42f7e615f2594c9ca928a26b718f34796807cae33b` |
| d422 / cc7 | Full suite command passed; enclosing source-set FAIL on 17 generated fixture lockfiles; later run retained them before authorized exact cleanup and unchanged final guard | `eab8bd818f1279c2b881b04c26eb8743a964c2f948f5059f7d64e98b2fcb4999` |
| 56004 / cc7 | Default native FAIL: two workspace fixtures inherited global runtime overrides; corrected cohort routes them independently, assertions unchanged | `5de6311d61e1eb54d995afd6923b5095d0f2d8ec1b03e4333cd08171a3e52343` |
| 56004 / cc7 | Mac ordinary delivery FAIL: private accounting ENOENT, cause unknown | `360ec70ef9781495555bacf13c5ffe0cc867a57b57df01b84da7d6d4c0bab3d5` (result JSON) |
| 56004 / cc7 | Mac component checks pass; enclosing global-inventory FAIL on 57 added crate archives, writer unknown | `f168002d0a146658329a12004f2bddffceaf4e20729f50c3d6136c91b9674ce0` (result JSON) |
| 56004 / cc7 | All 54 timing rows pass; enclosing matrix startup FAIL before compilation | `4dc553860d06d4d9744f29e31a9d56b1171fa45f269ec742dacf10e95765bb77` |
| 56004 / cc7 | Body compilers all exit0; enclosing reader FAIL on flat Report versus ProjectResult schema assumption; helper/determinism not run | `d7391873eedd03e7068abd6ccf5b14d789b6cfc656d5d132da9186e8bca66f3e` |
| 56004 / cc7 | Complete two-recipe matrix passes; enclosing determinism startup FAIL before any sample (legacy subprocess namespace omitted DEVNULL) | `6ee105910f016dbfe710ed6f33857a24887dc3c111de853be9c4ad76d3e0e7fd` |
