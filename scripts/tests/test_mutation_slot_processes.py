"""Cancellation identities; all real signals target private test children."""
import copy
import errno
import json
import fcntl
import importlib.util
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time
import tempfile
import threading
import unittest
from unittest.mock import Mock, call, patch

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location('pidfd_slots_test', SCRIPTS / 'mutation_slots.py')
slots = importlib.util.module_from_spec(spec)
spec.loader.exec_module(slots)


class MockedProcesses(unittest.TestCase):
    def setUp(self):
        apis = [(os, name) for name in ('kill', 'killpg', 'pidfd_open', 'close', 'fork', 'forkpty',
                'posix_spawn', 'posix_spawnp', 'system', 'popen', 'spawnl', 'spawnle', 'spawnlp',
                'spawnlpe', 'spawnv', 'spawnve', 'spawnvp', 'spawnvpe', 'execv', 'execve', 'execvp',
                'execvpe', 'execl', 'execle', 'execlp', 'execlpe') if hasattr(os, name) or name == 'pidfd_open']
        apis += [(subprocess, 'Popen'), (signal, 'pidfd_send_signal')]
        for module, name in apis:
            guard = patch.object(module, name, create=True, side_effect=AssertionError('unmocked process API: '+name))
            guard.start()
            self.addCleanup(guard.stop)
        for guard in (patch.object(slots.sys, 'platform', 'linux'),
                      patch.object(slots.signal, 'getsignal', return_value=signal.SIG_DFL),
                      patch.object(slots.signal, 'signal', return_value=signal.SIG_DFL)):
            guard.start()
            self.addCleanup(guard.stop)


class Identities(MockedProcesses):
    def test_reuse_before_open_is_refused_without_any_signal(self):
        with patch.object(slots, 'started_at', return_value=2), \
             patch.object(slots.os, 'pidfd_open') as opened, \
             patch.object(slots.signal, 'pidfd_send_signal') as sent:
            self.assertIsNone(slots.pin(424242, 1))
            opened.assert_not_called()
            sent.assert_not_called()

    def test_reuse_while_opening_closes_the_untrusted_handle(self):
        with patch.object(slots, 'started_at', side_effect=[1, 2]), \
             patch.object(slots.os, 'pidfd_open', return_value=99), \
             patch.object(slots.os, 'close') as closed, \
             patch.object(slots.signal, 'pidfd_send_signal') as sent:
            self.assertIsNone(slots.pin(424242, 1))
            closed.assert_called_once_with(99)
            sent.assert_not_called()

    def test_absent_identity_is_not_opened(self):
        with patch.object(slots, 'started_at', return_value=None), \
             patch.object(slots.os, 'pidfd_open') as opened:
            self.assertIsNone(slots.pin(424242))
            opened.assert_not_called()

    def test_exit_before_open_is_a_noop(self):
        with patch.object(slots, 'started_at', return_value=1), \
             patch.object(slots.os, 'pidfd_open', side_effect=ProcessLookupError):
            self.assertIsNone(slots.pin(424242, 1))

    def test_identity_read_exception_closes_open_descriptor(self):
        with patch.object(slots, 'started_at', side_effect=[1, ValueError('probe')]), \
             patch.object(slots.os, 'pidfd_open', return_value=99), \
             patch.object(slots.os, 'close') as closed:
            with self.assertRaisesRegex(ValueError, 'probe'):
                slots.pin(424242)
            closed.assert_called_once_with(99)

    def test_signals_use_the_pinned_handle_after_numeric_pid_reuse(self):
        owned = slots.PinnedProcess(424242, 1, 99)
        with patch.object(slots.signal, 'pidfd_send_signal') as sent, \
             patch.object(slots.os, 'kill') as numeric, \
             patch.object(slots.os, 'killpg') as group, \
             patch.object(slots, 'started_at', return_value=2):
            self.assertTrue(owned.send(signal.SIGSTOP))
            self.assertTrue(owned.send(signal.SIGKILL))
            sent.assert_has_calls([call(99, signal.SIGSTOP), call(99, signal.SIGKILL)])
            numeric.assert_not_called()
            group.assert_not_called()

    def test_exited_handle_does_not_fall_back_to_a_numeric_signal(self):
        owned = slots.PinnedProcess(424242, 1, 99)
        with patch.object(slots.signal, 'pidfd_send_signal', side_effect=ProcessLookupError), \
             patch.object(slots.os, 'kill') as numeric:
            self.assertFalse(owned.send(signal.SIGKILL))
            numeric.assert_not_called()

    def test_close_is_idempotent_and_closed_handles_cannot_signal(self):
        owned = slots.PinnedProcess(424242, 1, 99)
        with patch.object(slots.os, 'close') as closed:
            owned.close()
            owned.close()
            closed.assert_called_once_with(99)
        with self.assertRaisesRegex(RuntimeError, 'closed'):
            owned.send(signal.SIGKILL)

    def test_missing_pidfd_api_refuses_before_preparation_or_launch(self):
        run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [], 2, [])
        with patch.object(slots.os, 'pidfd_open', None), \
             patch.object(run, 'prepare') as prepared, \
             patch.object(slots.subprocess, 'Popen') as launched:
            with self.assertRaisesRegex(RuntimeError, 'pidfd APIs'):
                run.run()
            prepared.assert_not_called()
            launched.assert_not_called()

    def test_kernel_without_pidfds_refuses_before_launch(self):
        with patch.object(slots.os, 'pidfd_open', side_effect=OSError(errno.ENOSYS, 'no syscall')), \
             patch.object(slots.subprocess, 'Popen') as launched:
            with self.assertRaisesRegex(RuntimeError, 'cannot open'):
                slots.require_pidfds()
            launched.assert_not_called()


