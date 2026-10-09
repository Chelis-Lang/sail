# Small complete-run seed portfolio

The pinned Verus release and Z3 4.16.0 verify this polynomial identity.
At `--rlimit 0.01`, the unchanged baseline exhausts the nonlinear query's
budget, while seed 4 independently proves the entire one-function crate.
This is a small end-to-end portfolio example, separate from page-table's
allocation query. It makes no claim about proving the entire page-table crate.

```sh
sail verify --portfolio accept-first-proof --portfolio-seeds 4 --output json \
  --verus-source corpus/repros/z3-nla-seed-portfolio/input.rs -- \
  --crate-type lib --rlimit 0.01 --no-report-long-running
```

The owner-selected mode always tries the unchanged baseline first. Output
retains both attempts, selects seed 4 and labels solver identity, mode and
configuration. No function results are combined across attempts. Resource
exhaustion stays unservable on a warm cache; the seeded proof can be replayed.

The reproduction succeeded on macOS arm64 (fresh, warm and another fresh
cache) and Linux x86-64 in the local CI image. `observations.json` records
the solver hashes, outcomes and effort from those actual runs. These are
functional observations under development-machine load, not speedup benchmarks.
Exact effort can jitter; see ADRs 0009 and 0020. Verus gives its nonlinear
spinoff a smaller resource budget than ordinary queries; the CLI limit is
not itself a literal Z3 resource-unit count.
