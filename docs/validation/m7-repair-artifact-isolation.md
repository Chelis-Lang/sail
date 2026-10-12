# Repair artifact isolation: source candidate

The original `387ba80` benchmark failed before its first measured pair: M7's
GCD full result was Current and accepted, but reported three compiler drivers.
All three HumanEval repair selectors had completed their original warmups;
Vest and measured speedup remain unqualified. The failure and its original
one-driver oracle are unchanged.

The retained receipt archive has SHA-256
`45893285ea65fe5a53e780b55b31c482d9477942bd13e3848086f3ba554bcc2f`.
All 80 regular members were independently checked against their size/SHA manifest.
GCD, concatenate and strlen's committed vstd receipts have the same input revision
`09ae2e5dab9fac9c7a0f159979c0ed9fe4adf0f44dc214375af84deb623acb02`
and the same `.rmeta`/`.vir` path spellings. Their artifact hashes differ:

| Ready selector run | vstd `.rmeta` SHA prefix | vstd `.vir` SHA prefix |
| --- | --- | --- |
| GCD `23007-1791797756372830500` | `cf4bcab7` | `49112178` |
| concatenate `30587-1791798088455004501` | `ebc59d4b` | `1314cc5e` |
| strlen `37654-1791798704245040469` | `e2aa69f1` | `101ed9e1` |

Thus later selectors overwrite the byte generation certified by GCD's receipt.
Unchanged dependency inputs cannot authorize different artifacts. The final
measured GCD run also produces different hashes. The preserved journals replace
their earlier invocation at retry, so this evidence does not attribute every
phase or establish a sole refusal reason for each first invocation.

## Narrow change

`managed::runner::prepare` isolates the entire Cargo target and ordinary generation
export when the actual engine request has `function=Some`. Both live beneath
`compiler-artifacts/<family>/repair-targets-v1/<full-proof-context>/`. This uses
the unchanged complete proof context; it does not normalize selector, command,
environment, runtime or solver identities. Default and full-engine `function=None`
calls keep their original shared target/export. Executable aliases and the
context-then-family lock remain unchanged.

There is no artifact restoration or receipt import. Every existing input,
runtime, native, committed-receipt, artifact and invocation check remains.
Missing receipts retain fresh delegated verification and the original retry.
Optional ordinary script/source exports are private to the new target generation,
so another selector's output contract cannot certify this target's bytes.
The old shared target remains untouched; the first run in the new namespace is
cold. Each selector requires additional disk space and dependency setup. No
claim is made that this alone meets the original 120-second fixture or 2x benchmark.

## Draft controls and sensitivity

Two new Linux controls are formatted but uncompiled and unexecuted:

- Actual preparation paths preserve default/full sharing, isolate two selectors
  and their exports, retain aliases and family contention, and release ownership
  after a setup error.
- A real offline Cargo fixture uses the actual compiler with a transparent
  recording wrapper. A dependency embeds a selector-specific generated string,
  producing different real artifact bytes during A/B warmups. Returning to A
  and forcing only its requested package must record one actual requested Rust
  compiler call, keep the dependency fresh, preserve its bytes, and pass the
  existing exact local receipt validator. Restored-mtime artifact/input writes,
  another proof context and a missing commit still refuse receipt reuse.

The receipt reports in this ordinary Cargo fixture are synthetic storage controls,
not newly verified Verus evidence. The wrapper records real compiler calls but
does not establish the production benchmark's one-Verus-driver result. The fixture
reuses the existing GNU-timeout supervisor and bounded captures; it makes no
general orphan or uninterruptible-kernel guarantee.

The immutable [counterfactual patch](m7-repair-artifact-isolation/shared-target-counterfactual.patch)
removes only generation isolation. The same A/B/A byte-stability assertion must
fail when B overwrites A's artifact. The later one-compiler-call assertion is
unchanged. Required native work includes both controls, existing family/receipt
tests and strict checks, then the complete gate/solver, original repair/reader
and unchanged benchmark. No native actors or test execution occurred in this
source-only scope.

Separately, the sealed M6 focused baseline emitted a fresh artifact under
`target/verus-partial` after its managed clean targeted the parent Cargo directory.
Pinned Cargo-Verus appends that subdirectory for focus mode. The original M6
measured focus arm returned `SAIL3107` with zero verifier calls; its independent
full arm was Current. This baseline limitation is preserved and is not repaired
or reinterpreted by the M7 layout change.
