#!/usr/bin/env python3
"""Mutation controls for track `todomvc`: TodoMVC in Pleris, held to its own
spec (ADR-0366), its todos on the device since track `device-store`
(ADR-0366).

Each mutant undoes one thing the spec states:
- the program (`examples/todomvc/app.pw`): the todos kept on the device, under
  the spec's name; the hidden main and footer, the trim, the counter's words
  and what it counts, the toggle-all's rule and its checked state, each
  filter and the fragment's forms, the editing mode (which row, its class,
  its field and what it holds, Enter, Escape and blur), the clear button, a
  todo's toggle, an edit trimmed to nothing, an id one more than any before
  it, where a todo is added, and the field's focus and clearing;
- the host's media type for the checkbox's drawings;
- the browser's runtime (`spikes/own-renderer/public/pw-runtime.mjs`), where
  a patch sets a checkbox's state as well as its attribute, and the edit
  field a patch inserts takes focus (the runtime's own `autofocus` is
  controlled in `scripts/autofocus_mutations.py`, its device store and its
  morph in `scripts/device_store_mutations.py`). A mutant of the runtime
  rebuilds the pages too (`run.sh`), as every control that changes it does:
  the Playwright config refuses to run against a `dist` built with another
  runtime, and a suite that did not run kills nothing.

**A program mutant must still be a well-typed program, or name the rule it
takes away** (`scripts/mutant_programs.py`, the integrator's audit of train-7):
each is checked with `pw check` before anything runs, and one that does not
check is no mutant and fails this script (a rule that refused it would be
killing it for the wrong reason), unless the checker's refusal IS what it
undoes and `CHECKED` says so, by the rule's code; such a mutant is killed by the
checker, says so, and runs no test. A mutant is run against the server's tests
(`tests::todomvc::`, which build the program as `pw build` does) and, where
they hold, against the browser's (`e2e/todomvc.spec.mjs`, in Chromium); one
the browser alone kills is one only a browser sees.

Run from the repository root; `just e14-todomvc` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = ROOT / "examples/todomvc/app.pw"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RUNTIME_FILE = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

PROGRAM = "program"
HOST = "host"
RUNTIME = "runtime"

# The program mutants the checker refuses, by the rule that is the protection
# each takes away (`scripts/mutant_programs.py`): every other program mutant
# here leaves a program that checks. Each is killed by the checker, and no test
# is run for it.
CHECKED = {
    # The todos kept on the device say not whose they are.
    "the todos say not whose they are": {"PW6805"},
}

# (what is undone, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "the todos are kept under another name than the spec's",
        PROGRAM,
        APP,
        '    storage  device "todos-pleris"\n',
        '    storage  device "pleris-todos"\n',
    ),
    (
        "the todos say not whose they are",
        PROGRAM,
        APP,
        "    privacy  Device\n    conflict last_write_wins\n",
        "    conflict last_write_wins\n",
    ),
    (
        "main is not hidden with no todos",
        PROGRAM,
        APP,
        '            <section class="main" hidden={List.length(todos) == 0}>\n',
        '            <section class="main">\n',
    ),
    (
        "the footer is not hidden with no todos",
        PROGRAM,
        APP,
        '            <footer class="footer" hidden={List.length(todos) == 0}>\n',
        '            <footer class="footer">\n',
    ),
    (
        "a title is not trimmed",
        PROGRAM,
        APP,
        "    let trimmed = String.trim(draft)\n",
        "    let trimmed = draft\n",
    ),
    (
        "the counter says `items left` of one",
        PROGRAM,
        APP,
        '    if n == 1 {\n        "item left"\n',
        '    if n == 0 {\n        "item left"\n',
    ),
    (
        "the counter counts the completed",
        PROGRAM,
        APP,
        "fn left(todos: List<Todo>) -> Int !{} {\n    List.length(List.filter(todos, (t) => !t.completed))\n",
        "fn left(todos: List<Todo>) -> Int !{} {\n    List.length(List.filter(todos, (t) => t.completed))\n",
    ),
    (
        "the toggle-all completes only when every todo is active",
        PROGRAM,
        APP,
        "    let any_active = List.any(todos, (t) => !t.completed)\n",
        "    let any_active = List.all(todos, (t) => !t.completed)\n",
    ),
    (
        "the toggle-all is checked with no todos",
        PROGRAM,
        APP,
        "    List.length(todos) > 0 & List.all(todos, (t) => t.completed)\n",
        "    List.all(todos, (t) => t.completed)\n",
    ),
    (
        "the toggle-all does what clearing the completed does",
        PROGRAM,
        APP,
        "                    on:press={() => todos = toggled_all(todos)} />\n",
        "                    on:press={() => todos = cleared(todos)} />\n",
    ),
    (
        "the active filter shows the completed",
        PROGRAM,
        APP,
        '    if filter == "active" {\n        !t.completed\n',
        '    if filter == "active" {\n        t.completed\n',
    ),
    (
        "the completed filter shows the active",
        PROGRAM,
        APP,
        '    } else if filter == "completed" {\n        t.completed\n',
        '    } else if filter == "completed" {\n        !t.completed\n',
    ),
    (
        "the default filter shows nothing",
        PROGRAM,
        APP,
        "    } else {\n        true\n    }\n",
        "    } else {\n        false\n    }\n",
    ),
    (
        "the model does not filter",
        PROGRAM,
        APP,
        "    List.map(List.filter(todos, (t) => shown(t, filter)), (t) => Row {\n",
        "    List.map(todos, (t) => Row {\n",
    ),
    (
        "the clear button counts what is left",
        PROGRAM,
        APP,
        "fn done(todos: List<Todo>) -> Int !{} {\n    List.length(List.filter(todos, (t) => t.completed))\n",
        "fn done(todos: List<Todo>) -> Int !{} {\n    List.length(List.filter(todos, (t) => !t.completed))\n",
    ),
    (
        "no row is the one being edited",
        PROGRAM,
        APP,
        "        editing: t.id == editing,\n",
        "        editing: false,\n",
    ),
    (
        "an edited todo that is completed is not shown completed",
        PROGRAM,
        APP,
        '        "completed editing"\n',
        '        "editing"\n',
    ),
    (
        "an edited todo has no editing class",
        PROGRAM,
        APP,
        '    } else if editing {\n        "editing"\n',
        '    } else if editing {\n        ""\n',
    ),
    (
        "a completed todo has no completed class",
        PROGRAM,
        APP,
        '        "completed"\n    } else if editing {\n',
        '        ""\n    } else if editing {\n',
    ),
    (
        "a toggle sets what the todo already is",
        PROGRAM,
        APP,
        "        Todo { id: t.id, title: t.title, completed: !t.completed }\n",
        "        Todo { id: t.id, title: t.title, completed: t.completed }\n",
    ),
    (
        "an edit trimmed to nothing is kept, not destroyed",
        PROGRAM,
        APP,
        "        None => removed(todos, id),\n",
        "        None => todos,\n",
    ),
    (
        "destroy keeps the todo",
        PROGRAM,
        APP,
        "    List.filter(todos, (t) => t.id != id)\n",
        "    List.filter(todos, (t) => t.id == t.id)\n",
    ),
    (
        "clear completed clears none",
        PROGRAM,
        APP,
        "fn cleared(todos: List<Todo>) -> List<Todo> !{} {\n    List.filter(todos, (t) => !t.completed)\n",
        "fn cleared(todos: List<Todo>) -> List<Todo> !{} {\n    todos\n",
    ),
    (
        "a todo is added at the top of the list",
        PROGRAM,
        APP,
        "    List.concat(todos, [Todo { id: last + 1, title: title, completed: false }])\n",
        "    List.concat([Todo { id: last + 1, title: title, completed: false }], todos)\n",
    ),
    (
        "an id is one before it again",
        PROGRAM,
        APP,
        "    let last = List.fold(todos, 0, (m, t) => if t.id > m {\n",
        "    let last = List.fold(todos, 0, (m, t) => if t.id < m {\n",
    ),
    (
        "the active link goes to the completed",
        PROGRAM,
        APP,
        '<li><a class={selected(filter, "active")} href="#/active">Active</a></li>',
        '<li><a class={selected(filter, "active")} href="#/completed">Active</a></li>',
    ),
    (
        "an active address shows every todo",
        PROGRAM,
        APP,
        '"/active" => "active", "/completed"',
        '"/active" => "all", "/completed"',
    ),
    (
        "a completed address shows the active",
        PROGRAM,
        APP,
        '"/completed" => "completed"\n',
        '"/completed" => "active"\n',
    ),
    (
        "the selected link is every other",
        PROGRAM,
        APP,
        '    if filter == mine {\n        "selected"\n',
        '    if filter != mine {\n        "selected"\n',
    ),
    (
        "a double press does not begin an edit",
        PROGRAM,
        APP,
        "                                        editing = row.id\n",
        "                                        editing = 0\n",
    ),
    (
        "beginning an edit forgets the title",
        PROGRAM,
        APP,
        "                                        edit_text = row.title\n",
        '                                        edit_text = ""\n',
    ),
    (
        "the field starts empty",
        PROGRAM,
        APP,
        '<input class="edit" aria-label="Edit" autofocus value={row.title}\n',
        '<input class="edit" aria-label="Edit" autofocus value=""\n',
    ),
    (
        "what is typed in the field is not kept",
        PROGRAM,
        APP,
        "on:input={(e: InputEvent) => edit_text = e.value}",
        'on:input={(e: InputEvent) => edit_text = ""}',
    ),
    (
        "Escape does not leave editing",
        PROGRAM,
        APP,
        '                                        } else if e.key == "Escape" {\n                                            editing = 0\n',
        '                                        } else if e.key == "Escape" {\n                                            editing = editing\n',
    ),
    (
        "blur does not save",
        PROGRAM,
        APP,
        "                                        on:blur={(e: BlurEvent) => if editing != 0 {\n",
        "                                        on:blur={(e: BlurEvent) => if editing == -1 {\n",
    ),
    (
        "Enter does not save",
        PROGRAM,
        APP,
        '                                        on:keydown={(e: KeyEvent) => if e.key == "Enter" {\n                                            todos = retitled(todos, editing, edit_text)\n',
        '                                        on:keydown={(e: KeyEvent) => if e.key == "Enter" {\n                                            todos = todos\n',
    ),
    (
        "the new-todo field is not cleared",
        PROGRAM,
        APP,
        '                                todos = added(todos, title)\n                                draft = ""\n',
        "                                todos = added(todos, title)\n                                draft = draft\n",
    ),
    (
        "a title of blanks clears the field",
        PROGRAM,
        APP,
        "                            None => draft = draft,\n                        }\n                    }} />\n",
        '                            None => draft = "",\n                        }\n                    }} />\n',
    ),
    (
        "the new-todo field is not focused when the page loads",
        PROGRAM,
        APP,
        '<input class="new-todo" placeholder="What needs to be done?" aria-label="New todo" autofocus\n',
        '<input class="new-todo" placeholder="What needs to be done?" aria-label="New todo"\n',
    ),
    (
        "the checkbox's drawings are served as any file",
        HOST,
        SERVER,
        '        Some("svg") => "image/svg+xml",\n',
        '        Some("svg") => "application/octet-stream",\n',
    ),
    (
        "a patch that removes `checked` leaves the box as a person left it",
        RUNTIME,
        RUNTIME_FILE,
        '    if (op.name === "checked" && "checked" in element) element.checked = false;\n',
        "",
    ),
    (
        "a patch that sets `checked` leaves the box as a person left it",
        RUNTIME,
        RUNTIME_FILE,
        '  if (op.name === "checked" && "checked" in element) element.checked = true;\n',
        "",
    ),
    (
        "an edit field inside the row a patch inserts is not given focus",
        RUNTIME,
        RUNTIME_FILE,
        'node.matches("[autofocus]") ? [node] : [...node.querySelectorAll("[autofocus]")]',
        'node.matches("[autofocus]") ? [node] : []',
    ),
]

SERVER_TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "tests::todomvc::",
]
SPEC = ["e2e/todomvc.spec.mjs", "--project=chromium"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def checks():
    """Whether the program, as it is now, checks: what makes a mutant of it a
    mutant and not a refusal."""
    out, code = bounded(
        ["cargo", "run", "--quiet", "--locked", "-p", "pw-cli", "--", "check", "--plain",
         *sorted(map(str, (ROOT / "packages/pw-std").glob("*.pw"))),
         *sorted(map(str, (ROOT / "packages/pw-platform-web").glob("*.pw"))),
         *sorted(map(str, (ROOT / "examples/todomvc").glob("*.pw")))],
        cwd=ROOT,
    )
    return code == 0, out


def server_tests(_what=None):
    """(built, passed, failed) over the server's TodoMVC tests."""
    out, code = bounded(SERVER_TESTS, cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def browser_tests(_what=None, pages=False):
    """(built, passed, failed) over the browser's, after TodoMVC is built
    again from the source as it is and the server with it; with `pages`, the
    demo's pages too (`run.sh`), which the Playwright config holds to the
    runtime it serves."""
    if pages:
        rebuilt = subprocess.run(
            ["bash", "spikes/own-renderer/run.sh"],
            cwd=ROOT,
            env={**os.environ, "BUILD_ONLY": "1"},
            capture_output=True,
            text=True,
        )
        if rebuilt.returncode != 0:
            return False, 0, 0
    built = subprocess.run(
        ["bash", "spikes/own-renderer/todomvc.sh"], cwd=ROOT, capture_output=True, text=True
    )
    if built.returncode != 0:
        return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPEC, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


def runtime_browser_tests(_what=None):
    """The browser's tests, over pages built with the runtime as it is."""
    return browser_tests(pages=True)


# What each kind of mutant is run against, in order.
RUNS = {
    PROGRAM: [("server", server_tests), ("browser", browser_tests)],
    HOST: [("browser", browser_tests)],
    RUNTIME: [("browser", runtime_browser_tests)],
}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for name, run in (("server", server_tests), ("browser", browser_tests)):
        built, passed, failed = run()
        print(f"baseline ({name}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, kind, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        verdict = None
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            if kind == PROGRAM:
                ok, said = checks()
                named = CHECKED.get(what, set())
                refused = set(re.findall(r"error: \[(PW\d{4})\]", said))
                if not ok and refused and refused <= named:
                    verdict = f"KILLED by the checker ({', '.join(sorted(refused))}, the rule this mutant takes away)"
                elif not ok:
                    verdict = "INVALID: the mutant does not check, so it is no mutant"
                    print(said[-600:])
                elif named:
                    verdict = f"INVALID: the script names {sorted(named)}, and the program the mutant leaves checks"
            if verdict is None:
                for name, run in RUNS[kind]:
                    built, passed, failed = run()
                    if not built:
                        verdict = f"KILLED (does not build, {name})"
                    elif failed:
                        verdict = f"KILLED by the {name}'s tests ({failed} of {passed + failed} fail)"
                    if verdict:
                        break
                else:
                    verdict = "SURVIVED"
        finally:
            path.write_text(original)
        if verdict.startswith(("SURVIVED", "INVALID")):
            survivors += 1
        print(f"{what} [{kind}]: {verdict}", flush=True)
    # The pages and the server are built again from the restored source.
    browser_tests(pages=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
