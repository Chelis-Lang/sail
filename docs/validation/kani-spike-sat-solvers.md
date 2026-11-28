# Kani spike S7: SAT solver choice

Status: **passes**. On 96 harnesses from five projects, every finished run of
every harness gave the same per-check status list under CaDiCaL, Kissat and
MiniSat (636 runs, no disagreement). Measured one job at a time, the virtual
best solver takes total CBMC time from 5248 s to 3676 s, **1.43x** over each
project's default. The gain only shows up when the solver is picked per harness:
the best single solver per project gives 1.03x, and 19 harnesses supply all of
it. This is spike S7 of the Kani phase design (`docs/design/kani-phase.md`,
ADR 0180). Nothing here is product code.

## Environment

| | |
|---|---|
| Host | one `c7i.8xlarge` (32 vCPU Xeon Platinum 8488C, 16 cores, 61 GiB, no swap), us-east-2c, Ubuntu 24.04.5, kernel 7.0.0-1018-aws, lane `kani-s7`, 2026-11-27 21:03 to 2026-11-28 03:00 UTC (about 6 hours, including an interrupted first session) |
| Kani | 0.68.0 release bundle (`cargo install --locked kani-verifier --version 0.68.0`, `cargo kani setup`) |
| CBMC | 6.11.0 from the bundle (`cbmc` sha256 `01ae2ae7…1643a`), with CaDiCaL 3.0.0 and MiniSat 2.2.1 linked in |
| Kissat | 4.0.1, the bundle's `bin/kissat` (sha256 `53e430ee…8fecab`) |
| Rust | `nightly-2026-08-21`, rustc 1.100.0-nightly (8925ea358 2026-08-20) |
| Extra host package | libseccomp 2.6.0 built from the release tarball for Firecracker's `seccompiler` (Ubuntu ships 2.5.5) |

No project source was patched.

