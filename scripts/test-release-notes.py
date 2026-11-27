#!/usr/bin/env python3
"""The GitHub Release body is the curated release notes with working links."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('release_notes', Path(__file__).with_name('release-notes.py'))
notes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notes)


class ReleaseNotes(unittest.TestCase):
    def write(self, root, name, text):
        file = root / 'docs/release-notes' / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def test_body_is_the_curated_notes_with_absolute_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write(root, '9.8.7.md', '# Sail 9.8.7\n\nSee the [changelog](../../CHANGELOG.md), '
                       '[install](../install.md#linux), [ADR](../decisions/0001-x.md), '
                       '[web](https://example.com/a) and [here](#top).\n')
            text = notes.body('9.8.7', root=root, commit='abc123')
            base = 'https://github.com/Chelis-Lang/sail/blob/v9.8.7/'
            self.assertTrue(text.startswith('# Sail 9.8.7\n'))
            self.assertIn(f'[changelog]({base}CHANGELOG.md)', text)
            self.assertIn(f'[install]({base}docs/install.md#linux)', text)
            self.assertIn(f'[ADR]({base}docs/decisions/0001-x.md)', text)
            self.assertIn('[web](https://example.com/a)', text)
            self.assertIn('[here](#top)', text)
            self.assertIn('Built by scripts/release.sh from abc123.', text)
            self.assertNotIn('Draft release built', text)

    def test_prerelease_uses_its_base_notes_and_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write(root, '9.8.7.md', '[x](../x.md)\n')
            text = notes.body('9.8.7-rc.1', root=root)
            self.assertIn('blob/v9.8.7-rc.1/docs/x.md', text)

    def test_missing_notes_or_bad_input_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(FileNotFoundError):
                notes.body('9.8.7', root=root)
            self.write(root, '9.8.7.md', '[x](../../../outside.md)\n')
            with self.assertRaises(ValueError):
                notes.body('9.8.7', root=root)
            with self.assertRaises(ValueError):
                notes.body('v9.8.7', root=root)

    def test_unfinished_notes_are_refused_for_a_final_release(self):
        for text in ('# Sail 9.8.7\n\n> **Draft.** Figures are placeholders.\n',
                     '# Sail 9.8.7\n\nSpeed: **[S7 placeholder: ratio]**.\n',
                     '# Sail 9.8.7\n\n> **draft.** Not yet measured.\n'):
            with self.subTest(text=text), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.write(root, '9.8.7.md', text)
                with self.assertRaisesRegex(ValueError, r'9\.8\.7\.md:3 is unfinished'):
                    notes.body('9.8.7', root=root)
                self.assertIn(text.rstrip('\n'), notes.body('9.8.7-rc.1', root=root))

    def test_published_notes_render(self):
        # The rc suffix lets notes still being written for the next release
        # render; the final-release refusal is tested above.
        root = Path(__file__).resolve().parent.parent
        for file in sorted((root / 'docs/release-notes').glob('*.md')):
            with self.subTest(file.name):
                text = notes.body(f'{file.stem}-rc.1', root=root)
                self.assertNotIn('](../', text)


if __name__ == '__main__':
    unittest.main()