class Freeze(MockedProcesses):
    def root(self):
        root = Mock(spec=slots.PinnedProcess)
        root.pid = 313131
        root.send.return_value = True
        return root

    def test_discovered_pid_reuse_is_not_stopped(self):
        root = self.root()
        with patch.object(slots, 'stopped', return_value=True), \
             patch.object(slots, 'descendants_of', side_effect=[[(424242, 1)], []]), \
             patch.object(slots, 'pin', return_value=None) as pin:
            frozen = {}
            slots.freeze(root, frozen)
            self.assertEqual(frozen, {})
            pin.assert_called_once_with(424242, 1)
            root.send.assert_called_once_with(signal.SIGSTOP)

    def test_descent_lost_after_pin_is_not_stopped_and_handle_closes(self):
        root, child = self.root(), Mock(spec=slots.PinnedProcess)
        with patch.object(slots, 'stopped', return_value=True), \
             patch.object(slots, 'descendants_of', side_effect=[[(424242, 1)], [], []]), \
             patch.object(slots, 'pin', return_value=child):
            frozen = {}
            slots.freeze(root, frozen)
            self.assertEqual(frozen, {})
            child.send.assert_not_called()
            child.close.assert_called_once()

    def test_matching_descendant_is_pinned_and_stopped_once(self):
        root, child = self.root(), Mock(spec=slots.PinnedProcess)
        with patch.object(slots, 'stopped', return_value=True), \
             patch.object(slots, 'descendants_of', return_value=[(424242, 1)]), \
             patch.object(slots, 'pin', return_value=child):
            frozen = {}
            slots.freeze(root, frozen)
            self.assertIs(frozen[(424242, 1)], child)
            child.send.assert_called_once_with(signal.SIGSTOP)
            child.close.assert_not_called()

    def test_exited_root_does_not_discover_an_unrelated_reused_tree(self):
        root = self.root()
        root.send.return_value = False
        with patch.object(slots, 'descendants_of') as discovered:
            slots.freeze(root, {})
            discovered.assert_not_called()

    def test_scan_exception_closes_unaccepted_child(self):
        root, child = self.root(), Mock(spec=slots.PinnedProcess)
        with patch.object(slots, 'stopped', return_value=True), \
             patch.object(slots, 'descendants_of', side_effect=[[(424242, 1)], ValueError('scan')]), \
             patch.object(slots, 'pin', return_value=child):
            with self.assertRaisesRegex(ValueError, 'scan'):
                slots.freeze(root, {})
            child.close.assert_called_once()
            child.send.assert_not_called()

    def test_partial_scan_failure_still_kills_and_closes_every_pinned_child(self):
        run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [], 2, [])
        root, child, proc = self.root(), Mock(spec=slots.PinnedProcess), Mock()
        run.slots = [Mock(identity=root, proc=proc)]
        def interrupted(_root, frozen):
            frozen[(424242, 1)] = child
            raise ValueError('scan')
        with patch.object(slots, 'freeze', side_effect=interrupted):
            with self.assertRaisesRegex(RuntimeError, 'could not prove ownership'):
                run.stop()
        child.send.assert_called_once_with(signal.SIGKILL)
        child.close.assert_called_once()
        root.send.assert_called_once_with(signal.SIGKILL)
        proc.wait.assert_called_once()


class Startup(MockedProcesses):
    def run_object(self):
        run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [('control',)], 2, [])
        run.dealt = [[0]]
        return run

    def test_failed_root_pin_kills_and_reaps_only_the_unreaped_direct_child(self):
        run, proc = self.run_object(), Mock()
        with patch.object(slots.subprocess, 'Popen', return_value=proc), \
             patch.object(slots, 'pin', side_effect=OSError('pin')), \
             patch.object(slots.threading, 'Thread') as listener:
            with self.assertRaisesRegex(OSError, 'pin'):
                run.start([(slots.ROOT, Path('/tmp/slots-test'))])
            listener.assert_not_called()
        proc.kill.assert_called_once()
        proc.wait.assert_called_once()
        proc.stdout.close.assert_called_once()
        self.assertEqual(len(run.slots), 1)
        self.assertIsNone(run.slots[0].identity)
        self.assertIs(run.slots[0].proc, proc)

    def test_root_is_pinned_before_any_reaper_thread_can_start(self):
        run, proc, identity, events = self.run_object(), Mock(), Mock(), []
        listener = Mock()
        listener.start.side_effect = lambda: events.append('listen')
        def pin(_pid):
            events.append('pin')
            return identity
        with patch.object(slots.subprocess, 'Popen', return_value=proc), \
             patch.object(slots, 'pin', side_effect=pin), \
             patch.object(slots.threading, 'Thread', return_value=listener):
            run.start([(slots.ROOT, Path('/tmp/slots-test'))])
        self.assertEqual(events, ['pin', 'listen'])
        self.assertIs(run.slots[0].identity, identity)


