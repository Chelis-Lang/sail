# Native-artifact store fallback assessment

**Status:** isolated, unqualified proposal. Not selected or merged.

[ADR 0053](../decisions/0053-disable-unqualified-native-store.md) proposes
disabling v1 dependency-store reads and publication until native input identity
is qualified. This addresses the mechanism behind
[#187](https://github.com/Chelis-Lang/sail/issues/187) by making fresh targets
execute normal native compilation.

No test, comparator or benchmark baseline is changed. The existing unit test
expects the store enabled by default; the managed acceptance cases require
cross-project vstd reuse. Those requirements remain strict and are expected
to reject this candidate. It cannot authorize a merge in this state.

Native execution and measurements are pending. The next assessment must retain
the original successful donor, a matching fresh target, changed-linker runs
against the candidate and fresh stock, actual source/runtime/tool identities,
raw output and every timing sample. Historical v1 measurements cannot stand in
for the candidate's cost. No current-main or release correctness claim follows
from source inspection.
