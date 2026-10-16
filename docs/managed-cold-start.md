# Managed runs: first-run cost, reuse and timings

`sail verify` (managed verification, the default since
[ADR 0046](decisions/0046-managed-verification-by-default.md), and every
managed job from the CLI or MCP) builds
in its own Cargo target under the Sail cache and keeps a proof receipt for each
target that verified completely. The next run with unchanged inputs reuses
those receipts and starts no verifier. See
[ADR 0023](decisions/0023-managed-incremental-workspace-verification.md) and
[freshness](freshness.md) for what "unchanged" means.

Managed metadata probes, including Cargo-Verus's private metadata subprocess,
select `cargo-metadata/` under the Sail cache with a highest-priority
`build.target-dir` override. Neither caller environment nor Cargo configuration
can exclude source directories for these probes. Cargo metadata can otherwise create
`.rustc_info.json` in a source directory named by that override. Compilation
and cleanup keep the caller's environment value; an internal marker is removed
before compilation so a build script's own nested Cargo commands retain their
ordinary environment. Public fallback snapshots retain caller target selection.

## What the first run costs

Setting or unsetting `CARGO_TARGET_DIR` keeps the same managed context: Sail
always selects its own Cargo target. Declared project reads of that variable
still invalidate receipts when its value changes.

A new proof context has no compiled dependencies and no receipts. On a
machine that has never verified it, even a one-function project first
compiles its macro dependencies and verifies vstd, which takes tens of
seconds.

### Later new projects start from the dependency store

