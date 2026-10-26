# VI-0035: Compile-only invocations report successful whole-crate JSON

Status: reproduced option/observation boundary in pinned stock Verus; not an
upstream defect claim. No fork change or upstream report is proposed.

## Reproduction

The minimal [false-assertion source](../../corpus/repros/verus-no-verify-success/main.rs)
and [commands](../../corpus/repros/verus-no-verify-success/README.md) are checked
before implementing Sail's admission guard.

On native macOS arm64, stock Verus `0.2026.10.18.7d82994`, source
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and pinned Z3
`4.16.0` produce:

| Invocation | Exit | success | verified | errors | entire crate | function details |
|---|---:|---|---:|---:|---|---|
| ordinary | 1 | false | 0 | 1 | true | Includes the failed function |
| `--no-verify` | 0 | true | 0 | 0 | true | Empty |

The advertised skip option has exactly the spelling `--no-verify`. Abbreviated
spellings are rejected; assigning a value with `=` is rejected. Other options
that skip particular checks are separate configuration boundaries.

## Sail implication

The JSON describes a successful compile-only invocation. It is not evidence
that the false assertion was proved. A validator must bind verification
configuration to the actual invocation, rather than deriving verification
from exit status, success/count fields or an empty function list.

In particular, checking only the outer Cargo arguments misses effective
per-package driver options. Genuine successful zero-function verification
remains admissible when verification was enabled. This distinction belongs in
Sail's source-shared admission predicates and concrete invocation mapping.

The direct probe qualifies stock behavior only; it is not whole-corpus or
production acceptance. The production source-refinement record owns the
candidate guard, actual call-site mapping and remaining gates.