class ReviewRegressions(MockedProcesses):
    def run_with_private_mocks(self):
        run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [('control',)], 2, [])
        run.dealt = [[0]]
        root, child, proc = Mock(spec=slots.PinnedProcess), Mock(spec=slots.PinnedProcess), Mock()
        root.pid = 313131
        root.send.return_value = True
        root.wait_exited = Mock(return_value=True)
        child.wait_exited = Mock(return_value=True)
        run.slots = [Mock(identity=root, proc=proc)]
        return run, root, child, proc

    def test_send_capability_is_probed_and_descriptor_closes_on_refusal(self):
        with patch.object(slots.os, 'pidfd_open', return_value=99), \
             patch.object(slots.signal, 'pidfd_send_signal', side_effect=PermissionError('denied')), \
             patch.object(slots.os, 'close') as closed:
            with self.assertRaisesRegex(RuntimeError, 'signal'):
                slots.require_pidfds()
            closed.assert_called_once_with(99)

    def test_custom_child_reaper_refuses_before_pidfd_probe(self):
        with patch.object(slots.signal, 'getsignal', return_value=signal.SIG_IGN), \
             patch.object(slots.os, 'pidfd_open') as opened:
            with self.assertRaisesRegex(RuntimeError, 'reaping'):
                slots.require_pidfds()
            opened.assert_not_called()

    def test_failed_child_kill_does_not_skip_root_kill_close_or_reap(self):
        run, root, child, proc = self.run_with_private_mocks()
        child.send.side_effect = PermissionError('denied')
        def frozen(_root, found):
            found[(424242, 1)] = child
        with patch.object(slots, 'freeze', side_effect=frozen):
            with self.assertRaisesRegex(RuntimeError, 'PermissionError'):
                run.stop()
        root.send.assert_called_once_with(signal.SIGKILL)
        child.close.assert_called_once()
        proc.wait.assert_called_once()

    def test_failed_close_does_not_skip_other_close_or_reap(self):
        run, root, child, proc = self.run_with_private_mocks()
        other = Mock(spec=slots.PinnedProcess)
        other.wait_exited = Mock(return_value=True)
        child.close.side_effect = OSError('close failed')
        def frozen(_root, found):
            found[(424242, 1)] = child
            found[(424243, 2)] = other
        with patch.object(slots, 'freeze', side_effect=frozen):
            with self.assertRaisesRegex(RuntimeError, 'close failed'):
                run.stop()
        other.close.assert_called_once()
        root.send.assert_called_once_with(signal.SIGKILL)
        proc.wait.assert_called_once()

    def test_failed_reap_does_not_skip_the_next_slots_cleanup(self):
        run, root, child, proc = self.run_with_private_mocks()
        other_root, other_proc = Mock(spec=slots.PinnedProcess), Mock()
        other_root.wait_exited = Mock(return_value=True)
        proc.wait.side_effect = subprocess.TimeoutExpired('private mock', 5)
        run.slots.append(Mock(identity=other_root, proc=other_proc))
        with patch.object(slots, 'freeze'):
            with self.assertRaisesRegex(RuntimeError, 'reap'):
                run.stop()
        other_root.send.assert_called_once_with(signal.SIGKILL)
        other_proc.wait.assert_called_once()

    def test_repeated_signal_during_cleanup_is_deferred(self):
        run, root, child, proc = self.run_with_private_mocks()
        root.send.side_effect = lambda _sig: run.stopped_from_outside(signal.SIGINT, None)
        with patch.object(slots, 'freeze'):
            run.stop()
        proc.wait.assert_called_once()

    def test_launch_signal_at_popen_handoff_is_deferred_until_registered(self):
        run, _root, _child, proc = self.run_with_private_mocks()
        run.slots = []
        identity, listener = Mock(spec=slots.PinnedProcess), Mock()
        def launch(*_args, **_kwargs):
            run.stopped_from_outside(signal.SIGTERM, None)
            return proc
        with patch.object(slots.subprocess, 'Popen', side_effect=launch), \
             patch.object(slots, 'pin', return_value=identity), \
             patch.object(slots.threading, 'Thread', return_value=listener):
            with self.assertRaises(SystemExit):
                run.start([(slots.ROOT, Path('/tmp/slots-test'))])
        self.assertEqual(len(run.slots), 1)
        self.assertIs(run.slots[0].proc, proc)
        self.assertIs(run.slots[0].identity, identity)
        self.assertFalse(run.launching)
        listener.start.assert_called_once()

    def test_launch_signal_at_pin_handoff_is_deferred_until_registered(self):
        run, _root, _child, proc = self.run_with_private_mocks()
        run.slots = []
        identity, listener = Mock(spec=slots.PinnedProcess), Mock()
        def pin(_pid):
            run.stopped_from_outside(signal.SIGINT, None)
            return identity
        with patch.object(slots.subprocess, 'Popen', return_value=proc), \
             patch.object(slots, 'pin', side_effect=pin), \
             patch.object(slots.threading, 'Thread', return_value=listener):
            with self.assertRaises(SystemExit):
                run.start([(slots.ROOT, Path('/tmp/slots-test'))])
        self.assertIs(run.slots[0].identity, identity)
        listener.start.assert_called_once()

    def test_live_descendant_with_missing_identity_is_explicitly_refused(self):
        root = Mock(spec=slots.PinnedProcess)
        root.pid = 313131
        root.send.return_value = True
        with patch.object(slots, 'stopped', return_value=True), \
             patch.object(slots, 'descendants_of', return_value=[(424242, None)]), \
             patch.object(slots, 'pin') as pin:
            with self.assertRaisesRegex(RuntimeError, 'no provable'):
                slots.freeze(root, {})
            pin.assert_not_called()

    def test_descendant_exit_is_confirmed_before_descriptor_close(self):
        run, root, child, proc = self.run_with_private_mocks()
        events = []
        child.wait_exited.side_effect = lambda _deadline: events.append('exit-ready') or True
        child.close.side_effect = lambda: events.append('close')
        def frozen(_root, found):
            found[(424242, 1)] = child
        with patch.object(slots, 'freeze', side_effect=frozen):
            run.stop()
        self.assertEqual(events, ['exit-ready', 'close'])

    def test_exit_timeout_is_explicit_and_handles_still_close(self):
        run, root, child, proc = self.run_with_private_mocks()
        child.wait_exited.return_value = False
        def frozen(_root, found):
            found[(424242, 1)] = child
        with patch.object(slots, 'freeze', side_effect=frozen):
            with self.assertRaisesRegex(RuntimeError, 'did not terminate'):
                run.stop()
        child.close.assert_called_once()
        proc.wait.assert_called_once()

    def test_process_exit_wait_uses_retained_descriptor(self):
        owned = slots.PinnedProcess(424242, 1, 99)
        polling = Mock()
        polling.poll.return_value = [(99, select.POLLIN)]
        with patch.object(slots.select, 'poll', return_value=polling), \
             patch.object(slots, 'process_state') as numeric:
            self.assertTrue(owned.wait_exited(time.monotonic() + 1))
        polling.register.assert_called_once_with(99, select.POLLIN)
        numeric.assert_not_called()

    def test_high_descriptor_wait_has_no_select_fd_set_limit(self):
        owned = slots.PinnedProcess(424242, 1, 2048)
        polling = Mock()
        polling.poll.return_value = [(2048, select.POLLIN)]
        with patch.object(slots.select, 'poll', return_value=polling):
            self.assertTrue(owned.wait_exited(time.monotonic() + 1))
        polling.register.assert_called_once_with(2048, select.POLLIN)

    def test_poll_invalid_descriptor_is_an_explicit_failure(self):
        owned = slots.PinnedProcess(424242, 1, 99)
        polling = Mock()
        polling.poll.return_value = [(99, select.POLLNVAL)]
        with patch.object(slots.select, 'poll', return_value=polling):
            with self.assertRaisesRegex(RuntimeError, 'cannot be polled'):
                owned.wait_exited(time.monotonic() + 1)

    def test_unreadable_live_root_state_is_not_treated_as_exit(self):
        root = Mock(spec=slots.PinnedProcess)
        root.pid = 313131
        root.send.return_value = True
        root.wait_exited.return_value = False
        with patch.object(slots, 'process_state', return_value=None):
            with self.assertRaisesRegex(RuntimeError, 'live pinned process'):
                slots.stopped(root, time.monotonic() + 1)

    def test_unreadable_state_with_pinned_exit_is_a_valid_exit(self):
        root = Mock(spec=slots.PinnedProcess)
        root.pid = 313131
        root.send.return_value = True
        root.wait_exited.return_value = True
        with patch.object(slots, 'process_state', return_value=None):
            self.assertFalse(slots.stopped(root, time.monotonic() + 1))

    def test_startup_kill_failure_still_attempts_bounded_reap_and_pipe_close(self):
        proc = Mock()
        proc.kill.side_effect = PermissionError('denied')
        with self.assertRaisesRegex(RuntimeError, 'startup child cleanup failed'):
            slots.kill_unreaped(proc)
        proc.wait.assert_called_once_with(timeout=5)
        proc.stdout.close.assert_called_once()

    def test_startup_reap_timeout_still_closes_the_pipe(self):
        proc = Mock()
        proc.wait.side_effect = subprocess.TimeoutExpired('private mock', 5)
        with self.assertRaisesRegex(RuntimeError, 'TimeoutExpired'):
            slots.kill_unreaped(proc)
        proc.kill.assert_called_once()
        proc.wait.assert_called_once_with(timeout=5)
        proc.stdout.close.assert_called_once()

    def test_failed_pin_record_is_retried_without_a_listener_or_numeric_group(self):
        run, _root, _child, proc = self.run_with_private_mocks()
        run.slots = [Mock(identity=None, proc=proc)]
        with patch.object(slots, 'kill_unreaped') as cleanup:
            run.stop()
        cleanup.assert_called_once_with(proc)

    def test_final_handle_close_failure_does_not_skip_the_next_handle(self):
        run, first, second, _proc = self.run_with_private_mocks()
        run.slots = [Mock(identity=first), Mock(identity=second)]
        first.close.side_effect = OSError('close root')
        with patch.object(slots, 'require_pidfds'), patch.object(run, 'prepare', return_value=[]), \
             patch.object(run, 'merge', return_value=0):
            with self.assertRaisesRegex(RuntimeError, 'close root'):
                run.run()
        second.close.assert_called_once()

    def test_handlers_are_installed_for_both_cancellation_signals(self):
        run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [], 2, [])
        with patch.object(slots, 'require_pidfds'), patch.object(run, 'prepare', return_value=[]), \
             patch.object(run, 'merge', return_value=0), patch.object(slots.signal, 'signal') as installed:
            self.assertEqual(run.run(), 0)
        self.assertEqual(installed.call_args_list[:2], [call(signal.SIGTERM, run.stopped_from_outside),
                                                       call(signal.SIGINT, run.stopped_from_outside)])


