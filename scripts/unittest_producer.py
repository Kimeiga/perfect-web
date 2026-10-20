#!/usr/bin/env python3
"""Versioned unittest body provenance for the parallel runner's own controls.

The consumer freezes the loader's method IDs before changing any inputs. Reports
are published once, after the suite returns, to a fresh dedicated file. Terminal
unittest text is useful evidence, never the classification protocol.
"""
import argparse
import contextlib
import functools
import inspect
import json
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
from unittest_protocol import SCHEMA
_MISSING = object()
_STD_MODULE_CLEANUPS = unittest.case.doModuleCleanups
_STD_CLASS_CLEANUPS = inspect.getattr_static(unittest.TestCase, 'doClassCleanups')

_STD_CASE = unittest.TestCase
_STD_SUITE = unittest.TestSuite
_STD_RUNNER = unittest.TextTestRunner
_STD_OUTCOME = unittest.case._Outcome
_CASE_HOOKS = {name: inspect.getattr_static(_STD_CASE, name) for name in (
    'run', '__call__', '_callSetUp', '_callTearDown', '_callTestMethod', '_callCleanup',
    'doCleanups', 'doClassCleanups', 'subTest', '_feedErrorsToResult', '_addSkip',
    '_addExpectedFailure', '_addUnexpectedSuccess', 'addCleanup', 'addClassCleanup')
    if hasattr(_STD_CASE, name)}
_SUITE_HOOKS = {name: inspect.getattr_static(_STD_SUITE, name) for name in (
    'run', '__call__', '__iter__', '_removeTestAtIndex', '_handleClassSetUp',
    '_handleModuleFixture', '_handleModuleTearDown', '_tearDownPreviousClass',
    '_get_previous_module', '_createClassOrModuleLevelException', '_addClassOrModuleLevelException')
    if hasattr(_STD_SUITE, name)}
_RUNNER_HOOKS = {name: inspect.getattr_static(_STD_RUNNER, name) for name in (
    '__init__', 'run', '_makeResult') if hasattr(_STD_RUNNER, name)}
_OUTCOME_HOOKS = {name: inspect.getattr_static(_STD_OUTCOME, name) for name in (
    '__init__', 'testPartExecutor') if hasattr(_STD_OUTCOME, name)}
_MODULE_BINDINGS = [(target, name, getattr(target, name)) for target, name in (
    (unittest, 'TestCase'), (unittest.case, 'TestCase'),
    (unittest, 'TestSuite'), (unittest.suite, 'TestSuite'),
    (unittest.suite, '_isnotsuite'), (unittest.suite, '_call_if_exists'),
    (unittest, 'TextTestRunner'), (unittest.runner, 'TextTestRunner'),
    (unittest.case, '_Outcome'),
    (unittest, 'addModuleCleanup'), (unittest.case, 'addModuleCleanup'),
    (unittest, 'doModuleCleanups'), (unittest.case, 'doModuleCleanups'))
    if hasattr(target, name)]


def canonical_problems():
    problems = []
    for target, hooks in ((_STD_CASE, _CASE_HOOKS), (_STD_SUITE, _SUITE_HOOKS),
                          (_STD_RUNNER, _RUNNER_HOOKS), (_STD_OUTCOME, _OUTCOME_HOOKS)):
        for name, original in hooks.items():
            if inspect.getattr_static(target, name, _MISSING) is not original:
                problems.append('canonical execution binding changed: ' + target.__name__ + '.' + name)
    for target, name, original in _MODULE_BINDINGS:
        if getattr(target, name, _MISSING) is not original:
            problems.append('canonical module binding changed: ' + target.__name__ + '.' + name)
    return problems


def same_binding(actual, expected):
    function = getattr(expected, '__func__', _MISSING)
    if function is _MISSING:
        return actual is expected
    return (getattr(actual, '__func__', _MISSING) is function
            and getattr(actual, '__self__', _MISSING) is expected.__self__)


