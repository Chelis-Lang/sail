# Mutation corpus pilot: FPBench (ADR 0054)

**Status:** pilot complete on four FPBench catalogues. Sail matched plain
Verus on every unit, function and mutant, with no false verified. The
catalogues are generated outside this repository and are not yet pinned in
`corpus/corpus.toml` (open questions 32 and 33). `sail-bench mutants` and
`cargo xtask gen-mutants` are not implemented; runs used `sail-bench edit`
with a refill per mutant.

## Source, runtime and environment

- Base: FPBench `d7ade34e` (MIT), translated to Verus by a generator outside
  this repository, commit `7724de45d5df7552cb804cb2af81213b5f5edfa7` (phase
  1 f64 crates unchanged since `39a7f7b6`). Each catalogue is in the
  `corpus/edits.toml` format with a metadata sidecar.
- Sail `c848259` with fork runtime `73dea021`; plain Verus is the pinned
  release `0.2026.10.04.426d8b0`, Z3 4.16.0, in the CI image.
- Native EC2 c7i.4xlarge, us-east-1, one host per lane (ADR 0038). The
  project was added to a host-only copy of `corpus/corpus.toml` holding only
  it, one layout at a time (`crate_roots_dir` `f64`, `f64_split`, `int`,
  `int_split`); the catalogue was copied over `corpus/edits.toml`. Neither
  change is committed.
- Commands: `ci/local.sh conformance deep --project fpbench-verus`, then
  `ci/local.sh bench cold|warm|edit fast 3`.

## Catalogues and Linux expected outcomes

Expected outcomes were re-measured on the reference platform: stock Verus,
three runs per mutant and per unmodified crate, 900 s cap. No outcome varied
across runs and none reached the cap.

| Catalogue | Layout | Mutants | Linux errors / verified | Differs from macOS arm64 |
|---|---|---|---|---|
| `mutants.toml` | `f64`, one module per crate | 115 | 86 / 29 | 5 |
| `mutants_split.toml` | `f64_split`, one module per kernel | 115 | 77 / 38 | 1 |
| `mutants_int.toml` | `int`, one module per crate | 75 | 65 / 10 | 0 |
| `mutants_int_split.toml` | `int_split`, one module per kernel | 74 | 64 / 10 | 0 |

- Classes: `arith`, `constant`, `spec-weaken`, `spec-strengthen`,
  `rare-region`. The int translation proves interval bounds on 20 kernels
  with `nonlinear_arith`; 7 lemmas that never finish are excluded from every
  crate, and 4 that end with an rlimit error sit in `daisy_slow`, which has
  no mutants.
- Unmodified `f64/fptaylor_extra` fails on Linux at `delta` (rlimit) and
  verifies on macOS ([reproduction](../../corpus/repros/z3-rlimit-platform-difference/README.md)).
  Four of the five f64 differences follow from it; the fifth,
  `fptaylor_tests-test04_dqmom9-constant-0`, is mutant-specific. In
  `f64_split`, `delta` has `rlimit(30)` and verifies on both platforms; the
  one difference, `daisy-matrix_determinant-arith-4`, is marked
  layout-sensitive by the generator.
- `fptaylor_real2float-kepler0-arith-4` (int) takes 760–790 s per plain
  run on both platforms and ends with an rlimit error. The seven excluded
  lemmas include two with a standalone
  [reproduction](../../corpus/repros/z3-nla-rlimit-enforcement/README.md).

## Conformance

Every layout: pass-through, cache fill, cache warm and engine arms, all
units `same`, 0 unstable, 0 different, 0 infrastructure errors, 0 rlimit
drift.

| Layout | Units | Functions |
|---|---|---|
| `f64` | 5 | 28 |
| `f64_split` | 5 | 28 |
| `int` | 5 | 38 |
| `int_split` | 5 | 38 |

## Edit workload (every mutant, three rounds)

Each sail run is compared with that round's plain run. All 379 mutants were
`same` in every round and matched their Linux expected outcome; no false
verified.

| Layout | Plain | Sail | Speedup | Replies served |
|---|---|---|---|---|
| `f64` | 264.6 s | 268.0 s | 0.99× | 21% |
| `f64_split` | 206.3 s | 113.7 s | 1.81× | 72% |
| `int` | 1033.7 s | 1005.6 s | 1.03× | 17% |
| `int_split` | 255.6 s | 97.1 s | 2.63× | 72% |

Per class (served share; speedup):

| Class | `f64` | `f64_split` | `int` | `int_split` |
|---|---|---|---|---|
| `arith` | 35%; 0.99× | 86%; 1.54× | 0%; 1.00× | 70%; 3.58× |
| `constant` | 34%; 0.99× | 85%; 1.34× | 0%; 0.98× | 68%; 1.12× |
| `rare-region` | 0%; 0.98× | 66%; 1.89× | 0%; 0.97× | 66%; 1.58× |
| `spec-strengthen` | 26%; 0.99× | 70%; 1.80× | 45%; 1.74× | 77%; 4.01× |
| `spec-weaken` | 22%; 0.99× | 62%; 2.74× | 35%; 1.31× | 76%; 5.16× |

An edit ends cache reuse for the rest of its solver session (ADR 0008,
ADR 0011). One module per kernel gives each kernel its own session, so an
edit leaves the other kernels' replies servable. In the int crates the
kernels are `spec fn` definitions, emitted before any check, so a body edit
(`arith`, `constant`, `rare-region`) changes the session from its start;
per kernel, those classes are served 66–70%. `int`'s plain time is
dominated by `kepler0-arith-4` (785–790 s per plain run here), which is not in
`int_split` because it runs past 900 s there.

