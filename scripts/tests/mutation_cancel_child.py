"""Bounded subprocess probe for cancellation through mutation runner mains."""
import importlib.util
import json
import os
from pathlib import Path
from contextlib import ExitStack
import signal
import sys
from types import SimpleNamespace
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))
import mutation_baseline


def load_runner(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def main():
    name, target_name = sys.argv[1:]
    module = load_runner(name)
    target = Path(target_name)
    original = target.read_bytes()
    watch = SimpleNamespace(stopped=False)
    result = SimpleNamespace(valid=True, failed=0, passed=(), body_errors=(), assertion_failed=())
    previous = signal.getsignal(signal.SIGTERM)

    if name == "playwright_assertion_mutations":
        first = module.MUTANTS[0]
        module.MUTANTS = [(first[0], first[1], target, *first[3:]), *module.MUTANTS[1:]]
        expected = [item[1] for item in module.MUTANTS]
        expected += [f"{module.SELECTOR}.BrowserAssertions.extra_{i}"
                     for i in range(module.COUNT - len(module.MUTANTS))]
        result.passed = expected

        def run(selectors, _expected, _evidence):
            if selectors != [module.SELECTOR]:
                if target.read_bytes() == original:
                    raise AssertionError("native stub ran before source mutation")
                os.kill(os.getpid(), signal.SIGTERM)
            return result, "stub native receipt"

        patches = [
            mock.patch.object(mutation_baseline.mutation_bound, "start", return_value=watch),
            mock.patch.object(module.unittest_producer, "load", return_value=(None, None, expected)),
            mock.patch.object(module.producer_process, "run",
                              return_value=SimpleNamespace(returncode=0, stdout="")),
            mock.patch.object(module, "run", side_effect=run),
        ]
    elif name == "todomvc_compat_mutations":
        first = module.MUTANTS[0]
        module.MUTANTS = [(first[0], target, *first[2:]), *module.MUTANTS[1:]]
        expected = [f"{module.SELECTOR}.Compatibility.{method}" for method in module.WITNESSES]
        expected += [f"{module.SELECTOR}.Compatibility.extra_{i}" for i in range(3)]
        result.passed = expected
        calls = 0

        def run(_selector, _expected, _tests, _evidence):
            nonlocal calls
            calls += 1
            if calls > 1:
                if target.read_bytes() == original:
                    raise AssertionError("native stub ran before source mutation")
                os.kill(os.getpid(), signal.SIGTERM)
            return SimpleNamespace(result=result, output="stub native receipt")

        patches = [
            mock.patch.object(mutation_baseline.mutation_bound, "start", return_value=watch),
            mock.patch.object(module.guide_python, "freeze", return_value=expected),
            mock.patch.object(module.guide_python, "run", side_effect=run),
        ]
    else:
        raise ValueError(f"unknown runner: {name}")

    with ExitStack() as stack:
        for patcher in patches:
            stack.enter_context(patcher)
        try:
            module.main()
        except SystemExit as error:
            receipt = {
                "exit": error.code,
                "source_restored": target.read_bytes() == original,
                "handler_restored": signal.getsignal(signal.SIGTERM) is previous,
            }
            print("CANCEL_RECEIPT " + json.dumps(receipt), flush=True)
            return int(error.code)
    raise AssertionError("main returned without the injected SIGTERM")


if __name__ == "__main__":
    raise SystemExit(main())
