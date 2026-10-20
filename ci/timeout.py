#!/usr/bin/env python3
"""GNU `timeout` for hosts without coreutils (stock macOS).

    ci/timeout.py [--signal=SIG | -s SIG] [--kill-after=DURATION | -k DURATION]
                  DURATION COMMAND [ARGS...]

As GNU timeout without --foreground: COMMAND runs in its own process group,
which receives SIG (default TERM) after DURATION and KILL a further
--kill-after later. The exit status is 124 when the time ran out (137 if
KILL was needed), 125 for a bad command line, 126 or 127 when COMMAND cannot
run, and otherwise COMMAND's own status. A COMMAND killed by a signal, and
KILL after --kill-after, end the wrapper by the same signal, as GNU timeout
does, so a shell sees 128 + N.
Durations are numbers with an optional s, m, h or d suffix.
"""
import os
import signal
import subprocess
import sys

UNITS = {'s': 1, 'm': 60, 'h': 3600, 'd': 86400}


def duration(text):
    unit = UNITS.get(text[-1:])
    value = float(text[:-1] if unit else text)
    if value < 0:
        raise ValueError(text)
    return value * (unit or 1)


def signal_number(name):
    name = name.upper().removeprefix('SIG')
    return int(name) if name.isdigit() else signal.Signals['SIG' + name].value


def parse(args):
    sig, kill_after = signal.SIGTERM, None
    while args and args[0].startswith('-') and args[0] != '-':
        option = args.pop(0)
        if option == '--':
            break
        name, _, value = option.partition('=')
        if name in ('-s', '-k') and not value:
            value = args.pop(0)
        if name in ('--signal', '-s'):
            sig = signal_number(value)
        elif name in ('--kill-after', '-k'):
            kill_after = duration(value)
        else:
            raise ValueError(option)
    limit = duration(args.pop(0))
    if not args:
        raise ValueError('missing command')
    return sig, kill_after, limit, args


def main():
    try:
        sig, kill_after, limit, command = parse(sys.argv[1:])
    except (ValueError, IndexError, KeyError) as error:
        print(f'timeout: invalid arguments: {error}', file=sys.stderr)
        return 125
    try:
        # Its own process group, so a signal reaches every descendant.
        # start_new_session, not process_group (Python 3.11), because
        # stock macOS ships Python 3.9.
        child = subprocess.Popen(command, start_new_session=True)
    except PermissionError:
        return 126
    except OSError:
        return 127
    try:
        status = child.wait(timeout=limit or None)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=kill_after)
            return 124
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
            # GNU timeout ends by KILL too, so its caller sees status 137.
            os.kill(os.getpid(), signal.SIGKILL)
    if status < 0:
        # As GNU timeout: a command killed by a signal ends the wrapper by
        # the same signal, so a shell reports 128 + N either way.
        signal.signal(-status, signal.SIG_DFL)
        os.kill(os.getpid(), -status)
        return 128 - status
    return status


if __name__ == '__main__':
    sys.exit(main())
