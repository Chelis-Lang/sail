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
- `projects/<workspace-key>/<proof-context>/` holds proof receipts, run
  records and retained proofs (below).

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
records new receipts. A verified registry dependency such as vstd comes back
from the dependency store (below) instead of being verified again. A pruned receipt is never reused, and a job or MCP result
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

`dependency-store/v1/<key>/` holds verified registry dependencies, such as
vstd with its compiled dependencies, for new projects and worktrees to start
from ([ADR 0045](decisions/0045-dependency-store.md)). An entry for vstd takes
about 120 MB. Sail keeps the four most recently used entries and deletes older
ones itself; set `SAIL_DEPENDENCY_STORE_ENTRIES` to keep another number, or to
`0` to turn the store off. Deleting `dependency-store/` is safe at any time:
the next new project verifies vstd again and stores it. A run that was copying
from a deleted entry verifies vstd itself. Directories left by a run killed
while storing or deleting an entry are removed once they are an hour old.

## Retained proofs

`projects/<workspace-key>/<proof-context>/retained/` keeps earlier proofs of
the workspace's own targets, so that undoing an edit or checking out earlier
code restores them instead of verifying again
([ADR 0048](decisions/0048-retained-proofs-instant-revert.md)). Each proof
stores the files the compiler wrote for the target (its `.rmeta`, `.vir` and
dep-info) once per distinct content: about 20 KB for a small crate, about
30 MB for vest's `vest_lib`. Sail keeps the four most recently used proofs of
each target and deletes older ones, and their files, itself; set
`SAIL_RETAINED_PROOFS` to keep another number, or to `0` to turn retention
off. Deleting `retained/` is safe at any time: reverted content is then
verified again. `sail cache projects prune` removes it with the rest of the
workspace's directory.

## Development checkouts

Sail's own development uses one worktree per change under
`.claude/worktrees/`. Remove a worktree once its branch is merged, after
checking for uncommitted source. Build directories (`target/`) can be deleted
at any time. When removing Docker containers, keep their volumes and avoid a
global prune while other builds run. Raw logs stay outside the repository and
are cited by SHA-256 from the validation records ([M7 validation](validation/m7.md)).
