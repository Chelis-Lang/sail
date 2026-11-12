# Original mutation evidence examples

These nine original source files are preserved byte for byte from
`spike/v030-mutev`, commit `75c86cb8a546146632d973ef51f7050aee5a6b77`. They are standalone historical inputs, outside
every conformance tier. The two `repro_*` files preserve an old generated
neutral-contract translation; preserving that input does not restore or ship
the retired frontend.

The original experiment reported three passing bases and six rejected edits.
Several rejected postconditions had `unknown (incomplete quantifiers)`, while
the bit-vector example had a native `sat`. A rejected Verus proof and a native
solver counterexample are distinct observations under
[ADR 0111](../../../docs/decisions/0111-mutation-evidence.md). These historical
observations are not new measurements on the selected runtime or platform.

Run against an explicitly chosen native install, retaining each process's
original stdout, stderr, exit, command, source hash and elapsed time:

```sh
python3 corpus/repros/verus-mutation-evidence/repro.py VERUS_INSTALL \
  corpus/repros/verus-mutation-evidence/*.rs --runs 3 --cap 120 \
  --out EVIDENCE_DIRECTORY --verus-arg=--output-json --verus-arg=--time
```

The output directory must be new. Each attempt owns a separate working
directory. A deadline kills the whole Verus/Z3 process group and records
`timed-out`; that is never a verification outcome. `finished` records only
process completion: inspect the original diagnostic and verification output.
Version output identifies the requested install; this helper does not qualify
its inventory, prove conformance or classify a campaign.

Version probes use their own bounded process groups and retain raw streams and
result records in `verus-version/` and `z3-version/`. SIGINT and SIGTERM clean up
the active group and finalize its interrupted record before exiting. The
command, source hash and starting/running state are written before waiting; a
nonterminal record after an uncatchable interruption remains incomplete.

Every return path closes the owned process group. Descendants that remain
after a normally exited parent are killed and recorded as an infrastructure
error, preserving the actual parent exit; this is not a finished experiment.
