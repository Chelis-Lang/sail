# Gate-qualified local product construction

`scripts/local-product-install.py` builds and installs the exact source tree
that completed `scripts/local-combined-gates.py`. It refuses a failed, partial,
interrupted or uncertain gate/solver result. The host result, native result,
source archive and original phase logs must agree; the running container must
retain the gate's ID, image and birth. Source membership, bytes, file/directory
modes, symlinks, target binding, protected files and PID 1 birth are rechecked
before and after each product phase. No output is added to frozen source.

Use a gate that already includes the exact committed product, retirement and
shared lifecycle helpers. Stage its source using the revision-based product
source path convention required by the existing benchmark grant validator.
The source stays present after the completed gate's scratch evidence is retired.
Then run:

```sh
python3 scripts/local-product-install.py \
  --gate-output "$complete_host_gate_output" \
  --retained-gate "$verified_completed_run_backing" --container "$container" \
  --native-root "$fresh_native_product_output" \
  --output "$fresh_host_product_output" --engine "$selected_engine_inventory" \
  --execution "$honest_environment_and_load_label"
```

The engine JSON contains `engine_build`, `manifest_sha256` and `files`. The last
field maps all 13 manifest-relative runtime filenames to exact `bytes`, numeric
permission `mode` and `sha256`. The controller checks the selected source
artifact pin, runtime variant, patch/lock features, exact `sail-build.json`,
all runtime files and their direct directory membership. The product output root
uses the helper's `sail-local-product-` temporary-directory prefix. The installed
product uses the existing revision-based core-product directory convention.
Neither directory may replace an earlier scope.

The gate's automatic retirement must have completed with `PASS`, original status
`PASS` and `removed=true`. Its durable controller must equal the retirement
record in the original host gate controller. The installer verifies the full
compact gzip archive against its member inventory, including every retained
byte and mode; it also verifies the separately retained original source archive,
source inventory, protected inventory and terminal record. The original raw gate
and solver logs keep their exact hashes. Missing, corrupt, partial or changed
records refuse installation.

Only these records are restored into a new product-owned
`qualified-gate-evidence` directory. `restaging.json` records the original gate
root and backing hashes with `re_execution=false`. The product config points its
`gate_root` at that directory, while the original gate config and external source
path stay unchanged. Native product checks verify the restored records, regular
file types and modes before and after each phase, then perform the same original
source, target, protected-file and PID 1 checks. Restaging neither extracts nor
changes source and does not rerun or reinterpret a qualified gate.

The corrected helper must itself pass a new complete gate before native use.
The historical `1fbaa57` run remains source-control evidence: its differently
named external source cannot satisfy the original product source-path prerequisite.
No new config, helper exemption or installation grant is derived from that run.

The first phase is the ordinary command
`cargo build --release --locked -p sail-cli -p sail-shim`, with two Cargo jobs
and the original 600-second compiler cap. Existing ordinary Cargo intermediates
may be reused, but their presence cannot replace a successful command. The
second phase invokes the existing `scripts/m7-native-product.py install`, using
the newly observed exact ELF hashes and sizes, with a 60-second outer install
cap. It creates private owned copies and retains the actual credential and
0700 file/directory evidence. Failure stops the sequence; there are no retries.

Both gate and product controllers use the same `OwnedPhaseRunner`, existing
birth-checked group observation, bounded host transport and sticky cleanup
uncertainty. A Z/X leader requires stable, complete, bounded all-thread termination
evidence before its recorded group may close; leader state alone is insufficient.
Terminated rows remain retained separately; live, unreadable or incomplete group observations refuse.
Source/identity uncertainty prevents continuation and requires an
independent owner audit; recorded group closure does not prove arbitrary orphan
absence. Primary outcomes precede final closure checks. Complete raw logs,
partial failures and original gate records remain retained. On a returned
terminal result, logs are copied and rehashed on the host. A complete pass also
retains `product-grant.json` and the exact original `source.tar` together.

The grant is compatible with the existing benchmark `common.validate_grant`
contract: release-build and owned-install must both pass; source commit,
archive and path, container, its actual creation/start identity, an explicit
execution environment/load label, installed directory, two artifacts and ownership
are explicit. `installation_only` and `source_modified=false` describe delivery
only. Installation is not proof Currentness, repair acceptance, source-engine
qualification or performance evidence. Original product fixtures and benchmarks
remain separate required scopes.

Nineteen pure controls exercise complete-gate refusal, build-before-install,
sticky uncertainty, actual common-grant validation, exact installed identity,
selected engine completeness and the shared phase runner using fake processes.
Six restoration controls additionally use real temporary tar archives and member
inventories to check exact restoration, immutable originals, corrupt or missing
members, partial retirement, stale identity, source-path refusal and later raw
replacement. These controls launch no Docker, compiler or proof actor. Native
execution of the corrected product helper is pending.
