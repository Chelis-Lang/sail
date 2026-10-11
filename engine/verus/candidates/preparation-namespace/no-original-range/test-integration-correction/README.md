# Test integration correction

This file-only correction preserves the [first failed source scope](../first-source-failure.json).
It replaces new test references through the private engine module with the
existing verifier test-shim pattern. Three `cfg(test)` forwarding functions
expose only the required memo and direct/parallel controls. The engine module
and production interfaces remain private. Every test assertion is unchanged.
The new unchanged Pending helper moves before the existing test module to retain
the pinned strict item-layout policy; no lint suppression is added.

`identity.json` binds the full composed candidate, narrow correction and source
control launcher. Narrow application and full reverse application were checked
against the previously reviewed file-only source. The positive broker retirement
implementation and current-literal authority are unchanged. Corrected complete
source tests and both strict checks remain unexecuted; no runtime or qualification
is inferred from the earlier VIR result.

The controller requires the exact first failed result and source identity before
applying the correction in the closed private target. It records the new complete
source scope independently, keeps serial cases/internal concurrency and two
compiler jobs, and preserves all prior runtime/source seals. Runtime build and
original qualification remain separate required gates.
