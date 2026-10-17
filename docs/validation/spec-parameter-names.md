# Spec formal parameter naming screen

Status: useful targeted timing observation and Capy correctness matrix pass;
Anvil, broader qualification and selection remain pending.
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
