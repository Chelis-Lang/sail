#!/usr/bin/env python3
"""Resolve included guides' repository links for the book without copying prose."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'https://github.com/Chelis-Lang/sail'
SKILL = 'https://raw.githubusercontent.com/Chelis-Lang/sail/main/skills/sail-verification/SKILL.md'


def chapter_sources():
    result = {}
    for path in (ROOT / 'book/src').glob('*.md'):
        match = re.fullmatch(r'\s*\{\{#include (.+?)\}\}\s*', path.read_text())
        if match:
            source = (path.parent / match[1]).resolve()
            if not source.is_relative_to(ROOT) or not source.is_file():
                raise ValueError(f'Invalid chapter source: {path}')
            result[source] = path.name
    return result


def rewrite(content, source, chapters, revision):
    def destination(url):
        parts = urlsplit(url)
        if not parts.path or parts.scheme or parts.netloc:
            return url
        target = (source.parent / parts.path).resolve()
        if not target.is_relative_to(ROOT) or not target.exists():
            raise ValueError(f'{source.relative_to(ROOT)}: missing source link {url}')
        relative = target.relative_to(ROOT).as_posix()
        if relative == 'skills/sail-verification/SKILL.md':
            return SKILL
        suffix = ('?' + parts.query if parts.query else '') + ('#' + parts.fragment if parts.fragment else '')
        if target in chapters:
            return chapters[target] + suffix
        return REPOSITORY + '/blob/' + revision + '/' + quote(relative, safe='/') + suffix

    lines = []
    fence = None
    for line in content.splitlines(keepends=True):
        marker = re.match(r'^\s*(`{3,}|~{3,})', line)
        if marker:
            char = marker[1][0]
            if fence is None:
                fence = char
            elif fence == char:
                fence = None
            lines.append(line)
            continue
        if fence is None:
            line = re.sub(r'(!?\[[^\]\n]*\]\()([^\s)]+)([^)]*\))', lambda m: m[1] + destination(m[2]) + m[3], line)
            line = re.sub(r'^(\s*\[[^\]]+\]:\s*)(\S+)', lambda m: m[1] + destination(m[2]), line)
        lines.append(line)
    return ''.join(lines)


def main():
    if len(sys.argv) > 1 and sys.argv[1] == 'supports':
        return
    _context, book = json.load(sys.stdin)
    chapters = chapter_sources()
    by_name = {name: source for source, name in chapters.items()}
    revision = os.environ.get('SAIL_BOOK_SOURCE_REVISION')
    if not revision:
        revision = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    if not re.fullmatch(r'[a-f0-9]{40}', revision):
        raise ValueError('SAIL_BOOK_SOURCE_REVISION must be a full commit hash')
    def visit(items):
        for item in items:
            chapter = item.get('Chapter')
            if chapter:
                source = by_name.get(chapter['path'])
                if source:
                    chapter['content'] = rewrite(chapter['content'], source, chapters, revision)
                visit(chapter.get('sub_items', []))
    visit(book.get('sections', book.get('items', [])))
    json.dump(book, sys.stdout)


if __name__ == '__main__':
    main()
