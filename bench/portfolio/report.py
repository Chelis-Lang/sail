#!/usr/bin/env python3
"""Render the local portfolio cost probe from its retained JSON data."""
import json
import pathlib
import statistics
import sys


def render(result, filename):
    """Keep these small loaded-machine probes separate from reference benchmarks."""
    lines = [
        '# Portfolio cost probe', '',
        f'Generated from `{filename}`: three paired rounds per scope on a shared macOS arm64 development machine. These are small protocol/selection probes, not Linux reference speedup benchmarks. Both cold and warm runs use `accept-first-proof` mode and its isolated cache.', '',
        '| Requested scope | Plain median (range), ms | Portfolio cold median (range), ms | Portfolio warm median (range), ms | Outcome |',
        '|---|---:|---:|---:|---|',
    ]
    def stat(values):
        return f'{statistics.median(values):.1f} ({min(values):.1f}–{max(values):.1f})'
    for scope in ['good', 'whole-file']:
        rows = [r for r in result['rounds'] if r['scope'] == scope]
        outcome = ('baseline seed 0 wins; skipped failing function omitted' if scope == 'good'
                   else 'all three attempts fail; baseline failure preserved')
        lines.append('| '+scope+' | '+stat([r['plain_ms'] for r in rows])+' | '+stat([r['cold']['wall_ms'] for r in rows])+' | '+stat([r['warm']['wall_ms'] for r in rows])+' | '+outcome+' |')
    lines += ['', 'Warm runs serve cached checks; compiler and CLI work still run. Alternative configurations carry separate prefix keys. A hard unknown at its resource limit is always re-solved, so this probe does not predict the warm cost of page-table’s fragile allocation proof.', '']
    return '\n'.join(lines)


if __name__ == '__main__':
    source = pathlib.Path(__file__).with_name('cost-aarch64-macos-devload-2026-10-09.json')
    try:
        text = render(json.loads(source.read_text()), source.name)
        proof_file = pathlib.Path(__file__).with_name('page-table-allocate-aarch64-macos-2026-10-09.json')
        if proof_file.exists():
            proof = json.loads(proof_file.read_text())
            function = proof['functions'][0]
            text += '\n## Independent allocation proof\n\n'
            text += f"Retained in `{proof_file.name}`: seed {proof['winner_seed']} proved exactly `{proof['scope']}` on macOS arm64, with {function['timing']['rlimit']:,} resource units and zero errors. The observed effective limit was {proof['effective_rlimit']:,} ({proof['resource_fraction']:.1%} used): the function-specific attribute overrides the global `--rlimit 20` flag. "
            text += 'The unchanged baseline reached its wall timeout. Both attempts, solver hash and proving configuration are retained. This confirms only the requested allocation function, not the entire page-table crate, and is not a reference-platform speedup benchmark.\n'
        pathlib.Path(__file__).with_name('README.md').write_text(text)
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f'error[SAIL0116]: cannot render portfolio probe: {error}')
