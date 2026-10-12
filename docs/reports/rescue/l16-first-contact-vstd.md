# L16: first-contact latency and vstd re-verification

Rescue lane L16 asked why an agent's first `sail mcp` verify of a project
costs tens of seconds when every solver check is a cache hit, and whether
Sail can avoid re-verifying vstd.

Status: measurement and design. The implementation waits for L7c's rework of
`sail-verus` `managed/*`; the "after" column is filled when it lands.

## Environment

- **Host.** AWS EC2 `c7i.4xlarge` (16 vCPU), `us-east-1`, Ubuntu 24.04,
  kernel 7.0.0-1014-aws, native x86-64 (`scripts/perf-host.sh up l16`).
- **Container.** The CI image's toolchain layers (`ci/Dockerfile` up to the
  Verus install), built natively. The optional engine layer was left out
  because the committed engine pin is stale on `rescue/m7` (`SAIL0944`);
  nothing here uses the engine. Scratch, caches and targets were on ext4
  Docker volumes.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0, Rust 1.98.1.
- **Sail.** `rescue/m7` at `d4b8651`, release build, `sail` sha256
  `213906ec…`.
- **Driver.** A throwaway script (`measure.py`, sha256 `fccca71a…`, not
  committed). Raw rows: `before.json`, sha256 `fb5e8d6f…`.
- **Agent environment.** The MCP arm adds the variables Claude Code gives a
  tool process (`CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`,
  `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PID`, `AI_AGENT`, `TERM`, `COLORTERM`,
  `TERM_SESSION_ID`, `SSH_AUTH_SOCK`, `GIT_EDITOR`), with per-session values.

## What plain Verus does

`cargo verus verify` plans with `verify_deps = true`
(verus@426d8b0 `cargo-verus/src/plan.rs`). Every package with
`package.metadata.verus.verify = true` is verified, including vstd
(`is-vstd = true`), in the project's own Cargo target. The release's
prebuilt `vstd.vir` and `libvstd.rlib` serve only the direct `verus` driver;
the Cargo path never uses them. `cargo verus focus` builds dependencies with
`--no-verify` instead.

So **plain Verus pays for vstd once per fresh Cargo target**:

| Plain `cargo verus verify` | Wall (s) | Output |
|---|---:|---|
| Fresh project 1, cold | 31.4 | `2060 verified` (vstd), `1 verified` (demo) |
| Project 1, warm | 0.11 | nothing re-verified |
| Fresh project 2 (same `CARGO_HOME`), cold | 31.2 | `2060 verified` (vstd) again |

Reusing the release's prebuilt vstd (option a) would compile the project
against different vstd bytes than plain `cargo verus` does, which breaks
rule 1, and it would not cover projects that patch vstd to a source path
(the corpus and the repair catalogue do). Option (a) is rejected.

## Before: where first-contact time goes

Each block uses one fresh cache and daemon. "Warm solver" means the daemon
already holds every answer.

**Demo** (`cargo verus new --lib`, one function):

| Run | Wall (s) | Verifier runs | Notes |
|---|---:|---:|---|
| CLI `sail verify --incremental`, cold | 37.1 | 2 | vstd and demo |
| CLI, warm | 1.6 | 0 | both receipts reused |
| MCP `verify`, agent environment, first | 35.3 | 2 | new context: vstd again |
| MCP, same agent session, second | 3.8 | 1 | MCP forces the selected crate |
| MCP, CLI's environment | 3.9 | 1 | shares the CLI context |
| MCP, new agent session | 35.6 | 2 | session id changed: vstd again |
| Second fresh project, CLI, warm solver | 34.1 | 2 | plain pays 31.2 here |

**human-eval-verus** (`sail-bench repair prepare`, `-p human_eval_verus`):

| Run | Wall (s) | Verifier runs |
|---|---:|---:|
| CLI, cold | 58.9 | 2 |
| CLI, warm | 1.7 | 0 |
| MCP, agent environment, first | 43.2 | 2 |
| MCP, same session, second | 9.1 | 1 |
| MCP, CLI's environment | 9.1 | 1 |
| Copy of the project, CLI, warm solver | 42.7 | 2 |

Findings:

