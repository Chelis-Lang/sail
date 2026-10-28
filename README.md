# Sail

Sail (S.A.I.L., the Satisfiability Analysis & Inference Layer) makes
[Verus](https://github.com/verus-lang/verus) verification loops faster for
developers and coding agents. It ships a preverified vstd for matching new
projects, caches solver answers across edits, and can restore a previous proof
when you undo an edit. Measured speedups and remaining cold and unchanged-run
costs are in [performance](docs/performance.md).

`sail verify` runs `cargo verus verify` with the same arguments and puts a
cache in front of the Z3 solver that Verus uses. Sail ships its own Verus,
built from Sail's [Verus fork](https://github.com/Chelis-Lang/verus), which
removes known run-to-run variation in the solver queries stock Verus sends
for the same input (variation defeats caching). Sail's conformance suite checks per-function verdicts, diagnostics and exit
codes against plain stock Verus with its pinned Z3, with narrow
[documented reporting and integration exceptions](docs/divergences.md).
Code that uses `env!`, `option_env!` or compiler-time tools to inspect Sail's
named operational settings can compile different inputs from plain Cargo-Verus.
On top of that, Sail
adds complete per-target reports, durable background jobs, a file watcher and
an MCP server for agents. When anything in Sail fails,
verification falls back to the pinned Z3 and stays correct.

Sail supports Linux on x86-64 and macOS on Apple silicon. It works on Cargo
projects (see [Limits](#limits)).

## Install

For a guided install with Python 3.9 or newer, use
[`scripts/install.py`](scripts/install.py). Select a version explicitly:

```sh
python3 scripts/install.py --version 0.1.0 --install-toolchain
```

It checks the archive, validates the bundled runtime, and installs the pinned
Rust toolchain with `rust-src` using an existing rustup. It prints the `PATH`
line to add, leaving your shell files and default Rust toolchain alone. With
GitHub CLI signed in, it also verifies the archive's build-provenance
attestation (`--require-attestation` makes that mandatory). Local archives,
custom installation directories and manual provenance checks are covered in
the [installation guide](docs/install.md). The example version is a
placeholder; select a version listed on the releases page.

Sail is a prebuilt archive per platform on
[GitHub releases](https://github.com/Chelis-Lang/sail/releases). It holds
`sail`, the solver shim `sail-z3` and Sail's Verus (with the pinned Z3 and
the proof-repair engine). You install Sail and a Rust toolchain; no separate
Verus installation is needed, and an existing one is left alone.

**Prerequisites.** A C toolchain and `curl` (Rust and Verus need them). On
Debian or Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y build-essential curl
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

The check must print `OK`. With GitHub CLI, also
[verify the archive's provenance](docs/install.md#verify-provenance):
`gh attestation verify "sail-$VERSION-$TARGET.tar.gz" --repo Chelis-Lang/sail`.
Minimal Linux images have no `shasum` (it comes with perl); `sha256sum -c` with the same file is equivalent. Keep the unpacked directory as it is:
`sail` finds `sail-z3` beside it in `bin/` and its Verus in `engine/`, so
put that `bin/` on `PATH` rather than copying `sail` elsewhere. On macOS, an
archive downloaded with a browser instead of `curl` is quarantined; clear
that with `xattr -dr com.apple.quarantine ~/.local/opt/sail-$VERSION-$TARGET`.

**2. Rust.** Verus compiles your crate with the Rust toolchain it pins,
`1.99.1`. Install [rustup](https://rustup.rs) and that toolchain with its Rust
sources (`rust-src`):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup toolchain install 1.99.1 --profile minimal --component rust-src
sail engine status                 # checks Sail's Verus against its pins
```

Installing it does not change your default toolchain. `sail verify` runs
Cargo with this toolchain on its own, so a release's preverified vstd proof
(recorded with Cargo `1.99.1`) applies whatever your default Cargo is. A
`RUSTUP_TOOLCHAIN` you set, a project's `rust-toolchain.toml`, a `rustup
override` or an explicit `CARGO` keeps your choice instead; another Cargo
version may emit different compiler arguments and safely verifies vstd again.
Without `rust-src`, Sail runs your default Cargo and its JSON result notes
`SAIL3137` with the command above. Sail never installs a toolchain itself
([ADR 0058](docs/decisions/0058-managed-builds-use-the-pinned-cargo.md)).

Verus flags after `--` reach vstd too, as with `cargo verus verify`, so a
forwarded flag normally means vstd is verified again. Releases support
`--rlimit 100` through a separately verified proof or a recorded resource-limit
witness. Other raised limits use the proof only when that witness covers them;
otherwise vstd is verified normally
([details](docs/decisions/0060-shipped-vstd-under-a-raised-rlimit.md)). To
keep other flags off vstd, pass `--fwd-verus-args-to roots` before `--`, as
with stock: `sail verify --fwd-verus-args-to roots -- --rlimit 100`.

Without `rust-src` everything still works; only the preverified vstd is
skipped. When the preverified vstd does not apply (no `rust-src`, extra
toolchain components, another lock or Cargo), Sail still verifies vstd, but
answers byte-identical solver queries from signed replies shipped in the archive
([trust model](docs/trust.md#bundled-tier-the-releases-own-replies)). Keep that toolchain to the components above: `rustc-dev` or another
component that adds compiler libraries changes files rustc loads, so the proof
is not used. Whenever the preverified vstd does not apply to a new project,
a `SAIL3138` note says why: in `--output json` notes, and in human output
with `SAIL_EXPLAIN=1`. See [release prerequisites](docs/releasing.md#proof-bundle-prerequisites-and-first-use)
for how the shipped proof is recorded and checked.

**From source (contributors).** With rustup, a C toolchain and `git`:

```sh
git clone https://github.com/Chelis-Lang/sail
cd sail
cargo build --release -p sail-cli -p sail-shim   # installs the Rust toolchain Sail pins
export PATH="$PWD/target/release:$PATH"
python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json \
  --cache ~/.cache/sail                          # Sail's Verus; artifact-macos.json on macOS
```

A source build bundles no Verus; the `fetch` line downloads the pinned,
digest-checked runtime from the
[fork's releases](https://github.com/Chelis-Lang/verus/releases) (no
credentials needed), and `cargo xtask build-verus-engine` builds it instead.
Contributors running the conformance suite also need the stock Verus it
compares against: `cargo xtask fetch-verus`.

## First verification

Start a new project with `sail new`, then verify it:

```sh
sail new --lib demo
cd demo
sail verify
```

`sail new PATH` creates a Verus binary (`--lib` for a library, `--name` for
a package name other than the directory's), with the files `cargo verus new`
writes: a `vstd` dependency at the version that matches Sail's Verus,
Verus's `verify` flag, and a `src/lib.rs` (or `src/main.rs`) with one
verified function. It refuses a path that exists and is not an empty
directory. Sail makes the project with its own Verus and never puts that
Verus's `cargo-verus` on `PATH`, so your own `cargo verus` is unaffected.
When the release includes a matching preverified vstd, a new project also
gets its dependency lock. A lock already written by the
generator is preserved; ordinary installs without an admitted bundle keep the
generator's behavior.

In a Cargo project that Verus already verifies, run `sail verify` instead of
`cargo verus verify`; nothing needs to change in the project.

The [instant-revert guide](docs/instant-revert.md) explains restoration of a
previously verified revision, its qualified Linux/dependency-profile scope and
the measured time and storage tradeoffs.

`sail verify` takes the same arguments as `cargo verus verify`; its verdicts,
diagnostics and exit code follow the [documented equivalence contract](docs/divergences.md).
Cargo's parallel `Compiling`
lines can come out in another order, and a few arguments Sail cannot bind to
a proof receipt run without proof reuse, with one note line
([details](docs/workspace-verification.md#setups-verified-without-proof-reuse)).
Compiler-time code that observes Sail's named solver-route, wrapper, driver-option,
collection or interactive-engine settings can compile different inputs and produce different results
([accepted integration limit](docs/divergences.md#compiler-visible-integration-settings)).
Solver-answer correctness and cache binding to the actual query inputs remain required.
It builds in its own Cargo target under
the Sail cache and never touches your `target/`.

A new project or worktree can start from the release's preverified vstd when
the pinned Rust toolchain with `rust-src` is installed and its vstd dependencies
match (see [Install](#install)). Otherwise vstd is verified normally, with
signed solver replies available as a fallback cache. Other compiled dependencies
are still built once per project and worktree: sharing them between projects
is disabled until it is sound
([ADR 0053](docs/decisions/0053-disable-unqualified-native-store.md)).
`sail verify` starts the Sail cache daemon on demand; you never need to
start it yourself. Run it again
after an edit: targets whose inputs are unchanged keep their proof receipts
and are not verified again, and solver queries Verus has seen before are
answered from the cache. To stop using Sail, run your own
`cargo verus verify` again. Nothing else changes.

`sail verify --output json` prints one JSON document on stdout instead: a
complete per-function report for every target, including targets whose
proofs were reused because their inputs are unchanged
([output schema](docs/output-schema.md)). `sail COMMAND --help` lists each
command's options. `--incremental`, which used to switch this mode on, is
accepted and does nothing.

## Reports, jobs and watch

```sh
sail verify --output json   # complete report, reused targets included
sail verify --background    # submit a job and keep working
sail jobs list              # then: jobs status|result|logs|cancel ID
sail watch                  # re-verify 300 ms after files stop changing
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

Supported tools:

- verification: `verify`, `counterexample`, `cached_result`;
- background jobs: `start_verification`, `verification_status`,
  `verification_result`, `cancel_verification`, `verification_events`;
- evidence: `explain_failure`, `compare_verification`, `save_baseline`,
  `list_baselines`, `get_baseline`;
- repair: `start_repair_validation`.

The `fragility` tool and `verify`'s `portfolio` and `measure_fragility`
options are experimental, as are the integration tools
(`requirement_assurance`, `runtime_capabilities`, `runtime_execute`); their
descriptions start with "Experimental:". See
[MCP setup and tools](docs/mcp.md).

## Proof repair

`sail repair-check` checks a proof repair. It verifies one function first, so
a fix that still fails is reported quickly, then independently verifies the
complete target with the interactive engine inside Sail's Verus. Use it after
editing a proof, before trusting the edit:

```sh
sail repair-check --engine interactive --package demo --lib --function demo::sum_to
```

A repair is accepted (exit 0) only when the full-target verification is
complete, successful and Current; the focused result alone is never
acceptance, and a regression elsewhere in the target rejects it (exit 1). The
engine needs no setup beyond [Install](#install): it is part of the runtime
`sail engine status` checks. MCP agents use `start_repair_validation`. Follow
the [repair walkthrough](docs/proof-repair-walkthrough.md); the
[repair guide](docs/proof-repair.md) has the details.

## Evidence from completed jobs

These commands read evidence that verification jobs already recorded. They are
read-only and conservative: none re-verifies current code, generates proof
edits or turns provisional, partial or historical evidence into a verified
claim.

- `sail explain JOB --package-id ID --target NAME --kind lib|bin --function F`
  shows one function's exact obligations, solver models, diagnostics and
  the run that established them. Use it to see why a function failed.
- `sail baseline save NAME --job JOB` keeps a completed job's evidence under
  an immutable name (`list` and `get` read it back), independently of job
  pruning. Use it before a refactor you want to compare against.
- `sail compare BEFORE_JOB AFTER_JOB` (or `sail compare JOB --baseline NAME`)
  classifies each function as unchanged, regressed, recovered, added and so
  on. A scope or configuration mismatch is reported as incomparable or not
  observed, never as a proof regression.

See [workspace verification](docs/workspace-verification.md#evidence-from-completed-jobs).

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
- In CI, persist `~/.cache/sail/results` between runs (and `target/` when
  Sail notes `SAIL3112`): unchanged Anvil takes 64 s instead of 186 s on a
  fresh runner. See [caching Sail in CI](docs/ci-cache.md).
- If the cache daemon cannot start, `sail verify` verifies without the
  cache, with the same results. `--output json` notes why (`SAIL0003`), and
  `SAIL_EXPLAIN=1` prints it in human output.
- Human `sail verify` output is exactly plain `cargo verus verify`'s; set
  `SAIL_EXPLAIN=1` to also see Sail's notes (`SAIL0003`, `SAIL3112`,
  `SAIL3138`).

## Limits

- **Sail runs only its own Verus,** built from its fork at the Verus release
  it pins (`0.2026.10.18.7d82994`). Projects that need another Verus release
  are not supported yet.
- **Cargo library and binary targets only.** `sail verify`, `watch` and MCP
  run `cargo verus`. Projects verified with `verus file.rs` directly are not
  supported.
- **Some setups get no proof reuse.** They are verified exactly as
  `cargo verus verify` verifies them, with the solver cache, but no proof
  receipts are recorded or reused (`SAIL3112`, shown with `SAIL_EXPLAIN=1`
  and in `--output json`):
  - a Cargo outside the qualified 1.93.0 to 1.99.1 (for example 1.92 or a
    release newer than the pin), chosen by the project's
    `rust-toolchain.toml`, `RUSTUP_TOOLCHAIN` or `CARGO` (`SAIL3139`); a
    project pinning a qualified Cargo, such as Anvil's 1.98.0, does get
    receipts;
  - tests, benches and examples, `--target-dir`, Verus's `--log` options,
    `--no-verify`, Cargo `-v`/`--message-format` and `--config` overrides;
  - `LD_LIBRARY_PATH` or another loader variable.

  See [details](docs/workspace-verification.md#setups-verified-without-proof-reuse).
- **Speed is bounded by the solver's share of the work.** Sail caches solver
  answers; on an edit, Rust compilation and Verus's own frontend still run.
  An unchanged call replays its recorded result in milliseconds when nothing
  it read has changed; otherwise Sail's input checks, receipts and complete
  reports make small edits and unchanged calls slower than plain Verus.
  Current measurements, where Sail is faster and where it is not, and how to
  reproduce each number are in [performance](docs/performance.md).
- **Instant revert applies to qualified dependency profiles only.** With
  `SAIL_EXPLAIN=1`, `SAIL3146` says why it did not apply; see
  [instant revert](docs/instant-revert.md).
- **macOS.** Receipt reuse works on Linux and macOS. On macOS a Cargo
  configuration file created and deleted again during a single run is not
  detected; see [freshness](docs/freshness.md#what-these-checks-do-not-catch).
- **The shipped vstd proof applies to one exact setup:** the vstd version
  `sail new` writes, its default dependency resolution, the pinned Cargo and
  toolchain with `rust-src`, and no extra Verus arguments beyond a supported
  `--rlimit`. Anything else, for example another forwarded Verus flag or a
  project-chosen toolchain, verifies vstd in that project (about 15 s more on
  Linux), with the same results.
- **Open verdict differences.** Code that reads Sail's solver-route variables
  (such as `SAIL_Z3_PATH`) by name sees them, which plain Cargo-Verus does not
  ([#435](https://github.com/Chelis-Lang/sail/issues/435)).
- **Fragility, the portfolio, team caching and the integration APIs are
  experimental** (see [Experimental commands](#experimental-commands) and
  [Integration APIs](#integration-apis-experimental)).

See [known issues](docs/known-issues.md) for the full list.

## Experimental commands

These run, but they are not part of the supported command set and may change
in any release. `sail --help` does not list them; `sail help COMMAND` says
what each does.

- `sail verify --portfolio accept-first-proof --portfolio-seeds 4` tries
  independent seeded runs and accepts the first complete proof. This can change
  a verdict; see [divergences](docs/divergences.md).
- `sail verify --measure-fragility` and `sail fragility` measure how fragile
  proofs are across solver seeds. They change no verdict.
- Team caching shares signed solver results through a directory or an HTTPS
  server: `sail cache keygen`, `trust`, `publish`, `serve`, `connect` and
  `disconnect` ([HTTP team caching](docs/http-shared-cache.md),
  [trust](docs/trust.md)).
- `sail serve` runs the cache daemon in the foreground; `sail verify` starts
  it on demand.

## Integration APIs (experimental)

These interfaces serve integrations such as Oyster and may change in any
release.

- For contracts on ordinary Rust functions, the
  [`sail-contracts` library](docs/neutral-contracts.md) provides explicit
  requirement IDs and checked scalar preconditions, postconditions and old
  values. These runtime checks are separate from SMT verification.
- [Neutral source jobs](docs/neutral-contract-jobs.md) submit supported source
  through Sail's local job service and retain its actual contract proof
  evidence; `sail assurance` and MCP's `requirement_assurance` read it by
  requirement. [Bounded source mutation](docs/neutral-mutation.md) retains
  separate verifier rejection, survival and inconclusive evidence under
  unchanged contracts. [Scalar literal witnesses](docs/literal-witnesses.md)
  use separately attributed SMT queries and independent concrete evaluation to
  find admitted scalar contract violations while preserving the native
  verifier's original outcomes.
- [`sail runtime`](docs/runtime.md) submits complete raw SMT transcripts
  through Sail's pinned solver. Its versioned answers retain exact solver
  evidence and are separate from Rust verification reports. Use
  `--socket CACHE_SOCKET` to share `sail serve`'s runtime. MCP exposes the
  same service through `runtime_capabilities` and `runtime_execute`, with
  daemon autostart and explicit unavailable or incomplete outcomes.
- Release archives and builds of this workspace do not include native temporal
  jobs; `sail runtime capabilities` reports them unavailable. They need the
  private D.I.V.E. library and are built only by a separate
  [extension workspace](docs/private-dependencies.md).

## Documentation

- [Documentation index](docs/README.md): every user guide.
- [Performance](docs/performance.md): where Sail is faster, where it is not,
  and how to reproduce each number.
- [Error codes](docs/errors.md), [known issues](docs/known-issues.md),
  [compatibility](docs/compatibility.md).
- Validation status of each milestone gate is in
  [docs/validation/](docs/validation/), and the plan in the
  [roadmap](docs/ROADMAP.md).
- [Acceptance refinements](docs/design/acceptance-production-refinements.md):
  checked production operations, concrete model relations and remaining proof boundaries.
- Working on Sail itself: [AGENTS.md](AGENTS.md) (workflow, gates, Rust
  conventions), the [charter](docs/CHARTER.md) (scope and correctness
  contract) and the [decision records](docs/decisions/README.md).

## License

MIT. See [LICENSE](LICENSE).

## Contributing

Sail does not accept outside pull requests at this time; issues are welcome.
See [CONTRIBUTING.md](CONTRIBUTING.md).
