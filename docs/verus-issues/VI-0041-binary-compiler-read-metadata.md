# VI-0041: Binary includes can report empty-file compiler-read metadata

- **Tracks:** [#599](https://github.com/Chelis-Lang/sail/issues/599)
- **Source:** fork `0ad405eb`, Rust 1.99.1 (also reproduced on `47fb3be6`)
- **Status:** corrected in fork `c9af9534`, merged in [fork PR #16](https://github.com/Chelis-Lang/verus/pull/16); published native runtimes pinned by this candidate; composed Sail qualification in progress

The [minimal input](../../corpus/repros/non-utf8-compiler-read/README.md) proves
that an included non-UTF-8 binary has length one. Its compiler-input record
instead reports the empty-file digest and size zero. Emptying the actual file
makes a fresh verifier reject the proof. No stale Sail proof has been established;
independent source/context guards remain relevant.

Rust 1.99.1's [binary source-map loader](https://github.com/rust-lang/rust/blob/1.99.1/compiler/rustc_span/src/source_map.rs)
returns the original bytes to `include_bytes!`, but constructs empty text when
they are not UTF-8. Reading `SourceFile.checksum_hash` and its text length then
misrepresents the binary input. Reopening its path later cannot certify the
bytes the compiler consumed.

The bounded correction observes the bytes returned by the existing file loader,
preserving its actual behavior and errors. Conflicting reads or incomplete
attribution must withhold coverage. Qualification needs both compiler phases,
non-UTF-8 and empty inputs, same-path text/binary conflicts, restored-mtime
mutation, native input consumers and the ordinary source/corpus controls.

Both native runtimes pass the ten paired compiler controls (relative and absolute
paths, non-UTF-8, restored-mtime mutation, text, empty and CRLF bytes) in both
compiler phases. Linux's actual standalone receipt is Current, reuses with zero
verifier invocations, re-verifies a same-size restored-mtime edit, and rejects
the empty-file negative witness. The [runtime qualification](../validation/binary-compiler-inputs.md)
records complete source-suite, determinism and packaging scope. These controls
do not establish every possible native-artifact consumer.

## C9 native-consumer regression

The first composed shipped-proof factory exposes a regression in `c9af9534`:
its loader observer hashes SHA-256 even when Cargo requests BLAKE3. Native
source-map comparison then withholds every prerequisite certificate, so the
factory cannot record a shipped proof. The [minimal native reproduction](../../corpus/repros/native-checksum-observation/README.md)
passes compilation and exact stock output in both algorithms; C9 emits the
SHA-256 observation and omits the BLAKE3 observation. The correction uses the
compiler's configured algorithm through its existing `SourceFileHash` API,
retaining the same byte, conflict, size and complete-coverage checks. Unknown
algorithms decline coverage. Both native rebuilds are in progress; C9's earlier
source-suite pass is not composed adoption.
