# 0075: Preserve floating-point identity in retained JSON evidence

## Context

An actual Oyster production campaign retained a worker timing value of
`28712.190580000002`. Sail's default JSON parser decoded it one representable
`f64` below the value decoded by Oyster, which enabled `float_roundtrip`.
The service and consumer consequently derived different backend hashes from
the same original worker stream. Offline mutation validation correctly refused
the inconsistent receipt with `SAIL3144`. Both classifications remained
inconclusive, and neither mutant established a passing gate.

## Decision

Enable `serde_json/float_roundtrip` in Sail's workspace dependency. The API
inherits the feature, so consumers receive the same decoding behavior without
having to opt in independently. Retain the original worker streams and compare
the full typed backend digest exactly. This changes no solver query, answer,
proof classification, mutation classification or receipt schema.

Do not remove timings from existing hashes, round their values, weaken receipt
checks or rewrite retained evidence. A receipt derived using the older lossy
value remains invalid when its backend digest differs from the exact decoded
original. Preserve it and request a fresh campaign from a compatible producer.

## Validation

The retained-view regression exercises the observed adjacent floating-point
values in baseline and mutant originals, repeated whole-receipt round trips,
and rejection of the old lossy digest. It also requires the mutation to remain
inconclusive with a false passing gate. These are deterministic receipt controls,
not new verifier execution or a successful native mutation kill. Platform and
actual service qualification are recorded separately.
