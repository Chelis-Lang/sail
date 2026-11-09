#!/usr/bin/env python3
"""Spike only: make 0.2026.09.06.8dea4a2 the active pin and point the fork pin
at a lane-local rebased commit. Usage: repin.py REPO FORK_REPO FORK_COMMIT"""
import pathlib
import sys

repo, fork_repo, fork_commit = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
NEW = "8dea4a2196ebf99449fe2f141a2fb30acae3f17c"
OLD = "7d829946231d6413a398f6e4160cdfe951261dad"

HEAD = '''# SPIKE (v0.3.0 Track E, #237): 0.2026.09.06.8dea4a2 as the active pin, to
# measure what a second qualified pin costs. Not a proposal to move the pin.

[verus]
release = "0.2026.09.06.8dea4a2"
tag = "release/0.2026.09.06.8dea4a2"
commit = "8dea4a2196ebf99449fe2f141a2fb30acae3f17c"
rust_toolchain = "1.98.0"
# `rustc +1.98.0 -vV`, observed on the spike host.
rust_commit = "88d9e12ae178fab0fb5cc050a94da85685d449ea"
# verus@8dea4a2:source/vstd/Cargo.toml
vstd = "0.0.0-2026-09-06-0133"

[z3]
version = "4.16.0"

[assets.aarch64-apple-darwin]
url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.09.06.8dea4a2/verus-0.2026.09.06.8dea4a2-arm64-macos.zip"
sha256 = "23be2e8113b50bc91729377d8ac032f658cc5187e389a319d8458f9c37b07b4a"
dir = "verus-arm64-macos"
z3_sha256 = "edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb"

# The Linux archive bundles no z3 either (observed on the spike host,
# VI-0006). The spike copies 7d82994's z3 (same digest) beside `verus`.
[assets.x86_64-unknown-linux-gnu]
url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.09.06.8dea4a2/verus-0.2026.09.06.8dea4a2-x86-linux.zip"
sha256 = "13d01e134c0620c3b29770874707d16c33b3d227c843a489c8ceb744d43c0a16"
dir = "verus-x86-linux"
z3_sha256 = "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765"

[secondary."0.2026.10.18.7d82994".verus]
release = "0.2026.10.18.7d82994"
tag = "release/0.2026.10.18.7d82994"
commit = "7d829946231d6413a398f6e4160cdfe951261dad"
rust_toolchain = "1.99.1"
rust_commit = "58cf8f9c35b355199727627d07ae34a4e2846d67"
vstd = "0.0.0-2026-10-18-0147"

[secondary."0.2026.10.18.7d82994".z3]
version = "4.16.0"

[secondary."0.2026.10.18.7d82994".assets.aarch64-apple-darwin]
url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.10.18.7d82994/verus-0.2026.10.18.7d82994-arm64-macos.zip"
sha256 = "3fc56c58277b5f0999368f7b3a2b07ba4eb9ec2965454ce7a9f2769c0464d32b"
dir = "verus-arm64-macos"
z3_sha256 = "edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb"

[secondary."0.2026.10.18.7d82994".assets.x86_64-unknown-linux-gnu]
url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.10.18.7d82994/verus-0.2026.10.18.7d82994-x86-linux.zip"
sha256 = "e9af03732d4bdd32b93a8fcf471706739c24c1473fa5086618d96c47237845dd"
dir = "verus-x86-linux"
z3_sha256 = "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765"
'''

for rel in [
    "corpus/toolchain.toml",
    "crates/sail-verus/resources/corpus/toolchain.toml",
    "crates/sail-runtime/resources/corpus/toolchain.toml",
]:
    p = repo / rel
    if p.is_symlink():
        continue
    text = p.read_text()
    marker = '[secondary."0.2026.10.04.426d8b0".verus]'
    comment = "# The previous primary release"
    tail = text[text.index(comment) if comment in text else text.index(marker):]
    p.write_text(HEAD + "\n" + tail)

FORK = f'''# SPIKE (v0.3.0 Track E, #237): the sail series (7d82994..a0e11cff) squashed
# and applied to upstream 8dea4a2 in a lane-local clone. Never pushed.
[fork]
repo = "{fork_repo}"
branch = "spike/8dea4a2"
commit = "{fork_commit}"
upstream = "{NEW}"
'''
for rel in ["engine/verus/fork.toml", "crates/sail-verus/resources/engine/verus/fork.toml"]:
    p = repo / rel
    if not p.is_symlink():
        p.write_text(FORK)

p = repo / "corpus/corpus.toml"
text = p.read_text()
text = text.replace('verus_release = "0.2026.10.18.7d82994"', 'verus_release = "0.2026.09.06.8dea4a2"')
head, sep, rest = text.partition('name = "human-eval-verus"')
p.write_text(head.replace(OLD, NEW) + sep + rest)
print("repinned")
