# Compiler filename bytes in dep-info

[#408](https://github.com/Chelis-Lang/sail/issues/408) records a Sail declaration
decoder defect. Rustc escapes ASCII spaces in filenames; tabs, non-ASCII
whitespace and ordinary backslashes stay literal. The former decoder split
Unicode whitespace and removed arbitrary backslashes. This could replace one
consumed filename with different existing files while retaining declaration
coverage.

Run from the repository root, with a new output directory:

```sh
python3.12 corpus/repros/dep-info-filename-bytes/repro.py --out "$SAIL_REPRO_OUT"
```

The pinned Rust compiler accepts a constant assertion with an external file
containing **7**, emits the literal nonbreaking-space filename in dep-info,
and rejects the same assertion after only that file changes to **9**.
Both paths invented by the former decoder exist and stay unchanged. The
runner retains exact compiler output, dep-info, source/input hashes and the
former decoded list. It does not invoke Sail or demonstrate an end-to-end
Sail false Current result.

The native regression runs the actual compiler, passes its actual dep-info
through the repaired production decoder, and checks that changing the real
file changes the observed revision. A source-shared checker validates every
proposed filename against rustc's exact space-only encoding. This per-token
proof does not prove the native document splitting or complete compiler I/O.
Newline-containing or non-UTF-8 paths expose further limitations of rustc's
line-oriented, lossy text format. Unsupported multiline records withhold
coverage; arbitrary ambiguous encodings still require a stronger provider.
