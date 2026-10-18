# VI-0026: macOS native proc macros contain their absolute output path

- Found: 2026-10-17, macOS arm64, Rust 1.98.1 and fork `067d0d7c`.
- Status: expected native linker identity, reproduced; experimental positive
  public reuse/loader and same-host moved-root portable functionality pass;
  release/default first use remains pending. No upstream
  issue filed.
- Reproduction: [minimal Rust macro and consumer](../../corpus/repros/rust-macos-dylib-install-name/README.md).

The actual compiler-boundary vstd fill completes and publishes. A separate
fresh Mac project compiles its own native macro prerequisites, whose exact
bodies differ from the fill. `otool -l` on the retained fill library shows
`LC_ID_DYLIB` names its absolute Sail-owned target path. The strict body key
therefore cannot match across such project directories. The fresh control
also encounters a separate transient snapshot refusal; neither failure is
reclassified as success.

Four minimal actual pinned-rustc invocations reproduce the output property:
the ordinary two output directories yield different dylib SHA-256 values;
two invocations with a stable `@rpath` install name yield identical complete
bodies. All four compiler stderr streams are empty, and all four macro
consumers compile and execute successfully. Result-record SHA-256:
`0344a2e802e44ae627b1b0b73afa08a0ec5ed12e3c5421249b50967d0f51b5e5`.

The unselected boundary experiment now supplies the stable install-name
compiler option only for its admitted Mac proc-macro outputs. It retains the
exact Cargo basename and binds the additional literal tokens into native
compiler provenance before requesting the actual compiler observation. Native
artifacts remain freshly compiled, fully hashed and unmodified. Linux and
ordinary nonexperimental compilation receive no additional option. This
minimal control alone does not qualify the full vstd path. The later public
fill, fresh consumer and unchanged checks also pass with 1,872 identical
function verdicts, original fill provenance, complete native-body equality and
retained loader inspection. All 699 enabled Mac host tests and clippy pass
(35 tests skipped). See the [boundary record](../validation/vstd-compile-boundary.md).
Neither check establishes arbitrary linker support, adversarial qualification,
cross-platform performance or default selection.