1. **The environment is the cause of repeated first contact.** The proof
   context and the compiler family bind every environment variable except a
   short exclusion list (`managed/runner.rs`, `input_environment_key`). The
   job records show that the MCP submissions differ from the CLI's only by
   the ten agent and terminal variables above. With the CLI's environment,
   MCP reuses the CLI's context (3.9 s). With an agent environment, every
   new agent session, terminal tab or SSH agent socket makes a new context
   and a new compiler family, so it builds a new Cargo target and verifies
   vstd again (35 s on demo, 43 s on human-eval-verus).
2. **A new project costs about what plain costs.** The first managed run of
   a new project verifies vstd in its own target, as plain Verus does: 34.1 s
   against plain's 31.2 s on demo.
3. **CI.** The e2e legacy job's MCP call (about 49 s on 4 CPUs) is the first
   managed run of its project; legacy `sail verify` uses the project's own
   target. Plain Verus would also pay vstd on a fresh target, so step 1 does
   not remove it. The same holds for `managed-01` in the managed phase.
4. **MCP forces every call.** MCP's blocking `verify` sets
   `SAIL_MANAGED_FORCE=1`, so every call re-verifies the selected crate
   (9.1 s on human-eval-verus against 1.7 s for a warm CLI run). See below.

## Step 1 design: bind what Cargo treats as input

The lead replaced the original denylist of agent and terminal variables
with an allowlist: a denylist misses every harness and terminal nobody named
(Codex, Cursor, VS Code, CI systems). The proof context and the compiler
family bind the same environment that `cargo verus` itself treats as an
input, in two parts.

**(a) Variables the pinned tools are documented or known to read.** The
whole family is bound, with its current value or its absence:

