# Gate-qualified local product construction

`scripts/local-product-install.py` builds and installs the exact source tree
that completed `scripts/local-combined-gates.py`. It refuses a failed, partial,
interrupted or uncertain gate/solver result. The host result, native result,
source archive and original phase logs must agree; the running container must
retain the gate's ID, image and birth. Source membership, bytes, file/directory
modes, symlinks, target binding, protected files and PID 1 birth are rechecked
before and after each product phase. No output is added to frozen source.

Use a gate that already includes the exact committed product helper and shared
lifecycle helper. Stage its source using the revision-based product source path
convention required by the existing benchmark grant validator. Then run:

```sh
python3 scripts/local-product-install.py \
  --gate-output "$complete_host_gate_output" --container "$container" \
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

Thirteen new pure controls exercise complete-gate refusal, build-before-install,
sticky uncertainty, actual common-grant validation, exact installed identity,
selected engine completeness and the shared phase runner using fake processes.
The existing 31 pure gate controls also pass on Python 3.12. Native build/install
execution is pending; these controls launch no Docker, compiler or proof actor.
