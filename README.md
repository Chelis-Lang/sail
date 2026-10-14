# Sail

Sail (S.A.I.L., the Satisfiability Analysis & Inference Layer) makes
[Verus](https://github.com/verus-lang/verus) verification loops faster for
developers and coding agents, without changing what Verus tells you. The gain
is largest on solver-heavy projects; small projects and first runs take about
as long as plain Verus ([performance](docs/performance.md)).

`sail verify` runs your own `cargo verus verify` with the same arguments and
puts a cache in front of the Z3 solver that Verus uses. You get the same
verdicts, the same errors and the same exit code as plain Verus with its
pinned Z3; Sail's conformance suite checks exactly that. On top of that, Sail
adds complete per-target reports, durable background jobs, a file watcher, an
MCP server for agents and signed team caches. When anything in Sail fails,
verification falls back to the pinned Z3 and stays correct.

Sail supports Linux on x86-64 and macOS on Apple silicon. It works on Cargo
projects (see [Limits](#limits)).

## Install

Sail is a prebuilt archive per platform on
[GitHub releases](https://github.com/Chelis-Lang/sail/releases). It holds
`sail`, the solver shim `sail-z3` and the proof-repair engine. Verus itself is
installed separately (step 2), because Sail runs your own `cargo verus`.

**Prerequisites.** A C toolchain, `curl`, `unzip` and `git` (Verus needs
them, and `cargo verus new` runs `git init`). On Debian or Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y build-essential curl git unzip
```

On macOS, install the Xcode command line tools (`xcode-select --install`).

**1. Sail.** Pick the archive for your platform:

| Platform | Archive |
|---|---|
| Linux x86-64, glibc 2.39 or later (for example Ubuntu 24.04 or newer) | `sail-VERSION-x86_64-unknown-linux-gnu.tar.gz` |
| macOS on Apple silicon | `sail-VERSION-aarch64-apple-darwin.tar.gz` |

Download it with its `.sha256` file, check it, unpack it and put its `bin/`
on `PATH`:

```sh
VERSION=0.1.0                      # the release you are installing
TARGET=x86_64-unknown-linux-gnu    # or aarch64-apple-darwin
base="https://github.com/Chelis-Lang/sail/releases/download/v$VERSION"
curl -fLO "$base/sail-$VERSION-$TARGET.tar.gz"
curl -fLO "$base/sail-$VERSION-$TARGET.tar.gz.sha256"
shasum -a 256 -c "sail-$VERSION-$TARGET.tar.gz.sha256"   # Linux without perl: sha256sum -c
mkdir -p ~/.local/opt
tar -C ~/.local/opt -xzf "sail-$VERSION-$TARGET.tar.gz"
export PATH="$HOME/.local/opt/sail-$VERSION-$TARGET/bin:$PATH"   # add to your shell profile
sail --version
```

The check must print `OK`. Minimal Linux images have no `shasum` (it comes
with perl); `sha256sum -c` with the same file is equivalent. Keep the unpacked directory as it is:
`sail` finds `sail-z3` beside it in `bin/` and the engine in `engine/`, so
put that `bin/` on `PATH` rather than copying `sail` elsewhere. On macOS, an
archive downloaded with a browser instead of `curl` is quarantined; clear
that with `xattr -dr com.apple.quarantine ~/.local/opt/sail-$VERSION-$TARGET`.

**2. Verus.** If you already use Verus, keep it: Sail runs whatever
`cargo-verus` is on `PATH` (or `SAIL_CARGO_VERUS`). Otherwise install the
release Sail is tested with, Verus `0.2026.10.04.426d8b0`, and the Rust
toolchain it pins, `1.98.1`.

With the prerequisites above in place, install [rustup](https://rustup.rs)
and the Rust toolchain Verus pins:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup toolchain install 1.98.1 --profile minimal
```

Download and unpack Verus, and put it on `PATH`:

```sh
VERUS=0.2026.10.04.426d8b0
PLATFORM=x86-linux                 # or arm64-macos
mkdir -p ~/.local/opt/verus-$VERUS
cd ~/.local/opt/verus-$VERUS
curl -fLO "https://github.com/verus-lang/verus/releases/download/release/$VERUS/verus-$VERUS-$PLATFORM.zip"
unzip -q "verus-$VERUS-$PLATFORM.zip"            # creates verus-$PLATFORM/
export PATH="$HOME/.local/opt/verus-$VERUS/verus-$PLATFORM:$PATH"   # add to your shell profile
cd -
cargo verus --help
```

The archive's SHA-256 is
`624018662868563a7bec0dd07be5e4dd0c3d61124b4b55105765884bf2d4d1a0` for
`x86-linux` and
`83cafc50284caea07fa8762df04b00cc0af441f25cdf1631d6c30987c65af446` for
`arm64-macos` (`shasum -a 256 verus-$VERUS-$PLATFORM.zip`).

**From source (contributors).** With rustup, a C toolchain and `git`:

```sh
git clone https://github.com/Chelis-Lang/sail
cd sail
cargo build --release -p sail-cli -p sail-shim   # installs the Rust toolchain Sail pins
export PATH="$PWD/target/release:$PATH"
cargo xtask fetch-verus                          # the pinned Verus; prints its PATH line
```

A source build has no bundled engine; [proof repair](#experimental-proof-repair)
says how to get one.

## First verification

In any Cargo project that Verus verifies:

```sh
cargo verus new --lib demo     # or use your own project
cd demo
sail verify                    # instead of: cargo verus verify
```

`sail verify` takes the same arguments as `cargo verus verify` and prints the
same output. It starts the Sail cache daemon on demand; you never need to
start it yourself. Run it again after an edit: solver queries Verus has seen
before are answered from the cache. To stop using Sail, run
`cargo verus verify` again. Nothing else changes.

`sail verify --output json` also prints one JSON document on stdout with
per-function verdicts and per-obligation results
([output schema](docs/output-schema.md)). `sail COMMAND --help` lists each
command's options.

## Reports, jobs and watch

`--incremental` switches to managed verification: complete reports for every
target, including targets whose proofs were reused because their inputs are
unchanged, and durable jobs you can inspect later.

```sh
sail verify --incremental --output json  # complete report, reused targets included
sail verify --background                 # submit a job and keep working
sail jobs list                           # then: jobs status|result|logs|cancel ID
sail watch                               # re-verify 300 ms after files stop changing
```

Each result says whether it is still **Current** (its inputs are unchanged),
**Historical** or **Unknown**. See
[workspace verification](docs/workspace-verification.md) and
[freshness](docs/freshness.md).

## Agents (MCP)

`sail mcp` is an MCP server on stdio. Register it from the project directory:

```sh
claude mcp add --transport stdio sail -- sail mcp
```

Agents get `verify`, `counterexample`, `cached_result`, background jobs
(`start_verification`, `verification_status`, `verification_result`,
`cancel_verification`) and evidence tools. See [MCP setup and tools](docs/mcp.md).

## Team cache

Teams and CI can share solver results. Entries are signed, and a machine uses
one only if a key it trusts signed it for the same solver binary and platform:

```sh
sail cache keygen --out ci.key                        # once, where CI runs
sail cache publish --to /shared/sail --key ci.key     # after a CI run
sail cache trust add ci.key.pub                       # on each developer machine
SAIL_SHARED_STORE=/shared/sail sail verify            # read from the shared store
```

The store can be a shared directory or an HTTPS server (`sail cache serve`,
`sail cache connect`). See [HTTP team caching](docs/http-shared-cache.md) and
[trust](docs/trust.md).

## Maintenance

- The cache lives in `~/.cache/sail` (or `$SAIL_CACHE_DIR`). The daemon's
  replies are under `results/`, and its log is `results/daemon.log`.
- `sail status` shows the daemon's state and counters. The daemon stops after
  30 minutes without work.
- `sail cache stats|verify|gc --max-bytes N|clear --yes` inspect, check,
  shrink or empty the solver-reply cache.
- Managed verification keeps compiled targets per project, which can reach
  several GB. `sail cache projects list` shows them, and `sail cache projects
  prune --missing|--older-than DAYS|--all` removes them (the next run is a
  cold run). See [disk use and cleanup](docs/workspace-hygiene.md).
- If the cache daemon cannot start, `sail verify` prints one
  `warning[SAIL0003]` line and verifies without the cache, with the same
  results.

## Limits

- **Cargo projects only.** `sail verify`, `--incremental`, `watch` and MCP run
  `cargo verus`. Projects verified with `verus file.rs` directly are not
  supported (the opt-in portfolio is the exception, with `--verus-source`).
- **Speed is bounded by the solver's share of the work.** Sail caches solver
  answers; Rust compilation and Verus's own frontend still run every time.
  See [performance](docs/performance.md) for each claim and how to reproduce
  it. In short, on a 16-vCPU Linux host:
  - edits on a solver-heavy project (vest, 3,237 functions) are 1.3–1.5×
    faster (about 24 s against 34–38 s); the Rust and Verus frontend sets
    that 24 s floor;
  - small projects and cold first runs take the same time as plain Verus;
  - unchanged re-verification with `--incremental` or MCP `verify` takes
    0.3–1.8 s, where plain Cargo does nothing in 0.1–0.2 s; its value is the
    complete report for agents, not speed.
- **macOS.** Receipt reuse works on Linux and macOS. On macOS a Cargo
  configuration file created and deleted again during a single run is not
  detected; see [freshness](docs/freshness.md#what-these-checks-do-not-catch).
- **Each new project or worktree verifies vstd once** in managed mode, about
  30 s, before reuse starts.
- **Proof repair is experimental** (below).

See [known issues](docs/known-issues.md) for the full list.

## Experimental: proof repair

`sail repair-check` focuses verification on one function, then independently
verifies the full target, using an optional interactive engine built from
Sail's public [Verus fork](https://github.com/Chelis-Lang/verus). A repair is
accepted only when the complete target verifies.

The Linux and macOS release archives include the engine, and `sail` finds it
in the unpacked directory with no setup. Check it with:

```sh
sail engine status                  # checks every runtime file against its pin
```

The engine also uses the stock Verus from step 2 of [Install](#install). In a
source checkout, download the prebuilt runtime from the
[fork's releases](https://github.com/Chelis-Lang/verus/releases) (no
credentials needed) with
`python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json --cache ~/.cache/sail`
(`artifact-macos.json` on macOS), or build it with
`cargo xtask build-verus-engine`. Follow the
[repair walkthrough](docs/proof-repair-walkthrough.md); the
[repair guide](docs/proof-repair.md) has the details.

## Opt-in tools

These can change what is reported, or cost much more than a verification, so
they run only when asked:

- `sail verify --portfolio accept-first-proof --portfolio-seeds 4` tries
  independent seeded runs and accepts the first complete proof. This can change
  a verdict; see [divergences](docs/divergences.md).
- `sail verify --measure-fragility` and `sail fragility` measure how fragile
  proofs are across solver seeds. They change no verdict.

## Documentation

- [Documentation index](docs/README.md): every user guide.
- [Performance](docs/performance.md): where Sail is faster, where it is not,
  and how to reproduce each number.
- [Error codes](docs/errors.md), [known issues](docs/known-issues.md),
  [compatibility](docs/compatibility.md).
- Validation status of each milestone gate is in
  [docs/validation/](docs/validation/), and the plan in the
  [roadmap](docs/ROADMAP.md).
- Working on Sail itself: [AGENTS.md](AGENTS.md) (workflow, gates, Rust
  conventions), the [charter](docs/CHARTER.md) (scope and correctness
  contract) and the [decision records](docs/decisions/README.md).

## License

MIT. See [LICENSE](LICENSE).

## Contributing

Sail does not accept outside pull requests at this time; issues are welcome.
See [CONTRIBUTING.md](CONTRIBUTING.md).
