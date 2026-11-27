#!/usr/bin/env python3
"""Prints the GitHub Release body for a Sail version (docs/releasing.md).

The body is the curated docs/release-notes/X.Y.Z.md for the version's base
X.Y.Z (a prerelease such as 0.3.1-rc.1 uses 0.3.1.md), followed by a short
description of the assets. Relative links in the notes resolve against the
notes file and are rewritten to the tagged source on GitHub, because a release
body has no repository-relative base. A missing notes file is an error, so a
release never carries placeholder text.

    scripts/release-notes.py VERSION [COMMIT]
"""
import posixpath
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = 'Chelis-Lang/sail'
NOTES_DIR = 'docs/release-notes'
LINK = re.compile(r'(\]\()([^)\s]+)(\))')

ASSETS = ('Assets: one tarball per platform with a .sha256 file. Each holds bin/sail, '
          'bin/sail-z3, a source/native-byte-bound vstd proof bundle and Sail\'s Verus '
          '(built from Sail\'s Verus fork; also the repair engine) under engine/. Users '
          'need only a Rust toolchain besides. See the '
          '[install guide](https://github.com/{repo}/blob/v{version}/docs/install.md).')


def base_version(version):
    match = re.fullmatch(r'(\d+\.\d+\.\d+)(-[0-9A-Za-z.-]+)?', version)
    if not match:
        raise ValueError(f'not a release version: {version!r}')
    return match.group(1)


def absolute(target, version):
    """The GitHub URL for one link target written relative to the notes file."""
    if re.match(r'^[a-z][a-z0-9+.-]*:', target) or target.startswith('#'):
        return target
    path, hash_mark, fragment = target.partition('#')
    resolved = posixpath.normpath(posixpath.join(NOTES_DIR, path))
    if resolved.startswith('..'):
        raise ValueError(f'link leaves the repository: {target!r}')
    return f'https://github.com/{REPO}/blob/v{version}/{resolved}{hash_mark}{fragment}'


def body(version, root=ROOT, commit=None):
    notes = root / NOTES_DIR / f'{base_version(version)}.md'
    if not notes.is_file():
        raise FileNotFoundError(f'{notes.relative_to(root)} is missing; write the release notes first')
    text = LINK.sub(lambda m: m.group(1) + absolute(m.group(2), version) + m.group(3),
                    notes.read_text(encoding='utf-8')).rstrip('\n')
    built = f' Built by scripts/release.sh from {commit}.' if commit else ''
    return f'{text}\n\n---\n\n{ASSETS.format(repo=REPO, version=version)}{built}\n'


def main(argv):
    if len(argv) not in (2, 3):
        print(__doc__.strip().splitlines()[-1].strip(), file=sys.stderr)
        return 2
    try:
        sys.stdout.write(body(argv[1], commit=argv[2] if len(argv) == 3 else None))
    except (ValueError, FileNotFoundError) as error:
        print(f'release-notes: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
