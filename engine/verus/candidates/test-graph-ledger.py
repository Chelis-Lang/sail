#!/usr/bin/env python3
"""Local-only candidate oracle; requires separately built/qualified candidate."""
import argparse
import json
import pathlib
from importlib.machinery import SourceFileLoader

ROOT = pathlib.Path(__file__).resolve().parent.parent
base = SourceFileLoader('prepared_oracle', str(ROOT / 'test-prepared.py')).load_module()
source = SourceFileLoader('source_oracle', str(ROOT / 'test-engine.py')).load_module()


def run(args):
    # Keep the original checkpoint checks: unchanged callback, other-module edit,
    # stale spans, corruption, misfiling and oversized files; exact raw SMT bytes.
    base.run(args)
    original = source.FIXTURES
    source.FIXTURES = {
        'recursive': original['recursive'],
        'bitvector': original['bitvector'],
        'failed_focus': original['failed_focus'],
        'failed_other': original['failed_other'],
        'late_exec': original['late_exec'],
        'trait_cycle': 'trait T { spec fn value(&self) -> nat; } struct S {} impl T for S { spec fn value(&self) -> nat { 0 } } proof fn selected(s: S) ensures s.value() == 0 {}',
        'mutual_recursion': 'spec fn a(n: nat) -> nat decreases n { if n == 0 { 0 } else { b((n-1) as nat) } } spec fn b(n: nat) -> nat decreases n { if n == 0 { 0 } else { a((n-1) as nat) } } proof fn selected() ensures a(0) == 0 {}',
    }
    checkpoint_out = args.out
    args.out = str(pathlib.Path(checkpoint_out) / 'semantic-oracles')
    source.run(args)
    args.out = checkpoint_out
    summary = json.loads((pathlib.Path(args.out) / 'summary.json').read_text())
    assert any('stable' in name for name in summary['results'][2]['hits']), 'candidate provided no useful body-edit reuse'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['engine', 'stock', 'z3', 'out']:
        parser.add_argument('--' + name, required=True)
    run(parser.parse_args())
