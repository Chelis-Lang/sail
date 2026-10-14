# Disk use and cleanup

## Managed compiler targets

Managed runs (`sail verify --incremental`, jobs, MCP and repairs) build in
their own Cargo targets under the Sail cache:

- `projects/<workspace-key>/compiler-artifacts/<family-key>/target/` holds the
  compiled dependencies and targets, shared by every run of one configuration,
  including every repaired function;
- `compiler-artifacts/<family-key>/repair-targets-v1/` is no longer used.
  Caches from before 2026-10-13 may still hold one target per repaired
  function there; deleting it is safe;
- `projects/<workspace-key>/<proof-context>/` holds proof receipts and run
  records.

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
records new receipts. A pruned receipt is never reused, and a job or MCP result
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

## Development checkouts

Sail's own development uses one worktree per change under
`.claude/worktrees/`. Remove a worktree once its branch is merged, after
checking for uncommitted source. Build directories (`target/`) can be deleted
at any time. When removing Docker containers, keep their volumes and avoid a
global prune while other builds run. Raw logs stay outside the repository and
are cited by SHA-256 from the validation records ([M7 validation](validation/m7.md)).
