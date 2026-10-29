# Spec formal parameter naming screen

Status: qualified on the recorded source and runtime scopes. Both platform
archives, native performance, full source suite, gate, solver, end-to-end, FAST,
Anvil stock comparison, three-run FAST determinism, all four DEEP modes and
final source/runtime guards pass. This change selects the matching platform pins
under [ADR 0054](../decisions/0054-stable-spec-parameter-names.md); current
required PR checks still gate its merge. Sail query handling is unchanged.

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

The corrected follow-on used Sail `6af7d10f` on main `ee6914c7`, with the same
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

At that earlier stage, selection still required Anvil, literal
edited-versus-original caller inputs, counterexample/diagnostic controls and
the required corpus, determinism and matched delivery qualification. The final
combined-runtime record below supplies the completed gates. Cross-runtime solver bytes may
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
| Native delivery-source gate | PASS: complete `ci/run.sh gate` on exact Sail `070f4764`, 666 tests and two doctests, 35 solver tests skipped; includes both coherent platform pins and the default-discovery contract test. The separate fresh release build binds all six actual command/checker bodies, including the MCP checker |
| Native solver tests | PASS: all 35 solver tests on exact delivery source and the matched runtime |
| Native end-to-end | PASS: all phases plus local extra on exact delivery source, including managed refusal/mutation controls, MCP tool listing and real repair CLI/MCP; unchanged acceptance checks |
| Full FAST corpus | PASS: four modes, each 147 Same / 3,450 functions; zero Different, Unstable, infrastructure errors or skips. Each mode records 12 units with resource-counter drift, retained under the unchanged comparator |
| Final native performance | All 36 Capy/Anvil commands pass with complete input/restoration observations on an isolated c7i.8xlarge (32 vCPU), us-east-1. Rename medians fall 22.63% / 59.49%; cold and comment controls are nearly unchanged. Both stock comparisons now pass; the earlier Anvil time-budget refusal remains indexed |
| Full fork source suite | Observed 4,714 Same, 58 ignored, zero Different, Unstable or infrastructure errors. Final normal runtime, six command/checker bodies and tracked source guards pass. The enclosing run stops at the post-test fixture inventory: 199 fully captured extra files; all original bytes/modes remain valid. Independent complete final inventory audit passes; the original enclosing failure remains indexed below |
| Determinism | PASS: all 147 FAST units run three times (441 executions), 577 query buckets and no varying buckets or failures; actual outcomes and proof counts match the retained FAST reference |
| DEEP corpus | PASS: all four modes, each 159 Same / 12,726 functions, zero Different, Unstable or infrastructure errors; 18 resource-counter drifts. Exactly the two documented optional Singular projects are skipped; all active unit sets match STANDARD |
| Final guards | PASS: 3,886 runtime/stock files, all six actual command/checker bodies, both 601-file Sail source maps and the 1,482-file fork map remain exact; no source changes or unknown files |
| Selection | Qualification and independent review complete; matching platform pins selected by this change, subject to current required PR checks |

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

The native delivery gate observation SHA-256 is
`a1eb66fbc17d9357e182d27401ffd98719e24084e875616adfa7dbbda800414f`.
Fresh six-body build and actual default public engine discovery record SHA-256:
`d038ef5a20d6b655c2a1c3454480bea784aa38c1565acf117fb1ba955fd54dc8`.
The discovery command uses the installed, feature-derived directory with no
build, source or Cargo-Verus override. It validates the original complete
normal Manifest; it does not build Verus or qualify the pending corpus gates.
The complete end-to-end archive SHA-256 is
`702c2fd06606e9cf3ac527fed5e8d639dd5c924b87f5614c3338edf9b0cc6568`.
The full FAST summary and four strictly preserved arm-archive identities have
SHA-256 `1a0513bd7838b1b62c44bafa625d7ae4ce40bc02ed7c34d4e0160fe6b0508c5e`.