| Project | Commit | Run | Harnesses run / at least 5 s |
|---|---|---|---|
| aws/s2n-quic | `e11382fd` (the S3 commit, S3's `Cargo.lock`) | `s2n-quic-core`, `s2n-quic-platform`, `s2n-codec` | 40 / 19, 6 / 6, 66 / 0 |
| DataDog/lading | `7b3d3f78` | `lading_throttle` (`lading_payload` has no harnesses) | 17 / 1 |
| rust-osdev/x86_64 | `9fc13798` | `--all-features` | 34 / 12 |
| google/zerocopy | `610fd363` | package `zerocopy`, default features, `-Zfunction-contracts --randomize-layout` | 94 / 32 |
| firecracker-microvm/firecracker | `e9783a04` | `--workspace -Z stubbing -Z function-contracts -Z restrict-vtable` | 41 / 26 |

zerocopy's CI feature `__internal_use_only_features_that_work_on_stable` does
not compile on this nightly (E0658), so default features were used. Stock Kani
reports one zerocopy harness, `byte_slice::proofs::prove_split_at_unchecked`,
as failing; it ran under 5 s and is not in the timed set.

## How Kani selects a solver, and proof that each one ran

- The bundle's `bin/` holds `cbmc`, `goto-analyzer`, `goto-cc`,
  `goto-instrument`, `kani-compiler`, `kani-cov`, `kani-driver` and `kissat`.
  CaDiCaL and MiniSat are compiled into `cbmc`; Kissat is the separate binary.
- Kani maps `cadical` to `--sat-solver cadical`, `kissat` to
  `--external-sat-solver kissat`, and `minisat` to no flag (CBMC's default).
  Kani's own default is CaDiCaL.
- `#[kani::solver(...)]` pins: s2n-quic-core 30 Kissat, 9 CaDiCaL, 1 MiniSat;
  s2n-quic-platform 4 MiniSat, 1 Kissat, 1 CaDiCaL; zerocopy 47 Kissat;
  Firecracker 13 CaDiCaL. **`--solver` on the command line overrides a pin**:
  three s2n-quic-core harnesses pinned to Kissat, CaDiCaL and MiniSat each ran
  the CLI's solver under all three flags, by argv and banner.
- Every timed run printed the expected banner: `Solving with CaDiCaL 3.0.0`
  (221 runs), `Solving with External SAT solver` (209), `Solving with MiniSAT
  2.2.1 with simplifier` (206). A logging shim in front of `kissat` recorded
  one Kissat process per solver call, started by `cbmc`.
- **An absent Kissat looks like a failure.** CBMC run with `kissat` not on
  `PATH` prints `execvp kissat failed`, marks every check `ERROR`, reports
  `VERIFICATION ERROR` and exits 6. That is the earlier pilot's "FAILED". Kani
  itself refuses an absent solver up front (`--solver bin=nosuchsolver`:
  `the specified solver "nosuchsolver" was not found in path`).
- `--sat-solver glucose`, `lingeling`, `picosat`, `zchaff` and `booleforce` are
  accepted by this CBMC build and silently run MiniSat 2.2.1 (by banner).

## Method

1. **Pass 1.** `cargo kani` on every project with its default solvers. A
   wrapper installed as the bundle's `cbmc` kept the exact GOTO file, argv and
   output for each harness. 298 harnesses ran. The projects ran concurrently,
   so these times were used only to select harnesses: 96 took at least 5 s, so
   the threshold stayed at 5 s.
2. **Timed runs.** The real `cbmc` on the captured GOTO file with Kani's argv
   for that harness, changing only the solver flag, under `/usr/bin/time -v`,
   with a 20-minute cap. Total time is the process wall time. Solver time is the
   sum of CBMC's own `Runtime Solver` lines (for Kissat this includes writing
   the CNF and the child process). Peak RSS is `time -v`'s maximum, which
   includes the Kissat child. Statuses are compared at the CBMC level as the
   full sorted (property, description, status) list plus `cProverStatus`.
3. **Rep 1 ran six jobs at a time**, one per physical core (CPUs 1, 3, …, 11),
   gated on memory, to cover all 288 harness-solver pairs within the budget.
   This breaks the one-at-a-time rule. These numbers are used for agreement
   and for choosing what to time on the idle host, and are not used for the
   verdict.
4. **Reps 2 and 3 ran one job at a time** on the otherwise idle host (load
   1.00, CPU 1), in priority order until a fixed deadline: rep 2 of the default
   and the rep-1-fastest solver for every harness; rep 2 of the third solver
   for the 42 harnesses whose three rep-1 times sum to 60 s or less; then
   rep 3 of those pairs, cheapest first. 348 idle runs finished before the
   deadline. 146 harness-solver pairs have three reps, 56 have two and 86 have
   only the parallel rep 1. A pair capped in rep 1 was not rerun.
5. **Idle virtual best** for a harness is the fastest idle mean among the
   solvers timed idle. Every harness has idle times for its default and its
   rep-1-fastest solver. Where the third solver has no idle time, it was slower
   in rep 1 and is left out, which can only understate the speedup.

## Results

### Status agreement

**96 of 96 harnesses agree; 0 disagreements.** In 81 harnesses all three
solvers finished every run, and the 148,327 checks have identical status lists
in every run. In the other 15 harnesses one solver hit the 20-minute cap (14
MiniSat, 1 Kissat: Firecracker `verify_checked_new`); the two solvers that
finished agree there too. No run showed an `ERROR` status.

### Total CBMC time, idle host

| Project | Harnesses | Default s | Virtual best s | Speedup | Best single solver | Single-solver speedup |
|---|---|---|---|---|---|---|
| Firecracker | 26 | 2864.4 | 1937.9 | 1.48x | CaDiCaL | 1.00x |
| lading_throttle | 1 | 8.8 | 6.4 | 1.38x | MiniSat | 1.38x |
| s2n-quic-core | 19 | 1024.1 | 753.3 | 1.36x | CaDiCaL | 1.16x |
| s2n-quic-platform | 6 | 488.2 | 333.3 | 1.46x | CaDiCaL | 1.41x |
| x86_64 | 12 | 108.4 | 106.9 | 1.01x | CaDiCaL | 1.00x |
| zerocopy | 32 | 754.1 | 538.4 | 1.40x | Kissat | 0.85x |
| **All** | **96** | **5248.0** | **3676.2** | **1.43x** | | **1.03x** (one per project) |

Single-solver columns use idle means where measured and rep 1 otherwise (202
of 288 pairs idle), and count a capped run as 1200 s. CaDiCaL everywhere,
overriding every pin, gives 5096 s (1.03x). On the 42 harnesses with all three
solvers timed idle the virtual best gives 1.08x. Rep 1 under six-way load gives
1.40x overall.

The gain is concentrated. 19 harnesses are at least 1.3x faster with the best
solver, 9 at least 2x, and the top five give 63% of the 1572 s saved. There are
two patterns: Firecracker's virtio queue harnesses are 2.9x to 4.2x faster on
MiniSat than on the default CaDiCaL, and several Kissat-pinned s2n-quic and
zerocopy harnesses are 1.4x to 3.8x faster on CaDiCaL or MiniSat. Elsewhere pins are right:
zerocopy's Kissat-pinned `add_scaled_metadata` and `max_elems_for_bytes`
proofs take 5 to 9 s on Kissat, about 35 to 48 s on CaDiCaL and hit the cap on
MiniSat.

| Project | Harness | Default, idle s | Fastest, idle s | Ratio |
|---|---|---|---|---|
| Firecracker | `devices::virtio::queue::verification::verify_checked_new` | CaDiCaL 524.0 | MiniSat 131.3 | 3.99x |
| Firecracker | `devices::virtio::queue::verification::verify_pop` | CaDiCaL 237.3 | MiniSat 82.7 | 2.87x |
| zerocopy | `layout::proofs::prove_dst_layout_pad_to_align_dst_outer_alignment_formula` | Kissat 207.1 | CaDiCaL 55.1 | 3.76x |
| s2n-quic-platform | `message::cmsg::tests::round_trip_test` | Kissat 323.0 | CaDiCaL 172.9 | 1.87x |
| Firecracker | `devices::virtio::queue::verification::verify_undo_pop` | CaDiCaL 205.2 | MiniSat 72.1 | 2.85x |
| Firecracker | `devices::virtio::queue::verification::verify_spec_2_6_7_2` | CaDiCaL 154.2 | MiniSat 36.8 | 4.19x |
| s2n-quic-core | `slice::tests::vectored_copy_fuzz_test` | Kissat 192.9 | CaDiCaL 78.4 | 2.46x |
| Firecracker | `devices::virtio::queue::verification::verify_is_empty` | CaDiCaL 156.6 | MiniSat 43.6 | 3.60x |

### Solver share of CBMC time

The SAT solver is most of CBMC's time, so the solver choice matters. On the
idle default runs, `Runtime Solver` is 74.5% of CBMC wall time, time-weighted,
with a per-harness median of 68%. It is under 50% on 42 of 96 harnesses and
under 20% on 23, mostly small s2n-quic and zerocopy harnesses where symbolic
execution takes the time.

### Variance and memory

- Idle run-to-run spread, |rep 2 − rep 3| over the mean, on the 42 harnesses
  with all three solvers twice: CaDiCaL median 0.5%, p90 0.9%, max 3.3%;
  Kissat 0.5%, 1.5%, 2.5%; MiniSat 0.5%, 1.4%, 4.2%. That is far below the
  gains above.
- Rep 1 under six-way load against the idle mean, over 202 pairs: median 1.08x
  slower, p10 1.01x, p90 1.21x, max 1.37x.
- Peak RSS: CaDiCaL 25.1 GiB (Firecracker `verify_checked_new`; MiniSat needs
  9.2 GiB there), Kissat 8.2 GiB and MiniSat 10.9 GiB (s2n-quic-core
  `sync::spsc::tests::alloc_test`). Racing all three on the largest harness
  would need at least 34 GiB.

## Per harness

Each solver cell is total CBMC wall time / `Runtime Solver` time in seconds /
peak RSS in GiB. Times are idle-host means; `*` marks a time from the
six-at-a-time rep 1 only, and `cap` a run stopped at 20 minutes. Checks is the
number of CBMC checks; Agree compares every finished run of all solvers.

| Project | Harness | Default | CaDiCaL | Kissat | MiniSat | Checks | Agree |
|---|---|---|---|---|---|---|---|
| firecracker | `arch::x86_64::verification::verify_arch_memory_regions` | cadical | 6.0 / 1.4 / 0.4 | 13.7 / 9.4 / 0.2 | 7.6 / 3.0 / 0.3 | 590 | yes |
| firecracker | `devices::virtio::iovec::verification::verify_read_from_iovec` | cadical | 21.0 / 14.3 / 0.6 | 25.3* / 17.3 / 0.4 | 21.5 / 14.9 / 0.5 | 935 | yes |
| firecracker | `devices::virtio::iovec::verification::verify_write_to_iovec_0` | cadical | 103.5 / 91.3 / 11.5 | 211.6* / 200.6 / 4.2 | 266.1* / 253.7 / 3.2 | 5069 | yes |
| firecracker | `devices::virtio::iovec::verification::verify_write_to_iovec_1` | cadical | 213.5 / 196.4 / 11.4 | 368.0* / 352.4 / 4.1 | 451.8* / 433.7 / 3.3 | 5069 | yes |
| firecracker | `devices::virtio::iovec::verification::verify_write_to_iovec_2` | cadical | 373.5 / 352.2 / 13.0 | 438.7* / 419.8 / 3.8 | cap / - / 0.0 | 5069 | yes (2 finished) |
| firecracker | `devices::virtio::iovec::verification::verify_write_to_iovec_3` | cadical | 269.5 / 243.5 / 10.9 | 411.2* / 389.4 / 3.8 | cap / - / 0.0 | 5071 | yes (2 finished) |
| firecracker | `devices::virtio::queue::verification::verify_add_used` | cadical | 39.1 / 14.5 / 3.1 | 239.2* / 218.7 / 1.8 | 1105.0* / 1074.4 / 2.2 | 6454 | yes |
| firecracker | `devices::virtio::queue::verification::verify_avail_ring_idx_get` | cadical | 18.3 / 5.4 / 1.9 | 136.1* / 123.6 / 1.3 | 32.4* / 10.7 / 1.5 | 6116 | yes |
| firecracker | `devices::virtio::queue::verification::verify_avail_ring_ring_get` | cadical | 24.3 / 9.0 / 2.3 | 180.9* / 166.9 / 1.3 | 33.6* / 10.6 / 1.6 | 6132 | yes |
| firecracker | `devices::virtio::queue::verification::verify_avail_ring_used_event_get` | cadical | 20.8 / 7.1 / 2.3 | 137.1* / 125.3 / 1.3 | 27.8* / 9.8 / 1.6 | 6124 | yes |
| firecracker | `devices::virtio::queue::verification::verify_checked_new` | cadical | 524.0 / 481.7 / 25.1 | cap / - / 0.0 | 131.3 / 90.8 / 9.2 | 6938 | yes (2 finished) |
| firecracker | `devices::virtio::queue::verification::verify_initialize` | cadical | 48.5 / 19.8 / 5.9 | 325.9* / 298.2 / 3.3 | 73.0* / 31.5 / 3.5 | 6104 | yes |
| firecracker | `devices::virtio::queue::verification::verify_is_empty` | cadical | 156.6 / 142.4 / 4.0 | 442.1* / 428.7 / 1.8 | 43.6 / 27.8 / 1.8 | 6124 | yes |
| firecracker | `devices::virtio::queue::verification::verify_pop` | cadical | 237.3 / 217.9 / 5.1 | 686.3* / 666.8 / 2.6 | 82.7 / 61.1 / 2.1 | 6499 | yes |
| firecracker | `devices::virtio::queue::verification::verify_prepare_kick` | cadical | 140.3 / 124.7 / 4.6 | 527.4* / 512.6 / 3.1 | 193.9* / 171.3 / 1.8 | 6174 | yes |
| firecracker | `devices::virtio::queue::verification::verify_size_raised_after_initialize` | cadical | 38.1 / 15.7 / 3.0 | 275.1* / 254.1 / 1.7 | 832.7* / 802.4 / 2.0 | 6374 | yes |
| firecracker | `devices::virtio::queue::verification::verify_spec_2_6_7_2` | cadical | 154.2 / 136.9 / 4.6 | 484.6* / 468.8 / 2.2 | 36.8 / 18.5 / 1.8 | 6180 | yes |
| firecracker | `devices::virtio::queue::verification::verify_try_enable_notification` | cadical | 10.1 / 0.5 / 0.5 | 12.3 / 3.2 / 0.4 | 31.3 / 21.8 / 0.4 | 6620 | yes |
| firecracker | `devices::virtio::queue::verification::verify_undo_pop` | cadical | 205.2 / 185.5 / 5.3 | 404.9* / 387.2 / 2.5 | 72.1 / 51.5 / 2.1 | 6605 | yes |
| firecracker | `devices::virtio::queue::verification::verify_used_ring_avail_event` | cadical | 107.0 / 91.8 / 3.9 | 234.1* / 219.5 / 1.6 | 494.9* / 471.4 / 1.9 | 6225 | yes |
| firecracker | `devices::virtio::queue::verification::verify_used_ring_idx_set` | cadical | 21.2 / 6.7 / 2.4 | 134.6* / 122.2 / 1.5 | 180.8* / 162.4 / 1.7 | 6202 | yes |
| firecracker | `devices::virtio::queue::verification::verify_used_ring_ring_set` | cadical | 30.6 / 13.2 / 3.0 | 144.2* / 128.8 / 1.8 | 475.3* / 453.7 / 2.1 | 6218 | yes |
| firecracker | `rate_limiter::verification::gcd_contract_harness` | cadical | 25.7 / 24.8 / 0.8 | 51.5* / 50.9 / 0.3 | 10.1 / 9.3 / 0.3 | 154 | yes |
| firecracker | `rate_limiter::verification::verify_token_bucket_auto_replenish` | cadical | 13.1 / 12.2 / 0.3 | 32.0* / 31.1 / 0.1 | 67.7* / 66.6 / 0.2 | 578 | yes |
| firecracker | `rate_limiter::verification::verify_token_bucket_force_replenish` | cadical | 34.7 / 33.3 / 0.2 | 50.9* / 49.5 / 0.1 | cap / - / 0.0 | 1412 | yes (2 finished) |
| firecracker | `rate_limiter::verification::verify_token_bucket_reduce` | cadical | 28.1 / 26.6 / 0.2 | 69.8* / 68.2 / 0.1 | 1064.1* / 1062.6 / 0.9 | 1508 | yes |
| lading-throttle | `stable::verification::max_requests_per_interval_bounded` | cadical | 8.8 / 6.5 / 0.2 | 10.2 / 7.9 / 0.1 | 6.4 / 4.2 / 0.2 | 384 | yes |
| s2n-core | `crypto::tls::tests::parse_hello_test` | cadical | 7.1 / 0.1 / 0.4 | 8.8 / 1.9 / 0.3 | 7.2 / 0.2 / 0.3 | 1043 | yes |
| s2n-core | `frame::stream::tests::try_fit_test` | kissat | 6.1 / 3.2 / 0.2 | 9.7 / 6.9 / 0.1 | 6.2 / 3.4 / 0.2 | 589 | yes |
| s2n-core | `inet::checksum::tests::differential` | cadical | 99.8 / 78.6 / 1.6 | 154.5* / 132.1 / 1.0 | cap / - / 0.0 | 1034 | yes (2 finished) |
| s2n-core | `inet::checksum::tests::u32_u16_differential` | kissat | 5.2 / 0.9 / 0.3 | 15.5 / 11.3 / 0.2 | 4.7 / 0.6 / 0.2 | 906 | yes |
| s2n-core | `inet::ipv4::tests::header_getter_setter_test` | kissat | 9.2 / 0.1 / 0.3 | 9.1 / 0.3 / 0.2 | 9.0 / 0.1 / 0.2 | 1278 | yes |
| s2n-core | `inet::ipv6::tests::header_getter_setter_test` | kissat | 38.2* / 0.5 / 0.6 | 33.1 / 0.7 / 0.5 | 32.9 / 0.2 / 0.6 | 1576 | yes |
| s2n-core | `inet::ipv6::tests::scope_test` | kissat | 10.3 / 0.2 / 0.3 | 11.2 / 1.2 / 0.3 | 10.0 / 0.1 / 0.3 | 1019 | yes |
| s2n-core | `packet::number::sliding_window::test::insert_test` | kissat | 160.3 / 25.5 / 1.9 | 174.9 / 38.7 / 1.3 | 308.3* / 160.1 / 1.3 | 831 | yes |
| s2n-core | `packet::number::tests::round_trip` | minisat | 10.6 / 2.1 / 0.5 | 24.9 / 16.6 / 0.3 | 10.1 / 1.9 / 0.3 | 1450 | yes |
| s2n-core | `recovery::rtt_estimator::test::weighted_average_test` | cadical | 33.6 / 32.9 / 0.1 | 128.3* / 127.6 / 0.1 | cap / - / 0.0 | 353 | yes (2 finished) |
| s2n-core | `slice::tests::vectored_copy_fuzz_test` | kissat | 78.4 / 42.9 / 4.0 | 192.9 / 158.6 / 1.9 | 188.8* / 150.1 / 2.1 | 1582 | yes |
| s2n-core | `sync::cursor::tests::oracle_test` | kissat | 95.3 / 55.8 / 2.0 | 138.6 / 101.3 / 1.4 | 128.7* / 83.8 / 1.4 | 1820 | yes |
| s2n-core | `sync::spsc::tests::alloc_test` | kissat | 289.5* / 262.0 / 24.1 | 248.5 / 223.7 / 8.2 | 167.9 / 143.7 / 10.9 | 1263 | yes |
| s2n-core | `varint::tests::eight_byte_sequence_test` | cadical | 7.6 / 1.5 / 0.5 | 7.4 / 1.5 / 0.3 | 7.3 / 1.3 / 0.4 | 1061 | yes |
| s2n-core | `varint::tests::four_byte_sequence_test` | cadical | 7.2 / 1.1 / 0.5 | 6.9 / 0.9 / 0.3 | 6.6 / 0.7 / 0.4 | 1061 | yes |
| s2n-core | `varint::tests::one_byte_sequence_test` | cadical | 5.7 / 0.4 / 0.4 | 6.0 / 0.7 / 0.3 | 5.4 / 0.3 / 0.3 | 1039 | yes |
| s2n-core | `varint::tests::round_trip_values_test` | cadical | 8.4 / 1.3 / 0.5 | 14.2 / 7.2 / 0.3 | 9.5 / 2.3 / 0.4 | 1007 | yes |
| s2n-core | `varint::tests::two_byte_sequence_test` | cadical | 6.7 / 0.8 / 0.5 | 6.4 / 0.8 / 0.3 | 6.4 / 0.6 / 0.3 | 1063 | yes |
| s2n-core | `xdp::decoder::tests::decode_test` | cadical | 4.5 / 1.1 / 0.3 | 7.7 / 4.3 / 0.2 | 4.0 / 0.8 / 0.2 | 456 | yes |
| s2n-platform | `message::cmsg::tests::collect_test` | minisat | 81.4* / 61.6 / 2.9 | 165.9* / 146.7 / 1.4 | 77.3 / 60.1 / 1.5 | 467 | yes |
| s2n-platform | `message::cmsg::tests::iter_test` | minisat | 14.6 / 11.9 / 0.7 | 21.4 / 18.7 / 0.3 | 15.2 / 12.5 / 0.4 | 265 | yes |
| s2n-platform | `message::cmsg::tests::round_trip_test` | kissat | 172.9 / 38.1 / 4.5 | 323.0 / 206.1 / 2.5 | 1088.3* / 948.2 / 2.6 | 2489 | yes |
| s2n-platform | `message::msg::tests::address_inverse_pair_test` | cadical | 6.1 / 0.2 / 0.3 | 6.5 / 0.7 / 0.2 | 7.9 / 0.1 / 0.3 | 1436 | yes |
| s2n-platform | `message::msg::tests::handle_get_set_test` | minisat | 55.6* / 1.5 / 0.7 | 47.8 / 4.4 / 0.7 | 52.0 / 0.6 / 0.8 | 2109 | yes |
| s2n-platform | `message::tests::rx_message_test` | minisat | 15.7 / 6.0 / 0.6 | 16.4 / 7.6 / 0.4 | 14.6 / 5.0 / 0.5 | 1444 | yes |
| x86_64 | `structures::paging::frame::proofs::phys_frame_range_inclusive_nth` | cadical | 10.1 / 8.5 / 0.2 | 23.4 / 21.9 / 0.1 | 17.3 / 15.6 / 0.2 | 158 | yes |
| x86_64 | `structures::paging::frame::proofs::phys_frame_range_inclusive_nth_back` | cadical | 9.1 / 7.8 / 0.2 | 34.1 / 32.8 / 0.1 | 12.3 / 11.0 / 0.2 | 146 | yes |
| x86_64 | `structures::paging::frame::proofs::phys_frame_range_nth` | cadical | 7.3 / 6.3 / 0.2 | 25.6 / 24.7 / 0.1 | 7.8 / 6.8 / 0.1 | 130 | yes |
| x86_64 | `structures::paging::frame::proofs::phys_frame_range_nth_back` | cadical | 8.9 / 7.9 / 0.2 | 16.6 / 15.6 / 0.1 | 7.7 / 6.8 / 0.1 | 129 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_inclusive_nth` | cadical | 12.7 / 7.4 / 0.5 | 22.6* / 16.1 / 0.3 | 32.0* / 25.3 / 0.4 | 338 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_inclusive_nth_back` | cadical | 13.5 / 9.1 / 0.5 | 24.6 / 20.1 / 0.3 | 15.2 / 10.8 / 0.4 | 328 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_inclusive_nth_back_panic` | cadical | 8.1 / 4.2 / 0.5 | 15.0 / 11.0 / 0.3 | 10.5 / 6.7 / 0.3 | 328 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_inclusive_nth_panic` | cadical | 8.6 / 3.8 / 0.5 | 15.7 / 10.9 / 0.3 | 26.5 / 21.9 / 0.4 | 338 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_nth` | cadical | 9.5 / 5.2 / 0.4 | 16.7 / 12.4 / 0.2 | 9.5 / 5.3 / 0.3 | 304 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_nth_back` | cadical | 10.1 / 6.0 / 0.4 | 19.4 / 15.5 / 0.2 | 9.8 / 5.9 / 0.3 | 306 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_nth_back_panic` | cadical | 5.5 / 2.7 / 0.4 | 11.1 / 8.1 / 0.2 | 6.2 / 3.3 / 0.3 | 306 | yes |
| x86_64 | `structures::paging::page::proofs::page_range_nth_panic` | cadical | 5.1 / 2.1 / 0.4 | 8.4 / 5.4 / 0.2 | 10.7 / 7.6 / 0.3 | 304 | yes |
| zerocopy | `<layout::DstLayout>::extend::proof` | kissat | 22.4* / 6.4 / 1.4 | 29.4 / 13.8 / 1.7 | 20.7 / 6.2 / 1.4 | 458 | yes |
| zerocopy | `<layout::DstLayout>::extend::{closure#0}::{closure#1}::{closure#1}::proof` | kissat | 22.1* / 6.3 / 1.4 | 29.4 / 13.8 / 1.7 | 20.7 / 6.2 / 1.4 | 458 | yes |
| zerocopy | `<layout::DstLayout>::extend::{closure#2}::{closure#1}::proof` | kissat | 22.3* / 6.4 / 1.4 | 29.4 / 13.8 / 1.7 | 20.7 / 6.2 / 1.4 | 458 | yes |
| zerocopy | `<layout::DstLayout>::extend::{closure#3}::{closure#0}::proof` | kissat | 22.3* / 6.4 / 1.4 | 29.4 / 13.9 / 1.7 | 20.7 / 6.2 / 1.4 | 458 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::proof_padded` | cadical | 5.9 / 0.0 / 0.6 | 9.7 / 3.3 / 0.9 | 5.8 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::proof_zst` | cadical | 5.8 / 0.0 / 0.6 | 9.7 / 3.4 / 0.9 | 5.8 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#0}::{closure#1}::{closure#1}::proof_padded` | cadical | 5.8 / 0.0 / 0.6 | 9.6 / 3.3 / 0.9 | 5.8 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#0}::{closure#1}::{closure#1}::proof_zst` | cadical | 5.8 / 0.0 / 0.6 | 9.7 / 3.4 / 0.9 | 5.7 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#2}::{closure#1}::proof_padded` | cadical | 5.9 / 0.0 / 0.6 | 9.7 / 3.3 / 0.9 | 5.7 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#2}::{closure#1}::proof_zst` | cadical | 5.8 / 0.0 / 0.6 | 9.7 / 3.3 / 0.9 | 5.7 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#3}::{closure#0}::proof_padded` | cadical | 5.8 / 0.0 / 0.6 | 9.7 / 3.4 / 0.9 | 5.7 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `<layout::DstLayout>::for_unpadded_type::{closure#3}::{closure#0}::proof_zst` | cadical | 5.8 / 0.0 / 0.6 | 9.7 / 3.4 / 0.9 | 5.8 / 0.0 / 0.6 | 244 | yes |
| zerocopy | `byteorder::tests::test_endian_signed` | cadical | 12.3 / 1.3 / 0.4 | 13.4 / 2.5 / 0.3 | 11.6 / 0.7 / 0.3 | 957 | yes |
| zerocopy | `byteorder::tests::test_endian_unsigned` | cadical | 12.2 / 1.3 / 0.4 | 13.4 / 2.5 / 0.3 | 11.4 / 0.7 / 0.3 | 957 | yes |
| zerocopy | `layout::add_scaled_metadata::proof` | kissat | 47.3* / 47.1 / 0.1 | 5.3 / 5.1 / 0.0 | cap / - / 0.0 | 169 | yes (2 finished) |
| zerocopy | `layout::add_scaled_metadata::{closure#0}::{closure#1}::{closure#2}::proof` | kissat | 46.3* / 46.0 / 0.1 | 5.3 / 5.1 / 0.0 | cap / - / 0.0 | 169 | yes (2 finished) |
| zerocopy | `layout::add_scaled_metadata::{closure#2}::{closure#2}::proof` | kissat | 47.8* / 47.6 / 0.1 | 5.3 / 5.1 / 0.0 | cap / - / 0.0 | 169 | yes (2 finished) |
| zerocopy | `layout::add_scaled_metadata::{closure#3}::{closure#1}::proof` | kissat | 46.5* / 46.3 / 0.1 | 5.3 / 5.1 / 0.0 | cap / - / 0.0 | 169 | yes (2 finished) |
| zerocopy | `layout::max_elems_for_bytes::proof` | kissat | 35.7* / 35.5 / 0.1 | 8.8 / 8.6 / 0.0 | cap / - / 0.0 | 176 | yes (2 finished) |
| zerocopy | `layout::max_elems_for_bytes::{closure#0}::{closure#1}::{closure#1}::proof` | kissat | 34.4* / 34.2 / 0.1 | 8.8 / 8.7 / 0.0 | cap / - / 0.0 | 176 | yes (2 finished) |
| zerocopy | `layout::max_elems_for_bytes::{closure#2}::{closure#1}::proof` | kissat | 35.7* / 35.5 / 0.1 | 8.8 / 8.6 / 0.0 | cap / - / 0.0 | 176 | yes (2 finished) |
| zerocopy | `layout::max_elems_for_bytes::{closure#3}::{closure#0}::proof` | kissat | 34.7* / 34.5 / 0.1 | 8.7 / 8.6 / 0.0 | cap / - / 0.0 | 176 | yes (2 finished) |
| zerocopy | `layout::proofs::prove_dst_layout_extend_dst_panics` | cadical | 70.4 / 67.6 / 0.4 | 148.8* / 146.0 / 0.2 | 148.8* / 145.4 / 0.5 | 995 | yes |
| zerocopy | `layout::proofs::prove_dst_layout_pad_to_align_dst_inner_rounding_formula` | kissat | 6.2 / 6.0 / 0.1 | 6.1 / 5.9 / 0.0 | 4.0 / 3.8 / 0.1 | 133 | yes |
| zerocopy | `layout::proofs::prove_dst_layout_pad_to_align_dst_outer_alignment_formula` | kissat | 55.1 / 54.8 / 0.1 | 207.1 / 206.8 / 0.1 | 249.8* / 249.5 / 0.2 | 140 | yes |
| zerocopy | `layout::proofs::prove_max_trailing_bytes` | kissat | 56.6 / 55.6 / 0.1 | 59.5 / 58.6 / 0.1 | 139.4* / 138.3 / 0.2 | 495 | yes |
| zerocopy | `layout::proofs::prove_max_trailing_bytes_components` | kissat | 14.6 / 13.9 / 0.1 | 11.4 / 10.7 / 0.1 | 19.4 / 18.7 / 0.1 | 367 | yes |
| zerocopy | `layout::proofs::prove_num_ext_affine_fallback_matches_checked_arithmetic` | kissat | 19.4* / 19.3 / 0.1 | 6.2 / 6.1 / 0.0 | cap / - / 0.0 | 16 | yes (2 finished) |
| zerocopy | `layout::proofs::prove_slice_dst_validator_rejects_larger_trailing_size` | kissat | 54.6* / 53.6 / 0.1 | 29.6 / 28.6 / 0.1 | 61.3* / 60.2 / 0.2 | 485 | yes |
| zerocopy | `layout::proofs::prove_slice_dst_validator_selected_size_fits` | kissat | 9.5 / 9.2 / 0.1 | 9.5 / 9.2 / 0.0 | 10.8 / 10.5 / 0.1 | 234 | yes |
| zerocopy | `layout::proofs::prove_slice_formula_contains_trailing_slice` | cadical | 23.9 / 23.3 / 0.1 | 25.5* / 24.9 / 0.0 | 19.3 / 18.8 / 0.1 | 309 | yes |
| zerocopy | `layout::proofs::prove_trailing_slice_layout_advance` | cadical | 85.4 / 82.3 / 0.3 | 102.4* / 99.2 / 0.2 | 68.5 / 65.6 / 0.3 | 664 | yes |

## Against the pass rule

The rule is 100% status agreement and a virtual best of at least 1.3x over the
default on total CBMC time. Agreement is 96 of 96 harnesses (100%), and the
idle-host virtual best is 1.43x. **S7 passes.** The speedup comes from choosing
the solver per harness. A single solver per project gives 1.03x, so the
design's next step (solver identity in the Kani key, with a remembered best
solver per harness and an opt-in race) is where the gain is. The memory figures
above limit any race.

## Limits

- The task asked for three repetitions of every pair on an idle host. Rep 1
  ran six at a time, and only 146 of 288 pairs have all three reps. The verdict
  rests on idle times for the default and the fastest solver of every harness.
- 15 pairs capped at 20 minutes in rep 1 were not rerun. Their statuses are
  compared only between the two solvers that finished.
- Statuses were compared at the CBMC level on replayed inputs, not through
  `cargo kani`'s own result processing. Times are CBMC only, excluding build,
  `goto-cc` and `goto-instrument`.
- One host type, one Kani release and one commit per project.

## Attempts

- 2026-11-27, pass 1, first try: lading_throttle failed in a concurrent
  `rustup` toolchain download (`could not rename 'downloaded' file`); zerocopy
  rejected the extra check flags with a usage error (exit 2); Firecracker's
  build script failed on Ubuntu's libseccomp 2.5.5.
- 2026-11-27, pass 1, second try: zerocopy with its CI feature failed to
  compile (E0658, 7 errors); Firecracker failed the same way again.
- 2026-11-27, pass 1, third try: all built. zerocopy 93 of 94 harnesses
  successful, Firecracker 41 of 41, every other project all successful.
- 2026-11-27, first session interrupted at an agent usage limit during rep 1;
  the host kept running, and rep 1 finished at 23:40 UTC.
- 2026-11-28, idle reps 23:40 to 02:20 UTC, stopped at the planned deadline
  after 348 of 404 planned runs.

## Evidence

Raw outputs stay outside the repository (ADR 0038):

- `2026-11-27-kani-spike-sat-solvers.tar.gz`, 1,465,522,173 bytes, sha256
  `cb5ecac03b9c2980aa0253888b8269686f9acf8b7947f2ce14dee75514355bd3`: the
  host's work directory without GOTO files (their sha256 kept). It holds pass-1
  logs and captured argv, CBMC output and `time -v` for each harness, every
  timed run's argv, compressed JSON output, `time -v` and exit record, the
  solver-selection and Kissat checks, and the scripts.
- `2026-11-27-kani-spike-sat-solvers-analysis.tar.gz`, 47,213 bytes, sha256
  `358b47672afe486ba2238320c33a4022041dba980c8f08e78fc6c723d6d352c1`: the
  analysis scripts and their outputs, including the per-harness table (for each
  solver: idle wall time, solver time and peak RSS), `runs.tsv` and
  `harness.tsv`.
