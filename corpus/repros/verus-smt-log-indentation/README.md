# SMT file logging changes forwarded whitespace (VI-0024)

`emitter-test.rs` is the complete minimal AIR unit used against normal fork
`73dea02112d730e7ab52f58158012d440fb89512` plus the disabled context research
branch. It changes no production emitter code. Place the module in
`source/air/src/emitter.rs` in a disposable fork checkout, then run from `source`:

```sh
cargo +1.98.1 test --locked -p air --lib smt_file_log_changes_literal_pipe_indentation -- --nocapture
```

Actual macOS execution selects exactly one test and passes. With no file log,
the literal pipe bytes are:

```smt2
(push)
(assert
 (and
  true
  false
))
(pop)
```

With the file log attached, they are:

```smt2
(push)
(assert
  (and
   true
   false
 ))
(pop)
```

The file writer is an ordinary sink; its contents do not influence the test.
An earlier one-line `get-info` fixture produced identical bytes and did not
demonstrate the behavior. The difference needs a multiline node under a push.
This is a compiled AIR-emitter reproduction, not a native full Verus run or a
verdict disagreement. No upstream report has been filed.
