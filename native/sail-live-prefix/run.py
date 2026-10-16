#!/usr/bin/env python3
"""Bounded one-use sample with retained raw oracle and explicit failure limits.

Timing/CPU metrics require native x86-64 Linux. The guarded fresh reference
includes its actual-image validation overhead. No default Sail integration or
proof verdict is supplied. Failed/fallback samples are retained and exit1.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import threading
import time
import response_policy
import memory_lifecycle
from live_prefix import MAX_INPUT, MAX_INPUT_LIMIT, MAX_OUTPUT, OneUsePool, capture_running_image, path_digest, resident_bytes


def cpu_children():
    value = resource.getrusage(resource.RUSAGE_CHILDREN)
    return value.ru_utime + value.ru_stime


def proc_cpu(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--z3', required=True)
    parser.add_argument('--solver-sha256', required=True)
    parser.add_argument('--prefix', type=Path, required=True)
    parser.add_argument('--prefix-stdout', type=Path, required=True)
    parser.add_argument('--suffix', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--preparation-window', type=float, default=1)
    parser.add_argument('--ttl', type=float, default=5)
    parser.add_argument('--run-timeout', type=float, default=5)
    parser.add_argument('--oracle-timeout', type=float, default=5)
    parser.add_argument('--fallback-timeout', type=float, default=5)
    parser.add_argument('--max-bytes', type=int, default=128 * 1024 * 1024)
    parser.add_argument('--native-metrics', action='store_true')
    parser.add_argument('--max-input-bytes', type=int, default=MAX_INPUT)
    parser.add_argument('--fresh-only', action='store_true')
    parser.add_argument('--experiment-numeric-time-only', action='store_true',
                        help='Research only: only numeric :time response values may vary; raw bytes retained')
    parser.add_argument('--fresh-runs', type=int, default=3)
    args = parser.parse_args()
    if not 0 <= args.preparation_window <= 3600 or any(
            not 0 < limit <= 3600 for limit in [args.ttl, args.run_timeout, args.oracle_timeout, args.fallback_timeout]):
        parser.error('all limits must be bounded by 3600 seconds')
    if not 1 <= args.max_input_bytes <= MAX_INPUT_LIMIT or not 2 <= args.fresh_runs <= 8:
        parser.error('input budget must be1..16MiB and fresh runs2..8')
    if args.max_bytes <= 0:
        parser.error('resident memory budget must be positive')
    if args.native_metrics and (os.uname().sysname, os.uname().machine) != ('Linux', 'x86_64'):
        parser.error('performance metrics require native x86-64 Linux')
    args.out.mkdir(parents=True, exist_ok=False)
    z3 = str(Path(args.z3).resolve())
    manifest = dict(solver_sha256=args.solver_sha256, argv=[z3, '-smt2', '-in'], platform=list(os.uname()),
                    limits=dict(preparation_window=args.preparation_window, ttl=args.ttl,
                                serving=args.run_timeout, oracle=args.oracle_timeout,
                                fallback=args.fallback_timeout, bytes=args.max_bytes, input_bytes=args.max_input_bytes), raw_equal=False)
    manifest.update(cwd=str(Path.cwd()),
                    environment_sha256=hashlib.sha256(json.dumps(sorted(os.environ.items())).encode()).hexdigest(),
                    source_sha256={name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                                   for name in ['run.py', 'live_prefix.py', 'response_policy.py', 'memory_lifecycle.py']})
    manifest["response_policy"] = response_policy.EXPERIMENT if args.experiment_numeric_time_only else "exact-bytes"
    manifest["experiment_only"] = args.experiment_numeric_time_only
    pool = None

    def write_streams(name, result):
        for stream in ['stdout', 'stderr']:
            (args.out / (name + '.' + stream)).write_bytes(result[stream])

    def fresh(name, timeout, data):
        process = subprocess.Popen([z3, '-smt2', '-in'], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        buffers = [bytearray(), bytearray()]
        failure = []
        lock = threading.Lock()
        fd_lock = threading.Lock()
        pidfd = image = None
        threads = []
        verified = False
        peak_rss = peak_pss = 0
        rss_samples = 0
        identity = None
        memory_observations = []

        def kill():
            with fd_lock:
                try:
                    if pidfd is not None:
                        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                    elif process.poll() is None:
                        process.kill()
                except ProcessLookupError:
                    pass

        def drain(stream, output):
            try:
                while block := os.read(stream.fileno(), 65536):
                    available = MAX_OUTPUT - len(output)
                    output.extend(block[:available])
                    if len(block) > available:
                        with lock:
                            failure.append('output budget exceeded')
                        kill()
                        return
            except (OSError, ValueError) as error:
                with lock:
                    failure.append('output read: ' + str(error))
                kill()

        def send():
            try:
                process.stdin.write(data)
                process.stdin.close()
            except (OSError, ValueError) as error:
                with lock:
                    failure.append('input write: ' + str(error))

        try:
            if os.uname().sysname == 'Linux':
                pidfd, image, _ = capture_running_image(process.pid, args.solver_sha256)
                verified = True
                if pidfd is not None:
                    identity = memory_lifecycle.bind_task_identity(process.pid, pidfd)
            threads = [threading.Thread(target=drain, args=(process.stdout, buffers[0])),
                       threading.Thread(target=drain, args=(process.stderr, buffers[1])),
                       threading.Thread(target=send)]
            for thread in threads:
                thread.start()
            deadline = time.monotonic() + timeout
            while process.poll() is None:
                if failure or time.monotonic() >= deadline:
                    with lock:
                        failure.append('timeout or I/O failure')
                    kill()
                    break
                if verified:
                    try:
                        rss, pss = resident_bytes(process.pid)
                        peak_rss, peak_pss = max(peak_rss, rss), max(peak_pss, pss or 0)
                        rss_samples += 1
                        if rss > args.max_bytes:
                            failure.append('fresh resident memory budget exceeded')
                            kill()
                            break
                    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
                        observation = memory_lifecycle.terminal_observation(
                            process, pidfd, identity, error, rss_samples, deadline)
                        memory_observations.append(observation)
                        if not observation['terminal_race']:
                            failure.append('fresh memory observation: ' + str(error))
                            kill()
                        break
                time.sleep(.01)
            process.wait(timeout=2)
            for thread in threads:
                thread.join(timeout=2)
            if any(thread.is_alive() for thread in threads):
                failure.append('I/O did not finish')
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            failure.append(str(error))
            kill()
            # Capture startup diagnostics even if native image binding failed.
            if not threads:
                out, err = process.communicate(timeout=2)
                buffers = [bytearray(out[:MAX_OUTPUT]), bytearray(err[:MAX_OUTPUT])]
        finally:
            kill()
            process.wait(timeout=2)
            for thread in threads:
                if thread.ident is not None:
                    thread.join(timeout=2)
            for stream in [process.stdin, process.stdout, process.stderr]:
                try:
                    stream.close()
                except (OSError, ValueError) as error:
                    failure.append('stream close: ' + str(error))
            with fd_lock:
                for fd in [image, pidfd]:
                    if fd is not None:
                        os.close(fd)
                image = pidfd = None
        result = dict(stdout=bytes(buffers[0]), stderr=bytes(buffers[1]), exit=process.returncode,
                      error='; '.join(failure) if failure else None, image_verified=verified,
                      sampled_peak_rss=peak_rss, sampled_peak_pss=peak_pss,
                      rss_samples=rss_samples, memory_observations=memory_observations)
        write_streams(name, result)
        return result

    try:
        prefix, expected, suffix = args.prefix.read_bytes(), args.prefix_stdout.read_bytes(), args.suffix.read_bytes()
        data = prefix + suffix
        if len(data) > args.max_input_bytes or len(expected) > MAX_OUTPUT:
            raise ValueError('input/expected-output budget exceeded')
        manifest.update(input_sha256=hashlib.sha256(data).hexdigest(), prefix_sha256=hashlib.sha256(prefix).hexdigest(),
                        expected_prefix_stdout_sha256=hashlib.sha256(expected).hexdigest())
        for name, value in [('prefix.smt2', prefix), ('prefix.stdout', expected), ('suffix.smt2', suffix)]:
            (args.out / name).write_bytes(value)
        if args.fresh_only:
            manifest.update(mode='fresh-only', candidate_gate_pass=False, fresh_runs=args.fresh_runs)
            records = []
            first = None
            for index in range(args.fresh_runs):
                manifest['active_fresh_sample'] = index
                started, before_cpu = time.monotonic(), cpu_children()
                result = fresh('fresh-' + str(index), args.oracle_timeout, data)
                equal = first is None or all(result[key] == first[key] for key in ['stdout', 'stderr', 'exit'])
                record = dict(index=index, stdout_sha256=hashlib.sha256(result['stdout']).hexdigest(),
                              stderr_sha256=hashlib.sha256(result['stderr']).hexdigest(),
                              stdout_bytes=len(result['stdout']), stderr_bytes=len(result['stderr']),
                              exit=result['exit'], error=result['error'], image_verified=result['image_verified'],
                              equal_to_first=equal, sampled_peak_rss=result['sampled_peak_rss'],
                              sampled_peak_pss=result['sampled_peak_pss'], rss_samples=result['rss_samples'],
                              memory_observations=result['memory_observations'])
                if args.native_metrics:
                    record.update(guarded_wall=time.monotonic() - started,
                                  solver_cpu=cpu_children() - before_cpu)
                if args.experiment_numeric_time_only:
                    comparison = response_policy.compare((first or result)["stdout"], result["stdout"])
                    record["experiment_comparison"] = comparison
                    record["experiment_equal_to_first"] = (comparison["equal_except_numeric_time"] and
                                                            result["stderr"] == (first or result)["stderr"] and
                                                            result["exit"] == (first or result)["exit"])
                records.append(record)
                manifest['fresh_samples'] = records
                if first is None:
                    first = result
            manifest.pop('active_fresh_sample', None)
            manifest['identity_stable'] = path_digest(z3) == args.solver_sha256
            manifest['raw_equal'] = all(row['equal_to_first'] and not row['error'] for row in records)
            manifest['fresh_control_pass'] = (manifest['raw_equal'] and manifest['identity_stable'] and
                                               all(row['image_verified'] and row['exit'] == 0 for row in records))
            manifest['experiment_fresh_control_pass'] = (args.experiment_numeric_time_only and
                manifest['identity_stable'] and all(row['experiment_equal_to_first'] and not row['error'] and
                row['image_verified'] and row['exit'] == 0 for row in records))
        else:
            children_before = cpu_children()
            preparation_started = time.monotonic()
            preparation_completed = attached = preparation_started
            warm_cpu = None
            result = None
            entry = None
            try:
                pool = OneUsePool(z3, args.solver_sha256, slots=1, max_bytes=args.max_bytes,
                                  ttl=args.ttl, run_timeout=args.run_timeout, max_input_bytes=args.max_input_bytes,
                                  experiment_numeric_time_only=args.experiment_numeric_time_only)
                key = pool.seed(prefix, expected)
                ready = pool.wait_ready(key, args.preparation_window)
                manifest.update(prefix_key=key, ready=ready)
                entry = pool.entries.get(key)
                if entry is not None:
                    manifest['prepared_pid'] = entry.pid
                preparation_completed = time.monotonic()
                warm_cpu = proc_cpu(entry.pid) if args.native_metrics and ready else None
                attached = time.monotonic()
                result = pool.consume(key, data) if ready else None
                manifest['refusal'] = entry.failure if entry else pool.last_refusal
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                preparation_completed = attached = time.monotonic()
                manifest['refusal'] = str(error)
            finally:
                if pool is not None:
                    pool.close()
            if entry is not None:
                write_streams('preparation-attempt', dict(stdout=bytes(entry.stdout), stderr=bytes(entry.stderr)))
                manifest['preparation_attempt'] = dict(claimed=entry.claimed, wrote_prefix=entry.written,
                                                       exit=entry.process.returncode, failure=entry.failure)
                if not manifest.get('ready') and not manifest.get('refusal'):
                    manifest['refusal'] = 'not ready at request time'
            prepared_cpu = cpu_children() - children_before
            fallback_cpu_start = cpu_children()
            if result is None:
                result = fresh('candidate', args.fallback_timeout, data)
                manifest['mode'] = 'fresh-fallback'
            else:
                result.update(error=None, image_verified=True)
                manifest.update(mode='one-use-live-prefix', serving_pid=result['pid'],
                                sampled_peak_rss=result['sampled_peak_rss'], sampled_peak_pss=result['sampled_peak_pss'])
                write_streams('candidate', result)
            candidate_completed = time.monotonic()
            fallback_cpu = cpu_children() - fallback_cpu_start
            reference_cpu_start, reference_started = cpu_children(), time.monotonic()
            reference = fresh('fresh', args.oracle_timeout, data)
            reference_completed = time.monotonic()
            manifest.update(candidate_exit=result['exit'], fresh_exit=reference['exit'],
                            candidate_error=result['error'], fresh_error=reference['error'],
                            stdout_equal=result['stdout'] == reference['stdout'], stderr_equal=result['stderr'] == reference['stderr'],
                            exit_equal=result['exit'] == reference['exit'],
                            native_images_verified=result['image_verified'] and reference['image_verified'],
                            identity_stable=path_digest(z3) == args.solver_sha256,
                            candidate_memory_observations=result.get('memory_observations', []),
                            fresh_memory_observations=reference['memory_observations'])
            manifest['raw_equal'] = (all(manifest[k] for k in ['stdout_equal', 'stderr_equal', 'exit_equal', 'identity_stable'])
                                     and not result['error'] and not reference['error'])
            manifest['candidate_gate_pass'] = (manifest['raw_equal'] and manifest['mode'] == 'one-use-live-prefix'
                                               and manifest['native_images_verified']
                                               and result['exit'] == 0 and reference['exit'] == 0)
            if args.experiment_numeric_time_only:
                comparison = response_policy.compare(result['stdout'], reference['stdout'])
                manifest['experiment_comparison'] = comparison
                manifest['experiment_candidate_gate_pass'] = (comparison['equal_except_numeric_time'] and
                    manifest['stderr_equal'] and manifest['exit_equal'] and manifest['identity_stable'] and
                    manifest['mode'] == 'one-use-live-prefix' and manifest['native_images_verified'] and
                    not result['error'] and not reference['error'] and result['exit'] == 0 and reference['exit'] == 0)
            if args.native_metrics:
                manifest['native_metrics'] = dict(background_and_serving_cpu=prepared_cpu,
                    candidate_total_solver_cpu=prepared_cpu + fallback_cpu, fallback_solver_cpu=fallback_cpu,
                    prefix_cpu_observed_at_claim=warm_cpu, preparation_wall=preparation_completed - preparation_started,
                    foreground_wall_including_fallback=candidate_completed - attached,
                    guarded_fresh_reference_wall=reference_completed - reference_started,
                    fresh_reference_cpu=cpu_children() - reference_cpu_start,
                    scope='one guarded sample; real background CPU and validation overhead are included')
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        manifest.update(error=str(error), candidate_gate_pass=False, experiment_candidate_gate_pass=False,
                        experiment_fresh_control_pass=False)
    finally:
        try:
            if pool is not None:
                pool.close()
        except BaseException as error:
            manifest.setdefault('cleanup_errors', []).append(type(error).__name__ + ': ' + str(error))
            manifest.update(candidate_gate_pass=False, fresh_control_pass=False,
                            experiment_candidate_gate_pass=False, experiment_fresh_control_pass=False)
        finally:
            (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps(manifest))
    return 0 if any(manifest.get(key, False) for key in ['candidate_gate_pass', 'fresh_control_pass',
                'experiment_candidate_gate_pass', 'experiment_fresh_control_pass']) else 1


if __name__ == '__main__':
    raise SystemExit(main())
