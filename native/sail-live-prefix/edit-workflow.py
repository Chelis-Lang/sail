#!/usr/bin/env python3
"""Run the disabled registered-session edit experiment; application gate pending."""
import argparse
import json
from pathlib import Path

from edit_stream import MAX_BYTES
from edit_workflow import Workflow


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--experiment-numeric-time-only', action='store_true', required=True)
    parser.add_argument('--experiment-kernel-accounting-owner', action='store_true', required=True)
    parser.add_argument('--solver', required=True)
    parser.add_argument('--solver-sha256', required=True)
    parser.add_argument('--cwd', required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--source-sha256', required=True)
    parser.add_argument('--edit-file', required=True)
    parser.add_argument('--delay', type=float, default=0)
    parser.add_argument('--timeout', type=float, default=120)
    parser.add_argument('--out', required=True)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    with Path(args.edit_file).open('rb') as stream:
        edited = stream.read(MAX_BYTES + 1)
    if len(edited) > MAX_BYTES:
        parser.error('explicit source edit exceeds the bounded input size')
    workflow = Workflow(args.out, args.solver, args.solver_sha256, command, args.cwd,
                        edit_delay=args.delay, attempt_timeout=args.timeout)
    result = workflow.run(args.source, args.source_sha256, edited)
    print(json.dumps({key: result[key] for key in (
        'driver_exit', 'application_exit', 'application_acceptance', 'qualified',
        'selected', 'coverage', 'errors')}))
    # No incomplete-census success claim from this executable. The per-attempt
    # registered-session results and complete costs remain in result.json.
    return result['application_exit']


if __name__ == '__main__':
    raise SystemExit(main())
