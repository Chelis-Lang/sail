# The engine runtime rejects `by(integer_ring)` that a Singular build proves

**Status:** open ([known issues](../../../docs/known-issues.md)).

**Found:** 2026-10-13, by the deep tier's optional Singular run
(`ci/local-singular.sh`, [ADR 0021](../../../docs/decisions/0021-optional-singular-source-validation.md))
on a native host (rescue lane L25,
[deep-tier record](../../../docs/validation/deep-tier.md)).

**Pinned versions:** Verus `426d8b01e7ffb910a36c15e1f870ff985bb61eef`
(Singular source build, features `["singular"]`), the stock release
`0.2026.10.04.426d8b0`, engine runtime `engine-runtime-13a1176c-linux`
(manifest sha256 `5b866414…7603`), Z3 4.16.0, Singular
`1:4.3.2-p10+ds-1.1build1`.

**Machine:** Linux x86-64, EC2 c7i.8xlarge (us-east-1), the Singular image
built from CI image `sail-ci:bc8748852984b813` at `rescue/m7` `98f6292`.

## What happens

The engine runtime is built without Verus's `singular` Cargo feature, like
the stock binary release. A `by(integer_ring)` lemma therefore fails in the
engine with:

```text
error: Please cargo build with `--features singular` to use integer_ring attribute
```

A Singular-enabled source build of the same Verus commit proves it.

| Verifier | `integer_ring_min.rs` | Exit |
|---|---|---|
| Singular source build (`sail-build.json` features `["singular"]`) | `2 verified, 0 errors` (one `m != 0` warning) | 0 |
| Engine runtime `13a1176c` | the error above | 1 |
| Stock release `0.2026.10.04.426d8b0` | the error above | 1 |

The corpus engine arm compares the engine with the selected plain verifier.
With `SAIL_VERUS_SOURCE_BUILD` naming a Singular build, the project
requirement `verus-feature:singular` is checked against that source build
only, so the engine arm runs a verifier that lacks the feature. In the L25 run
all three Singular units (`integer_ring.rs`, `integer_ring_bound_check.rs`,
`verified-memory-allocator`) were `different`: plain exit 0, engine exit 1,
the feature error the only new message and every function missing from the
engine's results because compilation stopped. Pass-through, cache fill and
cache warm were `same` on all three.

The failure is in the safe direction: the engine reports an error and never
reports an unproved function as verified. It is still a verdict difference
from the plain verifier the arm selected, so it stays `different`.

## Run it

Inside the Singular image (`ci/local-singular.sh` builds it and the Singular
source build), from a Sail checkout mounted at `/work`:

```sh
src=$(cargo xtask build-verus-source --features=singular --print-path)
f=corpus/repros/engine-without-singular/integer_ring_min.rs
"$src/verus/source/target-verus/release/verus" "$f" --crate-type=lib   # exit 0
"$SAIL_ENGINE_BUILD/verus/source/target-verus/release/verus" "$f" --crate-type=lib  # exit 1
/opt/verus/verus-x86-linux/verus "$f" --crate-type=lib                 # exit 1
```

Raw evidence (lead's machine, `~/sail-raw-evidence/l25/`): the run log
`singular.log` (sha256 `99e572ae…5819`), its four reports
`singular-reports.tgz` (`f23cd982…89fd`) and this reproduction's output
`repro-integer-ring.log` (`8ed6cddf…60b1`).

## Possible directions (not decided)

Build the engine runtime with the `singular` feature as a separate variant, or
have the engine arm (and `sail verify --engine interactive`) refuse a project
whose `verus-feature:*` requirements the engine manifest does not record.
