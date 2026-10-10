# SpanData formatting requires the compiler session

This is an optional Sail engine defect, not an upstream Verus issue.
Pinned rustc is `48a229ceaefd4985c50990b14116b6d856af0985` (1.98.1).
`rustc_span::SpanData` Debug calls `self.span()`, which can use the session
span interner. Stock Verus explicitly forbids this operation on its ordinary
verification worker threads (`rust_verify/src/spans.rs`).

The engine used that formatter when binding local checkpoint provenance.
A small compressed span did not exercise the interner; this long span does:

```rust
std::thread::spawn(|| {
    let raw = rustc_span::SpanData {
        lo: rustc_span::BytePos(10),
        hi: rustc_span::BytePos(100000),
        ctxt: rustc_span::SyntaxContext::root(),
        parent: None,
    };
    format!("{raw:?}")
}).join();
```

An isolated source unit test reproduced the scoped TLS panic on the worker
without any solver or proof. The real HumanEval repair then showed the same
first panic, followed by worker/message-channel failures. Its full result was
incomplete and returned 101; it was never accepted as a proof.

The correction must use all exact owned SpanData fields without calling
Span conversion or compiler-context formatters, with a new local provenance
domain and cache version. Diagnostics and solver input remain subject to
exact differential checks.
