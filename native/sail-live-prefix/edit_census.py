"""Narrow research join for native strace -ff -xx exec/exit files.

Not a general strace parser: unsupported/partial relevant records refuse.
Every source-bound wrapper exec must have one actual start, finish and broker
row with the same PID. The native caller separately retains tool identity,
all raw trace files and stderr, including tracing failures.
"""
import re


def census(traces, wrapper, sessions, starts, finishes):
    """Join actual observed wrapper executions, never infer success from absence."""
    errors, observed = [], {}
    if not traces:
        errors.append('owned compiler process trace missing')
    expected = wrapper.encode()
    for pid, raw in traces.items():
        if type(pid) is not int or pid <= 0 or not isinstance(raw, bytes):
            errors.append('unsupported process trace identity/type')
            continue
        try:
            text = raw.decode('ascii')
        except UnicodeError:
            errors.append(f'non-ASCII xx trace pid {pid}')
            continue
        rows = text.splitlines()
        if not raw.endswith(b'\n'):
            errors.append(f'partial process trace pid {pid}')
        terminal = [re.fullmatch(r'[0-9]+\.[0-9]+ \+\+\+ exited with ([0-9]+) \+\+\+', line)
                    for line in rows]
        terminal = [int(match[1]) for match in terminal if match]
        killed = [line for line in rows if re.fullmatch(
            r'[0-9]+\.[0-9]+ \+\+\+ killed by SIG[A-Z0-9]+(?: \(core dumped\))? \+\+\+', line)]
        if len(terminal) + len(killed) != 1 or not rows or '+++ ' not in rows[-1]:
            errors.append(f'incomplete/ambiguous process termination pid {pid}')
        for line in rows:
            if 'execve(' not in line and 'execveat(' not in line:
                continue
            # -xx encodes every string byte. The wrapper path is an absolute
            # source-bound VERUS_Z3_PATH, never selected by future query bytes.
            match = re.match(r'[0-9]+\.[0-9]+ execve\("((?:\\x[0-9a-f]{2})*)", ', line)
            if match is None:
                match = re.match(r'[0-9]+\.[0-9]+ execveat\([^,]+, "((?:\\x[0-9a-f]{2})*)", ', line)
            if match is None:
                # An unfinished exec can conceal the configured wrapper.
                errors.append(f'unsupported/partial exec record pid {pid}')
                continue
            path = bytes.fromhex(match[1].replace('\\x', ''))
            if path != expected:
                continue
            if pid in observed:
                errors.append(f'repeated wrapper exec pid {pid}')
            successful = line.endswith(' = 0')
            observed[pid] = dict(exec_succeeded=successful,
                                 exit=terminal[0] if len(terminal) == 1 else None,
                                 killed=bool(killed))
            if not successful:
                errors.append(f'wrapper exec refused pid {pid}')
    try:
        by_pid = {}
        for row in sessions:
            identity = row['identity']
            pid = identity['pid']
            if pid in by_pid:
                raise ValueError('multiple wrapper sessions for one actual PID')
            by_pid[pid] = identity['session']
        if not observed or set(observed) != set(by_pid):
            raise ValueError('observed wrapper exec/broker PID set differs or empty')
        for pid, entry in observed.items():
            session = by_pid[pid]
            start, finish = starts[session], finishes[session]
            if (start['identity'] != finish['identity'] or finish['identity']['pid'] != pid or
                    entry['exit'] != 0 or entry['killed'] or
                    finish['exit'] != entry['exit'] or finish['error']):
                raise ValueError(f'actual wrapper exit/start/finish refused pid {pid}')
    except (KeyError, TypeError, ValueError) as error:
        errors.append(str(error))
    return dict(passed=not errors, errors=errors, observed=observed,
                scope='source-bound wrapper exec/exit join from complete owned strace files; native tool/process tracing qualification required',
                qualified=False, selected=False)
