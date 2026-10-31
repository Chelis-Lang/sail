# External input discovery recompiles an unchanged installed workspace

Observed on native arm64 macOS 27, mac-m4.metal / us-east-1c, Rust 1.99.1,
stock Verus `7d829946`, fork `47fb3be6`, bundled Sail baseline `715c0f5e`
and candidate `6cfa1606`. The ordinary output E2E's `discovered` case fails
on both; the baseline also fails with the prototype setting absent.

The bounded reduction has one verified function, an external `include_str!`,
a declared environment probe and an unrelated Cargo build in the conventional
output directory. It uses the ordinary Mac temporary directory. Keep both
logical and physical roots in the raw record; their observed difference does
not establish a cause.

With the pinned stock Cargo-Verus on `PATH` and a bundled baseline Sail:

```sh
python3 corpus/repros/macos-external-input-rebuild/repro.py \
  --sail "$SAIL_BASELINE_SAIL" --out "$SAIL_REPRO_OUT"
```

`SAIL_REPRO_OUT` must name a new directory. The helper uses a fresh workspace,
cache and target directories, retains every stdout/stderr/exit, the source and
managed results, then removes only its regenerable workspace. It clears inherited
Sail/Verus selectors; the selected Sail uses its own adjacent pinned runtime.
This is an output/freshness reproduction, not timing acceptance or a new oracle.

Both the original and checked-in parameterized native reductions reproduce this.
The executed native reduction's first plain/Sail calls both print vstd and
one verified function. The second plain call prints nothing; Sail recompiles
and prints both totals again. The third pair prints nothing on both sides.
All six exits are zero. One observed second pair takes 0.166 s stock and
14.957 s Sail; one context does not establish a performance distribution.
No wrong solver answer is demonstrated.

Original reduction archive SHA-256:
`b266c201921c6779610ce321fd124b2f4384f7673d1f92eab370f03c5636cc05`.
Complete default-baseline E2E failure archive SHA-256:
`c0d3061b21f3bbc4cd2b8d670613727871927cd64efb4140bd4252afc097e772`.
The E2E collector reaches its unchanged 1,024-file ceiling and marks that
archive incomplete; the critical original streams are retained. No check,
output or collection limit has been weakened. Root cause remains unknown.
