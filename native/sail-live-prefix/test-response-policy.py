"""Deterministic strict-boundary controls for the named timing-only experiment."""
import json
from pathlib import Path
import response_policy as policy

BASE = b'unsat\n(:memory 1.25\n :rlimit-count 900\n :time 0.01)\n<<DONE>>\n'
rows = []

def check(name, other, wanted, original=BASE):
    value = policy.compare(original, other)
    assert value['equal_except_numeric_time'] is wanted, (name, value)
    rows.append({'case': name, 'passed': True, 'raw_equal': value['raw_equal'],
                 'equal_except_numeric_time': wanted})

check('exact', BASE, True)
check('only-time-different-width', BASE.replace(b'0.01', b'123.456'), True)
check('integer-time', BASE.replace(b'0.01', b'0'), True)
check('verdict', BASE.replace(b'unsat', b'sat'), False)
check('unknown-verdict', BASE.replace(b'unsat', b'unknown'), False)
check('other-statistic', BASE.replace(b'900', b'901'), False)
check('memory-statistic', BASE.replace(b'1.25', b'1.26'), False)
check('time-whitespace', BASE.replace(b':time 0.01', b':time  0.02'), False)
check('response-whitespace', BASE.replace(b'unsat\n', b'unsat \n'), False)
check('framing-marker', BASE.replace(b'<<DONE>>', b'<<OTHER>>'), False)
check('missing-statistic', BASE.replace(b' :time 0.01', b''), False)
check('extra-statistic', BASE.replace(b':time 0.01', b':extra 0\n :time 0.02'), False)
for name, value in [('negative', b'-1.0'), ('nan', b'NaN'), ('infinity', b'inf'),
                    ('quoted', b'"1.0"'), ('exponent', b'1e3'), ('leading-zero', b'01.0')]:
    check('invalid-time-'+name, BASE.replace(b'0.01', value), False)
for name, original, other in [
    ('error-string', b'(error ":time 0.01")\n', b'(error ":time 0.02")\n'),
    ('model-string', b'(model (define-fun x () String ":time 0.01"))\n', b'(model (define-fun x () String ":time 0.02"))\n'),
    ('quoted-symbol', b'(|:time| 0.01)\n', b'(|:time| 0.02)\n'),
    ('nested-model', b'(model (:time 0.01))\n', b'(model (:time 0.02))\n'),
    ('comment', b'; :time 0.01\nunsat\n', b'; :time 0.02\nunsat\n'),
    ('duplicate-time', b'(:time 0.01 :time 0.01)\n', b'(:time 0.02 :time 0.01)\n'),
]:
    check(name, other, False, original)
for name, data in [('unclosed-list', b'(:time 1'), ('closing', b')'),
                   ('quote', b'(error "x'), ('depth', b'(' * 129),
                   ('sentinel', b'\x00sail-experiment-numeric-time\x00')]:
    try:
        policy.project(data)
    except ValueError:
        rows.append({'case': 'refuse-'+name, 'passed': True})
    else:
        raise AssertionError(name)
actual = BASE.replace(b'0.01', b'123.456')
for end in range(len(actual)):
    state = policy.prefix_state(BASE, actual[:end])
    assert state in ('incomplete', 'equal'), (end, state)
    if state == 'equal':
        # Trailing whitespace must be bound too; no premature equal is valid.
        assert policy.project(actual[:end])[0] == policy.project(BASE)[0]
assert policy.prefix_state(BASE, actual) == 'equal'
assert policy.prefix_state(BASE, actual + b'error\n') == 'mismatch'
rows.append({'case': 'every-stream-split-and-extra-output', 'passed': True,
             'split_points': len(actual)})
# All representations and original numeric values are retained, not rewritten.
value = policy.compare(BASE, actual)
assert value['left_time_tokens'][0]['value'] == '0.01'
assert value['right_time_tokens'][0]['value'] == '123.456'
assert value['raw_equal'] is False
rows.append({'case': 'retained-values-offsets-hashes', 'passed': True})
print(json.dumps({'status': 'PASS', 'cases': len(rows), 'rows': rows,
                  'scope': 'Comparator/streaming fixtures only; no solver/performance/native acceptance.'}, indent=2))
