# Caching Sail in CI

CI usually verifies code that has barely changed since the previous run. A
fresh runner starts with an empty cache, so plain `sail verify` pays the whole
cold cost on every job. This guide covers three things: which directories to
persist between jobs, what each one saves on real projects, and the limits
that apply today.

## Summary

| Persist | Size (zstd) | What it buys on a fresh runner |
|---|---|---|
| `~/.cache/sail/results/` (solver replies) | 0.1–3 MB | Every solver query of unchanged code is answered from cache. Anvil 64 s against 186 s cold (stock 193 s); CapybaraKV 42 s against 58 s (stock 64 s) |
| `target/` (your Cargo target), when Sail prints `note[SAIL3112]` | 76–204 MB | Dependencies are not rebuilt, as with stock `cargo verus`. CapybaraKV 12 s against stock's 30 s with the same cache; an Anvil edit commit 86 s against 159 s |
| `~/.cache/sail/projects/` (managed targets and proof receipts), in managed mode | 98–248 MB | Unchanged code reuses its receipts, on a new runner too: CapybaraKV 0.9 s against 41 s with `results/` alone, with `git restore-mtime`; Anvil 3 s on the same machine |
| `~/.cargo/registry`, `~/.cargo/git` | varies | Crate downloads, as for any Rust project |

The solver-reply cache is the part that works everywhere. It is small, and its
entries are keyed by the exact query bytes and the solver's identity, so an
entry from an older commit, lockfile or Sail version is just a miss, never a
wrong answer.

## Which mode your project is in

Run `SAIL_EXPLAIN=1 sail verify` once and look at the notes:

- **`note[SAIL3112]`.** Your project selects its own Cargo (for example a
  `rust-toolchain.toml` pinning another Rust). Sail then verifies as
  `cargo verus verify` does, in your own `target/`, with Sail's solver cache
  and no proof receipts. CapybaraKV (a nightly) is in this mode as published;
  Anvil (Rust 1.98.0) was until build-script receipts accepted Cargo 1.93 to
  1.99 ([ADR 0084](decisions/0084-qualified-cargo-range.md)). Persist `results/` and `target/`.
- **No such note.** Sail builds in its own managed target under
  `~/.cache/sail/projects/` and records proof receipts. Persist `results/`
  and `projects/`. A restored `projects/` is reused on a new runner because
  installed tools are identified by their bytes, not their inodes.

