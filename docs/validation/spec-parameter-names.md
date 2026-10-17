# Spec formal parameter naming screen

Status: both combined-runtime platform archives built; Mac component checks pass.
Final native performance, full-suite, corpus and determinism qualification are running.
The candidate is not selected or merged.
[ADR 0054](../decisions/0054-stable-spec-parameter-names.md) remains
proposed. No selected runtime, default or Sail query handling changes.

The 2026-10-17 native c7i.4xlarge screen (16 vCPU, us-east-1) compared genuine
`cc7bbce4` with candidate `aead21d2`, using the same Sail `4ce1f00e` code and
pinned Rust 1.98.1/Z3. The candidate's genuine normal runtime/vstd build and
three VIR plus five RVT regressions passed in the earlier retained supplier.
This screen admitted those exact packed bytes and original test evidence; it
rebuilt only Sail. Profiling and Cargo optimization overrides were absent.

All 18 CapybaraKV rows passed, with three rotated paired rounds. The edit is
only the committed `spec-wide` formal rename; the `noopend` control appends a
comment and forces frontend execution. It is not an unchanged Cargo rerun.

| Command scope | cc7 seconds, all samples | Candidate seconds, all samples | Median cc7 → candidate |
|---|---|---|---|
| Cold | 62.163, 61.278, 61.309 | 61.066, 60.850, 61.017 | 61.309 → 61.017 |
| Spec formal rename | 53.157, 51.873, 51.990 | 39.208, 39.130, 39.194 | 51.990 → 39.194 |
| Comment forced frontend | 40.682, 39.461, 39.437 | 39.090, 39.190, 39.761 | 39.461 → 39.190 |

The rename median improves by 24.61%; every candidate edit sample is below
every baseline edit sample. Recorded solver processes fall from 25 to zero on
each rename; both actors start 407 on each cold command. These are scoped
observations, not a correctness or whole-corpus acceptance. Every command's raw
output, recipe sources/manifests, macro/library origins and runtime hashes is
retained. Timing receipt SHA-256:
`615893e7407a1c688b02beedd199f7fadf559b1443e3265e8fcbaee6542cbc91`.

The enclosing attempt remains **FAIL**: its subsequent plain correctness matrix
omits the timed recipe's `--no-default-features`, enables Capy's default PMEM
feature and exits 101 in bindgen before verification because libclang is absent.
Anvil and the remaining matrix actors are unrun. This is not a candidate compiler
failure. Final Sail/source/input guards and the runtime comparisons bracketing
the timings pass; the later workload-guard stage is unrun. Final receipt SHA-256:
`00b4e545740d053f37cc3a1a7b0619444938c4a46e099104081b023efcc9192d`.
The prospective reader correction uses the exact timed Cargo arguments; it does
not change the recipe, stock oracle, comparator or expected verdicts. No timing
row is rerun or substituted by that correction.

The corrected follow-on used Sail `6af7d10f` on main `2936ead2`, with the same
genuine runtime bytes and captured Capy rename inputs. No Capy timing row was
repeated. On c7i.8xlarge (32 vCPU, us-east-1), both cc7 and the candidate passed
the unchanged whole-document/function checker against stock: vstd's 1,871 and
Capy's 675 function records were all paired, with no verifier diagnostics.
Literal query files and multiplicities are retained; cross-runtime query
identity is explicitly false, not normalized or used as an equality claim.
Capy matrix receipt SHA-256:
`88b9aec1710c340c4a91759f77c09fa1c3e012fd2f99a910c71de4c18c211df5`.

That enclosing attempt also remains **FAIL**. Its original Anvil recipe sent
the module selector through non-JSON `sail verify`, which deliberately preserves
plain Cargo-Verus's refusal of partial verification under `verify`. All 18 rows
were retained with exit 1 and zero solver queries; no Anvil compiler measurement
is valid. The Anvil matrix and all four full FAST arms are unrun. This is a
command-entry failure, not evidence of a candidate compiler defect. Final
Sail/source/input identity checks pass. Final receipt SHA-256:
`26727bfc73b328093da236ae73c2205bb8f553843b1ca2e9596a2c648fa9963a`.
The prospective recipe correction must use the supported public JSON path
without changing the module/workload selection or checker.

Remaining selection gates include Anvil, literal edited-versus-original caller
inputs, counterexample/diagnostic controls and the normal required corpus,
determinism and matched delivery qualification. Cross-runtime solver bytes may
change under the proposed binder renaming; raw differences remain visible.
Earlier failed attempts are indexed in
[VI-0023](../verus-issues/VI-0023-spec-parameter-names.md).


