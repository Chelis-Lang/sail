# Closed compiler and source archive retirement

The closed private component target released 1,011,568,640 allocated file bytes
by removing 1,367 non-executable compiler files: `.rlib`, `.rmeta`, build scratch
and incremental state. All 3,583 preserved target files retained their exact
SHA, size and mode. Executables, shared libraries and fingerprints remain.
All 204 previously protected records, current source/lock, raw control logs and
the actual tested image `971a04cb…` also match their seals.

Fresh pre/post cmd/cwd/FD/maps checks in both containers found no holders or
unreadable live processes for the private paths. The complete file inventory
and removal result have verified host copies; their identities and category
counts are in the [retirement record](m7-negative-generation-compiler-retirement.json).
The active common target, selected runtime, product packages and proof evidence
were untouched. No compilation, proof or Docker lifecycle action ran here.

Separately, 20 closed host source tar copies released 245,997,568 allocated
bytes. Each archive's embedded commit matched a tracked controller or validation
record, retained Git references, and a freshly streamed `git archive` with the
exact same SHA and byte count. An immediate exact-path host holder check was
empty. Five uncertain copies were skipped; the active gate archive remains.
The [archive record](m7-old-source-archive-retirement.json) lists every removed
and skipped copy, its source identity and reproducible hash. Controllers, logs,
raw records and tested images were preserved.

Combined removed file allocation is 1,257,566,208 bytes. Host backing space
observed across the private retirement increased from 4,057,366,528 to
5,165,006,848 bytes during concurrent activity. This observed change is not attributed solely
to the cleanup or equated with VM allocation recovery.
