#!/usr/bin/env python3
"""Small regressions for included-source links and the book's failure checks."""
import importlib.util
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]

def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

links = load('book_links', 'book-links.py')
checker = load('check_book', 'check-book-links.py')
chapters = links.chapter_sources()
source = ROOT / 'docs/quickstart.md'
rev = '1' * 40
text = '[Install](install.md#cargo-installation) [Skill](../skills/sail-verification/SKILL.md) [Details](decisions/0001-initial-architecture.md)\n```md\n[example](missing.md)\n```\n'
# Use a real archive file; its name is intentionally not part of the book map.
archive = next((ROOT / 'docs/decisions').glob('*.md'))
text = text.replace('decisions/0001-initial-architecture.md', archive.relative_to(source.parent).as_posix())
out = links.rewrite(text, source, chapters, rev)
assert '(install.md#cargo-installation)' in out
assert links.SKILL in out
assert '/blob/' + rev + '/' in out
assert '[example](missing.md)' in out
try:
    links.rewrite('[bad](does-not-exist.md)', source, chapters, rev)
except ValueError:
    pass
else:
    raise AssertionError('missing repository source link was accepted')
with TemporaryDirectory() as temp:
    root = Path(temp)
    page = root / 'index.html'
    page.write_text('<h1 id="ok">Book</h1><a href="#ok">Found</a>')
    assert checker.check(root) == []
    for broken in ['<a href="missing.html">Missing</a>', '<a href="#absent">Missing</a>', '<img src="missing.png">']:
        page.write_text(broken)
        assert checker.check(root), broken
print('book: source-link and negative link-check controls passed')
