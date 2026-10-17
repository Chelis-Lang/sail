"""Named research resource policy: complete kernel accounting, never a proof cache."""
import os
import selectors
import signal
import subprocess
import time
from live_prefix import MAX_OUTPUT, capture_running_image
from owned_wait4 import OwnedWait4, positive_accounting, drain_unstarted

EXPERIMENT = 'owned-wait4-lifetime-peak-v1'


def cpu(record):
    """Return only actual validated per-child CPU, never a missing-data zero."""
    if record is None:
        return None
    return record['accounting']['user_seconds'] + record['accounting']['system_seconds']


def fresh(z3, identity, data, timeout, maximum):
    """Run unchanged full input with one assigned owner before binding/reaping.

    Kernel lifetime peak is a final-result admission, not an OS live memory cap.
    Runtime/library/process-tree assumptions stay those of the pinned direct Z3
    experiment. No /proc memory sample or terminal-race classification is used.
    """
    owner = None
    pidfd = image = None
    buffers = {'stdout': bytearray(), 'stderr': bytearray()}
    errors = []
    accounting = None
    verified = False
    selector = selectors.DefaultSelector()
    offset = 0
    exit_code = None

    def kill():
        if owner is None:
            return
        if pidfd is not None:
            try:
                signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError as error:
                errors.append('owned signal: ' + str(error))
        owner.cancel_unstarted()

    try:
        owner = OwnedWait4([z3, '-smt2', '-in'], deferred=True,
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        # No waiter exists until both held descriptors and actual image match.
        pidfd, image, _ = capture_running_image(owner.pid, identity)
        verified = True
        owner.start_reaper()
        for name in ('stdout', 'stderr'):
            stream = getattr(owner, name)
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        os.set_blocking(owner.stdin.fileno(), False)
        if data:
            selector.register(owner.stdin, selectors.EVENT_WRITE, 'stdin')
        else:
            owner.stdin.close()
        deadline = time.monotonic() + timeout
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(owner.args, timeout)
            for key, _ in selector.select(min(remaining, .1)):
                if key.data == 'stdin':
                    try:
                        written = os.write(key.fileobj.fileno(), data[offset:offset + 65536])
                    except BlockingIOError:
                        continue
                    if written <= 0:
                        raise OSError('input write made no progress')
                    offset += written
                    if offset == len(data):
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                else:
                    try:
                        block = os.read(key.fileobj.fileno(), 65536)
                    except BlockingIOError:
                        continue
                    if not block:
                        selector.unregister(key.fileobj)
                        continue
                    output = buffers[key.data]
                    available = MAX_OUTPUT - len(output)
                    output.extend(block[:max(0, available)])
                    if len(block) > available:
                        raise ValueError('output budget exceeded; bounded raw prefix retained')
        if offset != len(data):
            raise ValueError('complete input was not written')
        owner.wait(timeout=max(0, deadline - time.monotonic()))
        accounting = owner.accounting()
        exit_code = accounting['exit']
        positive_accounting(owner, maximum)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        errors.append(type(error).__name__ + ': ' + str(error))
    finally:
        selector.close()
        if owner is not None:
            try:
                if errors or owner.poll() is None:
                    kill()
                owner.cancel_unstarted()
                owner.wait(timeout=2)
                accounting = owner.accounting()
                exit_code = accounting['exit']
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                errors.append('owned completion/accounting: ' + str(error))
                if pidfd is not None:
                    try:
                        observed = owner.cleanup_failed_accounting(pidfd)
                        exit_code = observed.get('actual_exit')
                    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as cleanup_error:
                        errors.append('held-pidfd error cleanup: ' + str(cleanup_error))
            # One I/O owner exists; after producer completion retain even failed
            # startup/nonzero/timeout streams before closing their descriptors.
            for name in ('stdout', 'stderr'):
                errors.extend(drain_unstarted(getattr(owner, name), buffers[name], MAX_OUTPUT))
            for stream in (owner.stdin, owner.stdout, owner.stderr):
                try:
                    stream.close()
                except (OSError, ValueError) as error:
                    errors.append('stream close: ' + str(error))
        for descriptor in (image, pidfd):
            if descriptor is not None:
                try:
                    os.close(descriptor)
                except OSError as error:
                    errors.append('held descriptor close: ' + str(error))
    return dict(stdout=bytes(buffers['stdout']), stderr=bytes(buffers['stderr']),
                exit=exit_code, error='; '.join(errors) if errors else None,
                image_verified=verified, kernel_accounting=accounting,
                kernel_raw_observation=owner.raw_observation() if owner else None,
                kernel_error_cleanup=owner.cleanup_observation() if owner else None,
                resource_policy=EXPERIMENT,
                sampled_peak_rss=None, sampled_peak_pss=None, rss_samples=0,
                memory_observations=[], input_bytes_written=offset,
                kernel_resource_gate_pass=not errors and accounting is not None)
