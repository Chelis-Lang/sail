# Disk use and cleanup

## Managed compiler targets

Managed runs (`sail verify --incremental`, jobs, MCP and repairs) build in
their own Cargo targets under the Sail cache:

- `projects/<workspace-key>/compiler-artifacts/<family-key>/target/` holds the
  compiled dependencies and targets, shared by ordinary and full-engine runs
  of one configuration;
- `compiler-artifacts/<family-key>/repair-targets-v1/<proof-context>/` holds a
  separate target for each repaired function, so each selected function pays
  its own first build and can duplicate dependency artifacts;
- `projects/<workspace-key>/<proof-context>/` holds proof receipts and run
  records.

`sail cache stats|verify|gc|clear` manage only SMT reply entries. They do not
limit or remove these directories, and there is no quota or garbage collection
for them yet. To reclaim space, stop every `sail` process that uses the cache
(`sail serve` included) and delete `projects/<workspace-key>/`, or all of
`projects/`. The next managed run of that workspace is a cold run: it compiles
its dependencies again and re-establishes its proof receipts. Deleting a
target never makes an old result Current; it only removes reuse.

## Development checkouts

Sail's own development uses one worktree per change under
`.claude/worktrees/`. Remove a worktree once its branch is merged, after
checking for uncommitted source. Build directories (`target/`) can be deleted
at any time. When removing Docker containers, keep their volumes and avoid a
global prune while other builds run. Raw logs stay outside the repository and
are cited by SHA-256 from the validation records ([M7 validation](validation/m7.md)).
