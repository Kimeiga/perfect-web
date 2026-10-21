"""The canonical guide launcher owns its group and preserves native receipts."""
import json
import os
from pathlib import Path
import signal
import select
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import guide_python
import producer_process
import mutation_source


def running(pid):
    try:
        state = Path(f'/proc/{pid}/stat').read_text().split(') ', 1)[1].split()[0]
        return state not in ('Z', 'X')
    except (FileNotFoundError, ProcessLookupError):
        return False


def stop_owned(record):
    """Emergency fixture cleanup never signals a reused numeric PID."""
    try:
        fd = os.pidfd_open(record['pid'])
    except ProcessLookupError:
        return
    try:
        try:
            start = Path(f"/proc/{record['pid']}/stat").read_text().split(') ', 1)[1].split()[19]
        except (FileNotFoundError, ProcessLookupError):
            return
        if start == record['start']:
            try: signal.pidfd_send_signal(fd, signal.SIGKILL)
            except ProcessLookupError: pass
            stopped = select.poll()
            stopped.register(fd, select.POLLIN)
            if not stopped.poll(5000):
                raise RuntimeError('owned fixture did not terminate after emergency cleanup')
    finally:
        os.close(fd)


def pid_writer(path):
    """Native fixture code publishes a complete identity before readiness."""
    temporary = path.with_suffix('.tmp')
    return (f"Path({str(temporary)!r}).write_text(json.dumps({{'pid':os.getpid(),"
            "'start':Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19]})); "
            f"Path({str(temporary)!r}).replace({str(path)!r})")


# This is fixture setup, before the production owner's execution deadline.
# A failed constructor still owns its native process until it kills/reaps it.
READY_LAUNCH = r"""
class ReadyNative(subprocess.Popen):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        try:
            temporary = FIXTURE_LAUNCHED.with_suffix('.tmp')
            temporary.write_text(json.dumps({'pid':self.pid, 'start':Path(f'/proc/{self.pid}/stat').read_text().split(') ',1)[1].split()[19]}))
            temporary.replace(FIXTURE_LAUNCHED)
            end = time.monotonic() + FIXTURE_STARTUP_BOUND
            while not FIXTURE_READY.exists() and time.monotonic() < end:
                time.sleep(.01)
            if not FIXTURE_READY.exists():
                raise RuntimeError('fixture startup did not become ready')
        except BaseException as startup_error:
            # This constructor still owns the unreaped child; the helper
            # under mutation has not received it. Cleanup is native here.
            owns_leader = True
            try:
                try:
                    try: os.killpg(self.pid, signal.SIGTERM)
                    except ProcessLookupError: pass
                    end = time.monotonic() + 2
                    while time.monotonic() < end:
                        try:
                            done = os.waitid(os.P_PID,self.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT)
                        except ChildProcessError:
                            owns_leader = False
                            raise
                        if done is not None: break
                        time.sleep(.01)
                finally:
                    if owns_leader:
                        try: os.killpg(self.pid, signal.SIGKILL)
                        except ProcessLookupError: pass
                    super().communicate()
            except BaseException as cleanup_error:
                raise startup_error from cleanup_error
            raise
"""


@unittest.skipUnless(
    sys.platform.startswith('linux') and Path('/proc/self/stat').is_file()
    and all(hasattr(os, name) for name in ('waitid', 'P_PID', 'WEXITED', 'WNOHANG', 'WNOWAIT', 'pidfd_open'))
    and hasattr(signal, 'pidfd_send_signal'),
    'Linux /proc, waitid and pidfd native process-group witnesses')
