#!/usr/bin/env python3
"""An installed skill must retain all of its relative references."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('check_links', Path(__file__).with_name('check-links.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class SkillLinks(unittest.TestCase):
    def test_links_survive_copying_only_the_skill_folder(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            names = {'skills/demo/SKILL.md', 'skills/demo/references/workflow.md', 'README.md'}
            for name in names:
                file = root / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text('# Example\n')
            entry = root / 'skills/demo/SKILL.md'
            with patch.object(checker, 'ROOT', root), patch.object(checker, 'tracked', return_value=(names, set())), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                entry.write_text('[Workflow](references/workflow.md)\n[Upstream](https://example.com/docs)\n')
                self.assertEqual(checker.main(), 0)
                # This is valid in the repository, but breaks after installation.
                entry.write_text('[Guide](../../README.md)\n')
                self.assertEqual(checker.main(), 1)
                entry.write_text('[Missing](references/absent.md)\n')
                self.assertEqual(checker.main(), 1)


if __name__ == '__main__':
    unittest.main()
