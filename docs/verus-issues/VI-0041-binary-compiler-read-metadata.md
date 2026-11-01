# VI-0041: Binary includes can report empty-file compiler-read metadata

- **Tracks:** [#599](https://github.com/Chelis-Lang/sail/issues/599)
- **Source:** fork `0ad405eb`, Rust 1.99.1 (also reproduced on `47fb3be6`)
- **Status:** metadata defect reproduced; exact-read correction in implementation

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
