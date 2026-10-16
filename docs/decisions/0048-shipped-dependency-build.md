# 0048: Releases ship vstd compiled and verified

- **Status:** accepted (owner direction 2026-10-15; implemented; the fork
  commit's runtimes still need publishing and re-pinning, see Status of the
  gates)
- **Date:** 2026-10-16
- **Issue:** #161
- **Extends:** [ADR 0045](0045-dependency-store.md) (the machine-local store)
  to the first run on a machine
- **Amends:** ADR 0045's "Release archives carry nothing new" and
  [releasing](../releasing.md)'s "The archive carries no prebuilt `vstd`";
  ADR 0039's fix rules (a fork change that is not an output-order or
  structured-output fix, below)

## Context

- **Owner direction (2026-10-15):** ship vstd compiled and verified so the
  first managed run on a fresh machine (CI runners, containers, agent
  sandboxes) skips vstd, not only later projects on the same machine; "we
  control Sail and the Verus fork, so make this work even if it needs
  invasive changes".
- **What ADR 0045 ruled out, and why it no longer holds.**
  - *Diagnostics name vstd's sources by absolute path.* rustc records each
    source file's name in the crate metadata, and a diagnostic pointing into
    vstd prints that name and quotes the file. Compiled with
    `--remap-path-prefix <registry source directory>=/rustc-dev/<rustc
    commit>`, vstd names its sources under rustc's own virtual rustc-dev base.
    rustc 1.98.1 translates any imported name under that base to
    `real_rustc_dev_source_base_dir` joined with the rest
    (`rustc_metadata` `imported_source_file`, `try_to_translate_virtual_to_real`)
    and then shows and reads that real path. A compiler driver can set the
    option. Measured on macOS with plain rustc (a crate remapped under Cargo
    home A, used under Cargo home B with the option pointing at B's
    registry): the `required by a bound` note names B's file and quotes it,
    byte for byte as when the crate is compiled under B; without the option
    it names a path under the sysroot and quotes nothing.
  - *Cargo rebuilds a unit compiled under another Cargo home.* True for
    copied units (Cargo's fingerprint hashes the absolute source path, and
    Cargo-Verus's tracked variables name each package by a hash of its
    manifest path). But Cargo does not need to find vstd fresh: Sail's driver
    proxy is Cargo's `RUSTC_WRAPPER` for every unit, so it can answer a
    compiler invocation itself. Cargo then fingerprints the unit as after any
    compilation. Measured with a wrapper that replayed recorded invocations
    (macOS, two Cargo homes and two target directories): all 19 units of
    vstd's closure were served, the next plain `cargo verus verify` found
    everything fresh (0.3 s), and stdout and stderr (without progress lines)
    equalled plain stock `cargo verus verify` byte for byte, including
    `::: <Cargo home B>/registry/src/…/vstd-…/std_specs/vec.rs:55:9` and its
    quoted line.
- **Verus's exported VIR** (`vstd.vir`) also records each vstd source file's
  canonical path, for files rustc did not import when the importer's span
  context was built (`FileStartEndPos`, cf. VI-0014); stock loads them from
  that path. A release build would record its own Cargo home there.
- **The gain.** First managed run of a new project on a fresh machine:
  measurements in [the validation record](../validation/shipped-dependency-build.md).

## Decision

1. **Sail's Verus maps the virtual base back** (fork commit
   `b4450da9`, `rust_verify/src/sail_source_base.rs`). With
   `SAIL_RUSTC_DEV_SOURCE_BASE=<virtual base>=<registry source directory>`,
   every compilation the driver runs (verified or not) sets
   `real_rustc_dev_source_base_dir` to that directory; exported VIR records
   source files under the directory by the virtual base, and imported VIR
   maps them back to the directory's canonical form, as stock would have
   recorded them. Without the variable nothing changes. This is a fork
   change of a new kind (ADR 0039 allows ordering and structured-output
   fixes): it changes which file a diagnostic names only for crates compiled
   with the virtual base, which only the shipped build is, and makes that the
   name stock prints for a crate compiled on the user's machine. Its
   obligation is the output identity below.
2. **Every managed run sets the variable** for Cargo-Verus, with the virtual
   base from the pinned toolchain (`corpus/toolchain.toml` `rust_commit`) and
   the crates.io registry source directory from `cargo metadata`. A target or
   store entry may hold shipped units from an earlier run.
