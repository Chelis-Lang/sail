#!/usr/bin/env python3
"""Check the built book's local assets, links and anchors without network access."""
from html.parser import HTMLParser
from pathlib import Path
import sys
from urllib.parse import unquote, urlsplit


def check(root):
    root = Path(root).resolve()
    class Page(HTMLParser):
        def __init__(self):
            super().__init__()
            self.ids = set()
            self.links = []
        def handle_starttag(self, tag, attributes):
            attrs = dict(attributes)
            if attrs.get('id'):
                self.ids.add(attrs['id'])
            if tag == 'a' and attrs.get('name'):
                self.ids.add(attrs['name'])
            for name in ('href', 'src'):
                if attrs.get(name):
                    self.links.append(attrs[name])
    pages = {}
    for path in root.rglob('*.html'):
        page = Page()
        page.feed(path.read_text())
        pages[path] = page
    if not pages:
        return ['No book HTML found']
    errors = []
    for path, page in pages.items():
        for href in page.links:
            link = urlsplit(href)
            if link.scheme or link.netloc:
                continue
            if link.path.startswith('/docs/sail/'):
                target = root / unquote(link.path[len('/docs/sail/'):])
            else:
                target = path.parent / unquote(link.path) if link.path else path
            target = target.resolve()
            if target.is_dir():
                target /= 'index.html'
            if not target.is_relative_to(root) or not target.is_file():
                errors.append(f'{path.relative_to(root)}: missing local link {href}')
            elif link.fragment and target in pages and unquote(link.fragment) not in pages[target].ids:
                errors.append(f'{path.relative_to(root)}: missing fragment {href}')
    return errors


if __name__ == '__main__':
    errors = check(sys.argv[1])
    if errors:
        raise SystemExit('\n'.join(errors))
    print('book: local assets, links and fragments passed')
