# Verification-phase environment dependency

This standalone fixture is the actual stale-positive witness for
[#386](https://github.com/Chelis-Lang/sail/issues/386). The ordinary external
project variable `ORDINARY_PROOF_FLAG` is read only in a proof through standard
`option_env!`. A present value establishes the assertion; absence must fail it.
It is separate from Sail's private transport variable problem.

Copy the fixture outside the repository. Select the qualified pinned fork with
`SAIL_CARGO_VERUS`, and use its same pinned Z3 through `VERUS_Z3_PATH` and
`SAIL_Z3_PATH`. Use fresh cache and target directories outside the copied source;
retain the same project/cache/arguments for the first three commands.

```sh
ORDINARY_PROOF_FLAG=7 sail verify --output json --lib
env -u ORDINARY_PROOF_FLAG sail verify --output json --lib
env -u ORDINARY_PROOF_FLAG SAIL_MANAGED_FORCE=1 sail verify --output json --lib
env -u ORDINARY_PROOF_FLAG "$SAIL_CARGO_VERUS" verus verify --lib -- --output-json --time
```

The real `4f46` candidate produced:

1. Present flag: exit0, Current, complete requested target, confirmed verified.
2. Flag absent: exit0, the same context, Current, complete and reused old proof.
3. Flag absent, forced: exit101, actual assertion failure, Current observed
   inputs, incomplete/unreused requested target.
4. Fresh plain same-fork verification with the flag absent: exit101 and the same
   assertion failure.

The retained positive belongs to the earlier flag-present program. This is an
actual stale confirmed result for a changed ordinary compiler input; neither
fresh verifier accepted the current failing VC. The successful establishing
receipt omitted the variable from its declared-environment map. The exact
verification/erased-compilation dep-info cause requires its own phase witness;
this fixture does not substitute a parser diagnosis for that investigation.

Source SHA-256 is
`3a1b9d3476f07da80e43a156ce59864bb551a0a29053a21b33275eea2ba04612`.
Sail executable SHA-256 is
`3743b6a31aaf008db98a1095af04223379a032e8fca3b06b550d9cd5734dcb5e`.
The issue records each raw stdout/stderr hash. Expected fixed behavior refuses
old positive authority after the flag is removed and preserves actual fresh
failure diagnostics. Unchanged inputs must still reuse, and exact reverts may
reuse only after current observations are established. Legacy records with a
missing verification-phase domain must not acquire coverage from an empty
later inventory. Full undeclared process I/O remains a separate boundary.