| Family | Read by | Source |
|---|---|---|
| `CARGO_*`, `__CARGO_*` | Cargo configuration (`CARGO_BUILD_*`, `CARGO_PROFILE_*`, `CARGO_TARGET_*`, `CARGO_HOME`, `CARGO_TERM_*`, `CARGO_UNSTABLE_*`, `CARGO_ENCODED_RUSTFLAGS` ...) | Cargo book, "Environment variables Cargo reads" and "Configuration: environment variables" |
| `RUST*` (`RUSTFLAGS`, `RUSTDOCFLAGS`, `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `RUSTC_BOOTSTRAP`, `RUSTDOC`, `RUSTUP_*`, `RUST_MIN_STACK`, `RUST_BACKTRACE`, `RUST_LOG`, ...) | Cargo, rustup's toolchain selection, rustc. `RUST_BACKTRACE` and `RUST_LOG`/`RUSTC_LOG` change the stderr of a compiler crash or log | Cargo book; rustup book "Environment variables"; std `RUST_MIN_STACK`/`RUST_BACKTRACE` docs |
| `VERUS_*`, `VARGO_*`, `VSTD_KIND` | Verus driver and launcher (`VERUS_Z3_PATH`, `VERUS_SINGULAR_PATH`, `VERUS_ROOT`, `VERUS_DRIVER_*`), `builtin_macros` (`VSTD_KIND`, an undeclared proc-macro read) | verus@426d8b0 `rust_verify/src/driver.rs`, `verus/src/main.rs`, `air/src/singular_manager.rs`, `builtin_macros/src/lib.rs` |
| `Z3_*` | none known in Z3 4.16.0; bound conservatively | — |
| `SAIL_*`, except the existing observation-only exclusions | Sail's shim, cache and daemon selection | `managed/runner.rs` exclusion list |
| `LD_*`, `DYLD_*` | dynamic loading of `librustc_driver` and Z3 | ld.so(8), dyld(1) |
| `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `COLUMNS`, `LANG`, `LANGUAGE`, `LC_*` | colour and layout of rendered diagnostics, locale | the lead's list; kept conservatively |

`PATH` and `HOME` are not bound as raw values: agents and terminals routinely
differ in both, and binding them would bring the repeated first runs back.
What they decide for a proof is bound instead:

- `PATH` selects which `cargo`, `rustc`, `rustup`, `cargo-verus`, `verus`
  and `z3` run. Sail resolves each to an absolute path and binds its identity
  (path and bytes, or L7c's stat-based identity) in the runtime identity.
- `HOME` locates the Cargo and rustup homes. Sail binds the resolved
  `CARGO_HOME` and `RUSTUP_HOME` paths (explicit value, or the `HOME`
  default) and the bytes of every Cargo configuration file it finds, as the
  context already does for configuration files.

Tools that build scripts find through `PATH` (`cc`, `cmake`, `pkg-config`)
are undeclared reads, which Cargo does not fingerprint either; they join the
documented limitation below.

Variables outside (a) that the tools read without changing verdicts or
diagnostics stay outside: `TERM` (Cargo's progress bar, which managed mode's
pipes disable), proxy variables (network fetches of checksum-pinned
packages), `TMPDIR` (scratch files only).

**(b) Every variable any unit declares.** rustc writes `# env-dep:NAME` into
dep-info for `env!`, `option_env!` and tracked reads; the Verus driver
declares its own (`VERUS_Z3_PATH`, `VERUS_SINGULAR_PATH`) the same way
(`rust_verify/src/cargo_verus_dep_tracker.rs`); build scripts declare
`cargo:rerun-if-env-changed=NAME`. After each run Sail collects the union of
these names over every unit in the run's Cargo target and records each
name's value (or absence) in the receipt. A receipt is reused only when every
recorded name still has that value. The same names stay in Cargo's own
fingerprints, so the shared compiler family rebuilds exactly when Cargo
would.

This is Cargo's freshness model: Sail reuses a result exactly when plain
`cargo verus` would consider the build fresh, and it binds the values by
content rather than trusting Cargo's tracking (rule 6). Child processes still
receive the whole environment. Undeclared reads by a build script or proc
macro, including tools a build script finds through `PATH`, remain a
limitation shared with Cargo; `VSTD_KIND` is the one such read
in the pinned toolchain, and (a) binds it.

Expected effect: MCP, CLI and new agent sessions on the same project share
one context, so a CLI warm run makes MCP's first run warm and vice versa.

## MCP's per-call forced verification

MCP's blocking `verify` submits its job with `SAIL_MANAGED_FORCE=1`
(`crates/sail-mcp/src/job_tools.rs`), which re-verifies every requested
target. It arrived in `71f74bb` ("share durable workspace verification across
watch and MCP", squashed into #67) together with a report merge that drops
reused targets (`merged_report` keeps `requested && !reused`). ADR 0024 records
it as: "Default blocking verify uses a forced-fresh job and preserves its
report-v1 response, selecting only requested fresh targets." `docs/mcp.md`
gives the reason: "so Cargo's unchanged-target shortcut cannot produce an
empty successful report."

That reason no longer applies. A reused target carries its establishing
run's complete report, and the managed acceptance test asserts that the
reused report equals the establishing one. A receipt is only published for a
successful, complete report under Current evidence for byte-identical inputs,
so reusing it cannot give a different verdict. Failed or incomplete runs
publish no receipt and always re-verify. The force therefore prevents no
wrong answer. The fix is to drop it, merge reused requested targets into the
report, and say in the summary which run established a reused report
(rule 7).

Target: an MCP second call on human-eval-verus at CLI warm parity (about
1.7 s, against 9.1 s before).

## Step 2: a shared verified vstd

Plain Verus pays for vstd once per fresh target, and a new project under
managed Sail pays about the same. Following the lane brief, the target is
parity with plain, which step 1 restores for every client and session of a
project. Beating plain needs option (b): a per-toolchain verified vstd keyed
by the Verus, rustc and Z3 identity and vstd's content. The workable form is
in the managed driver proxy: key the vstd unit by its canonical driver
arguments, environment, package-content digest and extern artifact digests,
and replay its outputs and report. It is not proposed now because:

- it moves artifacts between Cargo targets, which needs rustc output to be
  byte-reproducible across target directories (unmeasured) and rewriting
  target paths in dep-info;
- the replayed vstd report needs its own provenance (rule 7);
- it saves about 31 s once per project, not per edit cycle, and would not
  help CI's fresh caches unless an entry were baked into the image.

It is recorded as a follow-up that needs its own ADR.

## After

To be measured with the same driver once step 1 is implemented.
