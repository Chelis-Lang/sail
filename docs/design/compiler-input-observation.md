# Compiler-consumed input observation proposal

This is a source-reviewed implementation plan for
[#373](https://github.com/Chelis-Lang/sail/issues/373) and
[#374](https://github.com/Chelis-Lang/sail/issues/374), not an implemented
provider or a complete-service proof. The next useful slice records buffers
at their actual consumer boundary. It must not turn a completed declaration
list into authority over unobserved procedural-macro or build-script I/O.

## Source baseline and existing limits

The selected fork in [fork.toml](../../engine/verus/fork.toml) is
`36d65c3f3161de34efa0150231a47c1777369a5a`. Source review also covers candidate
`12459c9cc3a3022c91822643aa1cb8e7fac74871` in `Chelis-Lang/verus`.
Its `source/rust_verify/src/sail_compiler_inputs.rs` and
`sail_native_inputs.rs` retain the selected hooks unchanged. This proposal
does not select that candidate or change a runtime pin.

The reviewed fork entry points are the selected
[phase collector](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/rust_verify/src/sail_compiler_inputs.rs),
[compiler callbacks](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/rust_verify/src/driver.rs),
[VIR importer](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/rust_verify/src/import_export.rs),
[trait-check generator/loader](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/rust_verify/src/trait_check.rs),
[Cargo dependency tracker](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/rust_verify/src/cargo_verus_dep_tracker.rs)
and [Verus macro environment reads](https://github.com/Chelis-Lang/verus/blob/36d65c3f3161de34efa0150231a47c1777369a5a/source/builtin_macros/src/lib.rs).

The compiler hook runs after verification translation in
`VerifierCallbacksEraseMacro::after_expansion` in `verifier.rs`, and after
analysis of erased compilation in `driver.rs`. It retains each phase in memory
and requires both before publishing a successful final document. Native-only
compilation uses `DefaultCallbacks::after_analysis` and a distinct document.
The phase record contains:

- Checksums and unnormalized lengths of real, non-imported SourceMap files.
- `file_depinfo` path declarations and `env_depinfo` name/value declarations.
- An echoed request binding and process ID, with bounded final publication.

These are useful declarations. They contain no raw binary input buffers,
all-I/O interception, independently authenticated writer, or supported atomic
filesystem observation point. The hook's `complete` field accounts for its
two callbacks. It does not establish complete physical input discovery.
Imported SourceMap files are explicitly filtered out. Displayed filenames are
strings rather than lossless native pathname tokens.

The hook's existing CRLF unit case distinguishes the original source checksum
and unnormalized length from `SourceFile::src`, which already contains normalized
newlines. An observer must identify which representation it records. Rehashing
the normalized string cannot attest the original loader result.

The [procedural-macro fixture](../../corpus/repros/proc-macro-read-escape/README.md)
already changes a genuine verification result using an ordinary external file
read omitted from both phase maps and final dep-info. It is a standalone
provider-completeness witness, not an executed Sail false-Current reproduction.
The [undeclared build-script fixture](../../corpus/repros/undeclared-build-input/README.md)
shows the analogous Cargo rebuild limitation. No new fixture executions or
process/environment probes were performed for this proposal.

## Buffers and observations to account for

All paths below are relative to the Verus fork unless a Sail path is named.
Entries distinguish inspected consumer code from hooks whose implementation
still requires inspection of the pinned Rustc source.

| Input class | Actual consumer boundary | Required observation and remaining limit |
|---|---|---|
| Rust source and loader-served includes | Rustc `FileLoader::read_file` returns `String`; `read_binary_file` returns `Arc<[u8]>` | Record the exact successful delegate result before returning it, with a distinct text/binary kind. Preserve native requested path bytes and original phase. Establish include routing from the pinned Rustc implementation; do not assume every include uses these methods. |
| Generated trait-check source | `trait_check.rs::check_trait_conflicts` constructs `rust_code`, writes its bytes to child stdin; `main.rs` reads stdin into a `String`; `driver.rs::TCCallbacks` installs `TCFileLoader`, which returns that code | Relate the original generated buffer, actual write/EOF, actual child read and the same loader's returned buffer. The synthetic `dummyrs.rs` name is not a physical file read. Capture a separate trait-check phase and child relationship. |
| Imported VIR | `import_export.rs::import_crates` opens a `File`, wraps it in `BufReader`, and passes it directly to `bincode_next::serde::decode_from_std_read` | Observe reads on that owned reader, including success/error and decoder completion. Distinguish decoder-consumed bytes from buffered read-ahead. Do not replace this with a later path hash, assume EOF, or change acceptance of trailing data. |
| Rust metadata and compiler-loaded native libraries | `externs.rs::VerusExterns::to_args` supplies rlib and macro-library paths; loading happens inside Rustc/the native loader | The argv path establishes a request, not the actual loaded object. Locate metadata and library-load boundaries in pinned Rustc before claiming coverage; source loader wrapping alone does not cover them. |
| Builtin `env!`/`option_env!` and tracked includes | Current hook reads accumulated `env_depinfo`/`file_depinfo` after expansion | Retain the actual read result before erasure: present raw value, absent, invalid encoding and read error remain distinct. Inspect the pinned builtin expansion sites; a later environment snapshot or tracked-path name is insufficient. |
| Verus launch/dependency selectors | `cargo_verus_dep_tracker.rs::get_env` reads actual Unicode values/absence and `config_install` later inserts them into session dependency tracking; `cargo_verus.rs::handle_externs` selects an existing `.vir` candidate | Capture these earlier observations and lookup order too. Registering the selected file does not preserve earlier absent candidates. Chain the existing `track_state` callback rather than replacing it. |
| Verus procedural-macro environment | `source/builtin_macros/src/lib.rs::vstd_kind` directly reads `VSTD_KIND` and `CARGO_PKG_NAME`, memoizing its result in `OnceLock` | These direct `std::env::var` results are separate from Rustc builtin dependency tracking. A cooperative hook must retain the first observed result and later memoized use. Do not assume a Rust static in a loaded macro shares the compiler's observer instance. |
| Arbitrary procedural macros | The existing fixture reads through ordinary `std::fs::read_to_string` and emits tokens | Compiler text/binary loader hooks do not see this read. Macro artifact identity and expansion tokens do not provide a future-input invalidation rule for an undiscovered external file. |
| Cargo build scripts and descendant tools | Sail `managed/build_environment.rs` captures a Cargo cfg query; declaration providers retain Cargo output | Query output and `rerun-if-*` declarations do not capture arbitrary script/descendant I/O. Retain actual generated bytes consumed by the compiler, and separately establish when the producer must rerun. |
| Effective process inputs | Sail's owned outer `command_environment::Command` seals its own final environment operations before its own spawn | This covers the migrated outer command construction. It does not describe Cargo's inner compiler/build-script environments, inherited descriptors, stdin, current directory, loaded images or original-submission correspondence. |

For a successful text delegate call, the first increment proves the exact UTF-8
bytes of the returned `String`. Calling them the original filesystem byte
stream additionally requires a source-bound delegate/read contract, including
encoding and newline behavior. For a failed text call, retain its actual
failure classification; do not invent bytes the interface never returned.
The binary interface can retain the exact returned `Arc` without reopening the
path. Neither fact alone identifies an atomic filesystem version.

## First implementation slice: phase-owned consumer events

Add a private `consumer_inputs` module under `source/rust_verify/src/`, initially
independent of final document transport and default proof selection. The first
patch should cover the source and generated-code loaders and the VIR reader.
It should leave Rustc builtins, metadata loading and opaque plugin I/O explicitly
unsupported. This is small enough to review without introducing a new Rustc
fork, dependency edge, unsafe code or privileged host service.

Proposed internal interface, with names descriptive rather than a frozen ABI:

```text
ConsumerSession::begin(original_request, phase, owned_attempt) -> Session
Session::wrap_loader(original_loader) -> ObservingLoader
Session::wrap_vir_reader(original_owned_reader, native_path) -> ObservingReader
Session::generated_source(original_buffer, generator_relation) -> GeneratedInput
Session::finish(actual_phase_result) -> RecordedConsumerPhase
```

Constructors stay private to actual callback/reader construction. There is no
constructor accepting a supplied successful observation list. A phase token
retains its original request, phase, owned attempt and ordered events. Each
event includes a sequence identity, operation kind, native path or explicit
generated identity, exact observed outcome, and the buffer actually forwarded
to the compiler/decoder. Repeated requests remain repeated events; a pathname
map must not erase different reads of the same path. Bounded overflow, poisoned
state, missing callbacks, unsupported loader variants or capture failure latch
an incomplete observation. They do not replace the delegate's result.
Native names need a platform-tagged lossless encoding, with no displayed-path
round trip. Sensitive values stay in bounded private memory; retained/public
observations use domain-separated digests and outcome tags, not environment
values or source-buffer dumps. Per-read observations must not assume that a
later environment lookup equals an earlier one.

`ObservingLoader` delegates exactly once and returns the same result. If Rustc
already supplied a custom loader, wrap it rather than overwrite it; in
particular preserve `TCFileLoader`. Forward `file_exists` and
`current_directory` too, recording their actual results as observations rather
than silently treating absence or cwd as configuration constants. The reviewed
`TCFileLoader::file_exists` currently panics if called: wrapping must not invent
such calls. No extra existence checks, reads or normalizations belong in the
forwarding path.
In particular, `file_exists == false` records that Boolean interface result;
it cannot manufacture an OS `NotFound` outcome. A genuine delegate error can be
a valid observed input if the original compiler handles it and continues.
Capture failure and an observed compiler read error are different states.

For VIR, observe the actual reader supplied to the existing decoder. Record
each successful read prefix and decoder outcome while preserving its original
buffering and error behavior. Keep an inner observation on the original owned
file under `BufReader` and an outer observation at the decoder-facing `Read`
boundary if the relation needs both physical read-ahead and consumed prefixes.
Both must refer to that same owned reader chain. A completed decode does not
assert physical EOF.
Any claim about a full-file identity must account separately for unread suffixes
and the original opened object; do not change decoding to `read_to_end` merely
to simplify the proof.

Capture verification events from phase entry through the existing translation
callback, before that phase's state is dropped or ghost code is erased. Maintain
independent compilation and trait-check sessions. Move phase completion after
every relevant scoped consumer finishes; if a later read exists, an earlier
callback cannot be a terminal seal. The first patch must inventory actual
callback entry/exit paths, including early error and native-only paths, rather
than assume the existing two names exhaust all compiler execution.

The source-shared proof target is correspondence: each admitted event contains
exactly the returned/forwarded bytes and original phase/request identity;
missing, duplicate-terminal, foreign, capture-failed or unsupported events cannot produce
`RecordedConsumerPhase`. The native adapter's std/Rustc read semantics remain
named assumptions. This token deliberately cannot construct `CompilerDomain`
or `Current` by itself. Buffer retention limits and hashing cost need native
measurement before selection; digest-only equality retains the existing named
cryptographic condition rather than proving injectivity.

## Builtins and opaque code require different work

After the first slice, inspect the pinned Rustc implementation of builtin
includes/environment expansion and metadata loading. Where existing safe
interfaces expose the actual buffer/result, add a trusted compiler hook and
retain both phases. Where no hook exists, changing Rustc is a toolchain/backend
fork decision under the charter; it cannot be described as an ordinary Verus
callback patch. Changing Verus's cooperative builtin macros can expose their
own reads, but needs a real cross-library observation channel and does not
intercept user macros' direct standard-library calls.

The minimum opaque-input inventory includes file and directory reads, metadata
and failed lookups, symlink resolution, environment and physical argv, inherited
descriptors, stdin, native library loads, subprocesses, network input, clock and
randomness. Directory/negative observations matter because a later file can
change execution without changing any previously recorded positive file.
An event count, source checksum inventory, watcher, completed Cargo journal or
cooperative declaration list cannot certify this inventory exhaustive.

Owner architecture choices remain:

| Alternative | What it could establish | Decision/limit |
|---|---|---|
| Safe cooperative Rustc/Verus hooks only | Exact observed builtin/loader/decoder inputs within an explicitly supported provider | Implementable incrementally. Opaque direct I/O remains unsupported for complete reuse authority. |
| Mediate all relevant process-tree I/O at an OS boundary | Discover or enforce the supported external input domain, including plugins/descendants | Requires a concrete Linux/macOS mechanism, semantics for mmap/FDs/library loads/negative lookups, privileges/unsafe/dependency review as applicable, and an ADR. No such provider is established here. |
| Execute against a controlled immutable input world | Relate reads and executable images to retained immutable objects | Requires an owner-approved architecture and a fidelity argument. A copied workspace omits undeclared external inputs; denying an ordinary macro read changes behavior. |

No alternative is selected by this proposal. A provider that denies an ordinary
read may withhold reuse authority, but its new compiler error cannot replace
plain pinned-fork diagnostics under the default contract. Time/random/network
inputs require a specified observation/replay or non-reuse policy; they cannot
be silently omitted as volatile.

## Observation and launch interfaces for #374

Keep three separate private relations:

1. **Original launch intent:** original submission, actual argv bytes, cwd,
   explicit environment inheritance and final operations, stdin/descriptor
   policy, expected executable/runtime and attempt identity.
2. **Owned launch result:** that constructed native command's actual spawn,
   owned child, process identity, original output channels and normal wait
   result. The outer wrapper is a useful starting point; migrate inner
   adapter/compiler/trait-check commands without exposing mutable raw builders.
3. **Consumed-object observation:** each actual reader/buffer and executable
   image, with the provider's supported stability boundary. A path digest
   followed by an unrelated pathname launch does not establish this relation.

Existing opened-read endpoint stamps support a bounded read trace, not a
global atomic content point. A stronger provider must state whether it retains
an immutable object, a live descriptor/lease, a snapshot, or another concrete
mechanism, and how the actual execution uses that same object. Rust's standard
pathname `Command::spawn` does not supply an opened-image launch guarantee merely
because a file was hashed first. Platform-specific executable-handle/lease or
immutable-runtime execution requires source and OS contract review and any
necessary owner approval; this proposal adds none.

The control channel is also part of fidelity. Selected hooks use visible
`SAIL_COMPILER_INPUTS*` environment variables, and the candidate restores a
parser-local argv vector after stripping private observation flags. Neither
restores physical `std::env::args_os()` or hides controls from arbitrary macros.
The existing [boundary record](../validation/acceptance-boundary-refinements.md)
keeps those failures distinct. A protected channel must bind the original owned
launch to its observations without changing original observable arguments or
environment. An inherited descriptor or extra file is a candidate requiring
its own visibility/identity contract, not an automatically transparent answer.
Transport selection must precede default authority integration.

## Default behavior and integration gates

This proposal leaves existing default context identity and conformance checks
unchanged. The unselected full-parent fingerprint fixes the ordinary
[#386](https://github.com/Chelis-Lang/sail/issues/386) witness but fails the
unchanged unused-authentication reuse assertions in `ci/test-managed.py`:
unused auth changes reuse; an explicit credential read forces verification.
Do not select that shortcut or modify the assertions. Recording actual relevant
reads must distinguish these cases while preserving present/absent/raw values
and effective Cargo-generated child environments.

An optional collector failure must preserve original fresh output and exit
status and withhold the new reusable-complete/Current authority. Unsupported
opaque inputs cannot silently acquire that authority from a partial phase
record. This describes the new provider's admission rule, not a claim that the
current default stale-reuse defects are already corrected. Replacing legacy
admission needs a reviewed migration and the unchanged reuse/conformance gates.

Planned checks, **not run for this source-only proposal**, are:

- Direct stock-Verus checks of the shared consumer-event transitions, constructive
  nonempty witnesses, and ordinary mutations dropping/changing a forwarded
  buffer, request, phase, terminal outcome or actual read prefix.
- Normal loader and VIR-reader controls preserving success, errors, partial
  reads, CRLF, native pathname bytes, repeated reads and custom-loader behavior.
- Existing genuine ghost-environment, opaque proc-macro and generated-input
  fixtures, plus actual original-command fresh/warm admission when authorized.
  Separate declaration/standalone evidence from an executed Sail verdict.
- Unchanged authentication reuse, diagnostics, relay/cancellation and complete
  native/macOS/fast/standard gates, with exact source/runtime receipts and
  measured capture cost before selecting a runtime or enabling authority.

Closing #373 requires a supported provider contract covering every actual
proof-relevant input, not just these first hooks. Closing #374 additionally
requires an implemented native observation/launch boundary with its explicit
atomicity limits. Neither follows from a new schema, proof of supplied rows,
or successful callback accounting.
