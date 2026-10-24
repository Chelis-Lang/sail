# Evidence-acceptance model qualification

Status: initial standalone models pass direct stock-Verus checking on macOS
and native Linux. The production acceptance core, concrete adapter refinement
and composed service theorem remain unproved. This record qualifies the
[initial checker and models](../design/acceptance-proof-checker.md), not default
runtime behavior or release performance.

## Source, runtime and scope

The checked checkpoint is `8b0ae88540f8fb52a9ac2e443a94de124baa5b90`.
It contains the three component roots, complete function/count inventory,
six deliberate mutations and the reviewed independent Python checker. The
checker reads the selected commit's Git blobs and compiles those captured
bytes, records mutated-candidate digests, and checks normal termination and
full-crate scope. Its own implementation remains ordinary reviewed/tested
code; it is not formally verified.

The upstream stock pin is Verus `0.2026.10.18.7d82994`, commit
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and pinned
Z3 `4.16.0`. Each platform's archive and every extracted runtime file are
validated against `corpus/toolchain.toml`. The direct command always uses
`--no-cheating --output-json --num-threads 1`, an explicit pinned Z3 and a
sanitized environment. Neither Sail, its fork nor its caches establish these
proofs.

| Platform | Environment | Positive roots | Deliberately broken controls | Result |
|---|---|---:|---:|---|
| macOS arm64 | Local native host, pinned stock archive/runtime | 3 | 6 | All nine checks pass |
| Linux x86-64 | Native EC2 `c7i.4xlarge`, `us-east-1`, CI core image | 3 | 6 | All nine checks pass |

On each platform, positive verification counts are query association **24**,
input binding **22**, scope/result authority **28**: **74 total**, zero errors,
normal exit zero and whole-crate scope. Each negative control preserves the
entire function inventory and fails with one ordinary verification error,
normal exit one and whole-crate scope. No syntax/VIR error, signal, timeout or
missing document substitutes for the expected failure.

Raw receipts remain outside the repository:

- macOS receipt SHA-256: `81b9dc24f47aadc082cd48ab43939d16554151e753de15f5c145cff2dfa5c412`.
- Linux receipt SHA-256: `fda1a6921ce16aeaed65ad9ab50d9ea91f77bc6a45bf674777e159f4a07186d8`.

Each receipt includes the commit, exact source/runtime hashes, toolchain,
invocations and individual result documents. The nine checker failure tests
also pass locally. They exercise malformed evidence, omitted source/function
inventory, swapped runtime files, source-loading constructs and inherited
argument/loader settings; fixture tests alone do not qualify a real proof run.

## Remaining gates and limits

- The combined workspace gate and fresh hosted checks remain separate
  publication requirements. Existing runtime code, compiler/solver inputs and
  corpus selection are unchanged by this model slice; no new conformance or
  performance result is claimed.
- The query model still needs production parser/session/writer/recovery
  refinement and an admitted initial-store/writer contract.
- Input equality does not prove complete compiler-input observation or the
  Linux/macOS filesystem/process adapters. Digests are not assumed injective.
- Scope/result predicates are not shared production source. Actual report,
  receipt, repair, retained-job and CLI/MCP projections still need refinement.
- Runtime/checker/toolchain immutability is a trust assumption; pre/post hashes
  cannot establish atomic launch binding against transient replacement.
- Actual proof invocations are local-only in this slice. Hosted proof CI and
  its duration/packaging qualification remain open. The ordinary workspace
  gate includes the Python failure tests.

No complete gate attempt for these frozen models failed or was interrupted.
Intentional mutation failures are successful negative controls, not successful
verification of the mutated models.
