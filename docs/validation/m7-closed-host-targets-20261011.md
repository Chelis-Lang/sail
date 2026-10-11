# Closed host build storage

Two inactive Sail worktree targets were losslessly archived after their owners
confirmed they had no current or planned use. Before and after archiving, open
file checks found no holders. Every archive member was checked against its
original type, mode, size, content hash or symlink target; a second original
inventory also matched before removal.

The removed targets contained 1,246,440,848 regular-file bytes. Their archives
contain 441,723,323 bytes, saving 804,717,525 logical bytes before inventory
files. This is not a physical disk reclamation measurement. Actual test
executables, raw logs and original metadata remain in the archives under
`_retired-evidence/m7-closed-host-targets-20261011/`. Their full hashes and
inventory hashes are in the [retirement record](m7-closed-host-targets-20261011.json).
To recover an old target, extract its named archive into a separate directory;
each archive contains a `target/` root.

The active Linux source, compiler target, installed products, selected source
engine, container lifecycle and other projects were untouched. This operation
changes no validation result.
