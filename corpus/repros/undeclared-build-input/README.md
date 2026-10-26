# Undeclared build-script input boundary

The build script reads `external-value.txt` outside its Cargo package and emits
an included Rust constant. By default it declares only `build.rs` as an input.
This is the existing documented limitation on undeclared build-script reads;
this fixture does not establish a soundness bug in the verifier or an accepted
end-to-end production guarantee. It supplies a concrete physical input that
Cargo dep-info, package scanning and script declarations cannot discover.

Copy this whole directory to a fresh temporary location before running it.
Do not change the tracked fixture's value or compile into its source tree.

For the ordinary Cargo mechanism, from the copied `project` directory:

```sh
RUSTUP_TOOLCHAIN=1.99.1 cargo test --offline --lib -- --nocapture
printf '9\n' > ../external-value.txt
RUSTUP_TOOLCHAIN=1.99.1 cargo test --offline --lib -- --nocapture
RUSTUP_TOOLCHAIN=1.99.1 cargo clean
RUSTUP_TOOLCHAIN=1.99.1 cargo test --offline --lib -- --nocapture
```

The expected printed values are **7, 7, 9**: the second build does not know the
external input changed; cleaning reruns the actual reader. With the
`declared-input` feature, `rerun-if-changed` names the external path, so changing
its value from 7 to 9 produces **7, 9** without cleaning. These are ordinary
compiler/build-script observations, not Verus proof acceptance.

The owning native Sail witness must use the qualified pinned fork and Z3 and
record full reports, source/target inventories and actual invocation counts.
Its predicted sequence, with a fresh cache and `external-value.txt` initially 7:

1. Run `sail verify --output json --lib --features verification` until the
   establishing source declarations yield a Current complete requested target;
   retain each earlier establishing result separately.
2. Repeat unchanged. Expect Current, complete and reused target evidence for
   `sail_undeclared_build_input::external_value_is_seven`.
3. Change only the external value to 9. Under the currently documented declaration
   boundary, expect the same Current/reused positive evidence: the external path
   was omitted at the original build-script read. This prediction requires an
   actual native run before it is claimed as observed Sail behavior.
4. Run `SAIL_MANAGED_FORCE=1 sail verify --output json --lib --features verification`.
   The script now reads 9, and the genuine verifier must reject the assertion.
   No complete positive target or repair authority is admissible. A failed proof
   may still describe Current input observations.
5. Repeat from a fresh copy/cache with `--features verification,declared-input`.
   After establishing the value-7 proof, changing the external value must force
   fresh verification and refuse positive reuse; the genuine assertion fails.

No comparator, retry, proof predicate or declaration check is weakened to make
this fixture pass. It identifies the compiler-input provider contract needed by
[#373](https://github.com/Chelis-Lang/sail/issues/373): a supplied declaration list
cannot establish that every actual generator/compiler input was observed.
