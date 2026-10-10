# Cargo-Verus option ordering

Observed with pinned Verus `0.2026.10.04.426d8b0` on 2026-10-10.
Cargo-Verus stops parsing its verification-relevant Cargo selectors when it
encounters an option it forwards directly to Cargo. Ordinary Cargo accepts
the same options in either order. This is a documented parser restriction;
Sail must preserve it when injecting its own options.

In a Cargo project named `demo` that enables Verus verification:

```sh
cargo verus verify --target-dir target --message-format=json-render-diagnostics --package demo --lib
```

This fails before invoking Verus, reporting:

```text
The Verus-relevant command-line argument --package can't follow the Verus-irrelevant argument --message-format=json-render-diagnostics because that will cause the Verus-relevant argument to be ignored.
Args forwarded to Cargo must precede args forwarded to Verus
```

The accepted ordering is:

```sh
cargo verus verify --target-dir target --package demo --lib --message-format=json-render-diagnostics
```

Sail puts its owned `--target-dir` before the unchanged caller options and its
owned message format after them, before the `--` introducing Verus options.
Reordering caller options or dropping package/target selectors would change
their meaning. The real repair CI fixture covers explicit package and library
selection together.
