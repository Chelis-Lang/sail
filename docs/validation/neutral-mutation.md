# Bounded neutral mutation qualification

## Current result

Positive native kill acceptance is unestablished. The original strict
installed-service run on combined Sail source `62b8a1848cbc7d6aa668b7957ae8f5720b3c30ba` produced genuine original
NotProved evidence for both strong boolean-contract literal mutants. Each
recorded 2,062 UNSAT checks, zero SAT checks and one UNKNOWN with reason
`(incomplete quantifiers)`. The enclosing Cargo process returned 101; the
requested verifier report returned 1 with `complete: false`. Unknown remains
inconclusive and the campaign passing flag remains false. These counts describe
that retained attempt, not a general prediction about future verifier runs.

An independent review found that the first classifier incorrectly required the
target success flag and direct process exit 1 to admit a conclusive rejection.
The correction requires explicit completed Verus verification totals and the
actual failed requested-target report, allowing the known Cargo wrapper failure
status. It preserves complete attribution, successful frontend admission, exact
tool identity and all original SAT/no-unknown conditions. A backend-receipt
attachment correction also binds assurance to the exact original baseline report.
These fixes do not convert the native unknown into a kill.

The default acceptance example still fails without actual strong-contract kills.
Its separate `--observe-inconclusive` mode can qualify only the actual bounded
negative path plus weak-contract survival, deadline, cancellation, restart,
pruning and original-stream tamper refusal. It writes an explicit
`native_kill_qualified: false` receipt when native kills remain unestablished.
The observation runner has been implemented and linted; native execution of
that complete control sequence remains required. The retained original attempt ran on native Linux
`7.1.5-100.fc43.x86_64`, x86-64 with glibc 2.42. It is supplemental local
correctness evidence, not the required EC2 acceptance. The installed Sail
binary SHA-256 was `251445b7dabb367a04ccec655f86a2aa1d58f630aaf5656096a2a3dc4f0fae02`.
The Sail Verus build manifest SHA-256 was
`35fdb4942460887104955f409a7dd1745207de2abb829d89061b0ab93ce0b2dc`.
The original source-job controls passed for positive, body-edit, compound-field
and guarded-field cases, including ordinary assurance, restart and pruning,
before the strict mutation attempt failed. Those controls do not establish
positive mutation kills.

## Scope and remaining work

The backend-free classifier controls use edited recorded inputs to isolate
report shape. They are regression tests, not new backend execution evidence.
The completed earlier local gate (`1b8f4148`) passed 1,025 tests with 44 skips;
that gate did not establish native mutation kills. The classifier correction
(`dd34c560`) passed its focused policy and receipt tests and strict workspace
lint; final integrated native gates remain separate.

A complete mutation milestone needs actual qualified rejection evidence. Future
work can qualify a genuinely conclusive backend or introduce an explicit
source-runtime witness evidence family with admitted inputs, checked
preconditions, exact unchanged guarantees and reproducible original execution.
A compiled body-edit witness alone does not turn an SMT unknown into a native
kill. New evidence must have its own versioned interpretation and trust checks;
missing evidence, generic NotProved and observed unknowns never pass the gate.

## Retained attempts

| Source | Date | Result | Cause or limit |
|---|---|---|---|
| `1b6def41` | 2026-10-23 | Local gate refused | Mutation read helper exceeded the line-count lint; original failure log retained |
| `1b8f4148` | 2026-10-23 | Local gate passed | Structural and regression checks only; no native kill acceptance |
| `62b8a184` | 2026-10-23 | Strict native kill gate failed | Both strong mutants retained original incomplete-quantifiers UNKNOWN; original receipt and source-service log indexed below |

The [policy guide](../neutral-mutation.md) defines the public gate and receipt
limits. Raw native logs and receipts remain outside the repository:

| Original artifact | SHA-256 |
|---|---|
| Strong campaign `strong.json` | `11065acaa93c110556ada70c4fdc736eb912c2b044c62e5d5e18c641420a1c87` |
| Source-service log `final-source-service.log` | `606a9245f6530fe580fa47273f2e37f84013d96e7fc8a5e982aa720fb526aee5` |
| Installed source and artifact inventory `identity.json` | `883ab851701c70d9c12a0eeffcfec8c65221f258b34ec82617dec1ab9e5b15f9` |

These artifacts qualify the original failed strict attempt. A later fixed-source
observation run needs its own source, artifact, host and receipt identities;
it must retain the original failure and keep positive-kill qualification false
while the solver outcome remains unknown.
