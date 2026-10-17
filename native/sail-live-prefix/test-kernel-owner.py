"""Real owned children/accounting/I/O; Linux image/pidfd guards doubled on Mac.

No Z3, native image admission or performance qualification is supplied here.
"""
import contextlib
import hashlib
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import kernel_accounting
import live_prefix
import owned_wait4


class Controls(unittest.TestCase):
    def owner(self, code):
        return owned_wait4.OwnedWait4([sys.executable, '-B', '-c', code], deferred=True,
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def finish(self, owner):
        owner.cancel_unstarted()
        if owner.poll() is None:
            os.kill(owner.pid, signal.SIGKILL)
        owner.wait(3)
        for stream in (owner.stdin, owner.stdout, owner.stderr):
            stream.close()

    @contextlib.contextmanager
    def native_doubles(self, capture_error=False, ready=None, after_capture=None):
        descriptors = {}
        def capture(pid, identity):
            if ready is not None:
                import time
                deadline=time.monotonic()+3
                while not ready.exists():
                    if time.monotonic()>deadline:raise OSError('fixture startup deadline')
                    time.sleep(.005)
            if capture_error:
                raise OSError('injected actual-image refusal')
            first, second = os.open(os.devnull, os.O_RDONLY), os.open(os.devnull, os.O_RDONLY)
            descriptors[first] = pid
            if after_capture is not None:after_capture()
            return first, second, None
        def send(fd, sig):
            os.kill(descriptors[fd], sig)
        with patch.object(live_prefix, 'capture_running_image', capture), patch.object(
                kernel_accounting, 'capture_running_image', capture), patch.object(
                live_prefix.Entry, '_image_ok', lambda _: True), patch.object(
                os, 'pidfd_open', lambda *_: None, create=True), patch.object(
                signal, 'pidfd_send_signal', send, create=True):
            yield

    def test_owner_is_assigned_before_any_reap_and_can_cancel(self):
        owner = self.owner('import sys; print("started",flush=True);sys.stdin.buffer.read()')
        try:
            self.assertEqual(owner.stdout.readline(), b'started\n')
            self.assertFalse(owner._reaping_started)
            self.assertIsNone(owner.poll())
            with self.assertRaises(ChildProcessError):
                owner.wait(.01)
            owner.cancel_unstarted()
            self.assertEqual(owner.wait(2), -signal.SIGKILL)
            self.assertIsNotNone(owner.accounting())
            with self.assertRaises(ChildProcessError):
                os.wait4(owner.pid, os.WNOHANG)
        finally:
            self.finish(owner)

    def test_binding_failure_retains_startup_streams_and_actual_status(self):
        with tempfile.TemporaryDirectory() as temp:
            worker = Path(temp) / 'worker'
            ready = Path(temp) / 'ready'
            worker.write_text('#!' + sys.executable + '\nimport os,time,pathlib\nos.write(1,b"startup-out");os.write(2,b"startup-err");pathlib.Path(' + repr(str(ready)) + ').touch();time.sleep(20)\n')
            worker.chmod(0o700)
            def refuse(pid, _):
                # Synchronize only this failure fixture so actual startup bytes
                # exist before the intentionally injected image refusal.
                import time
                deadline=time.monotonic()+3
                while not ready.exists():
                    if time.monotonic()>deadline:raise OSError('fixture startup deadline')
                    time.sleep(.005)
                raise OSError('injected actual-image refusal')
            with patch.object(kernel_accounting, 'capture_running_image', refuse):
                result = kernel_accounting.fresh(str(worker), 'unused', b'', 1, 1024**3)
            self.assertIn('injected actual-image refusal', result['error'])
            self.assertEqual(result['stdout'], b'startup-out')
            self.assertEqual(result['stderr'], b'startup-err')
            self.assertEqual(result['exit'], -signal.SIGKILL)
            self.assertFalse(result['kernel_resource_gate_pass'])

    def test_reaper_start_failure_keeps_owner_and_streams(self):
        owner = self.owner('import sys; print("out",flush=True);print("err",file=sys.stderr,flush=True);sys.stdin.buffer.read()')
        try:
            self.assertEqual(owner.stdout.readline(), b'out\n')
            with patch.object(threading.Thread, 'start', side_effect=RuntimeError('injected start failure')):
                with self.assertRaises(RuntimeError):
                    owner.start_reaper()
            self.assertEqual(owner.wait(2), -signal.SIGKILL)
            self.assertEqual(owner.stderr.read(), b'err\n')
            self.assertFalse(owner.stderr.closed)
        finally:
            self.finish(owner)

    def test_positive_accounting_requires_positive_bounded_peak_and_zero_exit(self):
        owner = self.owner('pass')
        try:
            owner.start_reaper(); owner.wait(3)
            record = owned_wait4.positive_accounting(owner, 1024**3)
            self.assertGreater(record['accounting']['peak_rss_bytes'], 0)
            with self.assertRaises(ValueError):
                owned_wait4.positive_accounting(owner, 1)
            original = owner._result['accounting']['peak_rss_bytes']
            owner._result['accounting']['peak_rss_bytes'] = 0
            with self.assertRaises(ValueError):
                owned_wait4.positive_accounting(owner, 1024**3)
            owner._result['accounting']['peak_rss_bytes'] = original
            owner._result['exit'] = 7
            with self.assertRaises(ValueError):
                owned_wait4.positive_accounting(owner, 1024**3)
            owner._result['exit'] = 0
        finally:
            self.finish(owner)

    def run_fresh(self, code, data=b'original\x00bytes\xff', timeout=2, maximum=1024**3, after_capture=None):
        with tempfile.TemporaryDirectory() as temp:
            worker = Path(temp) / 'worker'
            ready = Path(temp) / 'ready'
            worker.write_text('#!' + sys.executable + '\nimport pathlib;pathlib.Path(' + repr(str(ready)) + ').touch()\n' + code)
            worker.chmod(0o700)
            with self.native_doubles(ready=ready,after_capture=after_capture):
                return kernel_accounting.fresh(str(worker), 'image-doubled', data, timeout, maximum)

    def test_fresh_complete_raw_io_and_actual_cpu(self):
        r = self.run_fresh('import sys\nd=sys.stdin.buffer.read();sys.stdout.buffer.write(d);sys.stderr.buffer.write(b"err")\n')
        self.assertIsNone(r['error'])
        self.assertEqual(r['stdout'], b'original\x00bytes\xff')
        self.assertEqual(r['stderr'], b'err')
        self.assertEqual(r['input_bytes_written'], 15)
        self.assertTrue(r['kernel_resource_gate_pass'])
        self.assertGreaterEqual(kernel_accounting.cpu(r['kernel_accounting']), 0)
        self.assertIsNone(r['sampled_peak_rss'])

    def test_fresh_nonzero_preserves_raw_and_refuses(self):
        r = self.run_fresh('import sys\nsys.stdin.buffer.read();print("raw",flush=True);print("bad",file=sys.stderr,flush=True);sys.exit(7)\n')
        self.assertEqual(r['exit'], 7)
        self.assertEqual(r['stdout'], b'raw\n')
        self.assertEqual(r['stderr'], b'bad\n')
        self.assertFalse(r['kernel_resource_gate_pass'])

    def test_fresh_timeout_preserves_raw_and_actual_killed_status(self):
        r = self.run_fresh('import sys,time\nsys.stdin.buffer.read();print("raw",flush=True);print("slow",file=sys.stderr,flush=True);time.sleep(20)\n', timeout=.15)
        self.assertEqual(r['exit'], -signal.SIGKILL)
        self.assertEqual(r['stdout'], b'raw\n')
        self.assertEqual(r['stderr'], b'slow\n')
        self.assertFalse(r['kernel_resource_gate_pass'])
        self.assertIn('TimeoutExpired', r['error'])

    def test_fresh_real_peak_over_bound_refuses(self):
        r = self.run_fresh('import sys\nsys.stdin.buffer.read();print("raw",flush=True)\n', maximum=1)
        self.assertEqual(r['exit'], 0)
        self.assertGreater(r['kernel_accounting']['accounting']['peak_rss_bytes'], 1)
        self.assertFalse(r['kernel_resource_gate_pass'])

    def test_pool_actual_prefix_then_suffix_complete_accounting(self):
        with tempfile.TemporaryDirectory() as temp:
            worker = Path(temp) / 'worker'
            worker.write_text('#!' + sys.executable + '\nimport os,sys\nline=sys.stdin.buffer.readline();os.write(1,b"unsat\\n");rest=sys.stdin.buffer.read();os.write(1,b"sat\\n")\n')
            worker.chmod(0o700)
            identity = hashlib.sha256(worker.read_bytes()).hexdigest()
            with self.native_doubles(), patch.object(live_prefix.os, 'uname', lambda: os.uname_result(('Linux','','','','x86_64'))):
                pool = live_prefix.OneUsePool(str(worker), identity, max_bytes=1024**3,
                    ttl=3, run_timeout=2, experiment_numeric_time_only=True,
                    experiment_kernel_accounting_owner=True)
                try:
                    key = pool.seed(b'prefix\n', b'unsat\n')
                    self.assertTrue(pool.wait_ready(key, 2))
                    result = pool.consume(key, b'prefix\nsuffix\n')
                    self.assertIsNotNone(result)
                    self.assertEqual(result['stdout'], b'unsat\nsat\n')
                    self.assertEqual(result['exit'], 0)
                    self.assertGreater(result['kernel_accounting']['accounting']['peak_rss_bytes'], 0)
                    self.assertIsNone(result['sampled_peak_rss'])
                    self.assertIsNone(pool.consume(key, b'prefix\nsuffix\n'))
                finally:
                    pool.close()

    def test_actual_raw_io_then_injected_read_failure_is_not_success(self):
        original=os.read
        injected=[]
        def read(fd,maximum):
            block=original(fd,maximum)
            if not block and not injected:
                injected.append(fd)
                raise OSError('injected EOF observation error')
            return block
        def install():kernel_accounting.os.read=read
        try:
            r=self.run_fresh('import sys\nsys.stdin.buffer.read();print("raw",flush=True);print("err",file=sys.stderr,flush=True)\n',after_capture=install)
        finally:kernel_accounting.os.read=original
        self.assertTrue(injected)
        self.assertEqual(r['exit'],0)
        self.assertEqual(r['stdout'],b'raw\n')
        self.assertEqual(r['stderr'],b'err\n')
        self.assertIn('injected EOF observation error',r['error'])
        self.assertFalse(r['kernel_resource_gate_pass'])

    def test_held_descriptor_close_failure_refuses_even_after_complete_exit(self):
        original=os.close
        fail=[]
        def close(fd):
            if not fail:
                fail.append(fd)
                # Model EBADF with an actually closed FD, so the fixture itself
                # never leaks. Production reports the error and refuses; it
                # does not retry an uncertain FD number that might be reused.
                original(fd)
                raise OSError('injected held descriptor close refusal')
            original(fd)
        # Patch at the completion boundary, not Popen's internal fd closure.
        positive=kernel_accounting.positive_accounting
        def admit(owner,maximum):
            value=positive(owner,maximum)
            kernel_accounting.os.close=close
            return value
        try:
            with patch.object(kernel_accounting,'positive_accounting',admit):
                r=self.run_fresh('import sys\nsys.stdin.buffer.read();print("raw",flush=True)\n')
        finally:
            kernel_accounting.os.close=original
        self.assertEqual(r['exit'],0)
        self.assertEqual(r['stdout'],b'raw\n')
        self.assertIn('held descriptor close',r['error'])
        self.assertFalse(r['kernel_resource_gate_pass'])

    def test_pool_binding_refusal_retains_owned_failed_startup(self):
        with tempfile.TemporaryDirectory() as temp:
            worker=Path(temp)/'worker'
            worker.write_text('#!'+sys.executable+'\nimport sys,time\nprint("start",flush=True);print("err",file=sys.stderr,flush=True);time.sleep(20)\n')
            worker.chmod(0o700)
            identity=hashlib.sha256(worker.read_bytes()).hexdigest()
            with self.native_doubles(capture_error=True), patch.object(live_prefix.os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))):
                pool=live_prefix.OneUsePool(str(worker),identity,experiment_numeric_time_only=True,experiment_kernel_accounting_owner=True)
                self.assertIsNone(pool.seed(b'prefix\n',b'unsat\n'))
                entry=pool.last_attempt
                self.assertIsNotNone(entry)
                self.assertTrue(entry.kernel_closed)
                self.assertEqual(entry.kernel_accounting['exit'],-signal.SIGKILL)
                self.assertIsNotNone(entry.process.raw_observation())
                with self.assertRaises(ChildProcessError):os.wait4(entry.pid,os.WNOHANG)
                pool.close()

    def test_pool_thread_start_refusal_has_no_owned_children_or_fds(self):
        # Fail every start position: sole reaper then writer/readers/monitor.
        # The production owner/process paths execute; only Thread.start is
        # deliberately unavailable once and native image admission is doubled.
        for position in range(1,6):
            with self.subTest(position=position),tempfile.TemporaryDirectory() as temp:
                worker=Path(temp)/'worker'
                worker.write_text('#!'+sys.executable+'\nimport sys\nsys.stdin.buffer.read()\n');worker.chmod(0o700)
                identity=hashlib.sha256(worker.read_bytes()).hexdigest()
                original=threading.Thread.start;calls=[]
                def start(thread):
                    calls.append(thread)
                    if len(calls)==position:raise RuntimeError('injected worker start failure')
                    return original(thread)
                with self.native_doubles(),patch.object(live_prefix.os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))),patch.object(threading.Thread,'start',start):
                    pool=live_prefix.OneUsePool(str(worker),identity,experiment_numeric_time_only=True,experiment_kernel_accounting_owner=True)
                    self.assertIsNone(pool.seed(b'prefix\n',b'unsat\n'))
                    entry=pool.last_attempt
                    self.assertEqual(entry.kernel_accounting['exit'],-signal.SIGKILL)
                    self.assertTrue(entry.kernel_closed)
                    self.assertIsNone(entry.image_fd);self.assertIsNone(entry.pidfd)
                    self.assertTrue(all(stream.closed for stream in (entry.process.stdin,entry.process.stdout,entry.process.stderr)))
                    self.assertTrue(all(t.ident is None or not t.is_alive() for t in entry.threads))
                    with self.assertRaises(ChildProcessError):os.wait4(entry.pid,os.WNOHANG)
                    pool.close()

    def test_real_driver_kernel_policy_and_actual_record_cost(self):
        for case in ('success','nonzero','over-limit','fresh-only','image-refusal','cleanup-failed'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                root=Path(temp);worker=root/'worker'
                worker.write_text('#!'+sys.executable+'\nimport sys\nsys.stdin.buffer.readline();print("unsat",flush=True);sys.stdin.buffer.read();print("sat",flush=True);sys.exit('+('7' if case=='nonzero' else '0')+')\n')
                worker.chmod(0o700)
                for name,data in [('prefix',b'prefix\n'),('suffix',b'suffix\n'),('expected',b'unsat\n')]: (root/name).write_bytes(data)
                spec=importlib.util.spec_from_file_location('kernel_driver',Path(__file__).with_name('run.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
                argv=['run.py','--z3',str(worker),'--solver-sha256',hashlib.sha256(worker.read_bytes()).hexdigest(),'--prefix',str(root/'prefix'),'--suffix',str(root/'suffix'),'--prefix-stdout',str(root/'expected'),'--out',str(root/'out'),'--native-metrics','--experiment-numeric-time-only','--experiment-kernel-accounting-owner','--preparation-window','2','--ttl','3','--run-timeout','2','--oracle-timeout','2','--fallback-timeout','2','--max-bytes',str(1 if case=='over-limit' else 1024**3)]
                if case=='fresh-only':argv+=['--fresh-only','--fresh-runs','3']
                import io,json
                original_close=live_prefix.OneUsePool.close
                closes=[]
                def close(pool):
                    original_close(pool);closes.append(True)
                    if case=='cleanup-failed' and len(closes)==1:raise OSError('injected completion cleanup refusal')
                with patch.object(live_prefix.OneUsePool,'close',close),self.native_doubles(capture_error=case=='image-refusal'),patch.object(live_prefix.os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))),patch.object(sys,'argv',argv),patch.object(module,'proc_cpu',side_effect=AssertionError('no proc CPU in kernel policy')),patch.object(live_prefix,'resident_bytes',side_effect=AssertionError('no sampled memory in kernel policy')),contextlib.redirect_stdout(io.StringIO()):
                    code=module.main()
                m=json.loads((root/'out/manifest.json').read_text())
                self.assertEqual(code,0 if case in ('success','fresh-only') else 1,m)
                self.assertEqual(m['resource_policy'],kernel_accounting.EXPERIMENT)
                if case=='cleanup-failed':
                    self.assertIn('injected completion cleanup refusal',m['error'])
                    self.assertFalse(m['experiment_candidate_gate_pass'])
                    self.assertEqual((root/'out/preparation-attempt.stdout').read_bytes(),b'unsat\nsat\n')
                    self.assertEqual(m['preparation_attempt']['exit'],0)
                    self.assertIsNotNone(m['preparation_attempt']['kernel_accounting'])
                    self.assertFalse((root/'out/fresh.stdout').exists())
                elif case=='fresh-only':
                    self.assertEqual(len(m['fresh_samples']),3)
                    self.assertTrue(all(r['kernel_resource_gate_pass'] for r in m['fresh_samples']))
                else:
                    self.assertEqual(m['experiment_candidate_gate_pass'],case=='success')
                    records=m['kernel_resource_records']
                    if case!='image-refusal':
                        expected=sum(kernel_accounting.cpu(records[k]) for k in ('prepared','fallback') if records[k] is not None)
                        self.assertEqual(m['native_metrics']['candidate_total_solver_cpu'],expected)
                        self.assertIsNotNone(m['preparation_attempt']['kernel_accounting'])
                    self.assertEqual((root/'out/fresh.stdout').read_bytes(),b'unsat\nsat\n' if case!='image-refusal' else b'')
                    if case=='success':
                        self.assertEqual(m['prepared_pid'],m['serving_pid'])
                        self.assertIsNone(m['native_metrics']['prefix_cpu_observed_at_claim'])
                        self.assertEqual(m['native_metrics']['fallback_solver_cpu'],0)

    def test_default_standalone_source_copy_needs_no_new_resource_module(self):
        import json,shutil
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            for name in ('run.py','live_prefix.py','response_policy.py','memory_lifecycle.py'):
                shutil.copyfile(Path(__file__).with_name(name),root/name)
            worker=root/'worker';worker.write_text('#!'+sys.executable+'\nimport sys\nsys.stdin.buffer.read();print("unsat")\n');worker.chmod(0o700)
            for name,value in [('prefix',b'prefix\n'),('expected',b''),('suffix',b'suffix\n')]: (root/name).write_bytes(value)
            launcher="import os,runpy,sys;os.uname=lambda:os.uname_result(('Darwin','','','','arm64'));p=sys.argv.pop(1);sys.argv[0]=p;runpy.run_path(p,run_name='__main__')"
            p=subprocess.run([sys.executable,'-B','-c',launcher,str(root/'run.py'),'--z3',str(worker),'--solver-sha256',hashlib.sha256(worker.read_bytes()).hexdigest(),'--prefix',str(root/'prefix'),'--prefix-stdout',str(root/'expected'),'--suffix',str(root/'suffix'),'--fresh-only','--fresh-runs','2','--out',str(root/'out')],capture_output=True,timeout=10)
            m=json.loads((root/'out/manifest.json').read_text())
            # Explicit Darwin/fresh-only guard double makes this source-copy
            # compatibility check portable; no native image admission is given.
            # Actual owned execution and literal outputs must still work.
            self.assertNotIn('error',m,(p.stdout,p.stderr))
            self.assertNotIn('resource_policy',m)
            self.assertEqual(len(m['fresh_samples']),2)
            self.assertTrue(all(r['exit']==0 and not r['error'] for r in m['fresh_samples']))
            self.assertEqual((root/'out/fresh-0.stdout').read_bytes(),b'unsat\n')

    def test_error_cleanup_requires_readable_terminal_events(self):
        # Event-policy doubles only. A real closed local descriptor supplies
        # the invalid-fd negative; actual Linux P_PIDFD is tested separately.
        import select
        from types import SimpleNamespace
        read,write=os.pipe()
        try:
            for name,events,wanted in [('readable',[(read,select.POLLIN)],True),('readable-hup',[(read,select.POLLIN|select.POLLHUP)],True),('hup-only',[(read,select.POLLHUP)],False),('invalid',[(read,select.POLLNVAL)],False),('error',[(read,select.POLLERR)],False),('readable-error',[(read,select.POLLIN|select.POLLERR)],False),('writable',[(read,select.POLLOUT)],False),('empty',[],False),('wrong-fd',[(write,select.POLLIN)],False),('duplicate',[(read,select.POLLIN),(read,select.POLLIN)],False)]:
                with self.subTest(name=name):
                    owner=owned_wait4.OwnedWait4.__new__(owned_wait4.OwnedWait4)
                    owner._reaping_started=True;owner._done=threading.Event();owner._done.set();owner._error=ChildProcessError('fixture missing accounting');owner._reaper=SimpleNamespace(ident=None);owner.pid=17;owner._cleanup_observed=None
                    class Poll:
                        def register(self,fd,mask):self.fd=fd
                        def poll(self,timeout=0):return events
                    with patch.object(os,'P_PIDFD',4242,create=True),patch.object(os,'waitid',side_effect=ChildProcessError('fixture ECHILD'),create=True),patch.object(signal,'pidfd_send_signal',return_value=None,create=True),patch.object(select,'poll',Poll,create=True):
                        if wanted:record=owner.cleanup_failed_accounting(read)
                        else:
                            with self.assertRaises(ChildProcessError):owner.cleanup_failed_accounting(read)
                            record=owner.cleanup_observation()
                    self.assertEqual(record['terminal_pidfd'],wanted)
                    self.assertEqual(record['terminal_poll_events'],[list(x) for x in events])
                    self.assertFalse(record['accounting_available'])
                    self.assertNotIn('actual_exit',record)
            os.close(read)
            owner._cleanup_observed=None
            # Actual invalid FD cannot become terminal through NVAL.
            with patch.object(os,'P_PIDFD',4242,create=True),patch.object(os,'waitid',side_effect=ChildProcessError('fixture ECHILD'),create=True),patch.object(signal,'pidfd_send_signal',side_effect=OSError('actual closed descriptor'),create=True),patch.object(select,'poll',Poll,create=True):
                events=[(read,select.POLLNVAL)]
                with self.assertRaises(ChildProcessError):owner.cleanup_failed_accounting(read)
            self.assertFalse(owner.cleanup_observation()['terminal_pidfd'])
        finally:
            try:os.close(read)
            except OSError:pass
            os.close(write)

    def test_default_key_does_not_gain_resource_label(self):
        with tempfile.TemporaryDirectory() as temp:
            worker = Path(temp) / 'worker';worker.write_bytes(b'not executed');worker.chmod(0o700)
            identity=hashlib.sha256(worker.read_bytes()).hexdigest()
            with patch.object(live_prefix.os, 'uname', lambda: os.uname_result(('Linux','','','','x86_64'))), patch.object(os,'pidfd_open',lambda *_:None,create=True), patch.object(signal,'pidfd_send_signal',lambda *_:None,create=True):
                for flag in (False, True):
                    pool=live_prefix.OneUsePool(str(worker),identity,experiment_numeric_time_only=True,experiment_kernel_accounting_owner=flag)
                    self.assertEqual('resource_experiment' in pool.binding,flag)
                    pool.close()


if __name__ == '__main__':
    unittest.main()
