# Trace-counter fixture

`two-checks.trace` is what the pinned Z3 4.16.0 (Linux x86-64) wrote for
`two-checks.smt2`:

```sh
z3 -smt2 -in trace=true trace_file_name=two-checks.trace < two-checks.smt2
```

Counted by hand from the trace, by reading every `[new-match]` and
`[instance]` line:

| qid | `[mk-quant]` | `[new-match]` | `[instance]` |
|---|---|---|---|
| `q_fg` (term `#27`) | 1 | 4 | 4 |
| `q_g` (terms `#30`, rewritten to `#39`) | 2 | 4 | 4 |
| `constructor_accessor_axiom` | 2 | 0 | 0 |

- First check: each quantifier is matched and instantiated once.
- Second check: each is matched three times and instantiated three times.
  Fingerprints `3512071533` (`q_fg`) and `201247922` (`q_g`) appear twice:
  the match is found before a backtrack (`[pop] 1 2`), and found and
  instantiated again after it.
- 24 `[instance]` lines in total: 8 quantifier instances (Z3's own
  `:quant-instantiations 8`) and 16 with fingerprint `0x0` or `0`
  (theory-solving), which name no quantifier.
- 2 `[begin-check]` lines, 335 lines, ends with `[eof]`.
