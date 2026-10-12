#!/usr/bin/env python3
"""Check repo-relative links in tracked Markdown files.

Every inline link `[text](target)` whose target is not a URL must name a
tracked file or directory, relative to the linking file. A `#fragment` on a
Markdown target must match one of that file's GitHub heading anchors. Links in
fenced code blocks and inline code are ignored. Exits nonzero on any broken
link.
"""
import posixpath
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r'!?\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)')
CODE = re.compile(r'`+[^`]*`+')


def tracked():
    try:
        out = subprocess.run(['git', 'ls-files', '-z'], cwd=ROOT, capture_output=True, check=True).stdout
        files = {p for p in out.decode().split('\0') if p}
    except (OSError, subprocess.CalledProcessError):
        # No usable repository (for example a synced copy): walk the tree.
        skip = {'.git', '.claude', 'target', 'conformance-out', '__pycache__'}
        files = {p.relative_to(ROOT).as_posix() for p in ROOT.rglob('*')
                 if p.is_file() and not skip.intersection(p.relative_to(ROOT).parts)}
    dirs = {str(Path(p).parent) for p in files}
    while True:
        more = {str(Path(d).parent) for d in dirs} - dirs
        if not more:
            break
        dirs |= more
    return files, dirs


def anchors(path, cache={}):
    if path not in cache:
        seen, result, fenced = {}, set(), False
        for line in (ROOT / path).read_text(encoding='utf-8').splitlines():
            if line.lstrip().startswith(('```', '~~~')):
                fenced = not fenced
            if fenced or not line.startswith('#'):
                continue
            title = line.lstrip('#').strip()
            slug = re.sub(r'[^\w\- ]', '', CODE.sub(lambda m: m.group(0).strip('`'), title).lower()).replace(' ', '-')
            n = seen.get(slug, 0)
            seen[slug] = n + 1
            result.add(slug if n == 0 else f'{slug}-{n}')
        for match in re.finditer(r'<a\s+(?:name|id)="([^"]+)"', (ROOT / path).read_text(encoding='utf-8')):
            result.add(match.group(1))
        cache[path] = result
    return cache[path]


def prose(text):
    """Blank fenced blocks and code spans, keeping line numbers."""
    lines, fenced = [], False
    for line in text.split('\n'):
        if line.lstrip().startswith(('```', '~~~')):
            fenced = not fenced
            lines.append('')
        else:
            lines.append('' if fenced else line)
    return CODE.sub(lambda m: '\n' * m.group(0).count('\n'), '\n'.join(lines))


def main():
    files, dirs = tracked()
    broken = 0
    for md in sorted(f for f in files if f.endswith('.md')):
        text = prose((ROOT / md).read_text(encoding='utf-8'))
        for match in LINK.finditer(text):
            target = match.group(1)
            if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target):
                continue
            path, _, fragment = target.partition('#')
            resolved, problem = md, None
            if path.startswith('/'):
                problem = 'absolute path'
            elif path:
                resolved = posixpath.normpath(posixpath.join(posixpath.dirname(md), path))
                if resolved not in files and resolved not in dirs:
                    problem = 'missing'
            if problem is None and fragment and resolved.endswith('.md'):
                if fragment.lower() not in anchors(resolved):
                    problem = 'missing anchor'
            if problem:
                broken += 1
                number = text.count('\n', 0, match.start()) + 1
                print(f'{md}:{number}: {problem}: {target}')
    if broken:
        print(f'{broken} broken link(s)', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
