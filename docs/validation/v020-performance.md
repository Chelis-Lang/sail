# v0.2.0 combined performance qualification

Status: all eight project/platform matrices are complete: 864 expected
command exits with populated memory fields. Raw memory/clock/exit records,
captured helpers and installed identities pass automated cross-checks.
Release acceptance is not established.
The original stock-parity and sampler-overhead requirements remain unchanged.

The complete macOS matrix retains 432 expected command exits and populated
memory observations, each checked against its raw clock/memory record. Exact
helper bytes match `c60dc918`; installed program hashes match the release
manifest and all four proof units. All four startup preflights pass (about
0.009–0.038 observed cores, with the original 0.10 ceiling).

Mac cold medians improve on IronKV, Vest and CapybaraKV. Anvil is slower:
101.002 s Sail against 99.340 s stock. First unchanged calls remain slower
on every project; full-path IronKV is 0.168 s against 0.173 s stock, while
the other three full-path rows remain slower. Sampled cold tree RSS falls
on IronKV and CapybaraKV but rises on Anvil (5,045 versus 4,359 MiB) and Vest
(3,159 versus 2,696 MiB). These observations do not waive the failed sampler
overhead requirement or establish memory acceptance.

All twelve normal Mac Vest reverts are 1.263–1.288 s, with zero shim sessions.
The historical rc.7 Mac slow restore miss is not reproduced; its cause is still
unknown. Keep the historical result separate from this new profile and helper.