3. **The shipped build.** A release archive carries
   `deps/<runtime variant>/`: one entry per compiler invocation Cargo made
   for vstd's registry closure (libraries, proc macros, build scripts; 19 for
   the pinned vstd), recorded at release time (`scripts/release.sh stage`,
   `SAIL_SHIPPED_RECORD`) from a new `sail new --lib` project in a fresh
   Cargo home, compiled with the remapping above by the archive's own
   runtime. An entry holds the outputs, the dep-info and stdout and stderr
   with the target and registry directories and the package's Cargo-Verus id
   as placeholders, the digests of every source file the dep-info names, and
   the declared variables. The release refuses an invocation that printed
   anything but artifact notices, failed, or whose outputs (other than the
   dep-info, and a dynamic library's install name) still name the registry or
   target directory, and fails unless vstd itself was recorded and a second
   new project, under another Cargo home, is served on its first run. The
   recorded `Cargo.lock` ships too.
4. **The key** of an invocation binds the shipped build's identity (the
   runtime's build manifest, which names every runtime file by digest, and
   the virtual base), the compiler arguments, the environment the compiler,
   the Verus driver and the closure's macros read (the invocation's own
   Cargo variables, `RUST*` and `VERUS_*` except paths and crash settings
   and `RUSTUP_*` (rustup's proxies set `RUSTUP_TOOLCHAIN` to the user's own
   default; the Verus launcher runs its driver with its pinned toolchain),
   Cargo-Verus's variables for this package, `VSTD_KIND` and Sail's solver
   mode; Apple's `SDKROOT` and `MACOSX_DEPLOYMENT_TARGET` only change how the
   closure's proc macros and build scripts are linked, not what they compute,
   and Python sets `SDKROOT` for every process it starts), with the target and registry
   directories and the package id replaced by placeholders, and the SHA-256
   of every `--extern` crate and its `.vir`. Rendering flags (`--color`,
   `--diagnostic-width`, ANSI rendering) are left out: an entry exists only
   for an invocation that printed no diagnostic. So are lint levels and
   expected `cfg`s (`--allow`, `-W`, `--check-cfg`, ...) of an invocation with
   `--cap-lints allow`, which Cargo passes for every registry package: capped
   lints print nothing, and Cargo 1.98 and 1.99 pass a package's `[lints]`
   table differently. A dependency compiled on the
   machine has other bytes, so every dependent is another key: shipped and
   local units never mix inconsistently.
5. **Serving.** Only on a managed run that creates its Cargo target
   (unfocused; no loader variables; no registry or target directory with
   whitespace, `:`, `#`, `"`, `\` or `verus_builtin`), the proxy looks up each
   registry invocation. Before anything is copied, every recorded source must
   have its recorded digest on this machine and every declared variable its
   recorded value; otherwise it compiles as usual (a silent miss, as plain
   Cargo prints nothing when it rebuilds). Files are copied with their digests
   checked, under temporary names, and renamed; the dep-info, stdout and
   stderr get this machine's directories. A damaged entry (unreadable, or a
   stored file's digest fails) prints `warning[SAIL3133]` and compiles as
   usual. For the verified unit (vstd) the replayed stdout is the verifier's
   recorded document, which becomes the target's report as for a fresh run,
   with `shipped` set to `<identity>/<key>`, the note `SAIL3132`, and
   attribution not recorded. The engine never replays.
6. **`sail new`** writes the shipped `Cargo.lock` (root package renamed) when
   the release has a shipped build, so a new project resolves vstd's closure
   as the shipped build did instead of to whatever crates.io holds later.
   Otherwise its files are still what `cargo verus new` writes.
7. **Order.** The dependency store (ADR 0045) seeds a new target first;
   Cargo then finds those units fresh and never asks the proxy. The shipped
   build serves what the store did not place; anything else compiles and
   verifies. `SAIL_SHIPPED_DEPS` (another shipped build directory, for tests)
   is observation-only, like the store's size.

## Conformance obligations

- **Unit tests** (`sail-verus` `managed::shipped`): keys ignore the machine's
  directories and unbound variables and bind arguments, verification, bound
  variables and linked crates; registry-only keying; whole-prefix path
  replacement; dep-info parsing; a recorded invocation is served on another
  machine with its paths; an edited source and a locally compiled dependency
  are silent misses that place nothing; a damaged file or entry warns once
  and places nothing; noisy, failed and path-naming invocations are never
  recorded; the shipped resolution renamed for a new project. Fork:
  `sail_source_base` maps names only when selected.
- **Local, against plain stock Verus** (`scripts/shipped-deps-check.py`, on a
  staged release): a new project, another Cargo home with a longer path, and
  a workspace package run from its directory are served on their first run;
  a rerun reuses vstd; a vstd with other features is verified with no
  warning; a damaged entry warns and verifies; every case's exit code,
  stdout and stderr (without progress lines and the warning) equal plain
  stock `cargo verus verify` with the same Cargo home in a new target, and
  diagnostics point into that Cargo home's vstd. Linux and macOS.
- **Release:** `scripts/release.sh stage` records the build and requires a
  second new project to be served.

## Consequences

- A fresh machine's first `sail verify` of a `sail new` project verifies only
  the project's own crate: 30.7 s → 1.29 s on a native Linux host. Existing
  projects benefit when their `Cargo.lock` resolves vstd's closure as the
  shipped build did; any other resolution is an ordinary first run, then the
  dependency store.
- Keys bind Cargo's arguments, so a Cargo that passes vstd's closure other
  arguments (another `-C metadata` scheme, or new flags) than the release's
  misses; Cargo 1.98.1 and 1.99.1 both hit. A user's `RUSTFLAGS` or profile
  settings for dependencies miss too, as they change the build.
- Archives grow by the shipped build (sizes in the validation record).
- A Verus or toolchain upgrade re-records the build at release time; there
  is nothing to migrate.
- The fork carries a change outside ADR 0039's two kinds; each fork upgrade
  reruns `scripts/shipped-deps-check.py`.
- The record and serve paths stay in Sail's driver proxy: plain
  `cargo verus verify` never serves, and the fork change is inert without the
  variable.
- Same trust boundary as the store: digests detect accidental damage, not a
  local writer who rewrites both the build and its manifest.