## Cold and warm (nothing changed)

Three rounds; warm is one fill plus three warm rounds. Every unit `same` in
every round. Cold sail was within 1% of plain in every layout.

| Layout | Plain | Warm | Speedup | Note |
|---|---|---|---|---|
| `f64` | 14.2 s | 4.3 s | 3.3× | `fptaylor_extra` 1.0× |
| `f64_split` | 11.9 s | 3.8 s | 3.2× | `fptaylor_extra` 3.9× |
| `int` | 216.2 s | 204.5 s | 1.06× | 4.7× without `daisy_slow` |
| `int_split` | 92.1 s | 80.3 s | 1.15× | 5.2× without `daisy_slow` |

A session whose check returns `unknown` is solved live from that check on,
because cached `unknown` replies are never served (ADR 0009, ADR 0011):
`f64/fptaylor_extra` (51 served, 141 re-solved) and `daisy_slow` in both int
layouts. This is the designed behaviour, not a cache miss.

## Limits

- One project family, generated by one generator, with five of ADR 0054's
  nine classes. It is not the independent base the ADR requires.
- Expected outcomes are from one Linux host type; near-limit outcomes are
  platform- and layout-specific.
- Timings are per-unit medians of three rounds on one host, summed; no confidence intervals.

## Evidence (raw files outside the repository, sha256)

- Linux expected outcomes (all four catalogues and the 20 unmodified
  crates): `96c2c0aa451c6596474a290d1c2c56536cdfae34da9f244845e3b0929aa9f638`;
  script `944ebea06886ce7bfe644f7f7c71d4ed7130449be2f03a9f501c15064cba9725`.
- `f64` at `c848259`: run log
  `d5b118487adfc654c6c5fbd51e2bd7ba118214e3fdd71959d5320b6f99ad4229`; results
  cold `280a7abf33fa18ae002d0fed88fe729b2ff0bd58b1931590d146717901b545f2`,
  warm `ab034dbcb0321b0e4e7ebb87e0948e645d419d7954da1b816d4164b5e307c0f9`,
  edit `a3deaa67e07872df1998828ae88722ee311869e407452b441ce27bbe57e47fe7`.
- `f64_split`, `int`, `int_split` at `c848259`: run log
  `b71b0f66e5a19a6694adb70f15ef3154b3b688b69bbc349f4e664326e422501e`;
  results `f64_split` cold
  `fbeadb2b2892a7b796ca796081c9f23acb4b28cd2fed7d26f1cb500b91bd5db7`, warm
  `cf9b5b67e3bbbeaccf66626a2e1f371ca7ec6828fda6c15733b4cd6ed8b408f3`, edit
  `e7f994028e144b2ba44ce7ff3c3bfd5f615b49f94ad3d67ae5054a4a11e22298`;
  `int` cold `a7180af85e0b60db3ca5211e9cb58dd75c198ee7dca8b60a68cf87597e44c456`,
  warm `fca0f0369b2423ff13bd064feef7876c5fbf05b6b2d2756714270b45f4ef9647`,
  edit `eb2fa492d731a971eb822053f21296086cf2a5e671257949a65d6fd005d71b2c`;
  `int_split` cold
  `f7a44f0b5d18a3d6636f470eb60b3fb41e662204c3f4cbf10263b09c13a3316f`, warm
  `b50d5982f5fbaba6407a824c438d863ca5c17b63510cbef3daf1d1fc6cde3466`, edit
  `eee944b859c94ff81c47989a6f0abfa8ba2452a3bd1331ec7802addfeb99a7c0`.

- Nonlinear rlimit reproduction, Linux, stock release, 2026-10-17: run logs
  and scripts `c433fd09359677adfe8a8971bf659008b131f2dbf52d00ddb08aa33cf24adbde`.

## Earlier attempts

- `8bd4aed`, 2026-10-16, `f64` at `39a7f7b6`: conformance 5/5 units and 28/28
  functions same in all four arms; edit 115 same, 0.99×, 21% served. Run log
  `942efc3c0a9b8c3c0cfd2b75e1c8eb1c8fbd8dfcb2f4fc212ee90ddc7f1093a5`, results
  `85432309e7195518b8776e7ac4eaac82f76868ee6fa9b4682dd991b157ff9554`.
  Superseded by the `c848259` run (same result).
- `8bd4aed`, 2026-10-16, first conformance start: failed before running,
  `git` refused the root-owned container's view of the copied repository
  ("dubious ownership"). Fixed by changing its owner; no result.
- `bfde6c7`, 2026-10-16, `f64` cold and warm: cold about 1.0×, warm 3.3×
  (`fptaylor_extra` 1.0×). Run log
  `7a67db1192b3205a5c276f29434cdc4b052f9498263a5d13ce2ef6edff393551`, results
  cold `ab9c8a4ea9c41d8ca101ec4ec55c6182d7f124b7c818038e57164facc6b85790`,
  warm `cf24bd135a4280a17356e8e66ce2974bac1d0891a66dc499db5cafe48a6d68a2`.
- `c848259`, 2026-10-17, first Linux `delta_loose` default-rlimit run: the
  900 s cap killed Verus but not its Z3, which held the output open; Z3 was
  still running after 47 min. Rerun with a process-group cap (above).
