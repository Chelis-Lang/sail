#!/usr/bin/env python3
"""Small regressions for included-source links and the book's failure checks."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
# sha256 of the highlight.js that `mdbook init --theme` writes for mdBook 0.5.2.
MDBOOK_HIGHLIGHT_SHA256 = 'abc7f01d0ce8c64e4ba9e131d651e48ae1a567b2a1254fea8aab03502288de58'

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

# Every fenced block in the book's sources names its language, so none is
# left to the highlighter's guess or rendered as uncolored text by accident.
bare = []
for source in sorted(chapters):
    fence = None
    for number, line in enumerate(source.read_text().splitlines(), 1):
        marker = re.match(r'^\s*(`{3,}|~{3,})(.*)$', line)
        if not marker:
            continue
        if fence is None:
            fence = marker[1]
            if not marker[2].strip():
                bare.append(f'{source.relative_to(ROOT)}:{number}')
        elif marker[1].startswith(fence) and not marker[2].strip():
            fence = None
assert not bare, 'code fences without a language: ' + ', '.join(bare)

# book/theme/highlight.js is mdBook 0.5.2's bundled highlight.js, unchanged,
# followed by Sail's grammars. A different mdBook bundle must be re-vendored.
theme = (ROOT / 'book/theme/highlight.js').read_text()
bundle, marker, extensions = theme.partition('\n/* ---- Sail extensions')
assert marker, 'book/theme/highlight.js lacks the Sail extensions section'
assert hashlib.sha256(bundle.encode()).hexdigest() == MDBOOK_HIGHLIGHT_SHA256, \
    'book/theme/highlight.js no longer starts with the mdBook 0.5.2 bundle'
for language in ('rust', 'bash'):
    assert f"hljs.registerLanguage('{language}'" in extensions, language

node = shutil.which('node')
if node:
    probe = r"""
const fs = require('fs'), vm = require('vm');
const context = {}; vm.createContext(context);
vm.runInContext(fs.readFileSync(process.argv[1], 'utf8') + ';this.hljs = hljs;', context);
const h = (lang, code) => context.hljs.highlight(lang, code).value;
process.stdout.write(JSON.stringify({
  sh: h('sh', 'SAIL_CACHE_DIR=x sail verify proof.rs --output json   # report\nfor f in a; do echo $f; done'),
  heredoc: h('sh', 'cat <<EOF\nfoo\nbar\nEOF\nls'),
  continued: h('sh', 'python3 run.py \\\n  bench/example \\\n  --rounds 6'),
  rust: h('rust', 'verus! {\n    proof fn p(x: nat) requires x > 0 ensures x >= 1 { assert(x == x); }\n}'),
  types: h('rust', 'fn f() -> Result<Self, Client> { Some(1) }'),
}));
"""
    result = json.loads(subprocess.run([node, '-e', probe, str(ROOT / 'book/theme/highlight.js')],
                                       check=True, capture_output=True, text=True).stdout)
    # Commands take the keyword scope, which every mdBook theme colors apart
    # from flags (attr).
    for fragment in ('<span class="hljs-variable">SAIL_CACHE_DIR</span>',
                     '<span class="hljs-keyword">sail</span>',
                     '<span class="hljs-title">verify</span>',
                     '<span class="hljs-attr">--output</span>',
                     '<span class="hljs-comment"># report</span>',
                     '<span class="hljs-keyword">for</span>',
                     '<span class="hljs-keyword">echo</span>'):
        assert fragment in result['sh'], (fragment, result['sh'])
    for fragment in ('<span class="hljs-built_in">verus!</span>',
                     '<span class="hljs-keyword">proof</span>',
                     '<span class="hljs-keyword">requires</span>',
                     '<span class="hljs-keyword">ensures</span>',
                     '<span class="hljs-keyword">assert</span>',
                     '<span class="hljs-built_in">nat</span>'):
        assert fragment in result['rust'], (fragment, result['rust'])
    # A here-document ends at its own delimiter, not at a one-word body line.
    assert '<span class="hljs-string">&lt;&lt;EOF\nfoo\nbar\nEOF</span>' in result['heredoc'], result['heredoc']
    assert '<span class="hljs-keyword">ls</span>' in result['heredoc'], result['heredoc']
    # A backslash-continued argument is not a command.
    assert 'hljs-keyword">bench' not in result['continued'], result['continued']
    assert '<span class="hljs-attr">--rounds</span>' in result['continued'], result['continued']
    for fragment in ('<span class="hljs-built_in">Result</span>', '<span class="hljs-keyword">Self</span>',
                     '<span class="hljs-title">Client</span>', '<span class="hljs-literal">Some</span>'):
        assert fragment in result['types'], (fragment, result['types'])
else:
    print('book: node not found; skipped the highlighting probe')

print('book: source-link, fence, highlighting and negative link-check controls passed')
