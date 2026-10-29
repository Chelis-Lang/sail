# Clean macOS: release candidate on a fresh Mac

The macOS half of the release-candidate check: the draft release's macOS
archive on a freshly provisioned Mac, through the release smoke test and a
new-user walkthrough that follows the README. It complements the
[macOS engine record](macos-engine.md) and repeats the macOS half of the
[earlier walkthrough](../reports/2026-10-13-new-user-walkthrough.md).

| Field | Value |
|---|---|
| Status | **Pass**: smoke test passed; every walkthrough step worked; output matched plain Verus |
| Source | Draft release `v0.1.0-rc.1`, built from `main` `2c42867`; archive `sail-0.1.0-rc.1-aarch64-apple-darwin.tar.gz`, sha256 `d6fe42d4ce0e21650a74769672eea3e01bce9ad6a36059afe77d09bc1497fd66` (matches the release's `.sha256` asset) |
| Environment | EC2 mac-m4.metal, Apple M4 (4 performance and 6 efficiency cores, 24 GiB), macOS 27.0 (26A428), us-east-1b, a fresh instance (`scripts/perf-host-mac.sh`), lane L39, 2026-10-14. The Xcode command line tools come with the image. Before these runs the same instance ran the [performance](../performance.md) and engine checks; their Sail cache and Cargo registry were moved aside first, so Sail started with no cache |
| Scope | `scripts/release-smoke.sh` on the archive; the README's install and usage steps on a new `cargo verus new` project and on vest (`-p vest_lib`), each in plain, `sail verify` and `--incremental` modes (cold, warm, edit, failing edit, fix, warm); MCP in the Claude Code CLI and from a scripted stdio client; `sail watch`; `sail cache projects`; the bundled repair engine; `--help` |
| Limits | One run of each step, so times are indicative (the [performance page](../performance.md) has three-round medians from the same machine). Archives were downloaded with `gh` and `scp`, so they had no quarantine attribute; how Gatekeeper treats a browser-downloaded archive (the binaries are ad-hoc signed, not notarized) was not tested |

## Release smoke test

`scripts/release-smoke.sh` in a new `/tmp` directory with an empty
`SAIL_CACHE_DIR`, the stock Verus release on `PATH` and Rust 1.98.1: the
bundled engine was selected, `sail verify` passed on a new project, and
`repair-check demo::max` was accepted through the bundled engine, in 47 s
(`mac-smoke-rc1.log` `40662593…5973`).

## Walkthrough

**Install, about one minute.** rustup with its defaults took 17 s. The
Sail archive was unpacked to `~/opt` with `bin/sail` and `bin/sail-z3`
symlinked into `~/.local/bin`, as [releasing](../releasing.md) describes,
because the README has no archive install section yet. Then the stock Verus
release was unpacked (10 s) and its Rust toolchain 1.98.1 installed (16 s).
The README installs Verus with `cargo xtask fetch-verus` "from the Sail
checkout", which an archive user does not have, so this step used the pinned
release's URL and checksum from `corpus/toolchain.toml`. `sail --version`
printed `sail 0.1.0`, and `sail engine status` selected the bundled engine
with no setup.

**Output matches plain Verus.** In all 12 plain-against-`sail verify`
comparisons (demo and vest, six phases each), exit codes and stdout were
identical, and stderr differed only in the crate names of the separate
copies, Cargo's `Finished … in` times and the order of Cargo's parallel
`Compiling` lines.

**Loop times** (seconds, one run each; vest's `--incremental` copy ran after
the `sail verify` copy had filled the solver cache, so its cold run is not a
cold-cache time):

| | cold | warm | edit | fail | fix | warm |
|---|---:|---:|---:|---:|---:|---:|
| demo plain | 16.4 | 0.18 | 0.44 | 0.38 | 0.43 | 0.18 |
| demo `sail verify` | 17.7 | 0.18 | 0.43 | 0.39 | 0.42 | 0.18 |
| demo `--incremental` | 16.2 | 0.35 | 0.71 | 0.70 | 0.62 | 0.35 |
| vest plain | 38.7 | 0.23 | 20.9 | 19.3 | 20.8 | 0.23 |
| vest `sail verify` | 39.0 | 0.23 | 13.3 | 12.9 | 13.1 | 0.24 |
| vest `--incremental` | 31.6 | 1.34 | 16.1 | 14.0 | 16.2 | 1.33 |

**Agents and tools.**
- Claude Code 2.1.299, installed with the official installer and no
  credentials: `claude mcp add --transport stdio sail -- sail mcp`, then
  `claude mcp list` shows `sail: sail mcp - ✔ Connected`.
- A scripted MCP stdio client listed 15 tools. `verify` took 16.1 s on the
  first call (managed cold run), 0.52 s unchanged, 0.79 s after a failing edit
  and 0.87 s after the fix. Replies were 2.7–3.8 KB. The failing summary
  names `demo_sail::max`, gives the counterexample `a = 1, b = 0, r = 1` and
  the error location, and counts `4 verified, 0 unknown`, explaining that
  Verus's totals count verification units. `counterexample` answered in
  0.06 s.
- `sail watch --output jsonl`: 1.5 s from writing a failing edit to its
  `finished` event, and 1.4 s for the fix. `--help` prints help.
- `sail cache projects list` showed three managed workspaces, 2.1 GiB.
- `sail repair-check --function demo_sail::max --engine interactive` was
  accepted in 23 s through the bundled engine.
- `--help` worked for every command tried (`sail`, `verify`, `watch`,
  `cache`, `cache gc`, `cache projects`, `jobs`, `mcp`, `repair-check`,
  `engine`), each exiting 0.

## Against the previous walkthrough's frictions

| # | 2026-10-13 (developer Mac, `71dc463`) | Clean M4, `v0.1.0-rc.1` |
|---|---|---|
| 1 | MCP unusable in Claude Code (tools fetch failed) | Fixed: Connected, tools listed |
| 2 | `verify` replies 4–16 MB | Fixed: 2.7–3.8 KB, with the counterexample in the summary |
| 3 | macOS managed path never reused: 26–69 s per run | Fixed: unchanged runs 0.35 s (demo), 1.33 s (vest); MCP 0.52 s |
| 4 | No documented way to install Verus | Partly: the README documents `fetch-verus`, but only from a Sail checkout; archive users need the release URL (and the README's archive section, in progress) |
| 5 | Speed only where the solver dominates | Unchanged, now documented: vest edits 1.6×, demo 1.0×; cold `sail verify` 1–3 s slower on macOS |
| 6 | Managed counts: 3,127 `unknown` after one failure | Fixed: function counts, with Verus's unit totals explained |
| 8 | Repair engine unobtainable | Fixed: bundled and selected with no setup; repair accepted |
| 10 | `--help` started a watcher or errored | Fixed for every command tried |
| 11 | Managed targets grow without a command | Fixed: `sail cache projects list` and `prune` |

Frictions 7, 9 and 12–15 were not retested.

## Evidence

Raw logs outside the repository: `l39-mac-walk.tgz`
`78b27c2463bf0f6a5e6f23167400edeee797d182d5116c0fa43f4e22783a9d54` (every
command's stdout, stderr and exit code, MCP replies and watch events);
`walk-1-install.log` `76f0f648…671d`, `walk-2-loops.log` `1349030c…c0ba`,
`walk-3-agents.log` `9478f972…c12b`, `walk-4-watch.log` `d1e8cb03…1eb1`,
`mac-smoke-rc1.log` `40662593…5973`.

Attempts:
- `v0.0.0-rc.1` (`e00ac7b`), 2026-10-14: install only; that draft was withdrawn for `v0.1.0-rc.1` before the smoke test.
