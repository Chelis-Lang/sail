# Existing-product execution rebind after restart

[The standalone helper](../../scripts/local-product-rebind.py) revalidates an
already installed product after a container restart. It performs no compilation,
installation, verification, cache preparation or receipt transfer. Historical
qualified gate/build/install results remain historical results, rather than new
qualification of the recovered environment.

The root coordinator supplies the exact old execution-grant digest and a fresh
dual-container identity file. That file maps `sail-m7-acceptance` and
`sail-m7-handoff-validation` to `Id`, `Image`, `Created`, `birth`, and `Mounts`.
`Mounts` includes all original Docker mount fields; only array order is ignored.
This supports a restart of the same container IDs and images, not replacement
with a different image. The helper requires a fresh independent recovery output.

```sh
python3 scripts/local-product-rebind.py \
  --revision c2a4b23 \
  --old-grant-sha256 OLD_EXECUTION_GRANT_SHA256 \
  --expected-containers CURRENT_DUAL_CONTAINER_IDENTITIES.json \
  --output /tmp/sail-local-product-rebind-c2a4b23-RECOVERY_ID \
  --commit
```

The command is an operator interface; the machine paths above are fixed runtime
namespaces, not portable evidence locations. Without `--commit`, it records the
revalidated candidate without changing the current pointer. No execution of this
helper has been claimed by its source-only controls.

Before any pointer change, the helper preserves the exact producer and old
execution-grant bytes in the recovery output. It rehashes the producer Git archive
and compares its entire inventory against the historical installation inventory.
Both native source trees must match every tracked byte, mode and link. The
producer's original external `target` symlink is separately required to point
exactly to the common target; it is not admitted as an additional tracked file.

The original benchmark source verifier and `native_seals` check the runtime's
13 artifacts, eight modified Verus sources, lock, original wrapper/support inputs,
M6 binaries, pinned corpus commits and product bytes. The selected runtime copy's
13 files and manifest are independently checked too. Product reads use the
original no-follow descriptor verifier, actual filesystem UID, private directory
ownership and original file modes. Whole-source file ownership is checked.

The selected inventory helper's read-only `holders` function and the shared
all-thread zombie guard run before and after the checks. Their scope is the exact
product/source/common-target aliases and protected inode bindings in each
container. Self exclusion requires the auditor's exact PID and kernel birth;
live siblings, changing/unavailable identities and incomplete scans refuse.
This is not a general orphan certificate. No inventory or deletion function is
invoked. Full native checks run twice in both containers, with matching file
bindings, PID1 birth, mountinfo, credentials and Docker mount/birth identities.
Docker inspection selects only those fields, never container environment values.

Only after all checks pass may the helper atomically replace the designated
`product-grant-acceptance.json` pointer. Its original build/install phase records,
product identity and producer provenance are retained verbatim. The new binding
adds the historical execution digest and revalidation evidence digest and states
that no build, installation or proof was performed. `product-grant.json` and the
historical acceptance-install evidence are never overwritten. Pointer changes
detected immediately before replacement refuse. Root-exclusive pointer ownership
is required; this is not a filesystem compare-and-swap against hostile writers.

Existing bounded transport/closure handling is reused. Each full read-only native
evaluation has a 600-second outer transport cap; it does not alter a proof or
acceptance deadline. Any failed/interrupted transport stops the sequence. An
uncertain native closure cannot be discharged by reaping the local Docker client.
No retry, new install or automatic execution follows a refusal.

Eight [host-only controls](../../scripts/test-local-product-rebind.py) cover
unchanged historical phases, producer mismatch, pointer commit/refusal,
holder uncertainty, birth/mount/byte changes, bounded metadata, duplicate JSON
and the original whole-source byte/mode/extra-member verifier. Native recovery
checks and subsequent unchanged acceptance workflows remain separate outcomes.
