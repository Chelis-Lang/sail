# Managed metadata changes physical compiler arguments

A procedural macro observes the physical `--time` argument and emits a proof
asserting whether it is present. Plain Cargo-Verus rejects this project. The
managed Sail wrapper adds that argument for its internal observations and
accepts the project instead. See [Sail #393](https://github.com/Chelis-Lang/sail/issues/393)
and the [owning reproduction record](../../../docs/validation/managed-physical-argv.md).
This is a compiler-input/default-mode conformance defect, not a solver arithmetic
or verifier translation counterexample.

Copy this directory to a scratch location before running it. Use the same pinned
Sail fork runtime in both arms, and a supported Cargo configuration. In
particular, an existing unsupported global Cargo configuration makes Sail take
its safe plain fallback and hides the affected managed path. A fresh
`CARGO_HOME` with no configuration supplies a controlled environment; both arms
must receive the same environment and unchanged source/lockfile.

With `CARGO_VERUS` naming that runtime's `cargo-verus`, `SAIL` naming the tested
Sail binary, and `SAIL_VERUS_BUILD` naming its validated runtime build root:

```sh
"$CARGO_VERUS" verify -- --output-json
"$SAIL" verify -- --output-json
```

Correct behavior requires the same rejected `project::witness` result. The
reproduced defect gives plain exit 101 with zero verified/one error and Sail
exit zero with one verified/zero errors. Adding original user `--time` to both
commands makes both reject: Cargo-Verus transports user options outside the
physical compiler argument vector, while Sail skips its native injected copy
when the user already requested it. Keep that control; a local argument-parser
roundtrip does not establish physical argument equivalence.

The macro source is the actual tested input and also offers `args_os` and Linux
`proc_cmdline` selectors. This reproduction invokes only `args`; the other
selectors are separate controls, not evidence of additional qualified cases.
The selected runtime remains unchanged. The full acceptance proof must remove
all compiler-visible operational metadata and bind the resulting private channel
without changing original compiler argv/environment, diagnostics or SMT bytes.
