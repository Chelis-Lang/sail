# Release-candidate validation

This record covers the release candidate for Sail's first open-source release
([roadmap](../ROADMAP.md#open-source-release-planned)): the required gates,
the standard and deep tiers, the release build and its smoke tests, and a
repeat of the [new-user walkthrough](../reports/2026-10-13-new-user-walkthrough.md)
on a clean Linux host, including the real Claude Code CLI. Clean-macOS
validation is a separate lane (L39) and has its own section below.

| Field | Value |
|---|---|
| Status | **Code: pass.** 0 Different in every suite and arm, Unstable only with plain-Verus evidence, and the Singular engine arm reports `engine-lacks-feature`. **Release build and Linux smoke: pass.** **Linux walkthrough: works end to end.** Three findings are to be fixed before publishing, in lanes L40–L42 (below); a short revalidation of `v0.1.0-rc.2` follows them. **Clean macOS:** L39, pending |
| Source | `main` at `7782b136564ad810adf33c72749c6d75ee9483f8` (2026-10-14) for every suite. Release candidate **`v0.1.0-rc.1`** (draft), built from `main` at `958dee95321165f30cca25b5ebe95ac1251eb62e` |
| Runtime | CI image `sail-ci:14df87b176833bea`, built on each host from the committed inputs; stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0; Linux engine runtime `engine-runtime-13a1176c-linux` (manifest `5b866414…7603`); `rust_verify_test` from the source build of `426d8b0` |
| Environment | Native EC2 c7i.8xlarge hosts (32 vCPU), us-east-1, Ubuntu 24.04, one suite or shard per host (layout below); `ci/run.sh host` on the developer Mac (Apple M3, macOS 26.5, shared with other lanes) |
| Limits | Correctness only; wall times are indicative, not benchmarks. Walkthrough times are one sample each. The Linux walkthrough host is the `scripts/perf-host.sh` image, which already has `build-essential`, `pkg-config`, `libssl-dev`, `python3` and `curl` |

## Suites

All suites ran on `7782b136`. The commits between it and the release
candidate are listed under [release build](#release-build); they change
versions, docs, benchmark scripts and a CI marker check, not code that the
suites exercise.

| Suite | Result |
|---|---|
| `ci/local.sh gate` | pass: 559 tests passed, 35 skipped; fmt, clippy, doctests, rustdoc, layering, deny, benchmarks page |
| `ci/local.sh solver` | pass: 35 passed |
| `ci/local.sh e2e` (legacy, managed, repair) | pass: identical to `cargo verus verify`; managed acceptance passed; `claude-code: listed 15 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance passed |
| `ci/run.sh host` (Mac) | pass: clippy clean; 548 tests passed, 35 skipped |

Conformance summary lines (units / Same / Unstable / Different / Infra; functions):

| Tier | Pass-through | Cache fill | Cache warm | Engine |
|---|---|---|---|---|
| fast PR subset | 33 / 33 / 0 / 0 / 0; 311 | 33 / 33 / 0 / 0 / 0 | 33 / 33 / 0 / 0 / 0 | 33 / 33 / 0 / 0 / 0; 32 focused |
| fast | 147 / 147 / 0 / 0 / 0; 3,450 | 147 / 147 / 0 / 0 / 0 | 147 / 147 / 0 / 0 / 0 | 147 / 147 / 0 / 0 / 0; 146 focused |
| standard | 159 / 159 / 0 / 0 / 0; 12,726 | 159 / 159 / 0 / 0 / 0 | 159 / 159 / 0 / 0 / 0 (151,393 entries) | 159 / 159 / 0 / 0 / 0; 158 focused |
| deep (Singular projects skipped, no Singular in the image) | 159 / 159 / 0 / 0 / 0; 12,726 | 159 / 159 / 0 / 0 / 0 | 159 / 159 / 0 / 0 / 0 (150,368 entries) | 159 / 159 / 0 / 0 / 0; 158 focused |
| Singular projects (`ci/local-singular.sh`, exit 0) | 3 / 3 / 0 / 0 / 0; 606 | 3 / 3 / 0 / 0 / 0 | 3 / 3 / 0 / 0 / 0 (8,080 entries) | 3 units `engine-lacks-feature` (`singular`), 0 Same, 0 Different |

The standard tier's default arms had one unit with rlimit drift and no
difference, as in earlier runs: VNR `log_entry_alive_value_wrap_around`,
51,948 against 51,945.

`ci/local.sh deep-rvt cache` (4,695 tests, 58 ignored; plain 4,635 ok and
the 2 source-build `cargo::cargo_new_*` FAILED in every mode, ADR 0019):

| Mode | Same | Unstable | Different | Infra | Wall s |
|---|---:|---:|---:|---:|---:|
| Pass-through | 4,690 | 5 | 0 | 0 | 1,131 |
| Cache fill | 4,693 | 2 | 0 | 0 | 1,113 |
| Cache warm | 4,694 | 1 | 0 | 0 | 1,082 |

Every Unstable is an `output_json` test (`test_json_custom_err_on_ensures`,
`…_custom_err_on_requires`, `…_proof_note_on_assume_and_req_with_no_cheating`,
`…_proof_note_on_requires`, `…_proof_note_on_ensures`). For each one the
report records `plain_varies: true` and `matches_a_plain_run: true`: the plain
runs disagreed among themselves on the order of `func-details` keys
([VI-0008](../verus-issues/VI-0008-func-details-hash-order.md)), and Sail's
output matched one of them (ADR 0009). Both sides `ok`.

### Shard layout

The owner approved splitting the long runs across hosts. Each shard had its
own host, cache, daemon and output directory, ran `7782b136` in
`sail-ci:14df87b176833bea` with the `--jobs` value `ci/run.sh` uses, and
covered whole arms, so every arm ran its tier's full unit list on one host.
The unit IDs of the four standard arms are identical (159), and so are those
of the four deep arms (159), with no duplicates.

| Host | Scope | Result |
|---|---|---|
| `l38` | gate, solver, e2e, conformance fast PR and fast (all arms), standard pass-through, fill and warm | pass. Its serial standard engine arm was stopped once warm finished (exit 137 by design), because `l38-sing` ran that arm |
| `l38-deep` | deep pass-through, fill and warm | pass. Its serial engine arm was stopped the same way |
| `l38-rvt` | `deep-rvt cache` | pass |
| `l38-sing` | `ci/local-singular.sh`, then the standard engine arm (`sail-conformance engine --tier standard --jobs 3`) | pass |
| `l38-s1` | deep engine arm (`sail-conformance engine --tier deep --jobs 3`) | pass |

## Release build

`scripts/release.sh 0.1.0-rc.1` built the **draft** release `v0.1.0-rc.1`
from `958dee95`. The Linux half ran on a native host (lane `l38-rel`, which
the script tore down). Both staged trees passed the script's self-check.
`sail --version` prints `sail 0.1.0`. The archives carry debug info
(`optimized + debuginfo`):

| Asset | Size | sha256 |
|---|---:|---|
| `sail-0.1.0-rc.1-x86_64-unknown-linux-gnu.tar.gz` | 57.5 MB | `fbc547bc300aa8ce35c79bc98acf81221dfbe8eb973f8eb2cbbfec8b5bc80fe3` |
| `sail-0.1.0-rc.1-aarch64-apple-darwin.tar.gz` | 35.3 MB | `d6fe42d4ce0e21650a74769672eea3e01bce9ad6a36059afe77d09bc1497fd66` |

Three commits separate the validated `7782b136` from `958dee95`:
- #104, `ci:`: `scripts/check-ai-markers.sh` rejects hostname-fallback commit
  identities;
- #105, `docs:`: `docs/performance.md`, the README's speed wording, and the
  `scripts/perf-loops.sh` and `scripts/perf-agents.sh` benchmark scripts;
- #106, `chore:`: the workspace version `0.0.0` to `0.1.0` (`Cargo.toml`,
  `Cargo.lock`, `CHANGELOG.md`), and a test-only change in
  `crates/sail-cli/src/report.rs`, so the golden report no longer depends on
  the version.

The diff between the two commits changes no product code, so the suites
above were not rerun. The owner sets the release version at publish time.

An earlier build, the draft `v0.0.0-rc.1` from `7782b136`, was made before
the version bump. Its Linux tarball (`27e4077b…18b1`) passed the same smoke
test on a fresh host (`l38-smoke`) and is the tarball the walkthrough
installed. That draft and the older `v0.0.0-preview.3` have been deleted.

## Smoke tests

`scripts/release-smoke.sh` with an empty `SAIL_CACHE_DIR`:

| Archive | Where | Result |
|---|---|---|
| `v0.1.0-rc.1` Linux (`fbc547bc…0fe3`) | fresh c7i.8xlarge (`l38-smoke2`), README steps 1, 2 and 4 only (apt prerequisites, rustup, `cargo xtask fetch-verus`) | pass: `sail 0.1.0`; bundled engine selected; `sail verify` passed; `repair-check demo::max` accepted through the bundled engine |
| `v0.0.0-rc.1` Linux (`27e4077b…18b1`, deleted draft) | fresh c7i.8xlarge (`l38-smoke`), README steps 1, 2 and 4 only (apt prerequisites, rustup, `cargo xtask fetch-verus`) | pass: bundled engine selected; `sail verify` passed (2,060 vstd and 2 demo verified); `repair-check demo::max` accepted through the bundled engine |

The macOS tarball's smoke test belongs to L39 (below).

## Linux walkthrough

The previous walkthrough repeated on a fresh c7i.8xlarge (`l38-walk`),
following only `README.md` and the guides it links to, with `main` at
`e1547b0` for the docs (it includes the [performance page](../performance.md),
#105). Install used the Linux tarball from the `v0.0.0-rc.1` draft, because
rc.1 had not been built yet. Its binaries and layout match rc.1, except that it
prints version `0.0.0`.

**Install.** apt prerequisites, rustup, then the tarball: `sha256sum -c`,
unpack under `~/.local/opt`, symlink `bin/sail` and `bin/sail-z3` into
`~/.local/bin`. Symlinks work: `sail engine status` resolves them and selects
the bundled engine. The README does not document this path (finding 1).
`cargo xtask fetch-verus` needs a Sail checkout. It downloaded the pinned
Rust 1.99 for xtask, installed Rust 1.98.1 for Verus, and printed the `PATH`
line, in 49 s.

**Demo** (`cargo verus new --lib`, five functions; one appended function
for the edits). Seconds, one sample each:

| demo | cold | warm | edit | fail | fix | warm |
|---|---:|---:|---:|---:|---:|---:|
| `cargo verus verify` | 33.8 | 0.14 | 0.85 | 0.63 | 0.86 | 0.13 |
| `sail verify` | 34.2 | 0.13 | 0.88 | 0.63 | 0.88 | 0.13 |
| `sail verify --incremental` | 31.9 | 0.35 | 1.16 | 0.87 | 1.09 | 0.36 |

**vest** (`11af01dd`, `-p vest_lib`; the edits append a function to
`vest_lib/src/lib.rs`), starting with an empty Sail cache and no managed
targets:

| vest | cold | warm | edit | fail | fix | warm |
|---|---:|---:|---:|---:|---:|---:|
| `cargo verus verify` | 72.3 | 0.23 | 39.7 | 35.3 | 39.8 | 0.25 |
| `sail verify` | 73.1 | 0.25 | 37.6 | 20.2 | 24.5 | 0.23 |
| `sail verify --incremental` | 58.4¹ | 1.48 | 28.1 | 22.3 | 27.7 | 1.47 |

¹ Run after `sail verify`, so its solver cache was warm.

`sail verify` matched `cargo verus verify` in stdout, stderr and exit code
at all 12 steps (after normalizing the directory name, `Finished … in` times
and the order of parallel `Compiling` lines). The numbers agree with the
README and the [performance page](../performance.md): cold and small runs are
the same as plain Verus. The first edit after cold gains little (37.6 s
against 39.7 s). Later vest edits gain 1.6× (24.5 s against 39.8 s).

The managed summary of the failing vest edit now reads `0 verified, 1
failed, 3127 passed by Verus's totals (not attributed one by one), 0 unknown
(functions; Verus counts verification units: 3237 verified, 1 errors)`.

**MCP.** `claude mcp add --transport stdio sail -- sail mcp` in the project,
then `claude mcp list` (Claude Code 2.1.299 on the host): `sail: sail mcp - ✔
Connected`. Then the real Claude Code CLI (2.1.299, `claude -p`, stream-JSON
transcript), with `sail mcp` on the Linux host over ssh stdio, ran a fresh
demo with a bug in `max`. It verified (FAILED, counterexample `a = 1, b = 0`),
fixed the swapped branches without touching the specification, and verified
again (VERIFIED), in 6 turns and 79 s. It reported both summary first lines
correctly. Tool replies seen by the model were 3.7 KB and 2.4 KB.

Reply sizes from a minimal stdio client (single JSON-RPC frames):

| Reply | demo | vest |
|---|---:|---:|
| `tools/list` | 17.8 KB | 17.8 KB |
| `verify`, unchanged (reused) | 2.9 KB, 0.56 s | 3.0 KB, 2.1 s |
| `verify`, failing edit | 3.8 KB, 1.2 s | 4.4 KB, 22.8 s |
| `counterexample` | 2.2 KB | 2.4 KB |
| `verification_result` | 4.6 KB | 5.5 KB |
| `explain_failure` | 3.1 KB | |
| `save_baseline` / `get_baseline` | 3.2 KB | 3.6 KB |
| `cached_result` | 2.3 KB | |
| `compare_verification` after the fix | 1.0 KB: `1 recovered, 4 unchanged` | **1.26 MB**: `95 incomparable, 2921 recovered` (finding 3) |

Counterexamples now show source variables only (`a = 1, b = 0, r = 1`;
`r = 0, x = 0`).

**`sail watch`.** An edit was queued about 0.45 s after the write and
finished 1.35 s after it; the fix finished 1.53 s after its write. The
lifecycle events (`queued`, `started`, `target-reused`, `target-started`,
`target-complete`, `finished`) carry the job's currentness and exit code.

**`sail cache projects`.** `list` showed 2 workspaces, 2.8 GiB (vest
2.5 GiB). `prune --all` and `prune --missing` worked.

**Repair engine from the tarball.** `sail engine status` selected the
bundled engine; `sail repair-check --package mcpdemo --lib --function
mcpdemo::max --engine interactive` was accepted with full evidence Current.
With the binaries copied away from `engine/`, `engine status` and
`repair-check` both fail fast with `SAIL3120` and say what to do.

**`--help`.** Every subcommand prints help and exits 0, including `watch`,
`cache gc`, `cache projects` and `verify` (Sail options first). Exception:
`sail bench --help` gives `error[SAIL0003]: cannot run sail-bench` (finding 6).

## Previous walkthrough: frictions

| # | Friction (2026-10-13) | Now |
|---|---|---|
| 1 | MCP unusable in Claude Code | **Fixed.** Connected, 15 tools; a real Claude Code loop verified, fixed and re-verified |
| 2 | MCP replies of 4–16 MB | **Mostly fixed.** 1–6 KB per reply, except `compare_verification` against a baseline with incomplete attribution: 1.26 MB on vest (finding 3) |
| 3 | No managed reuse on macOS | Not retested here (L39) |
| 4 | No documented Verus install | **Fixed for a source checkout.** `fetch-verus` installs the toolchain and prints the `PATH` line, and the README lists the prerequisites. Tarball users still need a checkout (finding 1) |
| 5 | "Much faster" holds only where the solver dominates | **Fixed in the docs.** The README and performance page say so, with numbers that match this run |
| 6 | Managed counts disagree with Verus | **Fixed** for `verify` (`passed by Verus's totals`, counting unit explained). Not for comparisons (finding 3) |
| 7 | A long cache path silently disables the daemon | **Fixed.** A 133-character `SAIL_CACHE_DIR` works; the socket moves under `/tmp/sail-<hash>/` |
| 8 | The repair engine cannot be obtained | **Fixed.** It is bundled in the tarball and accepted a repair; a missing engine fails fast with `SAIL3120` |
| 9 | Non-Cargo projects unsupported, undocumented | **Fixed in the docs** (README, Limits) |
| 10 | `--help` handling | **Fixed**, except `sail bench` in the release (finding 6) |
| 11 | Unbounded disk use | **Fixed.** `sail cache projects list|prune` |
| 12 | `jobs list` shows `Unknown` | **Fixed.** It shows `Current` and `Historical` |
| 13 | Reverting does not reuse | Not fixed; out of the release (B4) |
| 14 | vstd verified per project | Not fixed; out of the release (B1); documented in the README (about 30 s per new project) |
| 15 | User guides read as validation records | **Mostly fixed.** The README is a user guide. `docs/mcp.md` still mixes in validation wording (for example "does not establish a live-reader latency gain") |

| Doc error (2026-10-13) | Now |
|---|---|
| `workspace-verification.md` socket name | Fixed |
| `proof-repair.md` `get_verification_result` | Fixed |
| `proof-repair.md` `SAIL3120` | Fixed (observed) |
| README `daemon.log` location | Fixed (`results/daemon.log`) |
| README macOS "can require fresh proofs" | Reworded; macOS behavior is L39's to check |
| `mcp.md` Claude Code registration | Fixed (observed) |
| `mcp.md` `counterexample` summary example | Fixed (matches) |
| walkthrough `fetch-verus` "installs artifacts" | Fixed |
| `errors.md` `SAIL0101` | Fixed (points to `fetch-verus`) |
| README "Getting started" is contributor setup | Fixed |

## New findings, by user impact

Findings 1–3 must be fixed before publishing and are open. They are assigned
to lanes L40 (findings 1, 4, 6 and 7), L41 (finding 2) and L42 (finding 3). A
`v0.1.0-rc.2` cut after those lanes land gets a short revalidation: the gate,
fast PR conformance, the release smoke tests and the affected walkthrough
steps.

1. **The release tarball has no install path in the README.** *Open, L40.*
   - README Install still says release binaries "will be published" and
     documents only the source build.
   - Step 4 installs Verus with `cargo xtask fetch-verus` "from the Sail
     checkout", which a tarball user doesn't have. `errors.md` `SAIL0101` and
     the `SAIL3120` message also point to a checkout. Following it means
     cloning Sail and building xtask, which also downloads Rust 1.99.
   - The README's repair section tells Linux users to fetch the engine with
     `python3 ci/engine-artifact.py` from a checkout. It also says the macOS
     runtime is not pinned yet. Both are stale: the tarballs bundle the
     engine, and `engine/verus/artifact-macos.json` pins the macOS runtime.

   *Fix:* a tarball install section (checksum, unpack, keep `bin/` and
   `engine/` together, symlink or `PATH`). Add a Verus install that needs no
   checkout, such as the Verus release zip plus `rustup toolchain install
   1.98.1`, or a `sail` subcommand. Say that the engine is bundled.
2. **After a Rust compile error, each managed run re-verifies vstd.** *Open, L41;
   agents hit compile errors often.* On vest, `sail verify --incremental`
   on a crate that fails to compile took 37.9 s on each of two runs ("2
   verifier runs, 0 target reports reused"), where plain Verus fails in 6.2 s.
   The first successful run after the fix took 58.3 s, against about 28 s for
   an ordinary edit (`walk-linux-repro-compile-error.log`). MCP `verify` and
   `watch` use the same path.
3. **`compare_verification` is misleading after a failing run with incomplete
   attribution.** *Open, L42. It undoes A2 and A3 for the compare-after-fix step
   that agents use.* After fixing the one failing
   vest function, comparing with the failing run's baseline reported `95
   incomparable, 2921 recovered` in a 1.26 MB reply. The baseline's 3,127
   "passed by Verus's totals" functions are `unknown`, so each one counts as
   `recovered`, and the totals do not add up to the 3,128 functions.
   `docs/mcp.md` says a vest comparison is 33 KB. On the demo, with complete
   attribution, the same step correctly gives `1 recovered, 4 unchanged`.
4. **`docs/ROADMAP.md`'s release table is stale.** Every merged item (A1–A6,
   release builds, public engine runtimes, the performance page, disk
   cleanup) still reads "Not started". *Open, L40.*
5. **Headline counts on a failing vest edit.** `0 verified, 1 failed, 3127
   passed by Verus's totals` is accurate and explained, but "0 verified"
   first is easy to misread. Low.
6. **`sail bench` is listed in `sail --help` but not shipped.** The release
   has no `sail-bench`, so `sail bench --help` fails with `SAIL0003`. Low;
   *open, L40.*
7. **`sail --version` prints only the workspace version** (`sail 0.1.0` for
   `v0.1.0-rc.1`), so it cannot identify an RC. The archives also ship debug
   info. Low; *open, L40.*

## Clean macOS

Owned by lane L39 on a clean EC2 Mac (M4): the macOS tarball's smoke test,
the clean-macOS walkthrough and the Mac performance numbers. To be filled by
L39.

Before the scope change, `scripts/release-smoke.sh` on the deleted `v0.0.0-rc.1`
macOS tarball passed on the shared developer Mac, in a fresh directory with an
empty cache (`mac-smoke.log`). That run is not clean-macOS evidence.

## Evidence

Raw logs and reports are kept outside the repository, in
`~/sail-raw-evidence/l38/` on the lead's machine (sha256):

| File | Contents | sha256 |
|---|---|---|
| `l38-main.tgz` | gate, solver, e2e, fast PR, fast and standard (default arms) logs and reports | `38648d04f07a0102b805b097f5d1ed1052e84e130065f8cba678b7277fe2efac` |
| `l38-deep.tgz` | deep tier default arms | `f12c36769f8be1ee0b9bff258443ba01e13a0c9e91853cdbe732094aa25d7090` |
| `l38-s1.tgz` | deep engine arm | `7a8a05cf917bd0c35d1242caf826091061a009ebc2866b3c3e333e412a94d3b0` |
| `l38-sing.tgz` | Singular projects; standard engine arm | `c464246df64eacdc47514425d4009d0107252c2575ea8617276e4f135768cfdc` |
| `l38-rvt.tgz` | `deep-rvt cache` log and reports | `b74baf1e67eaeca77065197f5f2a356e6531a3b699845eae0f6136e0b4267491` |
| `mac-host.log` | `ci/run.sh host` | `d937944c29e476b4d9793e8cd2c42864484886544e041e6d0b7a51cb2239ab17` |
| `release-rc1.log` | `scripts/release.sh 0.1.0-rc.1` | `ca60584b2c551387eb1a21f231e63d10afd868cbacbff759bd9ccc0df3db5ce4` |
| `linux-smoke-rc1.log` | Linux smoke, `v0.1.0-rc.1` | `4a2ab590f83c107652be98437f0ad3fb3325a23d633fcd04ec81c9944c29f1ec` |
| `release.log` | `scripts/release.sh 0.0.0-rc.1` (deleted draft) | `c88e190609ea9d0c4fa0e1b9816fd491652c04e7965110dbff41bedc92d2f700` |
| `linux-smoke.log` | Linux smoke, `v0.0.0-rc.1` | `0a865278011ddff42bdce51eb94a55e0626d2274638e1542bb20bbb0b1fd19b3` |
| `mac-smoke.log` | macOS smoke on the shared Mac (not clean-macOS evidence) | `331f7fff4639c742669e67be88544cebde26d4c0ded1863c6270316b50278690` |
| `walk-linux-1-install.log` | install | `94a69de6867d185f51ed5e54d0c6d56e8684361e7378e51779e529517f097a3e` |
| `walk-linux-2-demo.log` | demo sequences | `7d357cd5d8e40724c9dce289c555d0dfa78a6f44054cc3064231fc33c7fc8813` |
| `walk-linux-3-vest.log` | vest sequences | `f3dd7b29c5ac5a8e6a2220adf724bae59e7d924381b355d5399a8de70d3d5a7c` |
| `walk-linux-repro-compile-error.log` | finding 2 | `3fd9a91b566888012442a1468d89a27b842faca8a53d0158819786cb6c3caf25` |
| `walk-linux-4a-mcp.log`, `walk-linux-4b-mcp.log` | MCP registration, listing, reply sizes | `38c164a4190ba69c4db6a57146c988393ee81da99a20a170550762cb272434de`, `d7e384633ea882722d6cabdb46744c95e7e010d88e88b45f7bbcc4b5effcc697` |
| `claude-code-loop.jsonl` | Claude Code transcript | `0183e3844915662dbed22cf9ce069e37f4fb488882e360b287d06167ef65bdf2` |
| `walk-linux-outputs.tgz` | per-step stdout, stderr, exit codes and times; MCP reply records; watch events | `e4574c1cec30cf0791f07f79681226d7430a177ea750731657d6af2cb4d03678` |
| `walk-linux-5-misc.log` | watch, cache projects, engine, `--help` | `fbc69ed0cfaa4d92608fa9ef971f3323243884f79f1f5c4da0a1cf140de059bf` |
| `walk-linux-7-papercuts.log` | jobs list, long cache path, missing engine | `4a5dccb79b22f8a6902e9a406f2de1eb9b6af160ca70ae0393e7ee669dbd9498` |

## Failed or interrupted attempts

- `7782b136`, 2026-10-14, Linux smoke (`l38-smoke`): the harness failed to
  read `fetch-verus`'s indented `PATH` line, so `cargo verus` was missing.
  The rerun on the same host passed. Cause: a harness error.
- `7782b136`, 2026-10-14, vest walkthrough, first attempt: the appended test
  function lacked `use vstd::prelude::*`, so the fail and fix edits did not
  compile. The rerun from an empty cache is the one above. Cause: a harness
  error. This attempt is what exposed finding 2.
