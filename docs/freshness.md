# Freshness of retained workspace proof reports

## What you get

Every managed result (`sail verify --output json`, `sail repair-check`, and the
same jobs through MCP) carries a Currentness label:

- **Current**: the result matches your code as it is now. No source file,
  manifest, lockfile, build-script input or Cargo configuration that Cargo
  compiled or read for the selected targets has changed since the run.
- **Historical**: one of those inputs has changed since the run. The result is
  still the true record of what was verified then, but it no longer describes
  your code. Re-run to get a current result.
- **Unknown**: Sail could not establish either, for example because a file
  could not be read or the run itself could not watch its inputs. Re-run.

These labels describe your inputs, not the solver's verdict. A Current result
can contain a failed proof. Only a Current, successful result can be reused as a
proof receipt or accept a repair. CLI and MCP clients receive the same label.

Any doubt makes a result Historical or Unknown, never Current. The cost of a
wrong "not current" is one re-run; a wrong "Current" is never acceptable.

## Reading a result later

When a run ends Current, Sail records the SHA-256 content digest of every file
in the selected targets' dependency closure, as Cargo resolved it: each closure
package's sources and build inputs, compiler-declared includes, the workspace
`Cargo.toml` and `Cargo.lock`, and every member manifest. It also records every
Cargo configuration path (`.cargo/config.toml` and `rust-toolchain` files in the
invoking directory's ancestors, and the Cargo home configuration), including the
ones that do not exist. The record is `read-check.json` in the run's proof
context under the Sail cache.

Each later read of the result (job Status or Result, `sail jobs`, MCP reads,
watch) rereads every recorded file completely, in parallel, and compares
digests. It also checks that the absent configuration paths are still absent.

- Every digest matches and nothing new appeared: **Current**.
- Any file changed, disappeared or appeared: **Historical**.
- A file cannot be read, or the record is missing or malformed: **Unknown**.

Nothing is inferred from timestamps, so an edit whose modification time was
restored still reads as changed. Files of packages that the selection never
builds, such as other workspace members' dependencies, are not part of the
record; editing them does not change the label. The toolchain itself (the Verus,
Cargo, Sail and Z3 binaries) is identified by the run's proof context rather
than reread on each read; a different toolchain gives a different context on
the next run.

Job submission uses the same check: a new request joins a running or completed
job only when the recorded inputs are unchanged. Without a record (no Current
run of that context yet), requests never coalesce.

## How a run decides it is Current

A run is Current only when all of the following hold:

- the content revision of its inputs (every source, manifest, lockfile,
  build-script input and compiler-declared include of the resolved packages,
  and the Cargo configuration) is the same after verification as before;
- the device, inode, size, mode, modification time and change time of every
  input file and directory are unchanged across the run. The change time cannot
  be set by user programs, so an edit that was restored during the run, even
  with its modification time put back, is still caught;
- every installed toolchain file still has the device, inode, size and
  modification time it had when the run started;
- every absent Cargo configuration file is still absent at the end. On Linux,
  none was created during the run either, even briefly: an inotify watch on
  those paths tells such a file apart from Cargo's own journal writes in its
  home directory. macOS has no equivalent synchronous, per-name notification,
  so there only the start and end of the run are checked (see below);
- every unit Cargo compiled belongs to the selected targets' dependency closure,
  so the read-time record covers everything that was compiled;
- the build-script and compiler declarations were complete.

Otherwise the run is Historical (an input changed) or Unknown (one of the
checks could not be made), with the note "inputs could not be confirmed
unchanged; re-run for a current result". Verification itself still runs and
its report is kept as the record of that run.

Build-script coverage does not wait for scripts outside an explicit `-p`
selection's dependency closure, because they never run for it. Without an exact
selection or a complete, unambiguous resolve graph, every resolved package is in
scope. Cargo's resolve graph is wider than the build, though: it keeps
dev-dependencies, other platforms' dependencies, and features that only another
workspace member enables. So after a run that Cargo finished (one
`build-finished` record), coverage also does not wait for a script package of
which Cargo reported no unit at all, compiled or fresh: nothing that run
compiled used its output. A package Cargo reported any unit of still needs its
script's record ([#121](https://github.com/Chelis-Lang/sail/issues/121)).

Files that build scripts generate inside Sail's own Cargo target and that the
compiler then reads, such as serde's `OUT_DIR/private.rs`, are build products,
not inputs. The target belongs to the run, which may clean a package and rerun
its script, so such a file can be rewritten with the same bytes or appear for
the first time. They are left out of the comparison across the run (content
and stamps); the inputs of the scripts that write them, outside the target,
are still compared. Rustc reads a generated file only after its script has
finished, so its content at the end of the run is the content compiled, and
that content is bound into the proof receipts and the read-time record like
any other input ([#120](https://github.com/Chelis-Lang/sail/issues/120)).

### What these checks do not catch

These cases can change what a run verified without changing any content
digest or stamp, if the change is undone before the run ends:

- a process writing through a writable shared memory mapping of a source file
  while that page is already dirty, which does not update the file's
  timestamps;
- a bind mount placed over an input and removed again, which needs root;
- on macOS only, a Cargo configuration file (`.cargo/config.toml`,
  `.cargo/config`, `rust-toolchain`, `rust-toolchain.toml` in the project's
  directory or its parents, or `config.toml` in the Cargo home) that did not
  exist when the run started, was created during the run and was deleted
  again before it ended. Sail checks that such files are absent at the start
  and end of the run and on every later read, but cannot see one that came and
  went in between. If you create a Cargo configuration file temporarily while a
  managed run is in progress, re-run after removing it.

Earlier versions held kernel read leases and watched the mount generation to
catch the first two, at a cost of about a quarter of a second on every run and
a large amount of code. Earlier macOS versions caught the third by checking the
nearest existing parent directory of each absent file, but Cargo writes its
cache database in its home directory on every build, and `/tmp` and the home
directory change constantly, so every macOS run read Historical and reused
nothing. Ordinary editors, formatters and agents write files, which
these checks do catch. If you run a tool that edits sources through shared
mappings during verification, re-run after it finishes.

## Cargo selection and compiled artifacts

Changing Cargo package or target selection, such as adding `--lib` to an
interactive repair after a full package verification, shares the same owned
Cargo artifact directory. Cargo still decides which units are fresh. Features,
profiles, target triples, compiler configuration, runtime files and environment
remain compiler-family inputs; ordinary and interactive engine requests stay
separate. Proof contexts still bind every original argument and focus selector.
A requested target cannot use a sibling context's proof receipt. Dependency
reuse still checks the complete source revision, declared environment and exact
artifact bytes against an established receipt. The candidate's
[qualification record](validation/managed-selection-family.md) identifies the
remaining gates and the original failed diagnostic.

## Which environment variables are proof inputs

A managed proof follows the input model of plain `cargo verus`, which is
Cargo's own. Two parts of the environment bind a result:

- **What the pinned tools read.** The proof context, and so the Cargo target,
  binds the current value or absence of every variable in these families:
  `CARGO_*` (except `CARGO_TARGET_DIR`) and `__CARGO_*` (Cargo configuration); `RUST*` (`RUSTFLAGS`,
  `RUSTC*`, `RUSTDOC*`, `RUSTUP_*`, `RUST_MIN_STACK`, `RUST_BACKTRACE`,
  `RUST_LOG`); `VERUS_*`, `__VERUS_*`, `VARGO_*` and `VSTD_KIND` (the Verus
  launcher, driver and macros); `Z3_*`; `SAIL_*`, except the observation-only
  job and trace variables and `SAIL_DEPENDENCY_STORE_ENTRIES`; `LD_*` and `DYLD_*` (dynamic loading); and colour,
  layout and locale (`NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `COLUMNS`,
  `LANG`, `LANGUAGE`, `LC_*`).
- **What the project declares.** After each run Sail collects every variable
  any unit declares: rustc's `# env-dep:` dep-info lines (`env!`,
  `option_env!`, and the Verus driver's own declarations) and build scripts'
  `rerun-if-env-changed`. Each receipt records their values, and it is reused
  only while every one is unchanged.

For build-script declarations, Sail binds `CARGO_FEATURE_*` and `CARGO_CFG_*`
to Cargo's generated values, not the parent process values. A private Cargo
unit graph supplies exact unit features. Native compiler cfg queries supply
target values. Each unit's profile supplies its debug-assertion setting.

This model uses Cargo source `5f94df4789f005f9a352888e8355ffc645b7ed0e`.
Unknown Cargo versions, missing observations, and conflicting unit values
refuse reuse. A declared name with an inherited parent value also refuses
reuse. Other Cargo-generated variables retain the original refusal.

The generated values participate in each script's content digest. Feature,
target, cfg, or declared environment changes invalidate affected records.
Output ownership, source-content checks, receipt checks, and dependent
invalidation remain unchanged.

`RUSTUP_TOOLCHAIN` binds its effective value. When the caller made no
toolchain choice, Sail runs Cargo with the engine's pinned toolchain
([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md)) and the
context records `RUSTUP_TOOLCHAIN=<pinned>` exactly as if the caller had set
it, along with that toolchain's Cargo sysroot. Selecting the pinned toolchain
and keeping the caller's Cargo are therefore different contexts, and so are
installing the pinned toolchain's `rust-src` or adding a `rust-toolchain`
file or rustup override, which change the selection.

`CARGO_TARGET_DIR` does not select a managed context: Sail always passes its
owned `--target-dir` to Cargo. Setting, changing or unsetting the caller's
directory therefore keeps the same compiled target. It still reaches child
processes, and an `env!` or build-script declaration binds its exact value
in proof receipts, just like any other declared read. Compiler-family script
observations also retain its exact presence/value: unlike Cargo-injected
variables, compilation preserves this caller value. The metadata override
is confined to separate metadata children.

Managed metadata probes, including Cargo-Verus's private metadata subprocess,
select `cargo-metadata/` under the Sail cache with a highest-priority
`build.target-dir` override. Neither caller environment nor Cargo configuration
can exclude source directories for these probes. Cargo metadata can otherwise create
`.rustc_info.json` in a source directory named by that override. Compilation
and cleanup keep the caller's environment value; an internal marker is removed
before compilation so a build script's own nested Cargo commands retain their
ordinary environment. Public fallback snapshots retain caller target selection.

Before reading or seeding any proof receipt, managed verification checks that
the physical Sail cache and every resolved package directory are disjoint.
Overlap in either ancestor direction, an unresolved alias, or an unidentifiable
input root takes the SAIL3112 ordinary-Cargo fallback without managed receipts.
Existing ancestors are resolved before a future cache suffix is considered.
Thus an excluded cache tree cannot hide an added source file, including when
the cache aliases a source directory or contains a checked-out package.
The default external cache keeps managed reuse.

`PATH` and `HOME` are not bound as raw values. What they select is: the
resolved `cargo`, `rustc`, `rustup`, `cargo-verus`, `verus`, `rust_verify`
and `z3`, identified by path and file stamp, and the resolved Cargo and rustup
homes with the bytes of every Cargo configuration file. Every other variable,
such as an agent's session id, `TERM` or `SSH_AUTH_SOCK`, is not a proof
input, so the CLI, MCP and every agent session or terminal share one proof
context for a project. Child processes still receive the whole environment.

**Limitation, shared with Cargo.** A build script or proc macro that reads a
variable without declaring it, including a tool that a build script finds
through `PATH` (`cc`, `cmake`, `pkg-config`), is invisible: Cargo does not
rebuild for it, and Sail does not re-verify for it. Declare such reads with
`cargo:rerun-if-env-changed` in the build script.

## Reader latency

The M7 reader gate (four clients, 200 Status and Result reads, p95 below
200 ms, every sample Current while another verification runs) is measured
with `scripts/reader-gate.sh`; see
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).

### Watch startup

`sail watch` completes its initial Cargo metadata discovery before starting
filesystem notifications. Creating the initial `Cargo.lock` is setup work and
does not cancel the first verification when macOS delivers a delayed event.
Subsequent lockfile, source and declared-input changes keep the existing
invalidation and cancellation behavior.

With the pinned Cargo, a requested proof failure keeps independently verified
dependencies when the current input checks pass. Platform-disabled packages
with no Cargo execution unit need no observed build-script contract. Missing
or unsupported planning evidence retains conservative cleanup; unexpected
actual units prevent a Current result.
