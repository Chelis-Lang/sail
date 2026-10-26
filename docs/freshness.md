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

If you explicitly select a Cargo version outside Sail's qualified build-script
input protocol, Sail verifies projects with build scripts using ordinary Cargo
freshness and its solver cache. It records no proof receipts and reports
Unknown with a `no_reuse` reason. Unchanged passing builds can then remain
Cargo no-ops instead of repeatedly rebuilding to establish receipts that this
Cargo cannot support. Select the pinned toolchain to enable proof reuse:
`RUSTUP_TOOLCHAIN=1.99.1 sail verify`. If `CARGO` names a different Cargo
binary explicitly, unset it or point it at the pinned Cargo too. Sail does not
change your toolchain selection or treat an unsupported protocol as proof authority.

These labels describe your inputs, not the solver's verdict. A Current result
can contain a failed proof. Only a Current, successful result can be reused as a
proof receipt or accept a repair. CLI and MCP clients receive the same label.

Any doubt makes a result Historical or Unknown, never Current. The cost of a
wrong "not current" is one re-run; a wrong "Current" is never acceptable.

A retained result must originally have been Current to remain Current after a
readback. Matching inputs do not upgrade a Historical or Unknown run. Reading a
result rechecks its original request before preserving whole-target completion;
losing that authority preserves the recorded diagnostics and input label.

## Reading a result later

When a run ends Current, Sail records the SHA-256 content digest of every file
in the selected targets' dependency closure, as Cargo resolved it: each closure
package's sources and build inputs, compiler-declared includes, the workspace
`Cargo.toml` and `Cargo.lock`, and every member manifest. It also records every
Cargo configuration path (`.cargo/config.toml` and `rust-toolchain` files in the
invoking directory's ancestors, and the Cargo home configuration), including the
ones that do not exist. The record is `read-check.json` in the run's proof
context under the Sail cache.

The record also binds source symlinks and linked ancestors of declared input
paths to their resolved destinations. Changing `src/lib.rs` from a link to
`variants/good.rs` into a link to `variants/bad.rs` changes input identity even
when both destination files already belong to the package and neither file's
contents changed. Directory symlinks and compiler-declared external includes
receive the same checks. Restoring the original binding and contents restores
the logical input identity.

Fresh managed content-digest reads keep one opened file handle and compare its
original/final device, inode, length, mode and modification/change timestamps
with the requested pathname. The hasher receives the exact checked read chunks;
incomplete EOF, an unreadable handle or a different final object supplies no
digest. Phase-local digest reuse also checks the original read seal against the
actual canonical cache-key object and the requested pathname; a changed
observation forces a fresh read.
A stable regular file retains the same digest as before. This is an
opened-object check, not an atomic filesystem snapshot or executable-launch
guarantee. Retained-result reads use the same checked read/hash transaction.

Each later read of the result (job Status or Result, `sail jobs`, MCP reads,
watch) rereads every recorded file completely, in parallel, and compares
digests. It also checks that the absent configuration paths are still absent.
Every recorded source binding must still resolve to the same destination.
Before any of these reads, and again after all content and binding reads,
Sail observes a mutation stamp over the exact recorded paths, resolved objects
and linked ancestors. A changed stamp makes the result Historical; an unavailable
stamp cannot establish Current. This prevents separately matching file reads
from hiding an observed mutation during the batch. Metadata endpoint equality
still does not establish an atomic snapshot: repeated metadata, filesystem
timestamp granularity and transient changes remain observation limits.

- Every digest matches and nothing new appeared: **Current**.
- Any file changed, disappeared or appeared: **Historical**.
- A file cannot be read, or the record is missing or malformed: **Unknown**.

Older read-check records without source bindings are refused as Unknown. Run
verification again to establish a complete record.

Nothing is inferred from timestamps, so an edit whose modification time was
restored still reads as changed. Files of packages that the selection never
builds, such as other workspace members' dependencies, are not part of the
record; editing them does not change the label. Each read also rechecks the
proof context's recorded runtime identity (the installed Sail, Verus, Cargo,
rustc and rustup files and the Cargo configuration). If any of them changed, or
the identity record is missing, an otherwise unchanged result reads as
**Unknown**, and the next run uses a different context.

Job submission uses the same check: a new request joins a running or completed
job only when the recorded inputs are unchanged. Without a record (no Current
run of that context yet), requests never coalesce.

## How a run decides it is Current

