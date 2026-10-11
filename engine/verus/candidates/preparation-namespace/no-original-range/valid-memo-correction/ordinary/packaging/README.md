# Local runtime package and fresh-install plan

This is prepared work, conditional on all thirteen original stages passing and
the independent post-qualification seals closing. It does not publish an asset,
select the runtime, or rebuild source/proofs. The retained private ordinary
repository snapshot already has the exact `aaa3b2e4` interactive patch and
`653e5c0c` source lock. Its existing `ci/engine-artifact.py` is byte-identical to
the repository tool (`590749607ca02a2005cb54c5c30e2f6bd998b362d658439dae8abf1abbf8e92b`).

After whole qualification closes, use that script's existing `pack` command on
the sealed `c77e1a1879302e06` variant. Write a new local archive and pin into an
empty owned evidence directory. The packer validates complete runtime provenance,
uses canonical member ordering/ownership/timestamps and includes the manifest
plus exactly thirteen direct runtime artifacts. Record archive size/digest, pin
bytes/digest, all fourteen member names/modes/sizes/digests, source/lock/manifest
identities, elapsed time, backing space and actor ownership. No asset ID is
invented or old release pin reused.

Use the same unchanged script's `install` command with that local archive/pin and
a new empty private cache. Its existing archive-digest, source-pin, safe-path,
256-member and 2 GiB expanded-size checks remain in force. Rehash the installed
manifest and complete thirteen-file runtime inventory against the original seal,
including the pinned solver. Preserve executable modes and reject symlinks or
extra direct files. Recheck the source candidate and all older runtime seals after
packaging/install; no accepted preparation, proof receipt or source target is
copied into the installed runtime.

The fresh installation establishes artifact delivery and identity, not product
compatibility or performance. The source-dependent product must subsequently be
rebuilt and gated against this exact patch, lock and archive pin. Updating the
committed release asset, CI pin or selected runtime remains separate from this
local package evidence and requires the lead's coherent integration sequence.
