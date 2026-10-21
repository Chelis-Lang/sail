# Neutral contract runtime qualification

The runtime and shared syntax implementation at `315d9251` passed the native
Linux CI image on 2026-10-21, using the dedicated `oyster-m10-contracts` host.
The qualification covers the real attribute macros, release-enabled checks,
shared source admission and unchanged existing verifier routes. It does not
establish an SMT proof of a neutral contract or qualify the separate Verus
shadow lowerer.

The complete native commands passed:

```sh
ci/local.sh gate
ci/local.sh solver
ci/local.sh e2e
ci/local.sh conformance fast pr
```

The solver lane passed every selected real-solver test. The full E2E lane
included managed build-script reuse, real CLI/MCP acceptance, engine provenance
and regression controls. The PR conformance subset found no differing verdicts,
unstable units or infrastructure errors. Existing allowances for resource-limit
drift remain unchanged. No default verifier or performance behavior was changed.

Raw native log SHA-256:

| Log | SHA-256 |
| --- | --- |
| `sail-m10-contracts-gate.log` | `063bdc7f29de5caf2d8c2045409de5c3f855e5baae2ebc3a21566a73c9cdfd16` |
| `sail-m10-contracts-solver.log` | `eac13cefdf64eb3b17d8cd59dde1115125a217cc523bfe12af030bcd878c79fa` |
| `sail-m10-contracts-e2e.log` | `fff78941d453f311dca34922f5eb36aba82268d84cc67fdf0b74242a78cec16e` |
| `sail-m10-contracts-conformance.log` | `df173bb2a8ae402c16e3631e3f5a1a9bef9e6d51fee3544ef1ae1e1cbc45efbf` |

The source also passed local macOS formatting, strict all-target linting,
runtime/shared-IR tests and doctests in debug and release builds. The
[specification](../spec/neutral-contracts.md) and
[ADR 0066](../decisions/0066-neutral-contract-runtime.md) describe the admitted
subset and the intentional contract-failure checks. Source-level purity alone
does not establish typed proof admission for arbitrary Rust field access.

After merging current main `4636ff20`, the complete native workspace gate passed
again. The runtime implementation is unchanged from the initial checkpoint;
the merge retains the separate Cargo benchmark feature and its gate checks.
The merged-source gate log `sail-m10-contracts-main-gate.log` has SHA-256
`25d8238754ac3d62894e2631b561478bb0c19b19a9752691681109df15010515`.
Documentation links were checked after adding this record.

A later current-main merge includes watch/store race correction `35b09255`.
The complete native workspace gate passed again on that merged source, with
all assertions unchanged. Its `sail-m10-contracts-watch-main-gate.log` SHA-256 is
`a4f26a41a24a8c4654ef8ecd844bb49232990ebf317ac2d4816cf1ea6cc74abf`.

The next main merge includes native pipelining correction `dee05795`.
The complete native workspace gate passed again with the runtime unchanged.
Its `sail-m10-contracts-pipelining-main-gate.log` SHA-256 is
`20f673da9e61f5fe7842c8dfefbf7ecb826797d880d7115548e7b99866a80702`.

Main `90de38a7` changes benchmark scripts and documentation only; the Rust and
Cargo inputs remain byte-identical to the preceding native gate checkpoint.
The changed native benchmark regression suite passed, shell syntax validation
passed, and documentation links passed. The native
`sail-m10-contracts-benchmark-main-tests.log` SHA-256 is
`06374e6ff95480759d8c6077672afc0640dbc2e422a3574a1d7588e5c6cc2caa`.
