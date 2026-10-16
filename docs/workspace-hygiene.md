# Disk use and cleanup

## Managed compiler targets

Managed runs (every `sail verify`, jobs, MCP and repairs) build in
their own Cargo targets under the Sail cache:

- `projects/<workspace-key>/compiler-artifacts/<family-key>/target/` holds the
  compiled dependencies and targets, shared by every run of one configuration,
  including every repaired function;
- `compiler-artifacts/<family-key>/repair-targets-v1/` is no longer used.
  Caches from before 2026-10-13 may still hold one target per repaired
  function there; deleting it is safe;
- `projects/<workspace-key>/<proof-context>/` holds proof receipts and run
  records.

The proof context can also hold `unscheduled-scripts.json`, an optional
scheduling memo capped at 1 MiB. It avoids cleaning scripts a successful
Current Cargo build did not schedule under exactly the same inputs. Deleting
it is safe: the next run uses conservative script cleaning and can establish
it again. It contains no proof receipt. Source edits and unknown selection
closures retain conservative cleaning; see the
[validation record](validation/unscheduled-script-cleaning.md) for scope.

These directories grow with every workspace, copy and configuration you
verify: one copy of a mid-sized project can take 2 GB. Nothing removes them
automatically. `sail cache stats` shows their total on its last line, and
`sail cache projects` lists and removes them per workspace:

```sh
sail cache projects list                        # size, last run, path, (missing) when deleted
sail cache projects prune --missing             # workspaces whose directory is gone
sail cache projects prune --older-than 14       # not run for 14 days or more
sail cache projects prune --older-than 14 --missing   # both conditions
sail cache projects prune --all                 # everything
```

`prune` removes a workspace's targets, proof receipts and run records
together. `--dry-run` shows what it would remove, and `--json` prints a
machine-readable result for either command. The cache is `$SAIL_CACHE_DIR`,
else `~/.cache/sail`. A workspace's path is known once one of its runs has
finished; until then it is listed by key, and `--missing` never selects it.

Pruning costs only time. The next managed run of a pruned workspace is a cold
run: it compiles its dependencies, verifies them and the workspace again, and
records new receipts. Vstd is compiled and verified again while native-artifact
store reuse is disabled (below). A pruned receipt is never reused, and a job or MCP result
that was Current before the prune can no longer be confirmed unchanged, so it
reads back Unknown ("inputs could not be confirmed unchanged; re-run") until
you verify again.

Pruning never waits and never removes a workspace that a managed run, job or
repair is using: it reports `skipped: in use` for it, and you can prune again
when it finishes. A run that starts while a prune is removing its workspace
waits for the prune and then starts cold. Runs of Sail versions without
`sail cache projects` are detected by their target lock only; stop older
`sail serve` daemons before pruning.

`sail cache stats|verify|gc|clear` manage only SMT reply entries and never
touch these directories.

## Dependency store

Older builds populated `dependency-store/v1/<key>/` with compiled registry
dependencies such as vstd ([ADR 0045](decisions/0045-dependency-store.md)); a vstd
entry takes about 120 MB. V1 consumption and publication are now disabled
([ADR 0053](decisions/0053-disable-unqualified-native-store.md)): its identity
could miss native compiler inputs and hide a fresh compilation failure.
`SAIL_DEPENDENCY_STORE_ENTRIES`, including a positive value, cannot re-enable it.
Existing entries stay intact and can be deleted safely. Fresh projects and
pruned targets compile and verify normally, then keep their own proof receipts.
Exact solver caching remains available. The
[assessment](validation/dependency-store-fail-closed.md) records the safety
reproduction, measured first-run cost and remaining performance work. A sound,
fast replacement is still required.

## Snapshot hints

`snapshot-hints/<key>.json` records, for each directory and argument list
you run `sail verify` with, which package directories and extra inputs its
last input snapshot read (a few kilobytes each). The next run reads those
inputs while Cargo prints its metadata, and uses that snapshot only if the
new metadata selects exactly the same inputs; otherwise it reads them again.
A hint never decides what is verified or reused. Deleting `snapshot-hints/`
is safe at any time: the next run of each invocation only takes a little
longer.

## Development checkouts

Native-host source sync compares file contents and gives changed files current
timestamps. This keeps reused Cargo volumes from treating an older checkout
as already compiled. Build artifacts stay on the native host.

Sail's own development uses one worktree per change under
`.claude/worktrees/`. Remove a worktree once its branch is merged, after
checking for uncommitted source. Build directories (`target/`) can be deleted
at any time. When removing Docker containers, keep their volumes and avoid a
global prune while other builds run. Raw logs stay outside the repository and
are cited by SHA-256 from the validation records ([M7 validation](validation/m7.md)).