## Combined runtime delivery candidate

The final candidate is fork `e5365c05d0a9f8c8cb655edc035ce9a2f6dde482`.
It preserves the original binder change and native compiler-observation commits
in its ancestry, and adds distinct scratch names for the five binder tests.
The compiler observations remain request-driven; this pin does not enable the
experimental native-artifact store or retained-content reuse. The selected v1
store quarantine remains in force.

| Gate | Actual scope and current result |
|---|---|
| Normal Linux producer | PASS: genuine normal runtime and release vstd, original measured build duration, 13 runtime files, unchanged packer, actual public Rust engine admission before corpus. No source-suite relabeling |
| Normal Mac producer | PASS: genuine normal runtime, 17 compiler library tests, six VIR tests, five binder regressions with four workers, and release vstd (2,060 verified, zero errors) |
| Mac artifact admission | PASS: unchanged archive installer followed by actual `sail engine status --output json`, full normal Manifest including measured build duration, exact fork/source-lock and all 14 runtime files |
| Mac public edit loop | PASS: public default engine discovery without runtime overrides, passing and unchanged reuse, failing assertion with Cargo exit 101, then fixed proof. Every call returns Current and 1,872 function records; successful maps agree |
| Full Mac host check | PASS: all-target clippy and 655 tests, 35 solver tests skipped by the host profile; coherent Linux/Mac delivery pins, no runtime development override |
| Final native qualification | Running on native c7i.8xlarge, us-east-1: fresh paired Capy/Anvil timings and matrices, solver, all end-to-end phases plus extra, four corpus arms, full fork source suite and determinism. No pending gate is inferred from earlier component evidence |
| Selection | Pending full qualification, review and current required PR checks |

The exact archive SHA-256 values are:

- Linux: `689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928`.
- macOS: `51d2a9cf4c46ab48447f6c5944e18228b7a20538e063e57fca93399d73eb2c5d`.

Linux normal-producer admission record SHA-256 is recorded with the final native
qualification receipt. The Mac component index SHA-256 is
`f626a287a1e6a927b04e02f29692944c739c48fe95022a67346fc406cbdc632d`.
The index binds the actual Sail `0cec0568` binaries, correct-root source guards,
original full Manifest, tests, installer and public CLI rows. Mac checks are
correctness checks on the local Apple Silicon host; no Mac performance result
or whole-corpus acceptance is claimed.

Additional failed or superseded attempts stay visible:

| Date/source | Result and cause | Raw index |
|---|---|---|
| 2026-10-17, aead Mac | Parallel binder controls FAIL because test cases shared a scratch directory; unchanged serial controls pass. Candidate test-only fix assigns distinct names and passes both parallel controls; final combined Mac rerun also passes | Included in Mac component index above |
| 2026-10-17, aead Mac | Minimally repacked runtime FAILS actual Rust admission: required `build_seconds` omitted. Python installation alone did not qualify it. Final combined runtime is a genuine measured build with the complete normal Manifest | Included in Mac component index above |
| 2026-10-17, e536 / Sail 0cec | Initial CLI smoke reader FAILS by expecting Verus exit 1 rather than Cargo exit 101 for a false assertion. The corrected prospective reader follows the existing managed test's Cargo contract and retains the actual failed verdict; no product check changes | Included in Mac component index above |

No component result establishes the final real-project gain or closes the
broader incidental-edit catalogue. Complete raw output and earlier failed
receipts remain outside the repository, addressed by the hashes above.

Full Mac host receipt SHA-256: `48b56acaddb1b97d88ee371c0c7a297038507639c5677d75466a8a9c6be4f2db`. The first invocation
failed two fixture-discovery tests because the runner set a global development
runtime override. The complete unchanged host command passes with that override
absent; the failed log remains indexed in this receipt. No test was changed.

Corrected default-discovery acceptance SHA-256: `ebd748b914bcb5b5ad70b6f75cb2aa96d4cb51bd047e4c0e51c241e32a88d6d3`.
The Mac installer now uses the exact feature-derived directory that Rust
discovers, `426d8b01e7ffb910a36c15e1f870ff985bb61eef-d5d82a680cfe73f5`.
An archive-derived directory in the superseded delivery commit `ff569dab`
passed explicit-override admission but refused actual default discovery.
The corrected installer, public status and complete edit loop pass without
engine/source/compiler overrides. A new Rust contract test binds both platform
pins to default discovery; the complete host rerun passes 655 tests. Runtime
archive bodies and the failed discovery evidence remain unchanged.