def check_effective(instance, hooks, label):
    for name, original in hooks.items():
        if inspect.getattr_static(type(instance), name, _MISSING) is not original:
            raise ValueError('custom execution descriptor: ' + label + '.' + name)
        expected = original.__get__(instance, type(instance)) if hasattr(original, '__get__') else original
        actual = getattr(instance, name)
        if not same_binding(actual, expected):
            raise ValueError('custom effective execution binding: ' + label + '.' + name)


def require_suite(node):
    if type(node) is not _STD_SUITE:
        raise ValueError('custom TestSuite execution is unsupported')
    check_effective(node, _SUITE_HOOKS, 'TestSuite')


def deferred(value):
    return inspect.isawaitable(value) or inspect.isgenerator(value) or inspect.isasyncgen(value)


class InvocationQueue(list):
    """Keep stdlib's executor/LIFO; observe the actual popped callback result."""
    def __init__(self, result, scope, values):
        super().__init__(values)
        self.result, self.scope = result, scope

    def pop(self, *args):
        self.result.guard_queue(self.scope)
        entry = super().pop(*args)
        if (type(entry) is not tuple or len(entry) != 3 or not callable(entry[0])
                or type(entry[1]) is not tuple or type(entry[2]) is not dict):
            self.result.refuse('unsupported cleanup entry: ' + self.scope['label'])
        callback, positional, keywords = entry
        observed = self.result.invocation(callback, self.scope['label'], scope=self.scope)
        return observed, positional, keywords


def load(selectors):
    loader = unittest.TestLoader()
    suite = loader.loadTestsFromNames(selectors)
    problems = canonical_problems()
    if problems:
        raise ValueError('; '.join(problems))
    def flatten(node):
        if isinstance(node, _STD_SUITE):
            require_suite(node)
            for item in node:
                yield from flatten(item)
        elif isinstance(node, _STD_CASE):
            yield node
        else:
            raise ValueError('unsupported loaded test object')
    tests = list(flatten(suite))
    ids = [test.id() for test in tests]
    if loader.errors:
        raise ValueError('loader/import failure: ' + '\n'.join(loader.errors))
    if not ids or len(ids) != len(set(ids)):
        raise ValueError('empty or duplicate method inventory')
    for test in tests:
        check_effective(test, _CASE_HOOKS, test.id())
        functions = [getattr(test, name) for name in (test._testMethodName, 'setUp', 'tearDown',
                                                     'setUpClass', 'tearDownClass')]
        module = sys.modules[type(test).__module__]
        functions += [getattr(module, name, None) for name in ('setUpModule', 'tearDownModule')]
        if any(inspect.iscoroutinefunction(fn) or inspect.isgeneratorfunction(fn) or inspect.isasyncgenfunction(fn) for fn in functions):
            raise ValueError('async/generator body or fixture is unsupported: ' + test.id())
    return suite, tests, sorted(ids)


def publish(path, report):
    # O_EXCL prevents stale or reused output from becoming fresh evidence.
    with open(path, 'x', encoding='utf-8') as output:
        json.dump(report, output, sort_keys=True)
        output.write('\n')
        output.flush()
        os.fsync(output.fileno())


class BodyResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.states = {}
        self.started = []
        self.finished = []
        self.events = []
        self.refusals = []
        self.bindings = []
        self.queues = []
        self.instance_queues = []
        self.restored = False
        self.scope_errors = []

    def refuse(self, message):
        self.refusals.append(message)
        raise ValueError(message)

    def checked(self, value, label):
        if deferred(value):
            self.refusals.append('deferred invocation result: ' + label)
            if inspect.isasyncgen(value):
                self.refusals.append('async generator has no synchronous cleanup: ' + label)
            else:
                close = getattr(value, 'close', None)
                if callable(close):
                    closed = close()
                    if deferred(closed):
                        self.refusals.append('deferred close result: ' + label)
                        if inspect.iscoroutine(closed) or inspect.isgenerator(closed):
                            closed.close()
                else:
                    self.refusals.append('deferred result has no synchronous close: ' + label)
            return None
        return value

    def bind(self, target, name, installed):
        original = target.__dict__.get(name, _MISSING)
        self.bindings.append((target, name, original, installed))
        setattr(target, name, installed)

    def invocation(self, callback, label, state=None, scope=None):
        @functools.wraps(callback)
        def observed(*args, **kwargs):
            if scope is not None:
                self.guard_queue(scope)
            if state is not None:
                state['fixture'] += 1
            try:
                return self.checked(callback(*args, **kwargs), label)
            except BaseException as error:
                if state is not None:
                    state['errors'].append((error, 'fixture'))
                else:
                    self.scope_errors.append((error, label))
                raise
            finally:
                if state is not None:
                    state['fixture'] -= 1
                if scope is not None:
                    self.guard_queue(scope)
        return observed

    def guard_queue(self, scope):
        if getattr(scope['target'], scope['name']) is not scope['proxy']:
            self.refuse('cleanup storage replaced: ' + scope['label'])

    def install_queue(self, target, name, label):
        original = getattr(target, name)
        if type(original) is not list:
            self.refuse('custom cleanup storage: ' + label)
        scope = {'target': target, 'name': name, 'original': original,
                 'initial': original[:], 'label': label}
        scope['proxy'] = InvocationQueue(self, scope, original)
        self.queues.append(scope)
        self.bind(target, name, scope['proxy'])
        return scope

    def cleanup_executor(self, original, scope):
        @functools.wraps(original)
        def execute(*args, **kwargs):
            self.guard_queue(scope)
            try:
                return original(*args, **kwargs)
            finally:
                self.guard_queue(scope)
        return execute

    def instrument_scopes(self, tests):
        problems = canonical_problems()
        if problems:
            self.refuse('; '.join(problems))
        if unittest.case.doModuleCleanups is not _STD_MODULE_CLEANUPS:
            self.refuse('module cleanup executor replaced')
        module_scope = self.install_queue(unittest.case, '_module_cleanups', 'module cleanup')
        observed = self.cleanup_executor(_STD_MODULE_CLEANUPS, module_scope)
        self.bind(unittest.case, 'doModuleCleanups', observed)
        if hasattr(unittest, 'doModuleCleanups'):
            if unittest.doModuleCleanups is not _STD_MODULE_CLEANUPS:
                self.refuse('public module cleanup executor replaced')
            self.bind(unittest, 'doModuleCleanups', observed)
        for cls in dict.fromkeys(type(test) for test in tests):
            scope = self.install_queue(cls, '_class_cleanups', cls.__module__ + '.' + cls.__qualname__ + ' class cleanup')
            self.bind(cls, 'doClassCleanups', staticmethod(self.cleanup_executor(cls.doClassCleanups, scope)))
            for name in ('setUpClass', 'tearDownClass'):
                self.bind(cls, name, staticmethod(self.invocation(getattr(cls, name), cls.__qualname__ + '.' + name)))
        for module in dict.fromkeys(sys.modules[type(test).__module__] for test in tests):
            for name in ('setUpModule', 'tearDownModule'):
                callback = getattr(module, name, None)
                if callback is not None:
                    self.bind(module, name, self.invocation(callback, module.__name__ + '.' + name))

    def restore(self):
        if self.restored:
            return
        self.restored = True
        for test, original in self.instance_queues:
            if test._cleanups is not original:
                self.refusals.append('instance cleanup storage replaced: ' + test.id())
        for scope in self.queues:
            if getattr(scope['target'], scope['name']) is not scope['proxy']:
                self.refusals.append('cleanup storage replaced: ' + scope['label'])
            original = scope['original']
            initial = scope['initial']
            if len(original) != len(initial) or any(a is not b for a, b in zip(original, initial)):
                self.refusals.append('detached cleanup storage mutated: ' + scope['label'])
            # Restore the original object with ONLY the remaining unconsumed
            # callbacks. Never replay the original pre-run queue.
            original[:] = scope['proxy']
        for target, name, original, installed in reversed(self.bindings):
            if target.__dict__.get(name, _MISSING) is not installed:
                self.refusals.append('instrumented binding replaced: ' + name)
            if original is _MISSING:
                delattr(target, name)
            else:
                setattr(target, name, original)
        self.refusals.extend(canonical_problems())

    def instrument(self, test):
        identity = test.id()
        state = {'id': identity, 'entered': 0, 'completed': 0,
                 'active': False, 'fixture': 0, 'errors': [], 'subtests': {}}
        self.states[identity] = state
        original = getattr(test, test._testMethodName)
        @functools.wraps(original)
        def body(*args, **kwargs):
            state['entered'] += 1
            state['active'] = True
            try:
                return self.checked(original(*args, **kwargs), identity + ' body')
            except BaseException as error:
                state['errors'].append((error, 'body'))
                raise
            finally:
                state['active'] = False
                state['completed'] += 1
        self.bind(test, test._testMethodName, body)
        if type(test._cleanups) is not list:
            self.refuse('custom instance cleanup storage: ' + identity)
        queue = test._cleanups
        self.instance_queues.append((test, queue))
        def guard_instance():
            if test._cleanups is not queue:
                self.refuse('instance cleanup storage replaced: ' + identity)
        for name in ('setUp', 'tearDown'):
            self.bind(test, name, self.invocation(getattr(test, name), identity + ' ' + name, state))
        original_cleanup = test._callCleanup
        def cleanup(callback, *args, **kwargs):
            guard_instance()
            observed = self.invocation(callback, identity + ' cleanup', state)
            try:
                # The original hook discards returns, so monitor its actual
                # callback rather than the hook's outer result.
                return original_cleanup(observed, *args, **kwargs)
            finally:
                guard_instance()
        self.bind(test, '_callCleanup', cleanup)
        original_cleanups = test.doCleanups
        def cleanups():
            guard_instance()
            try:
                return original_cleanups()
            finally:
                guard_instance()
        self.bind(test, 'doCleanups', cleanups)
        original_subtest = test.subTest
        @contextlib.contextmanager
        def subtest(*args, **kwargs):
            with original_subtest(*args, **kwargs):
                # Python3.10 queues result callbacks until after tearDown;
                # retain the actual subtest object and phase at execution.
                item = {'object': test._subtest, 'origin': 'body' if state['active']
                        and not state['fixture'] else 'fixture', 'error': None, 'matched': False}
                state['subtests'][id(test._subtest)] = item
                try:
                    yield
                except BaseException as error:
                    item['error'] = error
                    raise
        self.bind(test, 'subTest', subtest)

    def state(self, test):
        return self.states.get(test.id())

    def startTest(self, test):
        self.started.append(test.id())
        super().startTest(test)

    def stopTest(self, test):
        self.finished.append(test.id())
        super().stopTest(test)

    def event(self, test, kind, error=None, subtest=None):
        state = self.state(test)
        body = False
        if state is not None:
            if subtest is not None:
                item = state['subtests'].get(id(subtest))
                body = (item is not None and item['object'] is subtest
                        and item['origin'] == 'body'
                        and item['error'] is (None if error is None else error[1]))
                if item is not None and item['error'] is (None if error is None else error[1]):
                    item['matched'] = True
            elif error is None:
                body = state['entered'] == state['completed'] == 1
            else:
                # Occurrences, not a set: one exception may be raised again by
                # tearDown. Match each queued callback to its captured throw.
                for index, (caught, phase) in enumerate(state['errors']):
                    if caught is error[1]:
                        del state['errors'][index]
                        body = phase == 'body'
                        break
        if state is None and error is not None:
            for index, (caught, _) in enumerate(self.scope_errors):
                if caught is error[1]:
                    del self.scope_errors[index]
                    break
        origin = 'body' if body else 'fixture'
        self.events.append({'id': test.id(), 'kind': kind, 'origin': origin})
        if not body:
            self.refusals.append('non-body result: ' + test.id() + ': ' + kind)

    def addSuccess(self, test):
        self.event(test, 'success')
        super().addSuccess(test)

    def addFailure(self, test, err):
        self.event(test, 'failure', err)
        super().addFailure(test, err)

    def addError(self, test, err):
        self.event(test, 'error', err)
        super().addError(test, err)

    def addSubTest(self, test, subtest, err):
        kind = 'subtest_success' if err is None else (
            'subtest_failure' if issubclass(err[0], test.failureException) else 'subtest_error')
        self.event(test, kind, err, subtest=subtest)
        super().addSubTest(test, subtest, err)

    def addSkip(self, test, reason):
        self.refusals.append('skip: ' + test.id() + ': ' + reason)
        super().addSkip(test, reason)

    def addExpectedFailure(self, test, err):
        self.refusals.append('expected failure: ' + test.id())
        super().addExpectedFailure(test, err)

    def addUnexpectedSuccess(self, test):
        self.refusals.append('unexpected success: ' + test.id())
        super().addUnexpectedSuccess(test)

    def report(self, expected):
        # A captured throw with no matching callback is never evidence of a
        # clean fixture or a body result, even if forwarding suppressed it.
        unmatched = [label for _, label in self.scope_errors]
        for state in self.states.values():
            unmatched += [state['id'] + ' ' + phase for _, phase in state['errors']]
            unmatched += [state['id'] + ' subtest' for item in state['subtests'].values()
                          if item['error'] is not None and not item['matched']]
        refusals = self.refusals + ['unmatched captured exception: ' + label for label in unmatched]
        failed = sorted({event['id'] for event in self.events
                         if event['kind'] in ('failure', 'error', 'subtest_failure', 'subtest_error')
                         and event['origin'] == 'body'})
        return {'schema': SCHEMA, 'expected': expected, 'started': self.started,
                'finished': self.finished,
                'bodies': [{key: state[key] for key in ('id', 'entered', 'completed')}
                           for state in self.states.values()],
                'events': self.events, 'failed': failed,
                'passed': sorted(set(expected) - set(failed)),
                'refusals': refusals, 'complete': True}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--expected', type=Path)
    parser.add_argument('--inventory', action='store_true')
    parser.add_argument('selectors', nargs='+')
    args = parser.parse_args(argv)
    if args.report.exists():
        parser.error('report path already exists')
    try:
        suite, tests, ids = load(args.selectors)
        if args.inventory:
            publish(args.report, {'schema': SCHEMA, 'ids': ids})
            return 0
        if args.expected is None:
            raise ValueError('frozen expected inventory is required')
        expected = json.loads(args.expected.read_text(encoding='utf-8'))
        if expected != ids:
            raise ValueError('loaded method IDs differ from frozen inventory')
        # Instrument the exact loaded instances; do not discover a second suite.
        class Runner(unittest.TextTestRunner):
            def _makeResult(self):
                result = super()._makeResult()
                self.observed_result = result
                try:
                    result.instrument_scopes(tests)
                    for test in tests:
                        result.instrument(test)
                except BaseException:
                    result.restore()
                    raise
                return result
        runner = Runner(resultclass=BodyResult, verbosity=2)
        try:
            result = runner.run(suite)
        finally:
            if getattr(runner, 'observed_result', None) is not None:
                runner.observed_result.restore()
        report = result.report(expected)
        publish(args.report, report)
        if report['refusals'] or sorted(result.started) != ids or sorted(result.finished) != ids:
            return 2
        return 0 if result.wasSuccessful() else 1
    except Exception as error:
        print('producer refused: ' + type(error).__name__ + ': ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
