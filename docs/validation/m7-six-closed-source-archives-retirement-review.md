# Independent review of six closed source archives

The [review record](m7-six-closed-source-archives-retirement-review.json) verifies
the compact retirement mapping from commit `ff85036`. Its raw result and
controller hashes match. All six compressed file lengths/hashes match the
mapping, and independently streaming each complete gzip recovers the exact
original tar length and SHA. All six retired original host paths are absent.
Retained original holder logs are empty and their result fields agree; this
review does not retroactively perform that census.

The controller uses a fixed six-file allowlist. It checks each original's
nofollow opened/named identity and full SHA, verifies complete gzip readback
before unlink, performs the recorded bounded immediate holder check, then
verifies compressed and recovered bytes again after retirement. The original
tar streams preserve their contained file bytes and metadata exactly.

Original logical bytes total 138,526,720; compressed bytes total 33,492,789,
a 105,033,931-byte representation reduction. Recorded allocated file bytes
change from 138,530,816 to 33,509,376. These are file accounting figures, not a
claim of host or Docker physical reclamation. Current c2/730 source/product
archives, proof/benchmark archives, native images and raw logs are excluded.
No new deletion, native actor or validation authority follows from this review.
