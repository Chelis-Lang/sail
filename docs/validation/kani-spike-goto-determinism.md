# Kani spike S3: GOTO determinism

Status: **passes for rebuilds at a fixed checkout path, with a five-part
patch**. Stock Kani 0.68.0 reproduces 0 of 49 harness inputs byte for byte
across ten rebuilds. A 146-line patch to Kani makes 49 of 49 identical at the
CBMC-input level across ten rebuilds from one checkout path, with verdicts and
checks equal to stock. Across two checkout paths `arrayvec` stays 9 of 9;
`s2n-quic-core` drops to 1 of 40 because its own test sources compile the
checkout path into the program. The allocation-name fix alone is not enough:
four more causes had to be removed. This is spike S3 of the Kani phase design
(`docs/design/kani-phase.md`, ADR 0180); nothing here is product code.

## Environment

| | |
|---|---|
| Host | one `c7i.8xlarge` (32 vCPU, 64 GiB), us-east-2, Ubuntu 24.04, lane `kani-s3`, 2026-11-27, about 1.2 hours |
| Kani | 0.68.0 release bundle (`cargo install --locked kani-verifier --version 0.68.0`, `cargo kani setup`); source at tag `kani-0.68.0`, commit `0d2328a93f0e0ff66132d6bfa1a7d884877cf862` |
| CBMC | 6.11.0 from the release bundle for every arm (`cbmc`, `goto-cc`, `goto-instrument`, `kissat`), so arms differ only in `kani-compiler` and `kani-driver` |
| Rust | `nightly-2026-08-21`, rustc 1.100.0-nightly (8925ea358 2026-08-20) |
| `arrayvec` | 0.7.8 with the nine pilot harnesses from `spike/v030-kani` (`spikes/kani-pilot/pilot`, its `Cargo.lock`) |
| `s2n-quic-core` | aws/s2n-quic at `e11382fda8a52979a2f952a789efc616db4ff4e4`, 40 harnesses, `Cargo.lock` generated once with the pinned nightly (sha256 `5b7738d1…6cd2b3`) and copied to both checkouts |

Arms: **stock** is the release bundle; **source** is the unpatched source
build; **patched** is the source build with the patch below.

## Method

Each rebuild removes the target directory and runs `cargo kani -j N
--output-format terse` in the project. Odd-numbered builds run from
`~/work/co/a`, even-numbered from `~/work/elsewhere/deeper/second-b`. Builds 1
to 5 use 32 Cargo jobs and `-j 16`, builds 6 to 10 use 3 Cargo jobs and `-j 5`.

Two levels are hashed per harness:

- **L1**: the `*.symtab.out` file `kani-compiler` writes.
- **L2**: the exact file `cbmc` is started on, after Kani's own `goto-cc` link,
  `goto-cc --function` and `goto-instrument` passes. A wrapper installed as the
  bundle's `cbmc` copies and hashes that file and records argv before running
  the real binary. This replaces `--only-codegen`, which stops before
  `goto-instrument`. For rebuilds that only need the bytes, the wrapper replays
  one recorded CBMC output instead of solving.

Differences are classified by dumping one file per distinct hash with
`goto-instrument --show-symbol-table` (and `--show-goto-functions` at L2) and
comparing line multisets under named normalisations. Verdicts and checks come
from real runs: the raw `cbmc --json-ui` output captured by the wrapper, and
Kani's summary.

## Part A: stock Kani 0.68.0

Harnesses byte-identical across all ten rebuilds:

| Project | L1 compiler output | L2 CBMC input | L2 within one checkout path (5 builds) |
|---|---|---|---|
| `arrayvec` (9) | 1 of 9 | 0 of 9 | 1 of 9 |
| `s2n-quic-core` (40) | 0 of 40 | 0 of 40 | 5 of 40 |

The unpatched source build gives the same fractions (1 of 9 and 0 of 9; 0 of 40
and 0 of 40; 1 and 5 within a path), so it reproduces the release.

Every difference falls into one of five causes. Counts are harnesses at L1;
L2 carries the same causes plus the working directory.

