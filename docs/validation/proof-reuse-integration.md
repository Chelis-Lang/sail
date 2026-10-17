# Compiler-reuse research integration

Status: source integration of `4488f2df` (#161) and `446385e9` (#163), with
local macOS checks only. This integrated implementation is not native-qualified,
selected, or a runtime upgrade. Existing pins and default verification remain
unchanged. Component results retain their original source/runtime scope; see
[vstd boundary reuse](vstd-compile-boundary.md) and
[retained restoration](instant-revert.md).

## Switches and authority

Both experiments remain disabled by default. Only the literal value `1` enables
a switch:

| vstd boundary switch | retained-content switch | Behavior |
|---|---|---|
| Unset | Unset | Ordinary verification |
| `SAIL_VSTD_BOUNDARY_RESEARCH=1` | Unset | Existing ADR 0048 path |
| Unset | `SAIL_RETAINED_CONTENT_RESEARCH=1` | Existing ADR 0050 path |
| `1` | `1` | `SAIL3135` before runner setup or adapter child execution |

The combination is deliberately unsupported. Neither path can configure the
other's compiler request, publish its certificates, or lend it source authority.
The outer runner and compiler adapter independently reject the combination.
Native compilation still executes normally; observations, checked private
inputs, complete proof documents, original establishing runs and ordinary
Current publication gates remain mandatory for the individual experiments.

One owned dependency-graph implementation serves both paths. It retains the
bounded complete inventories, private copies, exact arguments and intactness
checks, plus retained storage accounting and idempotent publication at the
already-published exact content address. It does not union their certificates
or caches. The macOS native install-name option and its key binding are
unchanged from `4488f2df`.

## Separate source policies

The literal policies remain separate and unchanged:

| Policy | Authority and current limit |
|---|---|
| `engine/verus/vstd-boundary-inputs.json` | 15 published package archives, 662 files and their exact feature sets. SHA-256 `abf802d7deb7c0110b0aae0875e15a7b1ace82d0d02f2acaf8f26d1ba8a73049` |
| `engine/verus/retained-vest-inputs.json` | 15 provider packages, 718 files and exact feature sets; explicit final-fork provenance. SHA-256 `71c9bde23eb3af6d952fc58723f3ddc1e830888ff565119256258aa09ab15dd7` |

All 400 official provider files in the retained policy match the final
`e5365c05` Git source. The boundary policy describes published package bytes:
307 corresponding official files match that Git tree, 15 differ (six Cargo
manifests and nine Rust sources), and 22 package-only paths have no Git-tree
counterpart. These are different source views, not interchangeable policies.
No policy is rebound, relaxed, or inferred from the other. Registry archive
identities and all actual compiler/native-source checks remain independently
required. A later integrated native qualification must bind the actual source
view used by each experiment and the actual runtime; this source composition
does not qualify reuse with a new source view or verifier.

## Validation

Owned subprocess controls cover OFF, each individual switch, exact-value
parsing, and refusal at both actual entry points when both switches are set.
The shared snapshot and each experiment's source modules retain their component
checks. The complete macOS `ci/run.sh host` passes all-target workspace clippy
and 731 tests, with 35 solver tests skipped by the host profile. It uses the
existing shared workspace target with two build jobs; no verifier is rebuilt.
This checks both experiments' unit/filesystem controls and the ordinary OFF
path, including the new owned-child switch and entry-point controls.
Native ABI, source-mutation, same-workspace ABA, cross-project reuse, literal
queries, performance and merge gates have not been rerun for this integration.

Mac host stdout SHA-256: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`; stderr: `21fcc9ce95d0b68ac33c67510ce2937c00308fe4131210c4c1bc265417d1c84d`.
Component identity inventory SHA-256: `9ba0a89b862cd8564cda5d7a5cba8515fc42933e5e831ed4f2c51afaa7a1320a`; source-policy comparison: `1093f662f0d3fbbc3a454cf2f8b63cdc1949864edf698733725d97f4750509cb`.

| Earlier attempt | Result and limit |
|---|---|
| Initial integration draft, 2026-10-17 | Mac host stopped in clippy on three redundant method closures; no tests ran. Corrected locally before the complete passing host gate. Stderr SHA-256 `68a39f00d58ec0b7bd1fd7e93a889e8451c11168863eb19fa2e5333710e5ed01`. |
