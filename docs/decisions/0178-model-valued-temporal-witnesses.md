# 0178. Preserve native model values in temporal witnesses

Accepted for native temporal checking and source validation in development builds.

An ordinary TLC model value can coexist with a same-spelled TLA+ string,
including as distinct keys of one function. The published Disruptor two-writer
control needs such model-valued writer and reader IDs in its counterexample.
A generic ITF string cannot recover the distinction in untyped TLA+.
[Apalache's ITF value types](https://apalache-mc.org/docs/adr/015adr-trace.html).

The Sail trace wrapper declares `sail-model-values-v1` when a state contains
a `#sail-model` tagged atom. Conversion maps that single string-valued tag
to the native trace model value at the same structural position. Encoding
and decoding must round-trip exactly. Independent replay still checks Init,
every step and the actual ending against the submitted model.

A typed discriminator distinguishes the extended representation on the
wire. Existing public Rust trace literals retain their fields. Serialization
derives the discriminator from the states. A legacy document has exactly
its original representation and digest. A legacy reader encountering the
new marker rejects the document. The Verus counterexample output schema
continues to use its own rendering rules.