| Cause | Where | `arrayvec` | `s2n-quic-core` |
|---|---|---|---|
| `AllocId` numbers in constant-allocation names (Kani #4821) | `codegen/operand.rs`, `format!("{}::{alloc_id:?}", …)` | 8 | 26 |
| `DefId { id: N }` and `Ty { id: N }` table indices inside the description of the "Correct CBMC vtable size" sanity check | `codegen/rvalue.rs`, `{:?}` of `operand_type.kind()` | 7 | 20 |
| Symbol order in the file. The table is a `BTreeMap` keyed by interned string, ordered by interning order in the compiler process. Harnesses are taken from a `HashMap` with a random hasher and stably sorted by reachable-item count, so equally sized harnesses are compiled in a different order each run | `cprover_bindings` `write_symbol_table`; `codegen_units.rs`, `compiler_interface.rs` | 0 | 9 (the `ct::tests` family; three `varint` tests more once the first two causes are fixed) |
| Working directory of `goto-cc` (for `kani_lib.c`) and of `goto-instrument --add-library` recorded in source locations. L2 only | `kani-driver` `call_goto_cc.rs`, `call_goto_instrument.rs` | 9 of 9 files | 40 of 40 files |
| `env!("CARGO_MANIFEST_DIR")` compiled into the program as a byte array by bolero's `check!()` | the project's test source | 0 | 39 |

Only the first is in #4821, which reports "nothing else differs" for one
harness. The last is program content and no Kani change removes it.

Absolute paths in the bytes (build 1 of each arm, harness files containing the
string):

| Path family | L1 | L2 |
|---|---|---|
| Checkout root as a string | none | every file in stock and source; none when patched |
| Checkout root as a byte-array constant | 39 of 40 `s2n-quic-core`, 0 of 9 `arrayvec` | same |
| `CARGO_HOME` registry (`~/.cargo/registry/src/…`) | 9 of 9, 39 of 40 | same |
| Rust toolchain sources | every file: `/home/runner/.rustup/…` in the release (its builder's path), the local `~/.rustup/…` in a source build | same |
| Kani install directory (`kani_lib.c`) | none | every file, all arms |

CBMC's argv also carries the absolute path of the goto file.

## Part B: patched compiler

Build from source, following `docs/src/build-from-source.md`: `git submodule
update --init charon` (1 s), `scripts/setup/ubuntu/install_deps.sh` (23 s),
`cargo build-dev -- --release` (75 s). Rebuilds after each patch step took 6 to
17 s. The patched tree is a copy of the built tree.

The patch is `kani-0.68.0-deterministic-goto.diff` in the archive, 146 lines
over five files:

1. `operand.rs`: constant allocations are named
   `{crate}::alloc_{fingerprint}`, where the fingerprint is rustc's own stable
   hash of the global allocation (`AllocId::stable_hash` under
   `with_stable_hashing_context`). It covers bytes, init mask, provenance by
   content recursively, alignment and mutability for memory, type and trait for
   a vtable, and the definition path for a static. `codegen_const_allocation`
   already shares one symbol between equal `Allocation` values through
   `alloc_map`; that partition is unchanged. If a name is already taken by a
   different allocation, `::dup` is appended until it is free.
2. `rvalue.rs`: the sanity-check description prints the type with `Display`
   (`arrayvec::CapacityError<u8>`) in place of `Debug` of its kind.
3. `goto_binary_serde.rs`: symbols are written sorted by name.
4. `call_goto_cc.rs`: the link step runs in the directory of `kani_lib.c` when
   every path it is given is absolute.
5. `call_goto_instrument.rs`: `goto-instrument` runs in the same directory.

Harnesses byte-identical with the patched build:

| Project | Builds | L1 | L2 across both paths | L2 within each path |
|---|---|---|---|---|
| `arrayvec` | 10 (5 per path) | 9 of 9 | 9 of 9 | 9 of 9 |
| `s2n-quic-core` | 20 (10 per path) | 1 of 40 across paths | 1 of 40 | 40 of 40 in path A and 40 of 40 in path B |

The 39 `s2n-quic-core` harnesses that differ between paths take exactly two
values, one per path, and the dump comparison reduces every one of them to the
`CARGO_MANIFEST_DIR` constant.

Verdicts and checks, stock against patched, one real run each (`-j 16`):

| | `arrayvec` | `s2n-quic-core` |
|---|---|---|
| Kani summary | identical: 6 successful, 3 failed, same three names | identical: 40 successful |
| CBMC exit code and status per harness | 9 of 9 equal | 40 of 40 equal |
| Checks compared | 2,127 | 31,365 |
| Harnesses with identical (id, description, location, status) multiset, install-path prefixes mapped | 2 of 9 | 20 of 40 |
| The same, with the vtable sanity-check description masked | 9 of 9 | 40 of 40 |

Two fields keep the exact comparison from being 100%, and neither is a
verdict. Locations under the toolchain and the Kani library differ between a
release bundle and a source build because the install paths differ (source
against patched shows the same). The sanity-check description is the field the
patch changes on purpose; stock does not reproduce it either: two stock runs of
`arrayvec` agree exactly on 2 of 9 harnesses, in all nine pairs of build 1
against builds 2 to 10. The ten patched `arrayvec` runs agree exactly on 9 of 9.

Wall time for the full `s2n-quic-core` run was 277 s stock, 266 s source and
288 s patched, single samples on a host also running analysis, so no
performance claim.

## Edit experiment

Patched compiler, `s2n-quic-core`, checkout A, compared with the unedited
build from the same path.

| Edit | Harnesses whose CBMC input changed |
|---|---|
| Body of `Checksum::carry_rfc` in `inet/checksum.rs`, same line count (`state` to `state.wrapping_add(0)`) | 3 of 40: the three `inet::checksum` harnesses |
| One comment line inserted after the licence header of `varint/mod.rs` | 15 of 40: six `varint`, six `packet::number`, two `frame`, one `stream::iter` |

L1 changed for the same harnesses in both cases. The second row is the
line-shift miss the design accepts.

## Result against the pass rule

Rule: patched, at least 99% of harnesses byte-identical across rebuilds at the
CBMC-input level, with verdicts and checks identical to stock.

- Across rebuilds at one checkout path: 49 of 49 (100%), on both paths.
- Across two checkout paths: 9 of 9 for `arrayvec`, 1 of 40 for
  `s2n-quic-core`. The cause is the project's sources, and the bytes do differ.
- Verdicts identical. Checks identical except the one sanity-check description
  that stock itself does not reproduce.

Consequences for the design:

- The fork is five small fixes, two of them in `kani-driver`. The design's
  "name allocations by content hash" is one of the five.
- An exact-bytes key at the CBMC boundary hits across rebuilds in one checkout.
  It does not hit across checkouts for bolero-based harnesses, and across
  machines it also needs the same `CARGO_HOME`, toolchain and Kani install
  paths. The signed CI tier has to fix those paths or accept the miss.
- The key must not bind CBMC's argv verbatim: it contains the target path.
- The patch changes a check description, so the stock-versus-fork conformance
  has to name that field.

## Limits

- One host. The rule's second host was not run.
- Two projects, 49 harnesses. No harness here uses contracts or loop contracts.
- The patched build was not run through Kani's own regression suite.
- Whether any `::dup` name was produced was not counted.
- The harness compile order is still random among equally sized harnesses; the
  sorted writer hides it in every build measured.
- Goto binaries and raw CBMC JSON were not retained. Hashes, argv, compact
  per-check records and the dump classification were.

## Attempts

| # | Kani tree | Result | Cause |
|---|---|---|---|
| 1 | stock release | `arrayvec` L2 0 of 9, `s2n-quic-core` L2 0 of 40 | five causes above |
| 2 | source, unpatched | same fractions | same |
| 3 | allocation names only | `arrayvec` L1 2 of 9, L2 0 of 9 | debug indices in the sanity-check description |
| 4 | plus `Display` in the description | `arrayvec` L1 9 of 9, L2 0 of 9 | working directory from the `goto-cc` link |
| 5 | plus fixed `goto-cc` directory | `arrayvec` L2 3 of 9 | working directory from `goto-instrument --add-library` |
| 6 | plus fixed `goto-instrument` directory | `arrayvec` 9 of 9 at both levels; `s2n-quic-core` 28 of 40 within a path | symbol order follows harness compile order |
| 7 | plus sorted symbols (final) | tables above | |

Attempts 3 to 5 were overwritten by the next attempt, so their fractions come
from the run output; attempts 1, 2, 6 and 7 are in the archive. One analysis
run was lost when a `pkill` pattern matched its own shell; it was rerun
detached.

## Evidence

`2026-11-27-kani-spike-goto-determinism.tar.gz`, 4,028,481 bytes, sha256
`b7ba6896771b8810a3497b84774464fcc5b35f16ad4673fbb30c1d0542426b7a`, kept
outside the repository. It holds the patch and its intermediate form, every
script, per-build hashes at both levels, CBMC argv, driver logs, Kani metadata,
the classification with samples, compact per-check records for every real
run, the two edit diffs, build logs and tool hashes.
