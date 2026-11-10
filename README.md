# Sail

Sail makes [Verus](https://github.com/verus-lang/verus) verification faster
for developers and coding agents. `sail verify` is a drop-in replacement for
`cargo verus verify`: it takes the same arguments and gives the same verdicts,
diagnostics and exit codes, but it:

- caches Z3 answers across edits;
- reuses the proofs of targets whose inputs haven't changed;
- restores the earlier proof when you undo an edit;
- ships a preverified `vstd` in release archives for new projects.

Complete editing loops measured 1.22–2.38 times faster than plain Verus across
Vest, IronKV and CapybaraKV in six paired rounds on Linux and Mac. Standalone
files can also reuse an unchanged successful proof in a few hundredths of a
second. See [performance](docs/performance.md) for the measured scopes and
cases where Sail is still slower.

Solver-service failures fall back to the pinned Z3. If Sail cannot establish
complete verification evidence, it reports an error or an unconfirmed result
instead of claiming success. Sail runs on Linux x86-64 and Apple-silicon macOS.

Read the [Sail book](https://cproof.ai/docs/sail/) for the quickstart,
workflows, and architecture. Its sources are maintained in this repository.

## Install

Sail 0.2.0 is published as native archives and as the crates.io package
`sail-solve`. The executable remains `sail`.

```sh
rustup toolchain install 1.99.1 --profile minimal --component rust-src
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
sail engine install
```

This installs `sail` and its companion shim from Rust sources, then downloads
and checks the pinned Verus/Z3 runtime. It requires Rust 1.99 or newer, a C
toolchain, Rustup and Python 3. The [installation guide](docs/install.md)
records platform support and the differences from prebuilt archives.

Sail is distributed as a prebuilt archive per platform on
[GitHub releases](https://github.com/Chelis-Lang/sail/releases). Each archive
contains `sail`, the solver shim `sail-z3`, and Sail's own Verus with its
pinned Z3. You don't need a separate Verus install, and an existing one is
left alone.

You need a C toolchain, `curl`, [rustup](https://rustup.rs) and Python 3.9 or
newer. Then, from a checkout of this repository:

```sh
python3 scripts/install.py --version VERSION --install-toolchain
```

Replace `VERSION` with a release from the releases page. The installer
checks the archive, verifies its build provenance when the GitHub CLI is
signed in, and installs the Rust toolchain Verus needs (`1.99.1` with
`rust-src`) without changing your default toolchain. It then prints the
`PATH` line to add to your shell profile.

The [installation guide](docs/install.md) covers manual installation without
Python, offline archives, provenance checks, and how the Rust toolchain affects
the preverified `vstd`.

## First verification

```sh
sail new --lib demo
cd demo
sail verify
```

`sail new` creates a Verus project, like `cargo verus new` does. In an
existing Verus project, run `sail verify` wherever you would run
`cargo verus verify`; the project doesn't need any changes. Sail builds in its
own target directory under its cache and never touches your `target/`.
To stop using Sail, go back to `cargo verus verify`.

Run `sail verify` again after an edit. Targets whose inputs haven't changed
keep their proofs, and queries the solver has already answered come from the
cache. When nothing at all has changed, Sail replays the previous result
without running Cargo.

## Existing Verus files

Start from a tutorial file or proof experiment without creating a Cargo project:

```sh
sail verify proof.rs
sail watch proof.rs
sail repair-check proof.rs --function proof::lemma --engine interactive
```

The same file works through MCP with `source_file: "proof.rs"`. Complete
successful proofs can be reused on unchanged calls and undo. This feature is
available in published 0.2.0 and qualified on native Linux and macOS. Repair checks add an
exact function focus followed by fresh full-file validation; see the
[standalone workflow](docs/standalone-verification.md).

## Everyday commands

```sh
sail verify --output json   # one JSON report covering every target
sail verify --background    # run as a background job
sail jobs list              # then: sail jobs status|result|logs|cancel ID
sail watch                  # re-verify when files stop changing
sail status                 # cache daemon state
```

Each result says whether it is **Current** (its inputs are unchanged),
**Historical** or **Unknown**. See [workspace verification](docs/workspace-verification.md)
and [freshness](docs/freshness.md).

Sail's [background preparation](docs/background-preparation.md)
worker can prepare the next unchanged Cargo call after verification returns,
and start verifying a newly created project while you begin editing. Foreground
work takes priority. Use `SAIL_BACKGROUND_PREPARE=0` to opt out.

## Agents (MCP)

Install the [Sail skill](skills/sail-verification/SKILL.md) to give a coding
agent the verification and SMT workflows:

```sh
npx skills add Chelis-Lang/sail --skill sail-verification
```

The skill works with the CLI or an already configured MCP server. It does not
install Sail itself; use the installation instructions above first.

`sail mcp` runs an MCP server on stdio. To register it with Claude Code, run
this from the project directory:

```sh
claude mcp add --transport stdio sail -- sail mcp
```

It provides tools for verification, background jobs, failure explanations,
saved baselines and repair validation. See [MCP](docs/mcp.md).

## Proof repair

`sail repair-check` checks a proof fix. It verifies one function first, so a
fix that still fails is reported quickly, then verifies the whole target:

```sh
sail repair-check --engine interactive --package demo --lib --function demo::sum_to
```

A repair is accepted only when the whole target verifies. See the
[repair walkthrough](docs/proof-repair-walkthrough.md).

## Limits

- Sail runs only its own Verus, built from
  [Sail's Verus fork](https://github.com/Chelis-Lang/verus) at the release it
  pins. Projects that need another Verus release aren't supported yet.
- Published 0.2.0 supports Cargo projects and standalone Verus files. Standalone
  watch requires its source and discovered inputs beneath the working directory.
- The first unchanged call after a verification, and unchanged calls that
  can't replay, are slower than plain Verus: about 0.3–0.6 s more on mid-sized
  projects, and up to 2 s more on large ones.
- Some setups verify correctly but without proof reuse: a Cargo outside
  1.93–1.99.1, tests and benches, `--target-dir`, and a few others. See
  [setups without proof reuse](docs/workspace-verification.md#setups-verified-without-proof-reuse).
- Code that reads Sail's own settings (such as `SAIL_Z3_PATH`) through
  `env!` or `option_env!` can see them, which plain Cargo-Verus does not.
  See [divergences](docs/divergences.md).

[Known issues](docs/known-issues.md) has the full list.

## Documentation

- [Documentation index](docs/README.md)
- [Performance](docs/performance.md), [error codes](docs/errors.md),
  [compatibility](docs/compatibility.md)
- [Broader performance evaluation](docs/performance-evaluation.md)
- [Disk use and cleanup](docs/workspace-hygiene.md),
  [caching Sail in CI](docs/ci-cache.md),
  [CI reports](docs/ci-reports.md),
  [GitHub Action](docs/github-action.md),
  [background preparation](docs/background-preparation.md)
- Experimental features: fragility measurement, seed portfolios, team caching
  and the neutral-contract integration APIs. They aren't listed in `sail --help`; see the
  [documentation index](docs/README.md#experimental-features).
- Working on Sail: [AGENTS.md](AGENTS.md), the [charter](docs/CHARTER.md) and
  the [decision records](docs/decisions/README.md).

## Building from source

```sh
git clone https://github.com/Chelis-Lang/sail
cd sail
cargo build --release -p sail-solve -p sail-shim
export PATH="$PWD/target/release:$PATH"
python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json \
  --cache ~/.cache/sail          # use artifact-macos.json on macOS
```

The `fetch` step downloads Sail's pinned Verus runtime from the
[fork's releases](https://github.com/Chelis-Lang/verus/releases).

## License and contributing

Sail is MIT licensed; see [LICENSE](LICENSE). Issues are welcome. Outside pull
requests aren't accepted at the moment; see [CONTRIBUTING.md](CONTRIBUTING.md).