Setting `RUSTUP_TOOLCHAIN=1.99.1` moves a SAIL3112 project into managed mode
(see [compatibility](compatibility.md#explicit-cargo-selection)). That gives
proof receipts. For CapybaraKV with `git restore-mtime`, an unchanged commit
then takes 0.9 s on a new runner against 12 s with a cached `target/`; an edit
commit costs about the same either way.

## GitHub Actions example

This workflow persists the solver replies for every project, and the Cargo
registry and `target/` as for any Rust project. Replace `SAIL_VERSION` with
the release you use. Add the arguments you pass to `cargo verus verify`. The
workflow is assembled from the steps measured below, but it has not itself
been run on GitHub's runners.

```yaml
name: verify
on: [push, pull_request]

jobs:
  verify:
    runs-on: ubuntu-24.04
    env:
      SAIL_VERSION: 0.1.0
    steps:
      - uses: actions/checkout@v4

      - name: Install Sail
        run: |
          TARGET=x86_64-unknown-linux-gnu
          base="https://github.com/Chelis-Lang/sail/releases/download/v$SAIL_VERSION"
          curl -fLO "$base/sail-$SAIL_VERSION-$TARGET.tar.gz"
          curl -fLO "$base/sail-$SAIL_VERSION-$TARGET.tar.gz.sha256"
          sha256sum -c "sail-$SAIL_VERSION-$TARGET.tar.gz.sha256"
          mkdir -p ~/.local/opt
          tar -C ~/.local/opt -xzf "sail-$SAIL_VERSION-$TARGET.tar.gz"
          echo "$HOME/.local/opt/sail-$SAIL_VERSION-$TARGET/bin" >> "$GITHUB_PATH"
          rustup toolchain install 1.99.1 --profile minimal --component rust-src

      # Solver replies: small, and safe to reuse from any earlier commit.
      - uses: actions/cache@v4
        with:
          path: ~/.cache/sail/results
          key: sail-results-${{ runner.os }}-${{ env.SAIL_VERSION }}-${{ hashFiles('**/Cargo.lock', '**/rust-toolchain*') }}-${{ github.sha }}
          restore-keys: |
            sail-results-${{ runner.os }}-${{ env.SAIL_VERSION }}-${{ hashFiles('**/Cargo.lock', '**/rust-toolchain*') }}-
            sail-results-${{ runner.os }}-${{ env.SAIL_VERSION }}-

      # Managed mode only (no SAIL3112 note): managed targets and receipts.
      # - uses: actions/cache@v4
      #   with:
      #     path: ~/.cache/sail/projects
      #     key: sail-projects-${{ runner.os }}-${{ env.SAIL_VERSION }}-${{ github.sha }}
      #     restore-keys: sail-projects-${{ runner.os }}-${{ env.SAIL_VERSION }}-

      # Cargo downloads and, for SAIL3112 projects, the Cargo target.
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry/index
            ~/.cargo/registry/cache
            ~/.cargo/git/db
            target
          key: cargo-${{ runner.os }}-${{ hashFiles('**/Cargo.lock', '**/rust-toolchain*') }}-${{ github.sha }}
          restore-keys: |
            cargo-${{ runner.os }}-${{ hashFiles('**/Cargo.lock', '**/rust-toolchain*') }}-

      - run: sail verify

      # Bound what the post step saves.
      - if: always()
        run: sail cache gc --max-bytes 1G
```

Notes on the keys:

- Cache entries are immutable, so the primary key ends with the commit SHA,
  and every run saves its own entry. `restore-keys` picks the newest entry
  with the same prefix. For an edit commit, that is the previous commit's
  cache (the measured "edit commit" rows below).
- The solver-reply key needs only the OS and Sail version. A new Sail version
  can change the solver or the query bytes, and the old entries would then
  only miss, so a version-scoped key keeps size down. The lockfile and
  toolchain hash keeps the newest matching entry first.
- `target/` is the path for a single-package project. Use your workspace's
  target directory otherwise. In managed mode (no SAIL3112 note), Sail does
  not build in `target/`, so persisting it only helps any stock jobs you run.
- `~/.cache/sail` is the default cache location (`$SAIL_CACHE_DIR` or
  `$XDG_CACHE_HOME/sail` if you set them). The daemon that `sail verify`
  starts is still running when the post step saves the cache. Entries are
  written atomically and its socket files are skipped, so this is safe.
- GitHub's repository cache quota is 10 GB, and an entry not used for 7 days
  is evicted. The solver replies are a few MB per entry; `target/` is the
  large part.

### Optional: file times for packages with build scripts

`actions/checkout` gives every file the current time. When a package has a
build script without `cargo:rerun-if-changed` (CapybaraKV's does), Cargo
reruns the script and rebuilds the crate, and Verus then verifies it again,
with every solver query answered from cache. Setting file times to their last
commit time avoids that rebuild:

```yaml
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: sudo apt-get install -y git-restore-mtime && git restore-mtime
```

Measured on one machine, CapybaraKV's unchanged run with this step took
0.9 s in managed mode, with both targets reused from Current receipts,
against 12 s without it. In SAIL3112 mode the same step takes 0.5 s, because
Cargo then considers the crate fresh and Verus does not run. That is the same
trust you already place in a cached `target/` with stock `cargo verus`; see
[caveats](#caveats).

## Measured results

The measurements ran on a native EC2 c7i.4xlarge (16 vCPU, Intel Xeon
Platinum 8488C) in us-east-1, Ubuntu 24.04, on 2026-10-26. Sail was a Linux
release stage of main `b3ab7677` (Sail 0.1.0, fork runtime `36d65c3f`).
Stock was Verus 0.2026.10.18.7d82994. GitHub's standard runners have 4 vCPU,
so absolute times there will be longer.

Each project is a local Git repository with two commits, cloned afresh for
every run at the same path, as on a GitHub runner:

- **C0** is the pinned corpus commit. Its Verus dependencies are moved to the
  release's crates.io vstd, as a project adopting the pinned Verus would do
  (`scripts/perf-real.sh`, `SAIL_PERF_VSTD=registry`).
- **C1** adds one small lemma to a large proof file.

Caches were saved and restored with `tar`, so restored files have new inodes
and their original times, as `actions/cache` gives them. The Cargo registry
was warm in every arm. Values are seconds from one run each. Repeat runs of
the cold and restored rows agreed within 1–6 s.

### Anvil

`--lib -- --verify-module vdeployment_controller`: 144 functions at C0, 145
at C1, 0 errors. Sail ran with `--output json`, as `scripts/perf-real.sh`
does for partial verification, and stock ran `cargo verus focus`.

| Run | Stock | Sail, project's Cargo (SAIL3112) | Sail, `RUSTUP_TOOLCHAIN=1.99.1` |
|---|---:|---:|---:|
| C0, empty caches | 193.1 | 185.0 | 186.3 |
| C0, `results/` restored | — | 62.8 | 64.0 |
| C0, `results/` seeded from a signed team store instead | — | — | 64.0 |
| C0, `target/` (and Sail `results/`) restored | 1.1¹ | 1.7¹ | — |
| C0, whole Sail cache restored, same machine | — | — | 3.0 (Current, 0 verifier runs) |
| C1 edit, `results/` restored | — | 114.9 | 117.4 |
| C1 edit, `target/` (and `results/`) restored | 158.9 | 85.8 | — |
| C1 edit, whole Sail cache restored, same machine | — | — | 86.9 |
| C1, empty caches | 191.7 | — | — |

¹ Cargo found every unit fresh and Verus did not run. No verification summary
was printed, by stock or by Sail.

### CapybaraKV

`capybaraKV/capybarakv`, `--no-default-features`: 732 functions at C0, 733 at
C1, 0 errors. A cold run also verifies vstd (2,060 functions).

| Run | Stock | Sail, project's Cargo (SAIL3112) | Sail, `RUSTUP_TOOLCHAIN=1.99.1` |
|---|---:|---:|---:|
| C0, empty caches | 64.1 | 57.1 | 58.3 |
| C0, `results/` restored | — | 41.1 | 42.2 |
| C0, `results/` seeded from a signed team store instead | — | — | 43.9 |
| C0, `target/` (and Sail `results/`) restored | 29.6 | 11.7 | — |
| C0, whole Sail cache restored, same machine | — | — | 12.1; 0.9 with `git restore-mtime` |
| C1 edit, `results/` restored | — | 42.2 | 43.1 |
| C1 edit, `target/` (and `results/`) restored | 29.6 | 12.9 | — |
| C1 edit, whole Sail cache restored, same machine | — | — | 13.4 |
| C1, empty caches | 64.0 | — | — |

### New runner

These rows simulate a new runner on the same paths. Sail's installation and
the Rust toolchains were recreated with new inodes but identical bytes and
times, then the caches were restored into a fresh clone:

| Run | Result |
|---|---|
| CapybaraKV, managed mode, whole Sail cache, `git restore-mtime` | 0.94 s, Current, 2 targets reused² (40.7 s, nothing reused, before tool identity became content-based) |
| CapybaraKV, SAIL3112 mode, `results/` and `target/`, C0 / C1 | 12.2 s / 13.3 s |
| Anvil, SAIL3112 mode, `results/` and `target/`, C0 / C1 | 2.5 s¹ / 85.9 s |

So `target/`, `results/` and `projects/` all survive a new runner.

² Measured on main `1f812e12` with and without the content-identity change
([#428](https://github.com/Chelis-Lang/sail/pull/428)), same host type and
method. The first run on a new runner reads every installed tool once, about
0.15 s; later runs reuse a per-machine memo.

### Sizes

Bytes after one cold run, raw and with `zstd -3` (what `actions/cache`
uploads):

| Directory | Anvil | CapybaraKV |
|---|---|---|
| `~/.cache/sail/results/` | 1.0 MB / 0.14 MB | 18.0 MB / 2.9 MB |
| `~/.cache/sail/projects/`, managed mode | 1.33 GB / 248 MB | 417 MB / 98 MB |
| `~/.cache/sail/projects/`, SAIL3112 mode (unused by later runs) | 70 MB / 21 MB | 70 MB / 21 MB |
| `target/`, stock or SAIL3112 mode | 876 MB / 204 MB | 339 MB / 76 MB |

The rest of `~/.cache/sail` (`runtime-identities/`, `snapshot-hints/`)
is under 0.2 MB and regenerates on its own. Restoring it is harmless.

### Verdicts and output

Every run exited 0 with the same verdicts as stock: Anvil 144 then 145
verified, CapybaraKV 732 then 733 verified, vstd 2,060 where it was verified,
0 errors. For CapybaraKV, Sail's stdout equals stock's byte for byte in the
same Cargo state (cold against cold, and restored `target/` against restored
`target/`). Apart from Cargo's `Finished` time and the `SAIL_EXPLAIN` notes,
its stderr is identical for the restored runs. Cold runs print the same lines,
but parallel compilation orders the `Compiling` lines differently. For Anvil, the JSON function counts were compared
with stock's summary line. The CapybaraKV reuse rows report Current with
`reused_targets: 2`, and the Anvil same-machine row reports Current with 0
verifier runs. In the team-store rows, every lookup was a signed shared hit
(Anvil 1,478 of 1,478; CapybaraKV 7,212, plus 20,197 vstd replies from the
release archive). No lookup timed out.

## Teams: a signed store instead of a cache tarball

The solver-reply part can come from a team store instead of `actions/cache`.
A trusted job (for example on `main`) signs and publishes its local cache, and
other jobs and developers read it with the public key:

```sh
sail cache publish --to STORE --key team.key        # trusted writer
sail cache trust add team.key.pub                   # each reader, once
SAIL_SHARED_STORE=STORE sail verify                 # directory store
```

Readers check every entry's signature, key, solver and platform before
using it, so a reader never trusts the transport or the store. The rows above
show that seeding from a signed directory store matches restoring `results/`.
It shares only solver replies, not Cargo targets or receipts. For HTTP(S)
stores, credentials and limits, see [HTTP shared cache](http-shared-cache.md).
These team-cache commands are experimental.

## Caveats

- **Trust what you restore.** Sail's local cache is trusted because Sail is
  its only writer ([trust model](trust.md#local-disk-tier-what-its-checks-do-and-do-not-prove)).
  Restored entries are not signed. A forged entry with a plausible shape
  would be served, and so would a forged receipt in `projects/`. Restore only
  caches written by workflows you trust. GitHub scopes caches by branch: a
  run can restore caches from its own branch and the default branch, and a
  pull request's caches are visible only to that pull request, so a pull
  request cannot poison `main`'s cache. To accept replies from less trusted
  writers, use the signed team store, whose readers verify every entry.
- **A restored `target/` is Cargo's decision.** In SAIL3112 mode, as with
  stock, Cargo decides from its fingerprints whether Verus runs at all. In
  the Anvil rows marked ¹, neither tool re-verified the unchanged crate. That
  is the trust model of caching `target/` for any Rust project.
- **Managed targets and receipts need the same tools at the same paths.**
  They are keyed by the installed tools' bytes and absolute paths, so a
  restored `projects/` is reused when the runner installs the same Sail and
  Rust toolchain at the same locations, as the example workflow does. A
  different Sail version or toolchain is a miss, never a wrong reuse. It is
  large (98–248 MB compressed), so in SAIL3112 mode do not persist it. On a
  persistent self-hosted runner, keep `~/.cache/sail` in place instead of
  saving and restoring it. Prune old workspaces with
  `sail cache projects prune --older-than DAYS` ([disk use](workspace-hygiene.md)).
- **The checkout path matters.** Managed workspaces are keyed by their
  absolute path. GitHub-hosted runners always check out to the same path,
  `/home/runner/work/REPO/REPO`, but self-hosted runners with several work
  directories do not.
- **Unchanged-call replay starts over after a restore.** The first run after
  a restore takes the full path, because the replay record binds file inodes
  ([freshness](freshness.md#unchanged-call-replay)). Later identical calls on
  that runner replay.
- **The edit-commit cost is the frontend.** With every solver reply cached,
  an edited Anvil still spends about 86 s compiling and running Verus on its
  one large crate. The cache removes solver time, not Rust or Verus frontend
  time ([performance](performance.md)).