class GuideOwnership(unittest.TestCase):
    def probe(self, phase, ignores_term=False, startup_delay=0, startup_bound=5, startup_fails=False):
        with tempfile.TemporaryDirectory() as directory:
            tmp = Path(directory)
            source, pidfile, receipt = (tmp / name for name in ('source', 'pids', 'restored'))
            source.write_text('original')
            evidence = tmp / 'evidence'; evidence.mkdir()
            grandchild = "import signal,time; from pathlib import Path; " + ("signal.signal(signal.SIGTERM,signal.SIG_IGN); " if ignores_term else "") + "time.sleep(" + repr(startup_delay) + "); Path(" + repr(str(tmp/'grandchild-ready')) + ").touch(); time.sleep(60)"
            child = "\n".join([
                'import json,os,subprocess,sys,time', 'from pathlib import Path',
                "grandchild=subprocess.Popen([sys.executable,'-c',"+repr(grandchild)+"])",
                "Path("+repr(str(tmp/'spawned.tmp'))+").write_text(json.dumps([{'pid':pid,'start':Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split()[19]} for pid in [os.getpid(),grandchild.pid]]))",
                "Path("+repr(str(tmp/'spawned.tmp'))+").replace("+repr(str(tmp/'spawned'))+")",
                "while not Path("+repr(str(tmp/'grandchild-ready'))+").exists(): time.sleep(.005)",
                "print('owned stdout',flush=True)", "print('owned stderr',file=sys.stderr,flush=True)",
                'Path(' + repr(str(pidfile.with_suffix('.tmp'))) + ').write_text(json.dumps([os.getpid(),grandchild.pid]))',
                'Path(' + repr(str(pidfile.with_suffix('.tmp'))) + ').replace(' + repr(str(pidfile)) + ')',
                'time.sleep(60)'])
            program = r"""
import json,os,signal,subprocess,sys,time
from pathlib import Path
sys.path.insert(0,SCRIPTS)
import guide_python,producer_process
source,pidfile,receipt,evidence=map(Path,PATHS)
FIXTURE_READY=pidfile
FIXTURE_LAUNCHED=source.parent/"native-pid"
READINESS_HELPER
signal.signal(signal.SIGTERM, lambda *_: sys.exit(37))
class Injected(ReadyNative):
    def __init__(self,*args,**kwargs):
        super().__init__(*args,**kwargs)
        if PHASE=='launch':
            os.kill(os.getpid(),signal.SIGTERM)
subprocess.Popen=Injected
def running(pid):
    try: return Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split()[0] not in ('Z','X')
    except (FileNotFoundError, ProcessLookupError): return False
source.write_text('mutant')
try:
    status,output=guide_python._launch(evidence,[sys.executable,'-c',CHILD],[],.3 if PHASE=='timeout' else 60)
    (evidence/'return.json').write_text(json.dumps({'status':status,'output':output}))
finally:
    try:
        if pidfile.exists():
            pids=json.loads(pidfile.read_text())
            end=time.monotonic()+2
            while any(running(pid) for pid in pids) and time.monotonic()<end: time.sleep(.01)
            # Reaping the direct child is stricter than observing its death.
            try: os.kill(pids[0],0); leader_exists=True
            except ProcessLookupError: leader_exists=False
            receipt.write_text(json.dumps({'running':[running(pid) for pid in pids],
                                          'leader_exists':leader_exists}))
    finally:
        source.write_text('original')
"""
            settings = '\n'.join(f'{name}={value!r}' for name, value in [
                ('SCRIPTS',str(ROOT/'scripts')), ('PATHS',list(map(str,(source,pidfile,receipt,evidence)))),
                ('CHILD',child), ('PHASE',phase), ('FIXTURE_STARTUP_BOUND',startup_bound)])
            program=program.replace('READINESS_HELPER',READY_LAUNCH)
            wrapper=subprocess.Popen([sys.executable,'-c',settings+'\n'+program],
                stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
            pids=[]
            try:
                end=time.monotonic()+startup_bound+3
                while not pidfile.exists() and wrapper.poll() is None and time.monotonic()<end:
                    time.sleep(.01)
                if startup_fails:
                    output,_=wrapper.communicate(timeout=8)
                    self.assertEqual(wrapper.returncode,1,output)
                    self.assertIn('fixture startup did not become ready',output)
                    self.assertEqual(source.read_text(),'original',output)
                    self.assertFalse((evidence/'process.json').exists(),output)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(json.loads((tmp/'native-pid').read_text())['pid'],0)
                    if (tmp/'spawned').exists():
                        self.assertFalse(any(running(record['pid']) for record in json.loads((tmp/'spawned').read_text())),output)
                    return
                self.assertTrue(pidfile.exists(),'owned child/grandchild did not launch')
                pids=json.loads(pidfile.read_text())
                if phase=='running': wrapper.send_signal(signal.SIGTERM)
                output,_=wrapper.communicate(timeout=8)
                self.assertEqual(wrapper.returncode,0 if phase=='timeout' else 37,output)
                self.assertTrue(receipt.exists(),'source restoration was skipped: '+output)
                self.assertEqual(json.loads(receipt.read_text()),
                                 {'running':[False,False],'leader_exists':False},output)
                self.assertEqual(source.read_text(),'original')
                self.assertTrue((evidence/'process.json').exists(),'ownership receipt is missing')
                metadata=json.loads((evidence/'process.json').read_text())
                self.assertEqual(metadata['actual_status'],-signal.SIGTERM)
                self.assertIsNone(metadata['validated_status'])
                self.assertTrue(metadata['child_reaped'])
                self.assertEqual(metadata['timed_out'],phase=='timeout')
                self.assertEqual(metadata['cancelled'],phase!='timeout')
                self.assertEqual((evidence/'stdout.txt').read_text(),'owned stdout\n')
                self.assertEqual((evidence/'stderr.txt').read_text(),'owned stderr\n')
            finally:
                if wrapper.poll() is None:
                    wrapper.terminate()
                    try: wrapper.communicate(timeout=8)
                    except subprocess.TimeoutExpired: wrapper.kill(); wrapper.communicate()
                else: wrapper.communicate()
                if (tmp/'native-pid').exists(): stop_owned(json.loads((tmp/'native-pid').read_text()))
                if (tmp/'spawned').exists():
                    for record in json.loads((tmp/'spawned').read_text()): stop_owned(record)

    def test_launch_cancellation_reaps_before_restoration_and_stops_its_group(self):
        self.probe('launch')

    def test_running_cancellation_reaps_before_restoration_and_stops_its_group(self):
        self.probe('running')

    def test_timeout_stops_same_group_grandchild_and_cannot_validate(self):
        self.probe('timeout')
        self.probe('timeout',ignores_term=True,startup_delay=.6,startup_bound=.2,startup_fails=True)

    def test_native_files_status_and_clean_import_environment_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            tmp=Path(directory)
            command=[sys.executable,'-c',"import json,os,sys; print(json.dumps({k:os.environ.get(k) for k in ['KIOKUN_DATA','KIOKUN_APP','PYTHONPATH','PYTHONPYCACHEPREFIX','PYTHONDONTWRITEBYTECODE']})); print('separate stderr',file=sys.stderr); sys.exit(7)"]
            status,output=guide_python._launch(tmp,command,[tmp],5)
            self.assertEqual(status,7)
            self.assertEqual((tmp/'stderr.txt').read_text(),'separate stderr\n')
            native=json.loads((tmp/'stdout.txt').read_text())
            self.assertIsNone(native['KIOKUN_DATA']); self.assertIsNone(native['KIOKUN_APP'])
            self.assertEqual(native['PYTHONPATH'],os.pathsep.join(map(str,[tmp,ROOT])))
            self.assertEqual(native['PYTHONPYCACHEPREFIX'],str(tmp/'fresh-bytecode'))
            self.assertEqual(native['PYTHONDONTWRITEBYTECODE'],'1')
            self.assertEqual(output,(tmp/'stdout.txt').read_text()+'separate stderr\n')
            metadata=json.loads((tmp/'process.json').read_text())
            self.assertEqual((metadata['actual_status'],metadata['validated_status']),(7,7))
            self.assertTrue(metadata['child_reaped'])
            self.assertFalse(metadata['timed_out'])
            self.assertFalse(metadata.get('cancelled',False))


    def nested(self, timeout, ignores_term, recursive=False, startup_delay=0, startup_bound=5, startup_fails=False):
        with tempfile.TemporaryDirectory() as directory:
            tmp=Path(directory)
            (tmp/'evidence').mkdir()
            for name in ('inner-source','middle-source','outer-source'): (tmp/name).write_text('original')
            leaf="\n".join(['import json,os,signal,time', 'from pathlib import Path',
                'signal.signal(signal.SIGTERM,signal.SIG_IGN)' if ignores_term else 'pass',
                pid_writer(tmp/'leaf-started'),
                'time.sleep('+str(startup_delay)+')',
                pid_writer(tmp/'leaf-pid'), 'time.sleep(60)'])
            inner="\n".join(['import json,os,sys', 'from pathlib import Path',
                'sys.path.insert(0,'+repr(str(ROOT/'scripts'))+')',
                'import producer_process,mutation_source', 'root=Path('+repr(str(tmp))+')',
                pid_writer(tmp/'inner-pid'), 'try:',
                " with mutation_source.changed(root/'inner-source','original','mutant'):",
                '  producer_process.run([sys.executable,"-c",'+repr(leaf)+'],timeout=60,grace='+str(2 if recursive else 0)+')',
                'finally:', " (root/'inner-finally').write_text((root/'inner-source').read_text())"])
            if recursive:
                (tmp/'middle-evidence').mkdir()
                inner="\n".join(['import json,os,sys', 'from pathlib import Path',
                    'sys.path.insert(0,'+repr(str(ROOT/'scripts'))+')',
                    'import guide_python,mutation_source', 'root=Path('+repr(str(tmp))+')',
                    pid_writer(tmp/'middle-pid'), 'try:',
                    " with mutation_source.changed(root/'middle-source','original','mutant'):",
                    '  guide_python._launch(root/"middle-evidence",[sys.executable,"-c",'+repr(inner)+'],[],60)',
                    'finally:', " (root/'middle-finally').write_text((root/'middle-source').read_text())"])
            outer="\n".join(['import json,os,signal,subprocess,sys,time', 'from pathlib import Path',
                'sys.path.insert(0,'+repr(str(ROOT/'scripts'))+')',
                'import guide_python,mutation_source,producer_process', 'root=Path('+repr(str(tmp))+')',
                "FIXTURE_READY=root/'leaf-pid'", "FIXTURE_LAUNCHED=root/'native-pid'",
                'FIXTURE_STARTUP_BOUND='+str(startup_bound), READY_LAUNCH,
                'subprocess.Popen=ReadyNative',
                'signal.signal(signal.SIGTERM,lambda *_:sys.exit(37))', 'try:',
                " with mutation_source.changed(root/'outer-source','original','mutant'):",
                '  guide_python._launch(root/"evidence",[sys.executable,"-c",'+repr(inner)+'],[],'+str(.5 if timeout else 60)+')',
                'finally:',
                " if (root/'leaf-pid').exists():",
                "  pid=json.loads((root/'leaf-pid').read_text())['pid']",
                '  try: os.kill(pid,0); alive=True',
                '  except ProcessLookupError: alive=False',
                "  (root/'outer-finally').write_text(str(alive))",
                " else: (root/'outer-finally').write_text('not ready')"])
            wrapper=subprocess.Popen([sys.executable,'-c',outer],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
            owned=[]
            try:
                end=time.monotonic()+startup_bound+3
                while not (tmp/'leaf-pid').exists() and wrapper.poll() is None and time.monotonic()<end:
                    time.sleep(.01)
                if startup_fails:
                    output,_=wrapper.communicate(timeout=8)
                    self.assertEqual(wrapper.returncode,1,output)
                    self.assertIn('fixture startup did not become ready',output)
                    self.assertFalse((tmp/'evidence/process.json').exists(),output)
                    for name in ('inner-source','middle-source','outer-source'):
                        self.assertEqual((tmp/name).read_text(),'original',output)
                    for name in ('native-pid','inner-pid','middle-pid','leaf-started'):
                        if (tmp/name).exists():
                            with self.assertRaises(ProcessLookupError): os.kill(json.loads((tmp/name).read_text())['pid'],0)
                    return
                self.assertTrue((tmp/'leaf-pid').exists(),'nested leaf did not launch')
                owned=[json.loads((tmp/name).read_text())['pid'] for name in ('inner-pid','leaf-pid')]
                if not timeout: wrapper.send_signal(signal.SIGTERM)
                output,_=wrapper.communicate(timeout=8)
                self.assertEqual(wrapper.returncode,0 if timeout else 37,output)
                self.assertEqual((tmp/'inner-source').read_text(),'original',output)
                self.assertTrue((tmp/'inner-finally').exists(),output)
                self.assertEqual((tmp/'inner-finally').read_text(),'original',output)
                self.assertEqual((tmp/'outer-source').read_text(),'original',output)
                self.assertEqual((tmp/'outer-finally').read_text(),'False','leaf must be reaped before outer restoration')
                for pid in owned:
                    with self.assertRaises(ProcessLookupError): os.kill(pid,0)
                if recursive:
                    self.assertEqual((tmp/'middle-source').read_text(),'original',output)
                    self.assertTrue((tmp/'middle-finally').exists(),output)
                    self.assertEqual((tmp/'middle-finally').read_text(),'original',output)
                    with self.assertRaises(ProcessLookupError): os.kill(json.loads((tmp/'middle-pid').read_text())['pid'],0)
                metadata=json.loads((tmp/'evidence/process.json').read_text())
                self.assertEqual(metadata['actual_status'],128+signal.SIGTERM)
                self.assertIsNone(metadata['validated_status'])
                self.assertTrue(metadata['child_reaped'])
                self.assertEqual(metadata['timed_out'],timeout)
                self.assertEqual(metadata['cancelled'],not timeout)
            finally:
                if wrapper.poll() is None:
                    wrapper.terminate()
                    try: wrapper.communicate(timeout=8)
                    except subprocess.TimeoutExpired: wrapper.kill();wrapper.communicate()
                else: wrapper.communicate()
                for name in ('native-pid','inner-pid','middle-pid','leaf-pid','leaf-started'):
                    if (tmp/name).exists(): stop_owned(json.loads((tmp/name).read_text()))

    def test_nested_timeout_restores_both_owners_even_when_leaf_ignores_term(self):
        for ignores_term in (False,True):
            with self.subTest(ignores_term=ignores_term): self.nested(True,ignores_term,startup_delay=.8)

    def test_nested_cancellation_restores_both_owners_even_when_leaf_ignores_term(self):
        for ignores_term in (False,True):
            with self.subTest(ignores_term=ignores_term): self.nested(False,ignores_term)


    def test_exited_leader_does_not_spare_a_term_ignoring_grandchild(self):
        for phase in ('timeout','running'):
            with self.subTest(phase=phase): self.probe(phase,ignores_term=True, startup_delay=.6 if phase=='timeout' else 0)

    def test_recursive_timeout_has_descending_cleanup_deadlines(self):
        self.nested(True,True,recursive=True,startup_delay=3)
        self.nested(True,True,recursive=True,startup_delay=.8,startup_bound=.2,startup_fails=True)

    def test_recursive_cancellation_has_descending_cleanup_deadlines(self):
        self.nested(False,True,recursive=True)

    def test_budget_is_capped_and_reserved_without_mutating_environments(self):
        key='PW_PRODUCER_CLEANUP_MS'
        for allowance, requested, expected in [('1500',2,'1000'),('500',2,'0'),('1500',0,'0')]:
            with self.subTest(allowance=allowance,requested=requested):
                with mock.patch.dict(os.environ,{key:allowance}):
                    before=dict(os.environ)
                    env=dict(os.environ); env[key]='9999'
                    result=producer_process.run([sys.executable,'-c',
                        "import os; print(os.environ['PW_PRODUCER_CLEANUP_MS'])"],
                        timeout=5,grace=requested,env=env)
                    self.assertEqual(result.returncode,0,result.stdout)
                    self.assertEqual(result.stdout.strip(),expected)
                    self.assertEqual(env[key],'9999')
                    self.assertEqual(dict(os.environ),before)

    def test_exhausted_or_malformed_budget_refuses_before_native_launch(self):
        key='PW_PRODUCER_CLEANUP_MS'
        with tempfile.TemporaryDirectory() as directory:
            marker=Path(directory)/'launched'
            command=[sys.executable,'-c',"from pathlib import Path; Path("+repr(str(marker))+").touch()"]
            for allowance in ('0','-1','2001','NaN','inf','1.5',' 500','+500','５００',''):
                with self.subTest(allowance=allowance):
                    with mock.patch.dict(os.environ,{key:allowance}):
                        try:
                            producer_process.run(command,timeout=5,grace=2,env={})
                        except ValueError:
                            refused=True
                        else:
                            refused=False
                    self.assertTrue(refused,'invalid or exhausted parent authority was accepted')
                    self.assertFalse(marker.exists(),'invalid authority launched a native child')



    def test_abort_retains_the_leader_until_the_final_group_signal(self):
        seen=[]
        native=producer_process.stop_group
        def observed(child,number):
            if number==signal.SIGKILL:
                try:
                    result=os.waitid(os.P_PID,child.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT)
                    seen.append(result is not None)
                except ChildProcessError:
                    seen.append(False)
            native(child,number)
        with mock.patch.object(producer_process,'stop_group',observed):
            with self.assertRaises(subprocess.TimeoutExpired):
                producer_process.run([sys.executable,'-c','import time; time.sleep(60)'],timeout=.2,grace=.2)
        self.assertEqual(seen,[True],'leader must remain waitable until its final group signal')

    def test_missing_waitid_uses_bounded_grace_without_early_reaping(self):
        native=os.waitid
        seen=[]
        stop=producer_process.stop_group
        def observed(child,number):
            if number==signal.SIGKILL:
                try: seen.append(native(os.P_PID,child.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT) is not None)
                except ChildProcessError: seen.append(False)
            stop(child,number)
        del os.waitid
        try:
            with mock.patch.object(producer_process,'stop_group',observed):
                with self.assertRaises(subprocess.TimeoutExpired):
                    producer_process.run([sys.executable,'-c','import time; time.sleep(60)'],timeout=.2,grace=.2)
        finally:
            os.waitid=native
        self.assertEqual(seen,[True],'fallback must retain the exited leader before group KILL')

    def test_callback_storage_error_does_not_mask_cancellation_or_restoration(self):
        previous=signal.getsignal(signal.SIGTERM)
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'source'; source.write_text('original')
            pidfile=Path(directory)/'pid'
            def broken_receipt(*_): raise OSError('receipt storage unavailable')
            signal.signal(signal.SIGTERM,lambda *_:sys.exit(37))
            caught=None
            try:
                with mutation_source.changed(source,'original','mutant'):
                    producer_process.run([sys.executable,'-c',
                        'import os,signal,time; from pathlib import Path; Path('+repr(str(pidfile))+').write_text(str(os.getpid())); os.kill(os.getppid(),signal.SIGTERM); time.sleep(60)'],
                        timeout=5,on_reaped=broken_receipt)
            except BaseException as error:
                caught=error
            finally:
                signal.signal(signal.SIGTERM,previous)
            self.assertIsInstance(caught,SystemExit,'storage failure must not mask the saved cancellation handler')
            self.assertEqual(caught.code,37)
            self.assertIsInstance(caught.__cause__,OSError)
            self.assertEqual(source.read_text(),'original')
            with self.assertRaises(ProcessLookupError): os.kill(int(pidfile.read_text()),0)



    def test_cancellation_after_native_completion_never_signals_a_reaped_group(self):
        native=subprocess.Popen
        class Completed(native):
            def communicate(self,*args,**kwargs):
                result=super().communicate(*args,**kwargs)
                if self.returncode is not None:
                    os.kill(os.getpid(),signal.SIGTERM)
                return result
        previous=signal.getsignal(signal.SIGTERM)
        signal.signal(signal.SIGTERM,lambda *_:sys.exit(37))
        receipts=[]
        caught=None
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'source'; source.write_text('original')
            try:
                with mutation_source.changed(source,'original','mutant'):
                    with mock.patch.object(subprocess,'Popen',Completed), mock.patch.object(producer_process,'stop_group') as stopped:
                        try:
                            producer_process.run([sys.executable,'-c',"print('completed')"],timeout=5,
                                on_reaped=lambda *receipt:receipts.append(receipt))
                        except BaseException as error:
                            caught=error
            finally:
                signal.signal(signal.SIGTERM,previous)
            self.assertIsInstance(caught,SystemExit)
            self.assertEqual(caught.code,37)
            self.assertEqual(source.read_text(),'original')
            self.assertEqual(stopped.call_count,0,'a completed/reaped group identity must not be signalled')
            self.assertEqual(len(receipts),1)
            self.assertEqual(receipts[0][1:],(0,False,True))
            with self.assertRaises(ProcessLookupError): os.kill(receipts[0][0],0)


if __name__=='__main__': unittest.main()
