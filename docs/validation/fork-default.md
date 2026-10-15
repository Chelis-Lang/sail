# Default mode on Sail's Verus (ADR 0043)

Sail runs its own Verus, the runtime built from the pinned fork commit, for
every verification ([ADR 0043](../decisions/0043-default-mode-runs-the-fork.md)).
Charter rule 1 is read against plain stock Verus with the pinned Z3. This
record holds the evidence that Sail on the fork matches stock.

| Field | Value |
|---|---|
| Status | **Pass** on the checks below; standard tier as listed; deep tier and a fork-versus-stock `deep-rvt` arm not run |
| Source | Branch `feat/fork-verus-default` (lane L41), 2026-10-15 |
| Runtime | Fork `13a1176c` (pinned): Linux `engine-runtime-13a1176c-linux` (manifest `5b866414…7603`), macOS `engine-runtime-13a1176c-macos` (manifest `c95d5f9a…dce1`) |
| Oracle | Stock Verus `0.2026.10.04.426d8b0`, Z3 4.16.0 |
| Environments | Linux: EC2 c7i.4xlarge (16 vCPU), us-east-1, Ubuntu 24.04, CI image via `ci/local.sh` (`scripts/perf-host.sh up l41`). macOS: developer Mac (Apple silicon, macOS 26.5), shared with other work |
| Limits | Correctness only, one run of each; no performance measurement |

## Results

| Check | Result | Evidence (sha256 of the log, kept outside the repository) |
|---|---|---|
| `ci/local.sh gate` (Linux) | pass: 567 tests passed | `gate.log` `679ae564…c234` |
| `ci/local.sh solver` (Linux) | pass: 35 tests passed | `solver.log` `35363ce9…8922` |
| `ci/local.sh e2e` (Linux), all three phases | pass: `sail verify` on the fork byte-identical to stock `cargo verus verify` (exit 101, cold and warm; 20,236 cache entries); `--output json` and MCP agree with stock `--output-json`; portfolio cold and warm; managed acceptance; Claude Code lists 15 tools; repair CLI/MCP acceptance | `e2e.log` `d93ee5b9…f7fa` |
| `ci/local.sh conformance fast pr` (Linux) | pass-through, cache fill and warm: each 33 units, 33 same, 0 unstable, 0 different, 0 infra errors, 311 functions; 3,442 cache entries; engine arm 33 same, 32 focused units; report `sail_verus: fork:5b866414…` | `conf-pr.log` `3c6522bc…6368` |
| `sail-conformance run --tier fast` (macOS, pass-through) | 147 units, 147 same, 0 unstable, 0 different, 0 infra errors; 3,450 functions; 3 units with rlimit drift | `conf-fast-mac.log` `977d6602…c140` |
| `ci/run.sh host` (macOS) | pass: clippy, 556 tests passed | `mac-host.log` `82b7cee0…5e15` |
| Manual check (macOS) | fresh `cargo verus new` project: stock and fork `cargo verus verify`, and `sail verify` cold and warm, byte-identical stdout, stderr and exit; `--output json` attribution complete | (lane notes) |

## Remaining gates

- `ci/local.sh conformance standard` with Sail on the fork (see below).
- Deep tier and a per-test fork-versus-stock `deep-rvt` arm (ADR 0043).
- Performance claims in [performance](../performance.md) were measured on
  stock Verus and are not yet re-measured on the fork.
