# Cache-warm runs can print stock's errors in another module order

**Found:** 2026-10-13, by the standard tier's default cache-warm phase on a
native host (rescue lane L12).

**Pinned versions:** Verus `0.2026.10.04.426d8b0` (stock release, default
mode), Z3 4.16.0 (`corpus/toolchain.toml`).

**Machine:** Linux x86-64, EC2 c7i.8xlarge (us-east-1), CI image built from
`rescue/m7` plus lane L12's branch.

## What happens

`verus-examples/examples/debug_expand.rs` (Verus repository at the pinned
commit) fails with 14 errors in two modules: the root module and `M4`. Plain
Verus printed `M4`'s error (`debug_expand.rs:251:16`) first in every run on
this host. In one cache-warm run, `sail` (stock Verus with `sail-z3` answering
from the warm cache) printed the root module's error (`debug_expand.rs:17:12`)
first. Verdicts and the set of errors were the same; only the order differed,
so ADR 0009's three confirming plain runs classified the unit `different`.

| Arm | Runs | `M4` first | root first |
|---|---|---|---|
| plain Verus (confirming and extra runs) | 19 | 19 | 0 |
| `sail` cache-warm | 7 | 6 | 1 |
| `sail` pass-through and cache-fill | 6 | 6 | 0 |

Raw evidence (lead's machine, `~/sail-raw-evidence/l12/`): the failing
standard run `final/conformance-standard.log` (sha256 `a6953e30…`) and its
cache-warm report (`f2b43fea…`); the 10 extra plain runs and 5 extra warm
runs, with the first error of each, `dx/orders.txt` (`04d46b87…`) and
`dx/run.log` (`f0b28268…`).

**Suspected cause (not established):** stock Verus prints each module
bucket's diagnostics in the order the buckets first report
([VI-0007](../../../docs/verus-issues/VI-0007-nondeterministic-error-order.md)).
With every solver answer served from the cache, the root module's bucket can
finish before `M4`'s, which on this host it never did with the real solver.
The cache does not change which errors are reported.

## Run it

From a Sail checkout, in the CI image, run the unit's plain arm and then its
cache arms repeatedly, and compare the first error location of each stderr:

```sh
sel="--tier standard --project verus-examples --filter debug_expand"
./target/release/sail-conformance run $sel --jobs 1 --plain-runs 10 --out out/plain10
./target/release/sail serve --socket /tmp/dx.sock --cache /tmp/dx-cache --idle-timeout 0 &
SAIL_DAEMON_SOCKET=/tmp/dx.sock ./target/release/sail-conformance run $sel --jobs 1 --out out/fill
for i in 1 2 3 4 5; do
  SAIL_DAEMON_SOCKET=/tmp/dx.sock ./target/release/sail-conformance run $sel --jobs 1 --out out/warm-$i
done
grep -o -E 'debug_expand.rs:[0-9]+:[0-9]+' -m1 out/*/units/*/*/stderr.txt
```

The root-first order is intermittent (1 of 7 warm runs so far).