The separate Linux baseline `b70ec5ee` uses the earlier 5 ms observer and both
digest switches off. Its complete 432 rows pass raw memory/clock/exit checks.
Vest's final normal pair has 19.697 / 19.771 s revert outliers, with 266 shim
sessions and zero solver starts, against five earlier pairs around 1.14–1.20 s.
The matching baseline diagnostic rerun has twelve restores in 1.189–1.265 s.
That diagnostic capture retains stderr, rows and memory records, but omits
nested result notes, events and cost records. No cause or platform
specificity is proved.
The selected Linux candidate's twelve normal Vest reverts are 1.322–1.397 s,
with zero shim sessions. This does not establish what caused the baseline
misses. The selected Linux
six-pair diagnostic cohort also restores all twelve times in 1.279–1.319 s.
Its capture retains 54 result records, six event streams and six final cost
records; the final compile/retention ratios are 51.5–52.9 against the unchanged
threshold 16. No slow diagnostic case occurs, so these records do not diagnose
the earlier miss. Current CapybaraKV's 108 normal rows and the diagnostic
cohort's 108 rows each pass raw memory/clock/exit checks. The current capture's
helpers, installed program hashes and three-unit manifest also agree; its
Sail and shim binaries are byte-identical to the original Linux candidate.
Current CapybaraKV/diagnostic archive SHA-256:
`9802fbca09e8e50ca9cd27cd76041b560f36a4a921b0391d5d6582bfd5a48af0`.
See [#595](https://github.com/Chelis-Lang/sail/issues/595).
Baseline archive SHA-256:
`8da4895630e4f35aca2929cb64ff3b8c86a6286292f8216c85c125483bfa8bea`.

Linux cold medians, stock → Sail in seconds: IronKV 45.654 → 20.876;
Vest 81.720 → 54.517; CapybaraKV 58.711 → 54.692; Anvil 189.036 → 179.021.
Cold sampled tree RSS is higher on 3/4 Linux projects; all ranges remain
visible below. Across both platforms, first-unchanged medians exceed stock
in 8/8 rows and full-path medians in 7/8 rows. Stock parity remains unqualified.

## Source and measurement scope

The selected installed candidate is `c60dc918`, with fork runtime `47fb3be6`
and stock Verus `0.2026.10.18.7d82994` as oracle. Sail builds with Rust
1.99.0; verification uses Rust 1.99.1 except Anvil, which retains its own
Rust 1.98.0 toolchain. Production crate, script and CI bodies match the
Linux target-selection PR after its restack onto main.

Linux runs on a native EC2 c7i.8xlarge in us-east-1; macOS on mac-m4.metal
in us-east-1b, macOS 27.0. Projects run on isolated lanes, one project at a
time, with six
paired rounds per project and alternating arm order. Each arm starts from a
fresh project and empty Sail cache. Both use registry vstd; Vest passes
`--rlimit 100`; Anvil keeps `--lib -- --verify-module vdeployment_controller`.
Installed release layouts include three exact schema-13 vstd proof units on
Linux and four on macOS.

Source digest reuse is off on both platforms. The guarded owned-target
optimization is on by default only on Linux; macOS retains fresh target reads.
All source/target keys, original two-second settle checks, Current provenance,
receipt admission and dirty cleanup remain intact. The independent ten-pair
Linux target comparison saves 430 ms; Linux source comparison regresses by
120 ms. See [settled observations](settled-digest-observations.md).

The observer uses the corrected command clock and 1 ms Python scheduling
quantum, with unchanged 50 ms RSS sampling. macOS runs with an authenticated
idle console and the unchanged startup process-CPU preflight (at most 0.10
observed cores). This is a bounded startup check, not continuous proof of
an idle host. Earlier locked-console samples and all preflight refusals
remain diagnostics, with their actual source/helper scope.

Tree RSS sums sampled resident memory of concurrent descendants and can count
shared pages repeatedly. Largest-process memory includes the platform's
separate lifetime high-water observation; it is not a concurrent tree peak.
Daemon RSS includes pre-existing/detached Sail daemons observed separately.
Tree and daemon maxima need not coincide and must not be added. Values are
observations, not exact instantaneous peak-memory guarantees.

## Remaining acceptance gates

The complete sampler-overhead gate fails: native IronKV passes 17 of 18
arm/phase comparisons, while Mac demo plain-edit fails and two Mac IronKV
batches refuse the unchanged CPU preflight. All samples and refusals remain;
a complete matrix cannot waive this separate requirement. See
[memory instrumentation](benchmark-memory.md).

Default-enabled Linux ordinary/solver, full E2E plus extra, full fast/standard
conformance and all 21 mutation comparisons pass. Mac host and actual factory
pass. Deep corpus comparisons and all 217 mutation/control comparisons agree.
The fresh exact fork source-suite pass-through, cache-fill and cache-warm arms
all pass (4,902 Same each; all 4,839 nonignored plain tests pass). The strict
stock source-suite arm fails on JSON debug-array ordering, so the aggregate
deep-rvt gate remains failed; no normalization or reclassification is applied
([deep-tier record](deep-tier.md), [#627](https://github.com/Chelis-Lang/sail/issues/627)).
The fork artifact is unchanged; no new fork pin or determinism run is claimed.

No release, tag or publication is qualified by this record.

## Indexed interrupted attempts

- `c60dc918`, 2026-10-30, Linux: the measurement SSH session times out;
  the failure handler captures the available outputs.
  IronKV and Vest each retain all 108 required rows; CapybaraKV retains
  104/108 and Anvil is unrun. The transport cause is unknown. The partial
  CapybaraKV attempt is failed, not complete acceptance; all its samples
  remain in the retained archive. Recovery repeats complete six-round
  CapybaraKV and Anvil matrices on isolated native lanes, using detached
  services so transport failure does not terminate active measurements.
  Captured archive SHA-256:
  `20038aef3133b9f630cff7955f58b0451658cc186fcf3b7f90ba062b646f4e16`.

- `c60dc918`, 2026-10-30, both Linux recovery setups: the private driver
  omits the required `e2e-build` output directory and stops before timing.
  The corrected driver supplies that destination; no check changes.
  Raw setup archives SHA-256:
  `6ee579de9fe8d3b5854df2207d9ba5bc42c011f642d8e585347c1cc6028942e4`
  and `596e657e67f9f08bfe8ada09f899d2f900770355d8139435dddcce608ca23b75`.

## Installed program identities

All three Linux captures use identical Sail/shim binaries. Each capture
independently matches its release manifest, exact captured helper bytes and
three-unit proof manifest; macOS matches its four-unit manifest.

| Platform | Sail SHA-256 | Shim SHA-256 |
|---|---|---|
| Linux | `4f0b2e7778838aa33c84c61d18a114d352d17780246be325b5c7003c529fe634` | `00b391053eeebb1d9194331c352665daf60be7c8aa26ab3fb4a2ecf1216e494a` |
| macOS | `1fe23e6a9948d1d781e8585e07bfd0a5dee1ffa453986be3b37056cf2f8000fa` | `6ce8efe862c1cdc70a5522828e8b46869346f9b8433ed86fca22e7815dcc1577` |

## Linux (ironkv, vest)

Source: `c60dc9186baf36b943820f3cb8aac4ae481f7966`. Six rounds per arm and phase; all 216 required rows retain expected exits and populated tree/largest/daemon memory fields. Each value is median (minimum–maximum); no sample is removed.

Archive SHA-256:
`20038aef3133b9f630cff7955f58b0451658cc186fcf3b7f90ba062b646f4e16`.

### ironkv

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 45.654 (45.504–46.074) | 20.876 (19.850–21.665) | 2.19× |
| unchanged | 0.120 (0.118–0.153) | 0.237 (0.228–0.245) | 0.51× |
| unchanged2 | 0.118 (0.117–0.120) | 0.028 (0.026–0.028) | 4.23× |
| unchanged-noreplay | 0.118 (0.115–0.119) | 0.180 (0.165–0.184) | 0.66× |
| body-early | 10.517 (10.460–10.570) | 8.319 (7.853–8.356) | 1.26× |
| body-early-revert | 10.487 (10.341–10.580) | 0.794 (0.785–0.828) | 13.21× |
| fail | 9.651 (9.050–9.727) | 8.518 (8.425–8.588) | 1.13× |
| fix | 10.624 (9.829–10.647) | 8.336 (8.250–8.479) | 1.27× |
| fix-revert | 10.517 (9.758–10.617) | 0.799 (0.796–0.808) | 13.16× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 3160.6 (3002.8–3219.8) | 1684.8 (1652.6–1704.0) | 2482.6 (2468.0–2510.2) | 995.9 (995.6–997.5) | 223.1 (131.7–251.9) |
| unchanged | 19.7 (3.9–20.0) | 21.9 (21.1–22.2) | 91.5 (91.5–91.6) | 91.5 (91.5–91.6) | 85.6 (85.5–85.7) |
| unchanged2 | 19.8 (19.7–20.1) | 12.5 (12.3–12.8) | 91.6 (91.4–91.6) | 13.4 (13.4–13.5) | 85.6 (85.5–85.7) |
| unchanged-noreplay | 19.9 (19.6–20.1) | 20.4 (17.6–20.8) | 91.5 (91.4–91.6) | 91.5 (91.5–91.6) | 85.6 (85.5–85.7) |
| body-early | 1405.3 (1358.0–1412.5) | 1170.3 (1168.5–1183.0) | 954.9 (953.6–955.3) | 995.9 (995.0–998.0) | 86.1 (86.0–86.2) |
| body-early-revert | 1439.2 (1398.6–1457.8) | 113.9 (110.9–118.0) | 955.3 (953.9–956.7) | 91.6 (91.5–91.6) | 86.1 (86.0–86.2) |
| fail | 1398.0 (1369.5–1425.8) | 1177.8 (1172.9–1188.9) | 955.2 (953.9–955.6) | 996.1 (995.4–997.4) | 87.4 (86.9–87.8) |
| fix | 1399.6 (1381.7–1472.8) | 1167.7 (1166.5–1186.5) | 954.6 (954.1–956.5) | 996.3 (994.2–996.9) | 87.6 (87.3–88.2) |
| fix-revert | 1412.2 (1385.4–1490.9) | 113.8 (107.4–119.3) | 954.6 (954.4–955.1) | 91.5 (91.5–91.8) | 87.6 (87.3–88.2) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

### vest

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 81.720 (81.068–82.861) | 54.517 (54.301–56.216) | 1.50× |
| unchanged | 0.237 (0.216–0.254) | 0.689 (0.681–0.744) | 0.34× |
| unchanged2 | 0.218 (0.214–0.222) | 0.045 (0.043–0.049) | 4.83× |
| unchanged-noreplay | 0.219 (0.215–0.225) | 0.513 (0.507–0.528) | 0.43× |
| body-early | 46.079 (45.008–46.504) | 26.633 (26.497–27.053) | 1.73× |
| body-early-revert | 45.891 (45.063–47.016) | 1.339 (1.322–1.397) | 34.26× |
| fail | 40.740 (40.323–42.121) | 22.638 (22.380–22.829) | 1.80× |
| fix | 45.669 (45.173–48.164) | 26.829 (26.654–27.254) | 1.70× |
| fix-revert | 45.572 (45.039–48.194) | 1.349 (1.326–1.353) | 33.77× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 4193.0 (4163.2–4248.9) | 4946.7 (4914.7–5041.8) | 3060.5 (3050.9–3071.1) | 3404.2 (3375.0–3412.5) | 160.0 (108.9–218.6) |
| unchanged | 53.8 (44.9–54.9) | 95.2 (84.9–110.8) | 91.6 (91.5–91.6) | 98.1 (91.6–106.1) | 108.9 (108.6–109.4) |
| unchanged2 | 53.8 (52.8–55.6) | 21.4 (21.3–21.5) | 91.5 (91.5–91.6) | 21.4 (21.0–21.6) | 108.9 (108.6–109.4) |
| unchanged-noreplay | 53.9 (52.6–54.4) | 84.4 (81.1–84.5) | 91.6 (91.5–91.6) | 91.5 (91.4–91.6) | 108.9 (108.6–109.4) |
| body-early | 4224.8 (4175.9–4272.2) | 3988.8 (3947.3–4005.2) | 3063.4 (3054.4–3118.2) | 3729.3 (3688.2–3747.8) | 110.2 (109.9–110.8) |
| body-early-revert | 4213.4 (4179.5–4242.4) | 205.3 (201.3–211.9) | 3073.7 (3056.6–3099.2) | 97.4 (95.2–101.4) | 110.2 (109.9–110.8) |
| fail | 4196.3 (4163.5–4230.2) | 3962.5 (3919.1–3995.4) | 3066.2 (3048.3–3092.5) | 3699.3 (3662.2–3733.9) | 112.2 (111.4–112.9) |
| fix | 4207.8 (4164.4–4243.7) | 3963.4 (3944.9–4003.6) | 3073.9 (3042.9–3083.9) | 3709.0 (3689.1–3747.8) | 113.2 (112.5–114.1) |
| fix-revert | 4232.5 (4164.3–4240.4) | 202.7 (195.0–210.8) | 3060.9 (3043.0–3082.4) | 100.1 (94.0–100.9) | 113.2 (112.5–114.1) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

## Linux (capybarakv)

Source: `c60dc9186baf36b943820f3cb8aac4ae481f7966`. Six rounds per arm and phase; all 108 required rows retain expected exits and populated tree/largest/daemon memory fields. Each value is median (minimum–maximum); no sample is removed.

Archive SHA-256:
`9802fbca09e8e50ca9cd27cd76041b560f36a4a921b0391d5d6582bfd5a48af0`.

### capybarakv

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 58.711 (58.583–59.160) | 54.692 (54.530–55.308) | 1.07× |
| unchanged | 0.143 (0.142–0.177) | 0.536 (0.530–0.556) | 0.27× |
| unchanged2 | 0.145 (0.141–0.145) | 0.023 (0.022–0.025) | 6.34× |
| unchanged-noreplay | 0.144 (0.143–0.146) | 0.365 (0.363–0.370) | 0.39× |
| body-early | 26.427 (26.185–27.286) | 12.893 (12.813–13.005) | 2.05× |
| body-early-revert | 26.402 (26.303–27.365) | 11.390 (11.306–11.459) | 2.32× |
| fail | 24.067 (23.881–24.286) | 10.520 (10.444–10.608) | 2.29× |
| fix | 26.429 (26.368–26.679) | 12.917 (12.842–13.007) | 2.05× |
| fix-revert | 26.471 (26.284–26.633) | 11.335 (11.309–11.377) | 2.34× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 3969.3 (3767.1–4252.6) | 4083.2 (3551.3–4516.2) | 2536.4 (2525.4–2554.0) | 2795.4 (2746.6–2807.5) | 232.2 (180.2–313.1) |
| unchanged | 21.1 (20.8–33.8) | 60.7 (58.7–63.8) | 91.5 (91.5–91.6) | 91.5 (91.4–91.6) | 101.0 (100.5–101.4) |
| unchanged2 | 21.0 (20.9–21.2) | 18.4 (18.2–19.0) | 91.5 (91.5–91.6) | 18.1 (17.9–18.2) | 101.0 (100.5–101.4) |
| unchanged-noreplay | 20.9 (20.8–21.1) | 76.0 (73.9–79.2) | 91.6 (91.5–91.6) | 91.5 (91.5–91.6) | 101.0 (100.5–101.4) |
| body-early | 2943.7 (2917.8–3017.0) | 1991.6 (1986.6–1995.8) | 1638.3 (1634.2–1644.3) | 1829.3 (1822.9–1832.6) | 101.3 (100.8–101.7) |
| body-early-revert | 2950.0 (2908.0–2971.2) | 1956.4 (1947.0–1964.0) | 1632.7 (1629.5–1641.5) | 1828.9 (1822.5–1837.7) | 101.3 (100.8–101.7) |
| fail | 2945.9 (2928.6–2972.4) | 1995.1 (1990.6–2001.3) | 1634.0 (1629.9–1642.5) | 1831.4 (1821.3–1836.5) | 102.5 (102.0–103.0) |
| fix | 2966.2 (2918.4–3016.6) | 1988.6 (1981.8–1991.9) | 1640.1 (1630.5–1646.6) | 1830.2 (1820.9–1836.3) | 102.8 (102.0–103.3) |
| fix-revert | 2967.8 (2940.0–3002.6) | 1954.1 (1949.8–1965.9) | 1635.3 (1628.7–1646.0) | 1828.9 (1825.3–1837.1) | 102.8 (102.1–103.3) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

## Linux (anvil)

Source: `c60dc9186baf36b943820f3cb8aac4ae481f7966`. Six rounds per arm and phase; all 108 required rows retain expected exits and populated tree/largest/daemon memory fields. Each value is median (minimum–maximum); no sample is removed.

Archive SHA-256:
`0f51e005453a4c942f10c47dc3c78baef2dabb904d552538d91c9e2107006edb`.

### anvil

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 189.036 (182.993–192.644) | 179.021 (177.868–182.244) | 1.06× |
| unchanged | 0.360 (0.336–0.376) | 2.061 (2.017–2.074) | 0.17× |
| unchanged2 | 0.314 (0.309–0.325) | 0.042 (0.042–0.043) | 7.52× |
| unchanged-noreplay | 0.318 (0.305–0.332) | 1.224 (1.206–1.276) | 0.26× |
| body-early | 158.946 (154.922–160.377) | 87.790 (86.318–92.313) | 1.81× |
| body-early-revert | 156.715 (152.841–160.435) | 34.551 (34.007–35.042) | 4.54× |
| fail | 149.828 (145.916–156.074) | 27.437 (27.002–27.990) | 5.46× |
| fix | 158.665 (153.417–162.442) | 52.037 (51.519–52.905) | 3.05× |
| fix-revert | 154.795 (152.987–159.420) | 34.682 (34.131–34.924) | 4.46× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 6640.7 (6620.1–6650.2) | 7554.1 (7506.5–7560.9) | 4173.2 (4171.6–4175.6) | 4381.3 (4379.5–4385.2) | 82.2 (81.8–91.7) |
| unchanged | 83.1 (75.8–87.0) | 187.9 (186.5–194.4) | 88.0 (87.8–88.0) | 154.6 (138.4–159.1) | 82.0 (81.8–82.2) |
| unchanged2 | 84.7 (79.8–87.1) | 23.9 (23.6–24.1) | 79.2 (78.7–79.3) | 30.8 (30.2–30.9) | 82.0 (81.8–82.2) |
| unchanged-noreplay | 82.5 (79.9–86.9) | 180.2 (178.1–184.9) | 78.8 (78.5–79.3) | 142.2 (130.9–147.0) | 82.0 (81.8–82.2) |
| body-early | 6628.4 (6569.9–6670.6) | 4800.1 (4789.2–4814.6) | 4173.6 (4171.5–4174.5) | 4385.9 (4378.1–4390.6) | 82.0 (81.8–82.3) |
| body-early-revert | 6637.7 (6604.1–6663.2) | 4643.9 (4630.5–4649.9) | 4173.0 (4171.7–4175.2) | 4387.3 (4380.8–4393.6) | 82.1 (81.8–82.3) |
| fail | 6548.2 (6512.0–6564.5) | 4724.5 (4707.0–4734.0) | 4175.1 (4174.8–4175.7) | 4392.0 (4387.2–4404.7) | 85.5 (85.2–85.7) |
| fix | 6639.6 (6586.1–6666.7) | 4724.5 (4719.4–4727.6) | 4174.7 (4171.9–4176.9) | 4387.6 (4383.1–4392.4) | 85.5 (85.2–85.7) |
| fix-revert | 6631.6 (6614.7–6661.2) | 4651.0 (4640.4–4670.4) | 4171.9 (4170.4–4175.5) | 4388.5 (4382.7–4393.5) | 85.5 (85.2–85.7) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.


## macOS (ironkv, vest, capybarakv, anvil)

Source: `c60dc9186baf36b943820f3cb8aac4ae481f7966`. Six rounds per arm and phase; all 432 required rows retain expected exits and populated tree/largest/daemon memory fields. Each value is median (minimum–maximum); no sample is removed.

Archive SHA-256:
`8170e3b0386f54a0daacb9b4acfbeb931b2e1f4ac90b259b7bf7fae533080203`.

### ironkv

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 20.760 (20.615–20.886) | 12.573 (12.509–12.630) | 1.65× |
| unchanged | 0.175 (0.174–0.189) | 0.207 (0.204–0.210) | 0.85× |
| unchanged2 | 0.174 (0.173–0.176) | 0.022 (0.022–0.022) | 7.98× |
| unchanged-noreplay | 0.173 (0.173–0.177) | 0.168 (0.166–0.172) | 1.03× |
| body-early | 4.347 (4.329–4.362) | 3.278 (3.269–3.290) | 1.33× |
| body-early-revert | 4.361 (4.348–4.376) | 2.166 (2.151–2.206) | 2.01× |
| fail | 4.070 (4.047–4.104) | 3.115 (3.104–3.122) | 1.31× |
| fix | 4.361 (4.337–4.382) | 3.007 (2.993–3.025) | 1.45× |
| fix-revert | 4.372 (4.336–4.405) | 2.162 (2.151–2.170) | 2.02× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 2343.4 (2285.9–2368.1) | 1227.2 (1220.5–1251.0) | 2059.8 (2052.5–2067.9) | 749.9 (745.2–763.2) | 128.4 (127.9–128.8) |
| unchanged | 36.9 (35.5–42.3) | 19.2 (17.3–20.0) | 37.9 (37.9–38.0) | 36.7 (36.7–36.8) | 128.4 (127.9–128.8) |
| unchanged2 | 38.0 (36.8–38.3) | 4.6 (4.6–10.8) | 37.9 (37.9–38.0) | 14.7 (14.5–14.8) | 128.4 (127.9–128.8) |
| unchanged-noreplay | 37.1 (36.7–42.4) | 21.2 (19.6–21.5) | 37.9 (37.9–38.0) | 36.7 (36.7–36.8) | 128.4 (127.9–128.8) |
| body-early | 1003.6 (972.9–1008.7) | 885.2 (877.4–892.8) | 650.3 (637.6–659.2) | 730.3 (722.2–736.8) | 128.5 (128.0–128.9) |
| body-early-revert | 1007.4 (986.8–1026.9) | 838.4 (831.3–843.3) | 642.6 (635.6–664.8) | 731.4 (726.1–736.3) | 103.7 (103.2–104.1) |
| fail | 1017.8 (973.1–1048.1) | 884.9 (875.0–895.0) | 651.9 (638.9–667.3) | 730.9 (720.9–741.2) | 103.7 (103.2–104.1) |
| fix | 1015.1 (985.9–1028.5) | 877.0 (869.9–883.5) | 643.4 (638.3–673.5) | 725.5 (720.1–732.3) | 78.9 (78.4–79.3) |
| fix-revert | 1014.8 (979.9–1026.5) | 838.4 (834.0–841.0) | 649.1 (642.3–656.0) | 731.9 (727.7–734.1) | 79.0 (78.5–79.4) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

### vest

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 39.448 (39.208–39.521) | 30.520 (30.364–31.807) | 1.29× |
| unchanged | 0.232 (0.230–0.236) | 0.692 (0.682–0.695) | 0.34× |
| unchanged2 | 0.227 (0.224–0.229) | 0.035 (0.035–0.035) | 6.47× |
| unchanged-noreplay | 0.229 (0.223–0.230) | 0.480 (0.477–0.480) | 0.48× |
| body-early | 22.443 (22.283–22.519) | 12.764 (12.689–12.790) | 1.76× |
| body-early-revert | 22.497 (22.405–22.602) | 1.275 (1.266–1.287) | 17.65× |
| fail | 21.099 (21.056–21.129) | 10.929 (10.920–10.963) | 1.93× |
| fix | 22.494 (22.436–22.621) | 12.744 (12.691–12.867) | 1.77× |
| fix-revert | 22.580 (22.527–22.598) | 1.274 (1.263–1.287) | 17.73× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 2695.6 (2656.4–2784.6) | 3158.7 (3131.5–3173.8) | 2240.1 (2223.1–2313.4) | 2475.7 (2450.2–2524.1) | 133.2 (123.5–155.8) |
| unchanged | 60.2 (59.7–60.5) | 81.2 (75.6–82.8) | 69.8 (69.8–69.9) | 83.3 (79.8–84.4) | 133.2 (107.7–155.8) |
| unchanged2 | 60.4 (56.9–62.5) | 4.7 (4.6–5.0) | 69.9 (69.8–70.0) | 25.7 (25.6–25.9) | 133.2 (107.7–155.8) |
| unchanged-noreplay | 59.8 (56.8–60.7) | 80.7 (75.9–81.7) | 69.8 (69.4–70.0) | 76.6 (75.3–80.5) | 133.2 (107.7–155.8) |
| body-early | 2706.4 (2674.6–2749.7) | 2715.5 (2699.4–2738.7) | 2284.9 (2256.8–2310.0) | 2538.3 (2500.6–2566.2) | 133.4 (108.8–156.0) |
| body-early-revert | 2748.9 (2715.2–2777.9) | 195.4 (191.3–224.0) | 2315.7 (2294.0–2352.7) | 90.7 (88.0–93.5) | 109.4 (108.7–132.0) |
| fail | 2727.5 (2670.5–2745.8) | 2723.4 (2713.9–2728.0) | 2263.5 (2200.0–2307.2) | 2489.9 (2476.9–2502.9) | 110.3 (109.6–132.1) |
| fix | 2708.8 (2661.2–2732.2) | 2691.4 (2681.0–2732.8) | 2281.0 (2253.5–2325.7) | 2518.3 (2469.6–2567.0) | 110.8 (108.8–111.4) |
| fix-revert | 2705.5 (2669.9–2730.0) | 197.6 (191.6–203.1) | 2279.7 (2260.2–2297.6) | 91.0 (90.1–92.7) | 110.7 (108.7–111.3) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

### capybarakv

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 31.905 (31.647–31.972) | 30.133 (29.756–30.901) | 1.06× |
| unchanged | 0.199 (0.195–0.204) | 0.501 (0.494–0.505) | 0.40× |
| unchanged2 | 0.193 (0.192–0.195) | 0.018 (0.018–0.019) | 10.63× |
| unchanged-noreplay | 0.194 (0.191–0.204) | 0.367 (0.366–0.369) | 0.53× |
| body-early | 15.060 (14.946–15.086) | 5.939 (5.908–5.965) | 2.54× |
| body-early-revert | 15.114 (15.087–15.131) | 5.580 (5.569–5.598) | 2.71× |
| fail | 14.215 (14.188–14.236) | 4.963 (4.952–4.978) | 2.86× |
| fix | 15.096 (15.025–15.193) | 5.980 (5.954–6.025) | 2.52× |
| fix-revert | 15.158 (15.123–15.204) | 5.580 (5.543–5.602) | 2.72× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 2348.6 (2313.2–2387.8) | 2316.9 (2293.7–2335.9) | 2067.9 (2045.9–2121.5) | 2194.3 (2181.3–2215.5) | 172.6 (146.7–198.4) |
| unchanged | 47.7 (43.1–55.5) | 62.6 (50.2–65.6) | 42.4 (42.3–42.5) | 53.8 (52.5–57.0) | 172.6 (146.7–198.4) |
| unchanged2 | 56.1 (55.9–57.9) | 4.7 (4.6–5.2) | 42.4 (42.3–42.5) | 20.7 (20.3–20.8) | 172.6 (146.7–198.4) |
| unchanged-noreplay | 56.0 (40.3–59.8) | 63.3 (62.2–66.5) | 42.4 (42.4–42.5) | 48.7 (47.1–50.0) | 172.6 (146.7–198.4) |
| body-early | 1753.1 (1734.5–1813.4) | 1468.4 (1435.3–1480.7) | 1133.9 (1117.0–1165.8) | 1318.6 (1278.5–1326.0) | 172.8 (146.8–198.6) |
| body-early-revert | 1757.4 (1729.4–1803.3) | 1436.1 (1396.1–1457.9) | 1138.8 (1107.9–1181.3) | 1338.3 (1297.9–1357.5) | 172.9 (146.9–198.7) |
| fail | 1762.9 (1732.0–1801.4) | 1480.4 (1464.3–1529.4) | 1122.5 (1097.7–1138.2) | 1318.5 (1301.4–1358.4) | 173.0 (146.9–198.7) |
| fix | 1768.7 (1731.8–1796.7) | 1478.2 (1446.2–1496.9) | 1132.5 (1112.4–1167.9) | 1335.1 (1295.5–1350.6) | 136.2 (122.1–174.5) |
| fix-revert | 1753.9 (1737.9–1823.9) | 1437.6 (1386.8–1449.8) | 1129.0 (1111.5–1181.7) | 1340.1 (1286.9–1351.1) | 136.3 (122.3–174.6) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.

### anvil

| Phase | Stock seconds | Sail seconds | Stock/Sail |
|---|---:|---:|---:|
| cold | 99.340 (99.200–99.632) | 101.002 (100.442–102.440) | 0.98× |
| unchanged | 0.272 (0.264–0.279) | 1.879 (1.837–1.897) | 0.14× |
| unchanged2 | 0.221 (0.211–0.229) | 0.033 (0.032–0.034) | 6.59× |
| unchanged-noreplay | 0.219 (0.210–0.228) | 1.398 (1.391–1.404) | 0.16× |
| body-early | 78.461 (78.382–78.924) | 41.839 (41.713–41.930) | 1.88× |
| body-early-revert | 78.641 (78.536–78.950) | 14.669 (14.579–14.802) | 5.36× |
| fail | 74.850 (74.756–75.443) | 12.178 (12.155–12.240) | 6.15× |
| fix | 78.690 (78.460–79.234) | 24.282 (24.133–24.296) | 3.24× |
| fix-revert | 78.733 (78.579–78.952) | 14.638 (14.595–14.662) | 5.38× |

| Phase | Stock tree MiB | Sail tree MiB | Stock largest MiB | Sail largest MiB | Sail daemon MiB |
|---|---:|---:|---:|---:|---:|
| cold | 4358.6 (4326.8–4386.1) | 5045.0 (5003.8–5083.8) | 2999.0 (2970.0–3025.1) | 3442.7 (3413.8–3460.6) | 101.1 (100.9–126.2) |
| unchanged | 90.8 (88.8–91.0) | 202.6 (187.2–210.0) | 92.1 (92.0–92.7) | 136.5 (133.4–141.0) | 76.6 (76.4–101.5) |
| unchanged2 | 78.9 (74.8–82.8) | 4.6 (4.6–4.8) | 92.1 (91.6–92.1) | 40.6 (40.5–40.7) | 76.6 (76.4–101.5) |
| unchanged-noreplay | 79.8 (75.0–83.9) | 194.3 (186.5–205.4) | 92.0 (91.8–92.3) | 134.0 (129.1–137.1) | 76.6 (76.4–101.5) |
| body-early | 4351.9 (4335.8–4361.9) | 3891.4 (3868.9–3924.2) | 2997.9 (2975.4–3033.5) | 3418.6 (3398.2–3453.7) | 76.7 (76.5–101.6) |
| body-early-revert | 4347.1 (4331.5–4371.4) | 3646.8 (3611.6–3684.5) | 2985.5 (2970.2–3040.8) | 3395.1 (3382.0–3437.2) | 76.7 (76.5–101.6) |
| fail | 4364.0 (4342.7–4407.3) | 3706.8 (3660.8–3738.2) | 3022.4 (2999.7–3069.1) | 3391.6 (3385.6–3422.1) | 79.2 (77.9–101.6) |
| fix | 4365.6 (4333.2–4404.5) | 3790.8 (3776.6–3817.4) | 3004.1 (2965.8–3044.0) | 3404.5 (3389.5–3427.2) | 78.2 (77.5–79.3) |
| fix-revert | 4358.6 (4325.9–4432.8) | 3636.4 (3619.4–3652.4) | 2994.9 (2970.7–3105.4) | 3388.9 (3379.8–3418.6) | 78.2 (77.5–79.3) |

Stock daemon observations are zero in every row. Tree and daemon peaks occur independently and must not be added as a simultaneous peak.
