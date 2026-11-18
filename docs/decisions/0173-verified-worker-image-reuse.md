# 0173: Reuse the verified temporal worker copy under settled stamps

- **Status:** accepted (lead decision under the 2026-11-12 delegation)
- **Date:** 2026-11-18

## Context

Every native temporal job hashed the installed `sail-runtime-worker` (76 MB)
against its installation manifest and wrote a private copy to execute, and
the runtime's capability check hashed it once more. On c7i.8xlarge the copy
alone took 101–105 ms per job, so a 14-state check cost about 120 ms embedded
and 205 ms through `sail runtime execute` against 5 ms in the dive CLI
([#765](https://github.com/Chelis-Lang/sail/issues/765)). ADR 0082 already
trusts a file across runs by its complete stat stamp once that stamp is
settled, and the daemon's solver identity memo relies on it.

## Decision

Keep one verified copy per installed worker (keyed by its path and expected
SHA-256) in a per-user private directory, `sail-runtime-images-<uid>` under
the temporary directory, and reuse it for later jobs in any process:

- The installed worker's complete stamp is recorded only when it was settled
  before the bytes were read and unchanged after they were hashed. A job
  reuses the copy only while the installed worker still has exactly that
  stamp, so any write, replacement, `chmod` or `utimes` verifies the bytes
  again.
- The copy is mode `0500`, owned by the user, with one link, in a mode `0700`
  directory owned by the user; its own complete stamp must equal the one
  recorded after it was written. A directory with another owner, a symbolic
  link or wider permissions is never used.
- A record found at the current stamp also authenticates the worker for the
  capability check, which otherwise hashes it as before.
- Any miss (unsettled, foreign, changed or unwritable) falls back to a fresh
  verified per-job copy, exactly the earlier behaviour. If a reused copy
  disappears between lookup and spawn, the job verifies a fresh copy.
- Each job keeps its own private working and temporary directory; only the
  executable is shared.

## Consequences

The verification and copy cost is paid once per installed worker instead of
once per job. Integrity is unchanged against the threat the private copy
addresses: a changed installed worker is detected by its stamp and its bytes
are hashed again before anything runs. A process running as the same user
could already replace `sail` itself, so it is not a new attacker. Up to one
76 MB copy per installed worker stays in the temporary directory; entries
whose installed worker changed are removed when another entry is published.