The fresh combined-runtime Capy screen now passes all 18 commands, 24 input
snapshots and six restoration snapshots. On the isolated c7i.8xlarge, three
rotated rename samples are 54.199/54.174/54.604 s for cc7 and
41.764/41.943/41.933 s for e536: medians 54.199 → 41.933 s, 22.63% lower.
Cold medians are 62.712 → 62.482 s; forced comment medians are
41.812 → 41.719 s. These measurements use Sail `0cec0568`; the delivery gate
above uses `070f4764`. The Capy stock comparison also passes: both fork
actors match all 1,871 vstd and 675 Capy function records and diagnostics under
the unchanged checker. The captured query multisets differ from stock (111
stock queries versus 538 for each fork); no SMT-byte equality is claimed.
Its strict archive SHA-256 is
`a222507237162d92a7ead4c8e6bc3067dbba40c81db12f5dc7fc20b01ccdd027`.
The Anvil timing screen and its later stock comparison below pass.
This screen does not select the runtime. Its archive SHA-256 is
`54df09d75c9ad3dec9d38d8742982c01b516218cf262f4d8f3e21557054191dd`.

The Anvil screen also passes all 18 commands, 24 input snapshots and six
restorations. Three rotated rename samples are 110.642/108.402/108.648 s for
cc7 and 43.888/44.017/44.017 s for e536: medians 108.648 → 44.017 s,
59.49% lower. Cold medians are 133.903 → 133.065 s; forced-comment medians
43.791 → 43.959 s (0.38% slower). Its archive SHA-256 is
`91c36048abd5b9d966677a6db7a5e20efedee52d4e95fcf2c05b81a901c1a9af`.
The unchanged 900-second Anvil stock-comparison cap plus 300-second reserve
requires 1,200 seconds remaining. Only 1,109.636 remain, so the controller
refuses before starting any matrix command. All final source, runtime/MCP and
five command/checker-body guards pass. This is an enclosing DEFER, not a
passed stock comparison. The later continuation below supplies that component;
the original deferred attempt remains deferred.

The purpose-specific continuation subsequently passes the actual Anvil matrix
on native c7i.4xlarge (16 vCPU), us-east-1. Stock `426d8b01`, current fork
`cc7bbce4` and candidate `e5365c05` all exit zero. Both unchanged output checkers
agree on all 143 functions, 144 proved backend checks and all 45 scope notes
for `vdeployment_controller`. The report correctly remains partial; this is
not whole-project acceptance. Every actor retains 45 literal SMT files and
144 checks; cross-runtime query bytes differ and are not normalized.
Matrix fragility flags are zero in each actor; the six flags in every earlier
timing row remain recorded independently. This component's archive SHA-256 is
`15ecb4f2c1742bd6cea266b157bdfa52cab1b6bde58d827865d0a7e410deda98`.
The same continuation subsequently passes FAST determinism on all 147 units
across three fresh runs: 441 executions, 577 query buckets and no varying
buckets or failures. Actual exit outcomes and proof counts match the retained
FAST reference on the same normal runtime. Its strict archive SHA-256 is
`ac8d4d4773fd4e5a432f014f656b9b4f973d936b25c38d9de48ca70203b3a33c`.
DEEP pass-through subsequently matches all 159 active units and 12,726
functions, with zero verdict/diagnostic differences, Unstable classifications
or infrastructure errors. Its 18 resource-counter drifts remain visible.
The active unit set exactly matches STANDARD. Only the two documented optional
Singular projects (`verus-integer-ring` and `verified-memory-allocator`) are
skipped; no other unit is omitted. Its strict archive SHA-256 is
`09ae108d160cb733a7f47d4b9fa56f965a4bb54d28892656750a67cba0ccc461`.
DEEP cache-fill also matches all 159 active units and 12,726 functions, with
the same two optional skips, 18 resource-counter drifts and zero Different,
Unstable or infrastructure errors. Its archive SHA-256 is
`f5938438e0b1d2d83fa3b5645a0d3728bf8ec9024051c47e332ceea85db94c52`.
DEEP cache-warm also passes the same 159-unit / 12,726-function comparison,
with the same two optional skips, 18 resource-counter drifts and no
disagreements or infrastructure errors. Its strict archive SHA-256 is
`921bffbbd4f1a6b982453cf825705f3baeed99db0626654d18dcca356239157e`.
DEEP engine also passes the same 159-unit / 12,726-function comparison,
including 158 focused units, with identical optional skips and drift counts.
Its strict archive SHA-256 is
`c1942fcb30b9d558f6df9a29ca7ddb17c459ae7898307916e2a7a3b56ac02cbb`.
The aggregate four-mode archive SHA-256 is
`1c4a57cd629b0242442960892ed4dbb122c98ed71c34fc789c8b1bdbf31549f7`.
Final runtime and MCP admission, all six command/checker bodies, complete Sail
and fork source maps and workload guards pass without changes or unknown files.
The continuation completes all 18 steps successfully; its final archive SHA-256
is `4fd75b56b0f33159951bb059f115a94c7a08fbbf61cbe3532a7c5b04b309d43d`.
This completes the remaining qualification components without relabeling any
earlier failed or deferred attempt.

