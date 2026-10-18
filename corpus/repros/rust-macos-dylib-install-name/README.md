# macOS native proc-macro install names

On macOS, compile `lib.rs` with pinned Rust 1.98.1 twice, with identical source
and options but distinct output directories:

```sh
rustup run 1.98.1 rustc --crate-name sail_native_identity --crate-type proc-macro \
  --edition=2021 --emit=dep-info,link -C metadata=sailrepro \
  -C extra-filename=-repro -C embed-bitcode=no --out-dir OUTPUT lib.rs
otool -l OUTPUT/libsail_native_identity-repro.dylib
```

Use the same absolute source path and working directory for both invocations.
`LC_ID_DYLIB` contains each absolute output path, and the complete dylib bytes
differ. This is a native Rust/linker output property, not SMT nondeterminism.

Repeat with this additional compiler option:

```sh
-C link-arg=-Wl,-install_name,@rpath/libsail_native_identity-repro.dylib
```

The retained four-invocation control produces byte-identical complete dylibs
in the latter pair, with no compiler stderr. Each of the four libraries also
compiles and runs `consumer.rs` successfully through an explicit `--extern
sail_native_identity=LIBRARY`. This is a minimal native compiler and loader
control, not Sail conformance or a performance measurement. See
[VI-0026](../../../docs/verus-issues/VI-0026-macos-native-install-name.md).