class ProducerResults(MockedProcesses):
    IDS = ["private.Case.test_a", "private.Case.test_b"]

    def result(self, status, report):
        spec = importlib.util.spec_from_file_location('slots_producer_test', SCRIPTS / 'mutation_slots_mutations.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        text = report if isinstance(report, str) else json.dumps(report)
        return module.python_result(status, text, self.IDS)

    def report(self, failed=False):
        return {"schema": 1, "expected": self.IDS[:], "started": self.IDS[:], "finished": self.IDS[:],
                "bodies": [{"id": identity, "entered": 1, "completed": 1} for identity in self.IDS],
                "events": [{"id": identity, "kind": "failure" if failed and i == 0 else "success",
                            "origin": "body"} for i, identity in enumerate(self.IDS)],
                "failed": self.IDS[:1] if failed else [],
                "passed": self.IDS[1:] if failed else self.IDS[:], "refusals": [], "complete": True}

    def test_complete_pass_and_real_body_failure_are_accepted(self):
        self.assertEqual(self.result(0, self.report()), (True, 2, 0))
        self.assertEqual(self.result(1, self.report(True)), (True, 1, 1))

    def test_stops_unexpected_exits_and_exit_contradictions_refuse(self):
        for status in (-9, -15, 0, 2, 67, 101, 137, True):
            with self.subTest(status=status):
                self.assertEqual(self.result(status, self.report(True)), (False, 0, 0))
        self.assertEqual(self.result(1, self.report()), (False, 0, 0))

    def test_absent_partial_malformed_duplicate_and_unknown_protocol_refuse(self):
        for output in ('', '{', 'OK', '{"schema":1,"schema":1}',
                       json.dumps(self.report()).replace('{', '{"schema":1,', 1)):
            self.assertEqual(self.result(0, output), (False, 0, 0))
        for key in self.report():
            report = self.report()
            del report[key]
            self.assertEqual(self.result(0, report), (False, 0, 0))
        report = self.report(); report['surprise'] = 1
        self.assertEqual(self.result(0, report), (False, 0, 0))

    def test_exact_start_finish_body_and_expected_inventories_are_required(self):
        for key in ('expected', 'started', 'finished', 'bodies'):
            for alteration in ('drop', 'duplicate', 'different'):
                report = self.report()
                if alteration == 'drop': report[key].pop()
                elif alteration == 'duplicate': report[key].append(copy.deepcopy(report[key][0]))
                elif key == 'bodies': report[key][0]['id'] = 'different.Case.test_x'
                else: report[key][0] = 'different.Case.test_x'
                with self.subTest(key=key, alteration=alteration):
                    self.assertEqual(self.result(0, report), (False, 0, 0))

    def test_missing_repeated_or_unfinished_body_refuses(self):
        for key in ('entered', 'completed'):
            for value in (0, 2, True, '1'):
                report = self.report(); report['bodies'][0][key] = value
                self.assertEqual(self.result(0, report), (False, 0, 0))
        report = self.report(); report['complete'] = False
        self.assertEqual(self.result(0, report), (False, 0, 0))

    def test_fixture_skip_and_nonbody_events_earn_no_kill(self):
        report = self.report(True); report['events'][0]['origin'] = 'fixture'
        self.assertEqual(self.result(1, report), (False, 0, 0))
        for reason in ('setUp', 'tearDown', 'class fixture', 'skip', 'cleanup'):
            report = self.report(True); report['refusals'] = [reason]
            self.assertEqual(self.result(1, report), (False, 0, 0))

    def test_contradictory_method_partition_refuses(self):
        for key in ('failed', 'passed'):
            report = self.report(True); report[key] = []
            self.assertEqual(self.result(1, report), (False, 0, 0))
        report = self.report(True); report['failed'] *= 2
        self.assertEqual(self.result(1, report), (False, 0, 0))

    def test_subtest_events_count_one_failed_method(self):
        report = self.report(True)
        report['events'][:1] = [{'id': self.IDS[0], 'kind': kind, 'origin': 'body'}
                              for kind in ('subtest_success', 'subtest_failure', 'subtest_error')]
        self.assertEqual(self.result(1, report), (True, 1, 1))
        report['events'].append({'id': self.IDS[0], 'kind': 'error', 'origin': 'body'})
        self.assertEqual(self.result(1, report), (True, 1, 1))

    def test_missing_duplicate_contradictory_and_unknown_events_refuse(self):
        report = self.report(); report['events'].pop()
        self.assertEqual(self.result(0, report), (False, 0, 0))
        report = self.report(); report['events'].append(report['events'][0])
        self.assertEqual(self.result(0, report), (False, 0, 0))
        report = self.report(True); report['events'].append({'id': self.IDS[0], 'kind': 'success', 'origin': 'body'})
        self.assertEqual(self.result(1, report), (False, 0, 0))
        for key, value in (('kind', 'skip'), ('origin', 'loader'), ('id', 'other.Case.test_x')):
            report = self.report(); report['events'][0][key] = value
            self.assertEqual(self.result(0, report), (False, 0, 0))

    def test_mocked_profile_blocks_creation_signals_and_close_globally(self):
        for api, args in ((subprocess.Popen, ['not-a-real-command']), (os.fork, None),
                          (os.posix_spawn, None), (os.system, 'not-a-real-command'),
                          (os.kill, None), (os.close, None)):
            with self.subTest(api=api):
                self.assertIsInstance(api, Mock, 'global guard is installed before any call')
                with self.assertRaisesRegex(AssertionError, 'unmocked process API'):
                    api() if args is None else api(args)


    def test_shared_outcome_keeps_assertions_errors_and_distinct_methods(self):
        spec = importlib.util.spec_from_file_location('shared_outcome_test', SCRIPTS / 'unittest_protocol.py')
        protocol = importlib.util.module_from_spec(spec); spec.loader.exec_module(protocol)
        report = self.report(True)
        report['events'][:1] = [{'id': self.IDS[0], 'kind': kind, 'origin': 'body'}
                              for kind in ('subtest_failure', 'subtest_error')]
        outcome = protocol.validate_result(1, json.dumps(report), self.IDS)
        self.assertTrue(outcome.valid, outcome.reason)
        self.assertEqual(outcome.passed, (self.IDS[1],))
        self.assertEqual(outcome.assertion_failed, (self.IDS[0],))
        self.assertEqual(outcome.body_errors, (self.IDS[0],))
        self.assertEqual(outcome.failed, (self.IDS[0],))
        self.assertEqual(outcome.executed, 2)

    def test_invalid_shared_outcome_contains_no_positive_evidence(self):
        spec = importlib.util.spec_from_file_location('invalid_outcome_test', SCRIPTS / 'unittest_protocol.py')
        protocol = importlib.util.module_from_spec(spec); spec.loader.exec_module(protocol)
        report = self.report(True); report['refusals'] = ['actual fixture']
        for status, text in ((1,json.dumps(report)), (137,json.dumps(self.report(True))), (1,'{'),
                             (1,'['*2000+'0'+']'*2000)):
            with self.subTest(status=status, text=text):
                try:
                    outcome = protocol.validate_result(status,text,self.IDS)
                except RecursionError:
                    self.fail("deep malformed report escaped validation")
                self.assertFalse(outcome.valid)
                self.assertTrue(outcome.reason)
                self.assertEqual((outcome.passed,outcome.assertion_failed,outcome.body_errors,
                                  outcome.failed,outcome.executed), ((),(),(),(),0))

    def test_fresh_harness_loads_actual_local_validation_authority(self):
        # A retained producer can already have a canonical protocol cached. A
        # fresh child harness must still exercise its ACTUALLY mutated local file.
        spec = importlib.util.spec_from_file_location('canonical_cached_protocol', SCRIPTS / 'unittest_protocol.py')
        canonical = importlib.util.module_from_spec(spec); spec.loader.exec_module(canonical)
        folder = Path(tempfile.mkdtemp(prefix='private-protocol-authority-', dir=os.environ.get('TMPDIR','/tmp'))) / 'scripts'
        folder.mkdir()
        (folder / 'mutation_slots_mutations.py').write_bytes((SCRIPTS / 'mutation_slots_mutations.py').read_bytes())
        source = (SCRIPTS / 'unittest_protocol.py').read_text()
        anchor = '        if type(status) is not int or status != (1 if failed else 0):\n'
        self.assertEqual(source.count(anchor),1)
        altered = source.replace(anchor, '        if False and (type(status) is not int or status != (1 if failed else 0)):\n', 1)
        self.assertNotEqual(source,altered)
        (folder / 'unittest_protocol.py').write_text(altered)
        with patch.dict(sys.modules, {'unittest_protocol': canonical}):
            spec = importlib.util.spec_from_file_location('fresh_local_authority', folder / 'mutation_slots_mutations.py')
            fresh = importlib.util.module_from_spec(spec); spec.loader.exec_module(fresh)
            self.assertFalse(canonical.validate_result(137,json.dumps(self.report(True)),self.IDS).valid)
            self.assertEqual(fresh.python_result(137,json.dumps(self.report(True)),self.IDS),(True,1,1))

    def execution(self, proc, identity):
        spec = importlib.util.spec_from_file_location('slots_execution_test', SCRIPTS / 'mutation_slots_mutations.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        canonical = Mock()
        owner = Mock(slots=[], cancel_requested=False)
        canonical.Run.return_value = owner
        canonical.pin.return_value = identity
        log = Mock()
        context = log
        context.__enter__ = Mock(return_value=log)
        context.__exit__ = Mock(return_value=False)
        return module, canonical, owner, log, context

    def test_timeout_reuses_retained_cleanup_without_communicate_or_numeric_groups(self):
        proc, identity = Mock(), Mock()
        proc.wait.side_effect = subprocess.TimeoutExpired('private mock', 1)
        identity.wait_exited.return_value = False
        module, canonical, owner, log, context = self.execution(proc, identity)
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal'), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaisesRegex(RuntimeError, 'timed out'):
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        canonical.require_pidfds.assert_called_once()
        owner.stop.assert_called_once()
        identity.close.assert_called_once()
        proc.wait.assert_called_once_with(timeout=1)
        proc.communicate.assert_not_called()
        self.assertIs(proc.stdout, log)
        self.assertIs(owner.slots[0].identity, identity)

    def test_already_exited_root_explicitly_refuses_unproved_orphans(self):
        proc, identity = Mock(), Mock()
        proc.wait.side_effect = subprocess.TimeoutExpired('private mock', 1)
        identity.wait_exited.return_value = True
        module, canonical, owner, _log, context = self.execution(proc, identity)
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal'), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaisesRegex(RuntimeError, 'previously orphaned descendants have unproved ownership'):
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        owner.stop.assert_called_once()
        identity.close.assert_called_once()
        proc.communicate.assert_not_called()

    def test_failed_exit_probe_still_runs_canonical_cleanup_and_closes_root(self):
        proc, identity = Mock(), Mock()
        proc.wait.side_effect = subprocess.TimeoutExpired('private mock', 1)
        identity.wait_exited.side_effect = OSError('exit probe failed')
        module, canonical, owner, _log, context = self.execution(proc, identity)
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal'), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaisesRegex(RuntimeError, 'exit probe failed'):
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        owner.stop.assert_called_once()
        identity.close.assert_called_once()

    def test_cancellation_survives_stop_close_and_restore_failures(self):
        for cancellation in (SystemExit(17), KeyboardInterrupt()):
            proc, identity = Mock(), Mock()
            proc.wait.side_effect = cancellation
            identity.wait_exited.return_value = False
            identity.close.side_effect = OSError('close failure')
            module, canonical, owner, _log, context = self.execution(proc, identity)
            owner.stop.side_effect = OSError('stop failure')
            def handler(sig, callback):
                if callback is signal.SIG_DFL:
                    raise OSError('restore failure')
                return signal.SIG_DFL
            with patch.object(module, 'cleanup_module', return_value=canonical), \
                 patch.object(module.subprocess, 'Popen', return_value=proc), \
                 patch.object(module.signal, 'signal', side_effect=handler) as handlers, \
                 patch.object(module, 'open', create=True, return_value=context), patch('builtins.print', side_effect=BrokenPipeError('diagnostic sink') if isinstance(cancellation, KeyboardInterrupt) else None) as reported:
                with self.assertRaises(type(cancellation)) as caught:
                    module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
            self.assertIs(caught.exception, cancellation)
            owner.stop.assert_called_once()
            identity.close.assert_called_once()
            self.assertEqual(handlers.call_count, 4, 'both restores attempted after both installs')
            self.assertIn('stop failure', reported.call_args[0][0])
            self.assertIn('close failure', reported.call_args[0][0])
            self.assertIn('restore failure', reported.call_args[0][0])

    def test_output_close_failure_cannot_replace_primary_cancellation(self):
        for cancellation in (SystemExit(17), KeyboardInterrupt()):
            proc, identity = Mock(), Mock()
            proc.wait.side_effect = cancellation
            identity.wait_exited.return_value = False
            module, canonical, owner, log, context = self.execution(proc, identity)
            log.close.side_effect = OSError('output close failure')
            with patch.object(module, 'cleanup_module', return_value=canonical), \
                 patch.object(module.subprocess, 'Popen', return_value=proc), \
                 patch.object(module.signal, 'signal', return_value=signal.SIG_DFL), \
                 patch.object(module, 'open', create=True, return_value=context), patch('builtins.print', side_effect=BrokenPipeError('diagnostic sink') if isinstance(cancellation, KeyboardInterrupt) else None) as reported:
                with self.assertRaises(type(cancellation)) as caught:
                    module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
            self.assertIs(caught.exception, cancellation)
            owner.stop.assert_called_once()
            identity.close.assert_called_once()
            log.close.assert_called_once()
            context.__exit__.assert_not_called()
            self.assertIn('output close failure', reported.call_args[0][0])

    def test_another_threads_open_meets_no_mocked_output(self):
        # The flake (train-11, 2026-10-20): these tests patched the output's
        # `open` in `builtins`, so every thread's `open` met the mock while one
        # ran, and the memory bound's watcher, which reads /proc in a `with`,
        # called the mock's `__exit__` now and then: the test above failed so
        # once in a whole local run at load 110 and once on GitHub. Patched on
        # the module alone, another thread's `open` is the real one whenever
        # it runs: here a thread opens a file while execute's output is the
        # mock, each step waiting on an event, never a timer (a wait's bound
        # only fails the test).
        proc, identity = Mock(), Mock()
        inside, opened = threading.Event(), threading.Event()
        seen = []

        def reader():
            inside.wait(60)
            with open(os.devnull, 'rb') as handle:
                seen.append(handle)
            opened.set()

        def waiting(**_kwargs):
            inside.set()
            if not opened.wait(60):
                raise AssertionError('the other thread never opened its file')
            return 0

        proc.wait.side_effect = waiting
        module, canonical, owner, log, context = self.execution(proc, identity)
        other = threading.Thread(target=reader)
        other.start()
        try:
            with patch.object(module, 'cleanup_module', return_value=canonical), \
                 patch.object(module.subprocess, 'Popen', return_value=proc), \
                 patch.object(module.signal, 'signal'), \
                 patch.object(module, 'open', create=True, return_value=context):
                self.assertEqual(module.execute(['private mock'], Path('/unused/output'), {}, bound=1), 0)
        finally:
            inside.set()
            other.join(60)
        self.assertEqual(len(seen), 1)
        self.assertIsNot(seen[0], context)
        self.assertTrue(seen[0].closed)
        context.__exit__.assert_not_called()
        log.close.assert_called_once()

    def test_campaign_restores_source_and_launches_no_next_control_after_cancel(self):
        module, _canonical, _owner, _log, _context = self.execution(Mock(), Mock())
        source = Mock()
        source.read_bytes.return_value = b'pass\n'
        mutants = [('first', source, 'pass\n', 'True\n'), ('second', source, 'pass\n', 'False\n')]
        with patch.dict(sys.modules, {'mutation_baseline': Mock()}), \
             patch.object(module, 'freeze_profiles'), patch.object(module, 'MUTANTS', mutants), \
             patch.object(module, 'EVIDENCE', Path('/unused')), patch.object(Path, 'write_text'), \
             patch.object(module.signal, 'signal'), \
             patch.object(module, 'run_tests', side_effect=[(True, 1, 0), SystemExit(17), (True, 1, 0)]) as run:
            with self.assertRaises(SystemExit):
                module.main()
        self.assertEqual(run.call_count, 2, 'baseline and cancelled first control only')
        self.assertEqual(source.write_bytes.call_args_list, [call(b'True\n'), call(b'pass\n')])

    def test_preflight_rejects_identical_or_unchanged_substitutions(self):
        module, _canonical, _owner, _log, _context = self.execution(Mock(), Mock())
        source = Mock()
        source.read_bytes.return_value = b'pass\n'
        with self.assertRaisesRegex(ValueError, 'identical mutation'):
            module.preflight([('same', source, 'pass\n', 'pass\n')])
        with patch.object(module.hashlib, 'sha256', return_value=Mock(hexdigest=Mock(return_value='same'))):
            with self.assertRaisesRegex(ValueError, 'did not alter source SHA'):
                module.preflight([('changed but wrong hash', source, 'pass\n', 'True\n')])
        proof = module.preflight([('actual change', source, 'pass\n', 'True\n')])
        self.assertEqual(len(proof), 1)
        self.assertNotEqual(proof[0]['original_sha256'], proof[0]['altered_sha256'])

    def test_producer_root_pin_precedes_the_only_reaper(self):
        proc, identity, events = Mock(), Mock(), []
        proc.wait.side_effect = lambda **_kwargs: events.append('wait') or 0
        module, canonical, owner, _log, context = self.execution(proc, identity)
        def pin(_pid):
            self.assertEqual(len(owner.slots), 1)
            self.assertIs(owner.slots[0].proc, proc)
            self.assertTrue(owner.launching)
            events.append('pin')
            return identity
        canonical.pin.side_effect = pin
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal'), patch.object(module, 'open', create=True, return_value=context):
            self.assertEqual(module.execute(['private mock'], Path('/unused/output'), {}, bound=1), 0)
        self.assertEqual(events, ['pin', 'wait'])
        owner.stop.assert_not_called()
        identity.close.assert_called_once()

    def test_missing_producer_pin_retains_record_for_canonical_fallback(self):
        proc = Mock()
        module, canonical, owner, _log, context = self.execution(proc, None)
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal'), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaisesRegex(RuntimeError, 'cannot pin'):
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        owner.stop.assert_called_once()
        self.assertIs(owner.slots[0].proc, proc)
        self.assertIsNone(owner.slots[0].identity)
        proc.wait.assert_not_called()


    def callback_execution(self, proc, identity, phase=None):
        module, canonical, owner, log, context = self.execution(proc, identity)
        retained = module.cleanup_module()
        owner.launching = owner.stopping = False
        owner.stopped_from_outside = Mock(side_effect=lambda *args: retained.Run.stopped_from_outside(owner, *args))
        callbacks, released = {}, []
        def handler(sig, callback):
            if callable(callback):
                callbacks[sig] = callback
            else:
                released.append('restore-' + str(sig))
                if phase == ('restore', sig):
                    callbacks[sig](sig, None)
            return signal.SIG_DFL
        def output_closed():
            released.append('output')
            if phase == 'output':
                callbacks[signal.SIGINT](signal.SIGINT, None)
        def identity_closed():
            released.append('identity')
            if phase == 'identity':
                callbacks[signal.SIGINT](signal.SIGINT, None)
        log.close.side_effect = output_closed
        identity.close.side_effect = identity_closed
        return module, canonical, owner, log, context, callbacks, released, handler

    def test_first_callback_during_normal_release_is_promoted_after_every_release(self):
        for phase in ('output', 'identity', ('restore', signal.SIGTERM), ('restore', signal.SIGINT)):
            proc, identity = Mock(), Mock()
            proc.wait.return_value = 0
            values = self.callback_execution(proc, identity, phase)
            module, canonical, owner, log, context, callbacks, released, handler = values
            with patch.object(module, 'cleanup_module', return_value=canonical), \
                 patch.object(module.subprocess, 'Popen', return_value=proc), \
                 patch.object(module.signal, 'signal', side_effect=handler), patch.object(module, 'open', create=True, return_value=context):
                with self.assertRaises(SystemExit) as caught:
                    module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
            self.assertEqual(caught.exception.code, 1)
            self.assertEqual(released, ['output', 'identity', 'restore-'+str(signal.SIGTERM), 'restore-'+str(signal.SIGINT)])
            owner.stopped_from_outside.assert_called_once()
            owner.stop.assert_not_called()

    def test_secondary_cancellation_from_restored_handler_is_not_stringified(self):
        proc, identity = Mock(), Mock()
        proc.wait.return_value = 0
        values = self.callback_execution(proc, identity)
        module, canonical, owner, log, context, callbacks, released, handler = values
        first = KeyboardInterrupt('restored handler')
        def restored(sig, callback):
            value = handler(sig, callback)
            if not callable(callback) and sig == signal.SIGTERM:
                raise first
            return value
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal', side_effect=restored), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaises(KeyboardInterrupt) as caught:
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        self.assertIs(caught.exception, first)
        self.assertIn('restore-'+str(signal.SIGINT), released)

    def test_first_pending_exception_survives_a_later_installed_callback(self):
        proc, identity = Mock(), Mock()
        proc.wait.return_value = 0
        values = self.callback_execution(proc, identity, 'identity')
        module, canonical, owner, log, context, callbacks, released, handler = values
        first = KeyboardInterrupt('first pending')
        log.close.side_effect = first
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal', side_effect=handler), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaises(KeyboardInterrupt) as caught:
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        self.assertIs(caught.exception, first)
        owner.stopped_from_outside.assert_called_once()

    def test_existing_primary_cancel_keeps_identity_after_a_later_callback(self):
        proc, identity = Mock(), Mock()
        primary = SystemExit(17)
        proc.wait.side_effect = primary
        identity.wait_exited.return_value = False
        values = self.callback_execution(proc, identity, 'identity')
        module, canonical, owner, log, context, callbacks, released, handler = values
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal', side_effect=handler), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaises(SystemExit) as caught:
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        self.assertIs(caught.exception, primary)
        owner.stop.assert_called_once()
        owner.stopped_from_outside.assert_called_once()

    def test_pending_cancel_diagnostics_retain_ordinary_primary_and_secondary_errors(self):
        proc, identity = Mock(), Mock()
        proc.wait.side_effect = RuntimeError('primary timeout')
        identity.wait_exited.return_value = False
        values = self.callback_execution(proc, identity, 'output')
        module, canonical, owner, log, context, callbacks, released, handler = values
        owner.stop.side_effect = OSError('secondary stop failure')
        with patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc), \
             patch.object(module.signal, 'signal', side_effect=handler), \
             patch.object(module, 'open', create=True, return_value=context), patch('builtins.print') as reported:
            with self.assertRaises(SystemExit):
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
        self.assertIn('primary timeout', reported.call_args[0][0])
        self.assertIn('secondary stop failure', reported.call_args[0][0])

    def test_callback_cancel_restores_control_and_prevents_any_next_launch(self):
        proc, identity = Mock(), Mock()
        proc.wait.return_value = 0
        values = self.callback_execution(proc, identity, 'output')
        module, canonical, owner, log, context, callbacks, released, handler = values
        source = Mock(); source.read_bytes.return_value = b'pass\n'
        mutants = [('first', source, 'pass\n', 'True\n'), ('next', source, 'pass\n', 'False\n')]
        calls = []
        def run(*args):
            calls.append(args)
            if len(calls)>1:
                module.execute(['private mock'], Path('/unused/output'), {}, bound=1)
            return True, 1, 0
        with patch.dict(sys.modules, {'mutation_baseline': Mock()}), \
             patch.object(module, 'freeze_profiles'), patch.object(module, 'MUTANTS', mutants), \
             patch.object(module, 'EVIDENCE', Path('/unused')), patch.object(Path, 'write_text'), \
             patch.object(module, 'run_tests', side_effect=run), \
             patch.object(module, 'cleanup_module', return_value=canonical), \
             patch.object(module.subprocess, 'Popen', return_value=proc) as launched, \
             patch.object(module.signal, 'signal', side_effect=handler), patch.object(module, 'open', create=True, return_value=context):
            with self.assertRaises(SystemExit):
                module.main()
        self.assertEqual(len(calls), 2)
        self.assertEqual(launched.call_count, 1)
        self.assertEqual(source.write_bytes.call_args_list, [call(b'True\n'), call(b'pass\n')])


@unittest.skipUnless(sys.platform.startswith('linux') and hasattr(os, 'pidfd_open'), 'Linux pidfds')
class RealPrivateChildren(unittest.TestCase):
    def test_real_high_pidfd_supports_exit_wait(self):
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(120)'], start_new_session=True)
        identity = slots.pin(child.pid)
        high = None
        try:
            self.assertIsNotNone(identity)
            high_fd = fcntl.fcntl(identity.fd, fcntl.F_DUPFD_CLOEXEC, 2048)
            high = slots.PinnedProcess(child.pid, identity.started, high_fd)
            self.assertFalse(high.wait_exited(time.monotonic()))
            high.send(signal.SIGKILL)
            self.assertTrue(high.wait_exited(time.monotonic() + 5))
        finally:
            if high is not None:
                high.close()
            if identity is not None:
                identity.send(signal.SIGKILL)
                identity.close()
            child.wait(timeout=10)

    def test_new_session_descendant_is_killed_and_unrelated_child_remains_running(self):
        child_code = 'import time; time.sleep(120)'
        root_code = ('import subprocess,sys,time; '
                     f'p=subprocess.Popen([sys.executable,"-c",{child_code!r}],start_new_session=True); '
                     'print(p.pid,flush=True); time.sleep(120)')
        root = subprocess.Popen([sys.executable, '-u', '-c', root_code], stdout=subprocess.PIPE,
                                text=True, start_new_session=True)
        root_id = slots.pin(root.pid)
        child_id = bystander_id = bystander = None
        try:
            self.assertIsNotNone(root_id)
            self.assertTrue(select.select([root.stdout], [], [], 10)[0], 'private root announced its child')
            child_pid = int(root.stdout.readline())
            child_id = slots.pin(child_pid)
            self.assertIsNotNone(child_id)
            bystander = subprocess.Popen([sys.executable, '-c', child_code], start_new_session=True)
            bystander_id = slots.pin(bystander.pid)
            self.assertIsNotNone(bystander_id)
            run = slots.Run(slots.ROOT / 'scripts/toy_mutations.py', [], 2, [])
            run.slots = [Mock(identity=root_id, proc=root)]
            run.stop()
            self.assertIsNotNone(root.returncode)
            self.assertTrue(child_id.wait_exited(time.monotonic()), 'descendant has exited before cleanup returns')
            deadline = time.monotonic() + 5
            while slots.process_state(child_pid) not in (None, b'Z', b'X') and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertIn(slots.process_state(child_pid), (None, b'Z', b'X'))
            self.assertTrue(bystander_id.send(0))
            self.assertNotIn(slots.process_state(bystander.pid), (None, b'Z', b'X', b'T', b't'))
        finally:
            for identity in (child_id, root_id, bystander_id):
                if identity is not None:
                    identity.send(signal.SIGKILL)
                    identity.close()
            root.wait(timeout=10)
            root.stdout.close()
            if bystander is not None:
                bystander.wait(timeout=10)


if __name__ == '__main__':
    unittest.main(verbosity=2)