The first managed run on a machine stores the verified vstd it built, with
its compiled dependencies, its receipt and the verifier's output, in
`<cache>/dependency-store/`
([ADR 0045](decisions/0045-dependency-store.md)). The first managed run of
any later project, worktree or sandbox that resolves vstd identically copies
it into its new Cargo target and verifies only its own crates. Identically
means the same vstd version and features, the same versions of vstd's own
dependencies (as `Cargo.lock` resolves them), the same `[profile]` settings,
toolchain, Sail and Verus, Cargo configuration, relevant environment and
`sail verify` arguments, apart from which packages and targets they select
(`-p`, `--workspace`, `--exclude`, `--lib`, `--bin`, `--bins`): `sail verify
-p a` in a workspace starts from the vstd a single-package project stored.
On a native Linux host
(`c7i.4xlarge`, the method under [performance](performance.md#5-cold-first-run)):

| First managed run of | Before | With the store | Plain `cargo verus verify` |
|---|---:|---:|---:|
| a new one-function project (CLI) | 27.5 s | 1.3 s | 29.5 s |
| a new one-function project (MCP `verify`) | 27.7 s | 1.5 s | 29.5 s |
| a new vest worktree (`-p vest_lib`) | 52.3 s | 25.7 s | 66.1 s |

Medians of three rounds; the first managed run on the machine is unchanged
(demo 28.2 s, vest 67.9 s) and stores vstd. See the
[dependency store record](validation/dependency-store.md), which also checks
seeded runs against plain stock Verus byte for byte.

The copy is only a starting point. Cargo's own fingerprints decide whether
the copied units are fresh, exactly as in a target directory plain
`cargo verus` had built itself, and vstd is reported reused only when the
stored receipt matches the bytes Cargo found fresh. The run that copied it
marks vstd's target with `dependency_store` and the note `SAIL3130`, and names
the run that verified it as `establishing_run`; vstd's report is that run's
report. A different resolution, a path dependency on vstd, an edited registry
source or a dynamic-loader variable (`LD_*`, `DYLD_*`) in the environment
verify vstd as before, silently; a damaged entry does too, with
`warning[SAIL3131]`. A run killed while copying leaves no target behind.
vstd is stored only if its verification printed no diagnostics, since a
seeded run prints none; Verus's `has been running for N seconds` timing
notes, which a one-thread verifier prints for slow queries, do not count.
The store keeps the four most recently used entries, about 120 MB each
([disk use](workspace-hygiene.md#dependency-store)). A context changes when the arguments, the engine
selection, the relevant environment, the Cargo configuration or the installed
toolchain (Verus, Cargo, rustc, Z3, Sail, the sysroot libraries) changes;
installed files are identified by device, inode, size and modification time,
so reinstalling or rebuilding any of them makes the next run fresh.

A run in a workspace member's directory without `-p` verifies what Cargo
builds from there by default, the member alone, and reuses its receipts like
the same run from the workspace root with `-p`. Their arguments differ, so
they are separate contexts and each pays its own first run.

Contexts that differ only in the repair function selected share one Cargo
target ([ADR 0036](decisions/0036-managed-compiler-artifact-families.md)), so
a new repair function does not verify vstd again. Cargo reporting a target as
`fresh` is never enough on its own: a selected target is skipped only on an
intact receipt for the exact context and inputs. A dependency may also be
skipped on an intact receipt from another context of the same target that
covers its current bytes and inputs; the result names that receipt's run.

A run that ends in ordinary proof errors keeps vstd and the other compiled
dependencies, as plain `cargo verus verify` does, so the passing run after a
fix verifies only the edited crate. This needs a Current, unfocused run in
which every selected target's verifier finished normally with a complete
result. It holds for a project's first managed run too, which records the
build scripts' declared inputs: there Sail also requires that Cargo finished
its build and ran every build script it keeps a record for. Before this, a
project whose first runs failed verified vstd again on every run until one
passed (about 21–27 s each).

A run that ends in a compile error in a selected crate keeps them too, again
as plain Cargo does, when the run is Current and Cargo's own records show
that the failure is confined to that crate: Cargo started every selected
crate (so each of its dependencies had completed), every dependency verified
in that run succeeded, the failing crate's compiler stopped with an ordinary
error exit, Cargo finished the build, and Cargo ran (or found fresh) every
build script whose record is kept. On vest, a compile error then costs about
7 s instead of 34 s, and the fixed run 25 s instead of 52 s (plain
`cargo verus verify`: 6 s and 36 s; Sail's solver cache makes the fix faster).

A compile error in a dependency, a crash, an interrupted build, a partial
(`--verify-function`) run or uncertain inputs still withdraw the dependencies'
build-script records, and the next run rebuilds those packages. A failed proof
or compile error never produces a receipt, so the failing crate itself is
always verified again.

### Adding a dependency

vstd's receipt describes vstd, not the project around it. For a dependency
whose whole closure comes from a registry or git source, the receipt binds
how Cargo resolved that closure (package versions, enabled features and
edges), the workspace's `[profile]` table, the closure's own bytes, the
`rust-toolchain` files and the Cargo configuration, but not the bytes of the
workspace `Cargo.toml` and `Cargo.lock`
([#177](https://github.com/Chelis-Lang/sail/issues/177)). Adding a
dependency, a dev-dependency or a `build.rs` therefore verifies only the
crate, as plain `cargo verus verify` does. A change that reaches vstd's
closure (a profile setting, another version of one of its dependencies, a
feature some new dependency enables on a shared crate such as `syn`) still
verifies vstd, once: the run after it reuses vstd again. A path dependency
in the closure keeps the workspace manifest and lockfile in the binding.
Whether a run is Current, and every later read of it, still compare every
manifest and lockfile.

On a native `c7i.8xlarge` (us-east-1), one-function project, each step after
an established run:

| Step | Before | After | Plain `cargo verus verify` |
|---|---:|---:|---:|
| add `serde = "=1.0.228"` | 22.3 s, vstd verified | 2.6 s | 2.2 s |
| add a `libc` dev-dependency and an empty `build.rs` | 22.2 s, vstd verified | 2.1 s | 0.8 s |
| add `[profile.dev] debug-assertions = false` | 28.1 s, vstd verified | 28.5 s, vstd verified | 29.1 s |
| the unchanged run after it | 22.1 s, vstd verified | 0.5 s | 0.1 s |
| first `-p a` run of the #121 workspace, store filled by another project | 28.9 s, vstd verified | 1.3 s | |

## Build scripts

Sail records the inputs each build script declares and checks their content
on every run; a package whose recorded inputs changed is cleaned, so Cargo
reruns its script. Only that package, and the packages that depend on it,
lose their records: an edit to a crate whose `build.rs` declares nothing
(Cargo then watches the whole package directory) rebuilds that crate, not vstd
([#119](https://github.com/Chelis-Lang/sail/issues/119)). Files a build script
generates in Sail's target and the compiler includes, such as serde's
`OUT_DIR/private.rs`, are compared by content, not across the run, so a
project that depends on serde becomes Current on its first run
([#120](https://github.com/Chelis-Lang/sail/issues/120)). A script that Cargo
never runs for the build (a dev-dependency, another platform's dependency, or
a feature that only another workspace member enables) does not keep a run
Unknown ([#121](https://github.com/Chelis-Lang/sail/issues/121)). See
[freshness](freshness.md#how-a-run-decides-it-is-current) for the rules.

On a native `c7i.4xlarge` (us-east-1, idle), each issue's one-function
reproduction from an empty cache (wall time; the cold first run is 34–35 s in
every case):

| Reproduction | Before: unchanged / comment edit | After: unchanged / comment edit |
|---|---|---|
| `build.rs` without `rerun-if-changed` (#119) | 0.4 s / 30.8 s, vstd verified again | 0.4 s / 1.2 s |
| depends on serde 1.0.228 (#120) | 30.8 s / 30.8 s, Historical | 0.4 s / 1.1 s |
| `-p a`, another member enables `foo/serde` (#121) | 30.7 s / 30.9 s, Unknown | 0.5 s / 1.2 s |
| dev-dependency `libc`, Windows-only `windows-sys` | 31.2 s / 31.2 s, Unknown | 0.6 s / 1.4 s |

Before these fixes every run in the last three rows verified vstd again. On
an Apple M3 under heavy other load (load average 22–97) the same pattern held:
#119's edit run went from 84–121 s to 3.2–4.4 s.

## Concurrent runs

Two managed runs that share a Cargo target (the same workspace and
arguments, whatever the repair function) are serialised. The
second waits for the first to finish (polling every 50 ms), for up to
`SAIL_MANAGED_LOCK_WAIT_SECS` seconds (default 1,800), and then fails with
`SAIL3104`. The daemon also queues jobs of one workspace behind each other.

## What a warm run costs

On a native `c7i.4xlarge`, a warm HumanEval run with nothing changed takes
under a second, and a single-function edit run spends about a second in Sail
around Verus (plain `cargo verus` spends about 0.5 s). That Sail work is Cargo
metadata, identifying the toolchain, the input snapshot and its stamps,
receipt checks and the final checks. Independent work overlaps: Cargo metadata
runs while the toolchain is identified, the two sysroot probes run in parallel,
and large files (input snapshots, receipt artifacts, the driver's AIR and SMT
logs) are read, and input paths stamped, by up to eight workers. A private
alias that is a hard link to the installed `sail` is not hashed again, and the
targeted `cargo clean` leaves the target's rustc probe cache and rustc's
incremental state alone, so the next build need not probe the compiler again
or redo the edited crate's analysis from nothing. The input walk resolves
only symbolic links. Plain `sail verify` asks Verus for no AIR or SMT logs;
`--output json`, jobs and MCP do
([ADR 0047](decisions/0047-attribution-logs-on-request.md)). Natively, these cut managed overhead around Verus on a
HumanEval single-function edit from 3.03 s to 0.94 s (plain `cargo verus`:
0.54 s), L7c (rescue lane report, tag `archive/rescue-m7-98f6292`).

Reuse works the same way on macOS. On an Apple M3 under other load:

| Project | Unchanged | Edit | Failing edit | Fix |
|---|---|---|---|---|
| one-function demo (load 14–16) | 0.35–0.47 s | 0.76 s | 0.67 s | 0.69 s |
| plain `cargo verus verify` | 0.17 s | 0.50 s | 0.43 s | 0.47 s |
| Vest `-p vest_lib` (load 7–9) | 1.6 s | 30.8 s | 18.0 s | 20.3 s |
| plain `cargo verus verify` (load 9–18) | 0.2–0.5 s | 28.0 s | 26.6 s | 31.6 s |

Before this release every macOS run rebuilt the macro dependencies and
verified vstd again (demo 22–72 s, Vest 38–44 s), whatever changed. See
[freshness](freshness.md#what-these-checks-do-not-catch) for the macOS rule
that changed.

## Optional setup timings

With `SAIL_JOB_EVENTS` set to a file path, each managed run also writes an
adjacent `*.setup-PID-SERIAL.jsonl` file of fixed phase names with relative
start and elapsed milliseconds: `runtime-context`, `metadata` (the remaining
wait for Cargo metadata, which overlaps the context), `inputs` (owned paths,
snapshot and input checks), `target-cleanup`, `invocation` (Cargo-Verus),
`target-records` (receipt and artifact checks, and any retry), `post-run` (the
second metadata, snapshot and final checks) and `publication`. The files hold
no arguments, paths, source, environment values or diagnostics, are created
exclusively with mode 0600, and are limited to 64 files per process and 64
rows and 16 KiB per file. Any failure disables them without changing the run.
Choose a destination outside the workspace, so writing timings does not change
an input.
