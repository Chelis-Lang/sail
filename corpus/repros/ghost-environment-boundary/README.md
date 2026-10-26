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

## Executable candidate regression

Run the complete candidate control with a fresh output directory, a built Sail
binary beside `sail-z3`, an existing pinned fork build and a prepared Cargo
registry home:

```sh
python3.12 corpus/repros/ghost-environment-boundary/repro.py \
  --out "$SAIL_REPRO_OUT" --sail "$SAIL_CANDIDATE" \
  --build "$SAIL_FORK_BUILD" --cargo-home "$SAIL_REPRO_CARGO_HOME"
```

The runner validates every pinned runtime file and owns its cache daemon. It
requires **7** to establish proof, the unchanged command to reuse that proof,
and removing only the flag to change context and return the real assertion
failure without forcing. It also requires plain same-fork verification with
the flag absent to fail. Exact outputs, hashes and commands remain outside the
repository in the requested directory. Experiment `26ea66b3` passes this control but stays unselected because it fails
the unchanged unused-authentication reuse requirement. Default behavior keeps
that policy and still needs the actual per-compiler observation fix for #386.
This runner remains the required regression for that eventual correction;
passing it alone is insufficient for selection.
