# Proof-note JSON order

Run Sail's pinned fork repeatedly:

```sh
verus --output-json --multiple-errors 2 repro.rs
```

The proof deliberately fails. Its two note sets are stable, but their JSON
arrays appear in different orders on the affected fork. See
[VI-0021](../../../docs/verus-issues/VI-0021-proof-note-json-order.md) for
runtime identity and the retained eight-run evidence. No query normalization
or comparison exemption is applied.
