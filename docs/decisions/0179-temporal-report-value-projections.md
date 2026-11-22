# 0179. Read finite check reports with Dive’s native value renderer

The external temporal witness command accepts a fresh ordinary Dive check when a
saved word cannot establish a violation. The ordinary report exposes each finite
state both as typed native values and as a human-readable TLC JSON projection.
The same model atom can, for example, appear as a tagged value in the native
document and as a string in the displayed state. Functions can have structured
keys whose TLC spelling depends on the value printer.

Use the already-pinned `dive-trace` native decoder and `dive-value` TLC
renderer from `sail-dive` to compare these representations. The two crates
share the existing Dive version and revision. This keeps their conversion rules
with the implementation that writes the report. Periodic reports display native
tagged values directly, so their rows compare without rendering.