The independent complete final source-suite inventory audit has SHA-256
`ad579bb3ae17c2f08b95ddb3b5a9d1db93fe28b858d39c9ea5fc7683a26901d4`.
It checks the retained raw bytes, all 1,678 original members unchanged, the
complete 180-file source-copy set against the sealed fork tree, all 17 initially
absent fixture lockfiles against their unchanged manifests, and both Cargo
metadata files against their actual producer scope. No file is deleted, no
comparison is rerun or relaxed, and no unknown extra remains. This completes
the source-suite component's final input binding; the original enclosing
cohort remains failed. The producer's partial archive index explains why normal
ignored build outputs were classified as extra. Future complete inventories
include those build outputs prospectively. The later Anvil comparison and
determinism and all four DEEP components above pass on their recorded scopes.

Additional failed or superseded attempts stay visible:

| Date/source | Result and cause | Raw index |
|---|---|---|
| 2026-10-17, aead Mac | Parallel binder controls FAIL because test cases shared a scratch directory; unchanged serial controls pass. Candidate test-only fix assigns distinct names and passes both parallel controls; final combined Mac rerun also passes | Included in Mac component index above |
| 2026-10-17, aead Mac | Minimally repacked runtime FAILS actual Rust admission: required `build_seconds` omitted. Python installation alone did not qualify it. Final combined runtime is a genuine measured build with the complete normal Manifest | Included in Mac component index above |
| 2026-10-17, e536 / Sail 0cec | Initial CLI smoke reader FAILS by expecting Verus exit 1 rather than Cargo exit 101 for a false assertion. The corrected prospective reader follows the existing managed test's Cargo contract and retains the actual failed verdict; no product check changes | Included in Mac component index above |
| 2026-10-17, e536 / Sail 0cec | Native combined attempt FAIL: all 18 Capy timing commands exit zero, but the controller never invokes the required per-command input observer. No command snapshots exist, so these timings do not qualify the gain. Anvil, both matrices and wider qualification are unrun. Both recipes are repeated prospectively with the actual observed call chain | `f49b40b1c490f560b184efe0feaddbf8bdccebfb2a760b44a2d88caab726fbb4` |
| 2026-10-17, e536 / Sail 070f | Enclosing native correctness cohort FAIL after all 4,714 source-suite comparisons pass: post-test inventory finds 199 extra files (3,128,128 bytes), all fully captured. Original engine/suite bytes and modes, final runtime/MCP, six command/checker bodies and tracked source guards pass. Independent read-only audit accounts for every extra: 180 exact complete fork source copies, 17 source-proven generated fixture locks and two Cargo metadata files. The producer archive index omitted normal ignored build outputs; the old cohort remains FAIL. Determinism and DEEP are unrun | `b0fd35b4aee251a1f5600a9434878ec88728b9bdd689d7b28fc1d0c2a4c1d5a6` |
| 2026-10-17, e536 / Sail 070f, remaining-check continuation | FAIL before Anvil stock comparison, determinism or DEEP: the file-only restoration of the already-tested source suite omits its required empty Git refs directory, so Git refuses it with exit 128. Final normal-runtime, six-body and source guards pass. That suite is not consumed by any remaining comparison; the next continuation carries only their actual inputs. The separately accepted 4,714-case result and original failures retain their scopes | `1283b4d83c8db025aed796775b45b56bd9dbf4648dbdb5994d880e6efdc1c898` |
| 2026-10-17, e536 / Sail 070f, remaining consumers | FAIL at the actual stock Anvil command before verification: Cargo-Verus refuses relevant `--locked` after irrelevant `--lib`. Normal-runtime, six-body, restored-reader and final source guards pass; Anvil comparison, determinism and DEEP remain unrun. The successor preserves the same flags and checks, ordering `--locked` first; six actual stock/fork parser controls reach metadata as expected | `cc3950995dd97ad7c67fb1efc796762cdf0c0c28ef6a6cd8b0a37f215027f110` |

The paired measurements and completed correctness gates qualify the scoped
runtime selection above. They do not close the broader
incidental-edit catalogue. Complete raw output and earlier failed
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
