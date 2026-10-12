# Frozen current standard conformance

`scripts/local-standard-conformance.py` prepares two separate, fresh evidence
scopes. It first builds and seals `sail-conformance` from the exact qualified
product source. Execution then uses that immutable ELF, the same installed
product's `sail-z3`, and the stock pinned Verus/Z3 release. An interactive engine
inventory remains protected evidence; it does not replace the stock runtime.
The controller is local-only. At this source freeze, no native build or
conformance workload has run through it.

The caller supplies the complete combined gate output, its producer product
grant, and a stock runtime inventory. The producer grant must bind the same
source archive, complete gate/solver result and current container birth. All
helper bytes must occur unchanged in that archive. The controller itself is
staged outside the immutable extraction and its digest is retained separately.
The existing source verifier, engine seal, qualified runtime/source-eight/lock
checks and protected inventories run before and after each owned phase. A
bounded existing holder census covers the product, source, harness and compiler
target before actors start and after closure; this is scoped evidence, not a
certificate that every possible orphan is absent.

The build is the ordinary `cargo build --release --locked -p sail-conformance`,
using the archived `OwnedPhaseRunner`, two compiler jobs, no incremental
compilation and a 600-second outer ownership limit. It never adopts a prior
target executable after a failed build. The actual emitted ELF is copied into a
new private directory with mode `0700`, hashed before and after copying, retained
on the host and checked again at closure. The product binaries and original
installation records remain unchanged.

Execution uses the entire standard tier with no project/filter or diagnostic
logging options. It records one unit worker, as in the retained complete e0ca
scope; the ordinary `ci/run.sh` standard entry point uses three. Each arm retains
its original 3,600-second deadline, one initial plain run and up to two original
plain confirmation runs. A generous outer ownership bound is derived from all
159 units and their maximum four arms, plus 300 seconds; it does not shorten
these inner deadlines. Inherited Sail selectors are removed, then only the stock
home, corpus cache and original unit limit are supplied. The original runner
continues to force default solver mode.

The rounds are complete pass-through, fill, then warm. The latter two share one
fresh cache and socket owned by the existing `OwnedDaemons` helper. Its original
bounded readiness/shutdown remains, and the phase's existing process snapshots
also feed the same birth-checked daemon `Groups` primitive. Each nonzero phase
stops the sequence. Any timeout, signal or uncertain known-group closure is
sticky and prevents later phases. All 159 unique unit identities must agree
across completed rounds; the original comparison classifier is preserved,
including separately recorded Unstable results. Original cache population must
be nonzero. Native streams/workspaces remain in their original locations;
host retention includes phase logs, complete reports and the build ELF, not a
claim that all native raw evidence has already been exported.

Run the pure controls without a native actor:

```sh
python3 -B scripts/test-local-standard-conformance.py
```

These ten pure controls also run in the `ci/run.sh gate` workspace check. The
ordinary harness build and full native conformance rounds remain local-only;
the gate does not launch them.

After the root has closed other workloads and provided the exact paths, invoke
the same caller first with `--stage build`, then with `--stage run`. Both stages
require fresh host and native evidence roots:

```sh
python3 -B scripts/local-standard-conformance.py --stage build \
  --gate-output <complete-gate-output> --product-grant <producer-product-grant> \
  --container sail-m7-handoff-validation --stock <complete-stock-inventory> \
  --native-root <fresh-native-build-root> \
  --output <fresh-host-build-output> --execution '<actual environment/load>'

python3 -B scripts/local-standard-conformance.py --stage run \
  --gate-output <same-gate-output> --product-grant <same-producer-product-grant> \
  --build-output <closed-host-build-output> \
  --container sail-m7-handoff-validation --stock <same-stock-inventory> \
  --native-root <fresh-native-run-root> \
  --output <fresh-host-run-output> --execution '<actual environment/load>'
```

Native evidence roots must use the controller's fixed temporary namespace.
The stock inventory has the pinned stock installation `home` and `files` containing every direct
regular file in its `verus-x86-linux` release directory, with absolute native
`path`, byte length, integer mode and SHA-256. The existing verifier rejects
symlinks, omitted executables and changed direct membership. The default corpus
cache is the already pinned corpus cache; supplying another absolute cache does
not authorize importing proof receipts or accepting a different corpus pin.

The earlier complete standard failure remains 158 Same and one stock Verismo
timeout. Its cache rounds were unattempted. Neither the qualified product gate,
the old selected transcript diagnostic nor this file-only controller supplies
current complete standard conformance credit.
