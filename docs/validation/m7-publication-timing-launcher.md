# Original repair publication timing diagnostic

The prepare-only launcher `scripts/m7-publication-timing-diagnostic.py` loads the
byte-exact original `ci/test-repair.py` with `runpy` and calls its unchanged
`run` function. It fixes the original proof/tool budget at 120 seconds and retains
the fixture's 30-second CLI/MCP operation cap and 5-second MCP cleanup waits,
assertions, source changes,
owned actor cleanup and public output collection. It enables only the existing
`SAIL_JOB_RPC_TIMINGS=1` observation. No verifier, daemon or acceptance actor was
started while preparing or testing this launcher.

After the fixture's original `finally` attempts owned shutdown, a narrow
`TemporaryDirectory.__exit__` adapter copies only `rpc-timings.json` and
`events.setup-*.jsonl` from numeric job directories. Original temporary-directory
cleanup runs even when retention fails. Retention errors cannot replace an
original fixture exception. The original public evidence remains in `public/`;
validated timing files and their byte-exact digest inventory go into `timings/`
and `diagnostic.json`. Private submissions, environment, context, runtime and
report-cache files are neither read nor copied by the adapter.

The adapter rejects links, directories, FIFOs, changing files, unknown fields,
duplicate JSON keys and nonnumeric payloads outside fixed enum labels. Its
no-follow directory descriptors and fixed filename patterns limit traversal to
the original jobs root and one directory level. It examines at most 4,096
entries and reads at most 4 MiB, including growth sentinels and bytes later
rejected by schema or identity checks. It retains at most one 256 KiB RPC export
with 256 rows and 256 setup files of at most 16 KiB and 64 rows each. These bounds
limit work and storage; filesystem calls remain subject to OS scheduling and I/O.

The timing ring retains only its latest 256 rows. An earlier fixture produced
377 operations: overwritten initial publication rows cannot explain cold cost
from the latest rows. Missing, dropped, overwritten or unfinished rows produce
explicit incomplete coverage; absent stage values remain unavailable. Setup
files can independently retain the runner's recorded publication phases. RPC
rows contain no job identifier, so attribution needs corroborating public
fixture evidence. Neither attempted owned shutdown nor this adapter certifies
the absence of all descendants.

The launcher requires actual immutable product bytes and their SHA-256, the
validated source engine directory, and matching pinned Verus source checkout:

```sh
python3 scripts/m7-publication-timing-diagnostic.py \
  --fixture ci/test-repair.py \
  --sail PRODUCT_ELF --sail-sha256 PRODUCT_SHA256 \
  --engine-build QUALIFIED_ENGINE_DIRECTORY \
  --verus-source PINNED_SOURCE_CHECKOUT \
  --out NEW_DIAGNOSTIC_DIRECTORY --environment-label ACTUAL_MACHINE_AND_LOAD
```

Product and fixture hashes are checked before and after the original run.
Optimized Python mode is refused to preserve the fixture's assertions. An
original fixture failure still propagates; complete retention does not convert
it into success. Even a successful run remains a separately labelled timing
diagnostic with no acceptance or performance credit.

The initial twelve pure controls passed without subprocesses. They cover exact retained
bytes and exclusion of private records, schema/identity/type refusal, links and
FIFOs, aggregate read and row bounds, overwritten or absent rings, cleanup
ordering, original-exception preservation, original run arguments, environment
restoration and pre-load digest refusal. Exact hashes and scope are in
[the launcher record](m7-publication-timing-launcher.json).

The current repository version also validates `output_guarded`, giving the
23-field row schema and thirteen pure controls. The
[current-product controller](m7-current-publication-controller.md) invokes this
version from the matching sealed product source, without the old host wrapper's
hardcoded launcher path/hash or obsolete schema. The earlier launch record and
failed execution scope remain unchanged.
