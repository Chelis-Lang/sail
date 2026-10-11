# Current-product publication diagnostic controller

Status: prepared and tested with pure mocked controls only. No Docker,
compiler, verifier, daemon or acceptance actor ran during preparation. The
next execution follows the combined product's normal build and input seals;
this controller adds no separate approval step or qualification claim.

[The host controller](../../scripts/m7-current-publication-controller.py)
replaces the retired host wrapper's hardcoded `a627f3f` launcher. It invokes
[the repository timing launcher](../../scripts/m7-publication-timing-diagnostic.py)
from the actual sealed product source. That launcher owns the 23-field recorder
schema, including `output_guarded`; the host does not duplicate the schema or
validate against an obsolete 22-field list. The original fixture remains
byte-exact SHA-256
`9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7`.

## Exact execution recipe

Run the controller definition from the same product source revision as the
supplied installation record. Fill in actual reviewed identities, not a branch
name or an older binary. The controller can use either the original producer
record or the byte-identical product's separate execution installation record.

```sh
python3 scripts/m7-current-publication-controller.py \
  --product-grant PRODUCT_INSTALLATION_RECORD \
  --product-grant-sha256 INSTALLATION_RECORD_SHA256 \
  --runtime-manifest-sha256 SELECTED_ENGINE_MANIFEST_SHA256 \
  --verus-source PINNED_VERUS_SOURCE_CHECKOUT \
  --source-pin PINNED_VERUS_COMMIT \
  --out NEW_ABSOLUTE_OUTPUT_DIRECTORY
```

The record supplies exact product hashes, source archive, installed service
identity, engine directory, native container and environment label. Before
the fixture, the controller checks the unchanged producer archive and the
actual executable definitions, private installed binary bytes/ownership,
the selected manifest and all 13 runtime artifacts, fixture source commit and
container birth. The same executable, product, manifest and runtime checks run
afterward. Existing root protected checks remain responsible for the complete
source extraction, selected engine source/lock inventory and source-checkout
bytes; a matching Git HEAD alone is not a complete checkout-content seal.
Those protected checks must accompany the execution record. These installation
and byte checks are environmental prerequisites, never Current authority.

The native actor records its PID/start identity and exact argv before replacing
itself with the existing timing launcher. No engine build, manual preparation,
proof receipt, target or old fixture state is copied. Original engine install
and Cargo fetch remain part of the unchanged fixture setup.

## Bounds, failures and closure

The proof/tool scenario keeps its original 120-second clock, 30-second operation
cap and original actor cleanup. The host's separate 300-second transport
envelope includes initial engine installation, dependency fetching, that
unchanged scenario and cleanup. It does not increase any inner deadline.
Host setup/metadata phases have 15-, 30- or 60-second caps; raw archive transfer
has a 60-second cap. Each phase is attempted once. Metadata is decoded only
after a bounded 1 MiB read; complete stdout/stderr streams stay in files.
Filesystem I/O and writing the final evidence record remain OS operations,
not promises of hard scheduling deadlines or bounded total evidence disk use.

`controller.json` is checkpointed through execution and finalized on setup,
fixture, transport, post-seal, missing-report or archive failures. A fixture
failure retains its nonzero exit, logs, available native report and all later
seal/closure errors separately. The native original public reports and bounded
timing capture remain untouched; the host attempts a raw archive even after
failure and seals any partial transfer explicitly. Failure of storage can
still prevent a new checkpoint; it cannot certify success.

An attempted external timeout, transport error or interruption is `UNCERTAIN`:
terminating or losing a
Docker client does not prove the native actor stopped. The controller never
retries the fixture, restarts Docker, signals guessed descendants or asserts
universal native closure. The reused bounded post-cleanup census reports
actual candidate holders and incompleteness separately. It can examine up to
8,192 process entries for one second and at most 64 descriptor names per process;
its original OS-read limitations remain. An empty later scan cannot upgrade
an uncertain execution to success. Root must resolve any surviving owned
actor before another heavy workload. Original cleanup errors and missing,
dropped or overwritten timing observations stay visible.

An attempted run also remains `UNCERTAIN` when its native terminal record is
missing, malformed or does not bind the fixture and product, or when the bounded
closure observation is unavailable, incomplete or contains candidate holders.
A genuine nonzero fixture result with its matching failed terminal record and a
complete bounded empty census remains `FAIL`. The primary status, original
error and phase exit codes remain in the record alongside uncertainty reasons;
no failure becomes success. `native_closure_certified` remains false even for
an empty census.

Eleven pure controls cover one exact invocation and unchanged argv; genuine
fixture failure; timeout, transport errors and interruption with an apparently
empty later census; changed seals; missing, malformed, unbound or false native
reports; incomplete or nonempty closure observations; unconfirmed Docker exit;
setup-copy failure before invocation; and the repository's actual 23-field
schema. They run without subprocesses:

```sh
python3 scripts/test-m7-current-publication-controller.py
```

This prepared controller has no publication-performance evidence. The original
[failed timing scope](m7-publication-timing-plan.md), repair bounds and full
reader acceptance remain unchanged. See the [worker freshness review](m7-worker-freshness-publication-review.md)
for the unchanged terminal publication and Current-readiness contract.
