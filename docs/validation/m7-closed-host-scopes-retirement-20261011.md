# Three closed host scopes archived

The authorized closed output-service `169d6d7` control copy and handoff
`1e8b633`/`7da9579` copies are preserved in private gzip archives under
`sail-m7-host-closed-retirement-20261011`. Only their uncompressed host
duplicates were removed. Every tested image and raw record survives; small
logs and metadata also remain directly readable in `retained-metadata`.

[The exact record](m7-closed-host-scopes-retirement-20261011.json) binds all
three archives and source inventories. Every member's digest, size, mode,
ownership and entry type was verified. Sources were unchanged immediately
before removal, with no outside hardlinks. Host FD inspection and both Sail
containers' command/cwd/FD scans found zero holders and unavailable entries.
All-container mount and birth identities remained unchanged, with no host
scratch alias mounted into a container. A preliminary comparison stopped
before deletion because Docker reordered its mount list; a full field
comparison keyed by destination confirmed no mount change.

The removed duplicates occupied 508,956,672 allocated bytes. Their archives
occupy 139,853,824 bytes, recovering 369,102,848 bytes before small metadata.
Private archive directories use 0700 and archives 0600. Current gate/product
paths, native targets, active work and foreign projects were untouched. This
storage maintenance supplies no validation or acceptance credit.