Every verification call reads the current input bytes. Its checks can share a
complete source observation within that call while the original mutation
stamps and selected paths still agree. Changes, newly discovered inputs,
uncovered aliases or incomplete coverage require fresh reads. Source hashes
are not carried between calls, and compiler identity checks still read the
actual compiler bytes.

A run is Current only when all of the following hold:

- the content revision of its inputs (every source, manifest, lockfile,
  build-script input and compiler-declared include of the resolved packages,
  and the Cargo configuration) is the same after verification as before;
- the device, inode, size, mode, modification time and change time of every
  input file and directory are unchanged across the run. The change time cannot
  be set by user programs, so an edit that was restored during the run, even
  with its modification time put back, is still caught;
- every installed toolchain file still has the content (SHA-256) it had when
  the run started. A file whose complete stat stamp, change time included,
  equals the one recorded when it was last read on this machine is not read
  again (ADR 0041, 2026-10-26 note);
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

Compiler declarations also require a completed observation from every Cargo
child that the adapter declared before spawning. Sail binds each retained
artifact message to the actual child PID, checks the final frame count after
stdout EOF and a normal child exit, and requires a tagged usable dep-info read
for every ordinary compiler artifact. An observed empty child is distinct from
a missing, malformed or truncated journal
([issue #377](https://github.com/Chelis-Lang/sail/issues/377)). Missing, repeated, foreign or
unreadable dep-info outcomes withhold coverage. A normal nonzero exit can
complete this accounting without establishing a successful proof or receipt.
Malformed or truncated JSON-looking child output and unsupported JSON events
also withhold completion; ordinary verifier summaries and complete multiline
Verus documents remain supported. Optional output accounting stops at 64 MiB
without suppressing forwarded bytes or changing the child exit status.
Fresh verifier diagnostics remain available when declaration coverage is
unknown. Previously uncovered source inventories do not become covered merely
because a later observation is empty. A rerun can therefore remain Unknown;
removing only the source inventory does not establish missing compiler reads.
A complete independently checked compiler-input reestablishment protocol remains
separate work.

Compiler filename decoding follows rustc's space-only filename encoding:
ASCII spaces are escaped; tabs, non-ASCII whitespace and ordinary backslashes
remain literal. Every proposed decoded token is checked against its original
encoded bytes. Unexpected multiline records, foreign phony targets and NUL or
carriage-return token bytes withhold declaration coverage. Source inventory
schema 3 retains older paths but rejects schema 2's coverage: the older decoder
could omit a consumed input while recording different existing paths
([#408](https://github.com/Chelis-Lang/sail/issues/408)). This does not establish
that a first-line text record can represent every possible OS pathname.
Arbitrary newline-containing filenames and rustc's lossy non-UTF-8 rendering
still require a stronger input provider.

This check establishes the observed declaration ledger. Cargo/rustc's guarantee
that dep-info names all actual compiler inputs and the OS's process/filesystem
semantics remain trusted boundaries; the check does not prove atomic compiler
launch identity. See the [input refinement component](design/acceptance-inputs.md)
for the exact proved scope and remaining obligations.

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

### Original compiler histories

A fresh managed proof uses an exclusive per-attempt directory for its raw output,
logs and engine evidence. A normal exit and a familiar filename do not establish
completion. The selected reference must match the original run, target, final
command, process status and complete recorded payload; a later unfinished attempt
cannot reuse its predecessor's completion. Journal failures preserve fresh
verifier output and status while withholding proof reuse and completeness.

Older private receipt formats require a fresh check. Stored native-artifact
transfers preserve and validate the original producer subtree rather than create
a new live invocation from serialized metadata. Missing or modified history
refuses reuse. This does not enable the disabled native-artifact store, establish
a full compiler-input domain or change the existing qualified bundled-proof
import. The experimental same-workspace arena also copies its original receipt,
commit and complete selected-attempt history. A restored run owns a separate
archive copy and retains the original producer identity; removing old run files
or evicting the arena cannot turn it into a new live invocation. Every later
reuse checks that copied history and the current receipt's own commit. Older
arena entries or damaged copies miss conservatively. This does not complete
physical compiler-input or filesystem observation proofs. See
[invocation and result authority](design/acceptance-authority.md).

### Explicit native include directories

For a dependency with Cargo's `links` name, Sail preobserves existing absolute
include directories configured through `<LINKS>_DIR/include` or
`<LINKS>_INCLUDE_DIR` (uppercase links name). For example, `OPENSSL_DIR` lets
Sail read those headers before the first build, rather than discover them only
when the build script declares them afterwards.

These paths add input coverage. The script's actual declarations, complete
content reads, mutation guards and before/after scheduling checks still decide
whether reports can be reused. Other directory layouts use ordinary declaration
learning. Relative and absent hints supply no coverage. See the
[Anvil control](validation/exact-scope-reuse.md#first-recheck-after-native-header-discovery).

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

Successful erased compilation may replace a failed verification's dep-info
with a smaller declared-environment name set. After a complete fresh scan,
receipt admission preserves names from independently valid original receipts
for the same admissible compiler context and scope, and freshly samples every
name in the common union, including absence. The exact name/value-map equality
check remains in place. An unreadable or malformed fresh scan still refuses
reuse, and invalid receipts cannot supply names. This prevents unrelated
successful compilation from discarding a dependency's known declaration
history. It does not observe ghost-only inputs that no successful original
record captured; [issue 386](https://github.com/Chelis-Lang/sail/issues/386)
remains a separate verification-phase coverage gap.

Concurrent dependency publishers reserve a private staging directory before
writing. A name collision leaves the other publisher's files untouched; a
failed publication cannot remove a directory it did not create. This does not
change receipt, source or artifact admission checks.

## Owned command environment observations

Outer managed Cargo and optional build-planning commands use a private owned
builder that records explicit inheritance and final set/remove/clear operations.
It checks actual `get_envs` readback against those independent records immediately
before spawning the same command. Optional records bind the owned child PID and
only environment digests. Capture failure preserves fresh output/status while
withholding Current and retained completeness; it cannot be repaired by a later
input-only refresh. This does not establish complete inner compiler inputs.

A separate full-parent reuse experiment (`26ea66b3`) fixes the ordinary ghost
variable witness but stays unselected: it contradicts the unchanged requirement
that unused authentication changes reuse the same proof context. The default
context/declaration policy below remains unchanged. Completing actual compiler
input observation or an owner contract decision must resolve that conflict.

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
  the target's dependency closure declares: rustc's `# env-dep:` dep-info lines (`env!`,
  `option_env!`, and the Verus driver's own declarations) and build scripts'
  `rerun-if-env-changed`. Each receipt records their values, and it is reused
  only while every one is unchanged and the current declaration inventory
  contains exactly those same names and values.

Dep-info names retain meaningful whitespace and decode rustc's escaped newline,
carriage-return and backslash bytes before looking up a value. Build-script
names follow Cargo's directive interpretation: surrounding instruction-line
whitespace is removed before recognizing the prefix; leading whitespace inside
the name payload remains significant. For example, rustc's
`# env-dep: FLAG =7` names ` FLAG `, while Cargo's
`cargo:rerun-if-env-changed= FLAG ` names ` FLAG`.

An unreadable directory entry, declaration file, malformed name or unsupported
symlinked declaration path supplies no complete declaration scan. A successful
empty scan remains distinct. Fresh verifier reports and diagnostics remain
available, but incomplete scans record no reusable proof receipt and cannot
establish Current input observations. Missing or unreadable build-script output
also supplies no complete script contract. Legacy receipt/read-time records and
proof contexts from the earlier declaration policy cannot authorize new Current
proof evidence; historical reports remain readable.
- **What ghost code reads.** Verus compiles a verified crate twice, and the
  erased compilation rewrites its dep-info without the verification phase's
  reads, so a read in a `proof fn`, spec or assertion alone is not declared
  ([VI-0036](verus-issues/VI-0036-ghost-env-reads-missing-from-dep-info.md),
  #386). For every verified target Sail requests the fork's report of the
  variables rustc tracked in both phases (`SAIL_COMPILER_INPUTS`; the source
  checksum the report needs, `-Zchecksum-hash-algorithm=sha256`, travels with
  Sail's other Verus options in ` __VERUS_DRIVER_ARGS__`, not on the
  compiler's command line). After a
  successful verification it appends each missing read to the unit's dep-info
  in rustc's own form, so Cargo recompiles when it changes, exactly as for an
  exec read, and records the observed names in the invocation. Variables the
  proof context already binds (the families above, including the driver's
  forwarded arguments) are not appended. A receipt requires that observation:
  a verified target whose report is missing, incomplete, unbound to its
  compiler process, inconsistent between phases, or whose dep-info cannot be
  located or rewritten publishes no receipt. Replays of Sail's shipped vstd
  build keep their own recorded compiler environment
  ([ADR 0048](decisions/0048-shipped-dependency-build.md)).

Private dependency authentication (`SAIL_DEPENDENCY_READ_TOKEN` and
`SAIL_PRIVATE_GIT_READY`) does not select a different compiler proof context.
Changing those values alone leaves an unchanged CLI proof reusable. Fetched
dependency contents remain bound by the ordinary source checks. If a source
file or build script explicitly declares a read of either variable, its
receipt still requires that value to match. This exemption does not change
which environment a background job requests.

For build-script declarations, Sail binds `CARGO_FEATURE_*` and `CARGO_CFG_*`
to Cargo's generated values, not the parent process values. A private Cargo
unit graph supplies exact unit features. Native compiler cfg queries supply
target values. Each unit's profile supplies its debug-assertion setting.

A Current run keeps its unit graph and cfg queries for the next run of the
same proof context. The next run restores them instead of asking Cargo again
when their key is unchanged. The key binds the proof context (arguments,
environment, tool identities, Cargo configuration and named target
specifications), the complete Cargo metadata, the bytes of every manifest and
the lockfile, the invoking directory and the owned target directory. Source
edits keep the key. Every restored unit must name a target the current
metadata lists exactly. A configuration that may include another file, a
custom target specification, or a key that changed during the run keeps
nothing.

This model uses Cargo source `5f94df4789f005f9a352888e8355ffc645b7ed0e`.
Unknown Cargo versions, missing observations, and conflicting unit values
refuse reuse. A declared name with an inherited parent value also refuses
reuse. Package declarations `CARGO_MANIFEST_DIR`, `CARGO_MANIFEST_LINKS`,
`CARGO_PKG_NAME` and `CARGO_PKG_VERSION` (including its major, minor, patch
and prerelease components) use the current package metadata. `OUT_DIR` uses
Cargo's recorded directory for that exact build unit, bound to its checked
output file inside the owned target. Other Cargo-generated variables retain
the original refusal.

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
and cleanup keep the caller's environment value. The private metadata setting
never enters the environment, so a build script's own nested Cargo commands
retain their ordinary environment. Public fallback snapshots retain caller
target selection.

Sail's managed Cargo and Verus-driver aliases take their private settings
(`SAIL_MANAGED_*`: the real Cargo, the runtime and artifact journal, the
shipped plan, the output socket, the original driver) from a file beside them
in the compiler family directory, not from the environment. Cargo, build
scripts, procedural macros and compilers therefore see the caller's
environment as under plain Cargo-Verus, including the caller's own `CARGO`
and any `SAIL_MANAGED_*` the caller set (#385). The exceptions are listed in
[known issues](known-issues.md).

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

Receipt declaration scans retain the actual dep-info or build-script output
file of each name. Cargo's artifact paths attribute those files to packages.
Only a file with one known owning package outside the target's complete resolved
dependency closure is excluded. Unknown or ambiguous ownership, or an incomplete
metadata graph, keeps the conservative global name domain. A failed declaration
scan withholds reuse; ownership information cannot repair it.

Originally admitted names are retained only from the same package, Cargo target
name and kind, with the original receipt's scope, version, bytes and values still
valid. They are sampled again, including absence; old values are never copied
into a fresh observation. This preserves verification-phase declarations lost
from later dep-info without importing an unrelated target's names. A new relevant
name or a changed/removed recorded value still refuses exact-map reuse. This
accounts for declared inputs; it does not discover opaque compiler or macro I/O.

**Limitation, shared with Cargo.** A build script or proc macro that reads a
variable without declaring it, including a tool that a build script finds
through `PATH` (`cc`, `cmake`, `pkg-config`), is invisible: Cargo does not
rebuild for it, and Sail does not re-verify for it. Declare such reads with
`cargo:rerun-if-env-changed` in the build script.

## What a proof receipt binds

Currentness compares every file of the selection's packages. A proof receipt
is narrower: it is reused while the files that can change its target are
unchanged. In a path package without a build script, after Sail has recorded
the compiler's declarations of that package's targets, these are the files the
target's own compilation read (its dep-info, which includes `include_str!`
files), the files of the library it links, every other package's files that
the compiler declared, the manifests, lockfile and toolchain files, anything
in a `.cargo` directory, and the other spelling of a declared module file
(`src/m/mod.rs` next to `src/m.rs`). So, as with plain Cargo-Verus:

- a new or removed file that no `mod` names verifies nothing;
- adding a `src/bin` file verifies only that binary, and removing it again
  verifies nothing;
- editing a binary's source re-verifies that binary and its package.

Registry and git packages, packages with build scripts, and packages whose
declarations are not yet recorded bind every file. A declared file that no
longer exists drops out of the record, and its removal still counts as a
change ([ADR 0046](decisions/0046-managed-verification-by-default.md),
2026-10-26 amendment).

## Unchanged-call replay

A `sail verify` identical to an earlier call that ended Current without
starting a verifier replays that call's output without running Cargo
([ADR 0082](decisions/0082-unchanged-call-replay.md)). The call is identical
when its arguments, output mode, terminals, selected tools and every variable
the tools or Sail read are the same. Replay also needs the full stat stamp
(device, inode, size, mode, mtime and ctime) of every input path and walked
directory to match, along with the owned Cargo target's entries and every
installed tool and sysroot file. The proof context's cache records must match
by content, and the variables units declare (ghost-only reads included) must
have the same values.
Anything else takes the full path. Only Cargo's `Finished` time is
regenerated.

The first unchanged call after a verification takes the full path and
records; a file changed less than 2 s before a run is not recorded, so the
next call records it. A failing project re-verifies on every call, as plain
Cargo-Verus does, and is never replayed. Job workers and `SAIL_MANAGED_FORCE`
never replay; `SAIL_MANAGED_NOOP=0` disables replay, and
`SAIL_MANAGED_NOOP_TRACE=FILE` logs each hit, miss reason and recording.

## Reader latency

The M7 reader gate (four clients, 200 Status and Result reads, p95 below
200 ms, every sample Current while another verification runs) is measured
with `scripts/reader-gate.sh`; see
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).

### Watch startup

`sail watch` completes initial Cargo metadata discovery and subscriptions
before starting its first debounce window. macOS can replay earlier creation
notifications, and under load it can deliver an edit's notifications late or
more than once. Immediately before submitting each run, watch records a
complete snapshot of the input bytes and file identities, including change
times. A later notification batch cancels that run only when the inputs no
longer match the snapshot, so a late or repeated notification for an edit the
run already includes does not restart it. The initial snapshot plays the same
role during startup. Real edits still cancel verification, including edits
whose bytes and modification time are restored. A removal or rename of an
input always cancels, because moving a directory away and back keeps every
identity the snapshot records. Unreadable state remains conservative; proof
admission checks do not use this filter.

Native events are only the fast path. Under load, macOS can drop a watcher's
events and send its drop notice late or not at all. So every two seconds, in
every mode, watch compares a metadata stamp of its inputs (path list, file
identity, size, and modification and change times; no file is read) with the
last state it saw. Only when the stamp changes does it rehash the inputs and
restart. On macOS and Linux an input edit therefore cancels the run in flight
within about two seconds, plus one stamp pass, even if its notification never
arrives. The same edit is never reported twice by a notification, the stamp
check and a scan. On an Apple silicon Mac, one stamp pass over Vest's
workspace (3,458 entries) costs about 19 ms of CPU, under 1% of one core at
one pass every two seconds. A full rehash (about 120 ms) runs only when a
stamp changes.

When native notification coverage is incomplete, watch stops reading native
events and adds content scans every five seconds; delayed native events do not
queue the same edit again. Scans compare against the last state seen. When a completed run widens the
watched inputs, watch first checks the previous inputs against that baseline,
so an edit made before the next scan is never adopted as already verified. The
snapshot includes empty input directories, while declared build-output
subtrees remain excluded.

### Partial verification reports

The [exact-scope report reuse](decisions/0069-exact-scope-report-reuse.md)
keeps partial results explicitly incomplete. A report may be reused only for
its exact ordered arguments and unchanged source, runtime and artifact bytes.
Whole-target, shared dependency and instant-revert proof consumers refuse it.
Native bounded controls pass; final release qualification is pending.

With the pinned Cargo, a requested proof failure keeps independently verified
dependencies when the current input checks pass. Platform-disabled packages
with no Cargo execution unit need no observed build-script contract. Missing
or unsupported planning evidence retains conservative cleanup; unexpected
actual units prevent a Current result.

