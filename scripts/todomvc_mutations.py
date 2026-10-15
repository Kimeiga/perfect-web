#!/usr/bin/env python3
"""Mutation controls for track `todomvc`: TodoMVC in Pleris, held to its own
spec (ADR-XXXX).

Each mutant undoes one thing the spec states, or one thing the layer keeps:
- the program (`examples/todomvc/app.pw`): the hidden main and footer, the
  trim, the counter's words and what it counts, the toggle-all's rule and its
  checked state, each filter and the fragment's forms, the editing mode (which
  row, its class, its field and what it holds, Enter, Escape and blur), the
  clear button, a todo's toggle, an edit trimmed to nothing, the key the page
  reads by, and the field's focus and clearing;
- the layer (`spikes/own-renderer/server/src/todomvc.rs`): where a todo is
  added, ids a destroyed todo had, each write, the bound, and the session
  each operation holds a command to;
- the host's media type for the checkbox's drawings;
- the browser's runtime (`spikes/own-renderer/public/pw-runtime.mjs`), where
  a patch sets a checkbox's state as well as its attribute, and the edit
  field a patch inserts takes focus (the runtime's own `autofocus` is
  controlled in `scripts/autofocus_mutations.py`).

**A program mutant must still be a well-typed program**: each is checked with
`pw check` before anything runs, and one that does not check is no mutant,
and fails this script (a rule that refused it would be killing it for the
wrong reason). A mutant is run against the server's tests
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
LAYER = ROOT / "spikes/own-renderer/server/src/todomvc.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RUNTIME_FILE = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

PROGRAM = "program"
LAYERED = "layer"
HOST = "host"
RUNTIME = "runtime"

# (what is undone, which tests, file, anchor, replacement)
MUTANTS = [
    (
        "main is not hidden with no todos",
        PROGRAM,
        APP,
        '            <section class="main" hidden={board.total == 0}>\n',
        '            <section class="main">\n',
    ),
    (
        "the footer is not hidden with no todos",
        PROGRAM,
        APP,
        '            <footer class="footer" hidden={board.total == 0}>\n',
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
        "    let left = List.length(List.filter(todos, (t) => !t.completed))\n",
        "    let left = List.length(List.filter(todos, (t) => t.completed))\n",
    ),
    (
        "the toggle-all completes only when every todo is active",
        PROGRAM,
        APP,
        "    Ok(complete_all(current_session(), List.any(todos_of(current_session()), (t) => !t.completed)))\n",
        "    Ok(complete_all(current_session(), List.all(todos_of(current_session()), (t) => !t.completed)))\n",
    ),
    (
        "the toggle-all is checked with no todos",
        PROGRAM,
        APP,
        'checked={board.left == 0 & board.total > 0}',
        'checked={board.left == 0}',
    ),
    (
        "the toggle-all does what clearing the completed does",
        PROGRAM,
        APP,
        "                    on:press={() => match toggle_all() {\n",
        "                    on:press={() => match clear_completed() {\n",
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
        "        rows: List.map(List.filter(todos, (t) => shown(t, filter)), (t) => row_of(t, editing)),\n",
        "        rows: List.map(todos, (t) => row_of(t, editing)),\n",
    ),
    (
        "the total is what the filter shows",
        PROGRAM,
        APP,
        "        total: List.length(todos),\n",
        "        total: List.length(List.filter(todos, (t) => shown(t, filter))),\n",
    ),
    (
        "the clear button counts what is left",
        PROGRAM,
        APP,
        "        done: List.length(todos) - left,\n",
        "        done: left,\n",
    ),
    (
        "no row is the one being edited",
        PROGRAM,
        APP,
        "editing: t.id.value == editing }\n",
        "editing: false }\n",
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
        "        Some(t) => complete(current_session(), id, !t.completed),\n",
        "        Some(t) => complete(current_session(), id, t.completed),\n",
    ),
    (
        "an edit trimmed to nothing is refused, not destroyed",
        PROGRAM,
        APP,
        "        None => remove(current_session(), id),\n",
        "        None => Err(NotFound),\n",
    ),
    (
        "destroy keeps the todo",
        PROGRAM,
        APP,
        "{\n    remove(current_session(), id)\n}\n\ncommand clear_completed",
        "{\n    Ok(id)\n}\n\ncommand clear_completed",
    ),
    (
        "clear completed clears none",
        PROGRAM,
        APP,
        "    Ok(remove_completed(current_session()))\n",
        "    Ok(0)\n",
    ),
    (
        "a toggle tells no page",
        PROGRAM,
        APP,
        "command toggle(id: TodoId) -> Result<Todo, TodoError>\n    idempotent_by InteractionId\n    emits         TodosChanged(current_session())\n{\n",
        "command toggle(id: TodoId) -> Result<Todo, TodoError>\n    idempotent_by InteractionId\n{\n",
    ),
    (
        "the board is read the same for each filter",
        PROGRAM,
        APP,
        "    key            session, filter, editing\n",
        "    key            session, editing\n",
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
        "                                    editing = row.id\n                                }}>{row.title}</label>\n",
        '                                    editing = ""\n                                }}>{row.title}</label>\n',
    ),
    (
        "beginning an edit forgets the title",
        PROGRAM,
        APP,
        "                                    edit_text = row.title\n",
        '                                    edit_text = ""\n',
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
        '                                    } else if e.key == "Escape" {\n                                        editing = ""\n',
        '                                    } else if e.key == "Escape" {\n                                        editing = editing\n',
    ),
    (
        "blur does not save",
        PROGRAM,
        APP,
        '                                    on:blur={(e: BlurEvent) => if editing != "" {\n',
        '                                    on:blur={(e: BlurEvent) => if editing == "?" {\n',
    ),
    (
        "the answer to a blur's save ends another todo's edit",
        PROGRAM,
        APP,
        '                                    on:blur={(e: BlurEvent) => if editing != "" {\n                                        let saved = editing\n                                        match save_edit(TodoId(saved), edit_text) {\n                                            Ok(_) => if editing == saved {\n',
        '                                    on:blur={(e: BlurEvent) => if editing != "" {\n                                        let saved = editing\n                                        match save_edit(TodoId(saved), edit_text) {\n                                            Ok(_) => if true {\n',
    ),
    (
        "the new-todo field is not cleared",
        PROGRAM,
        APP,
        '                                let typed = draft\n                                draft = ""\n',
        "                                let typed = draft\n                                draft = draft\n",
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
        "a todo is added at the top of the list",
        LAYERED,
        LAYER,
        "                    list.rows.push(row);\n",
        "                    list.rows.insert(0, row);\n",
    ),
    (
        "an id is a destroyed todo's again",
        LAYERED,
        LAYER,
        '                        id: format!("t{}", s.written + added + 1),\n',
        '                        id: format!("t{}", s.rows.len() + added + 1),\n',
    ),
    (
        "a todo is completed the other way",
        LAYERED,
        LAYER,
        "                    if let Some(row) = list.rows.iter_mut().find(|r| r.id == id) {\n                        row.completed = done;\n",
        "                    if let Some(row) = list.rows.iter_mut().find(|r| r.id == id) {\n                        row.completed = !done;\n",
    ),
    (
        "complete-all completes one todo",
        LAYERED,
        LAYER,
        "                    for row in &mut list.rows {\n                        row.completed = done;\n",
        "                    for row in list.rows.iter_mut().take(1) {\n                        row.completed = done;\n",
    ),
    (
        "a retitle changes nothing",
        LAYERED,
        LAYER,
        "                        row.title = title;\n",
        "                        let _ = title;\n",
    ),
    (
        "a destroy removes nothing",
        LAYERED,
        LAYER,
        "                Change::Remove(id) => list.rows.retain(|r| r.id != id),\n",
        "                Change::Remove(_) => {}\n",
    ),
    (
        "clear completed removes every todo",
        LAYERED,
        LAYER,
        "                Change::RemoveCompleted => list.rows.retain(|r| !r.completed),\n",
        "                Change::RemoveCompleted => list.rows.retain(|_| false),\n",
    ),
    (
        "a session's list is unbounded",
        LAYERED,
        LAYER,
        "                    if s.rows.len() + added >= MAX_TODOS {\n",
        "                    if false && s.rows.len() + added >= MAX_TODOS {\n",
    ),
    (
        "an operation acts for another session than its command's",
        LAYERED,
        LAYER,
        "    if called == session {\n        Ok(called)\n",
        "    if true || called == session {\n        Ok(called)\n",
    ),
    (
        "a read gives the first session's todos",
        LAYERED,
        LAYER,
        "            .sessions\n            .get(session)\n            .cloned()\n            .unwrap_or_default();\n        reads_of(session, Arc::new(list))\n",
        "            .sessions\n            .values()\n            .next()\n            .cloned()\n            .unwrap_or_default();\n        reads_of(session, Arc::new(list))\n",
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


def browser_tests(_what=None):
    """(built, passed, failed) over the browser's, after TodoMVC is built
    again from the source as it is and the server with it."""
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


# What each kind of mutant is run against, in order.
RUNS = {
    PROGRAM: [("server", server_tests), ("browser", browser_tests)],
    LAYERED: [("server", server_tests), ("browser", browser_tests)],
    HOST: [("browser", browser_tests)],
    RUNTIME: [("browser", browser_tests)],
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
                if not ok:
                    verdict = "INVALID: the mutant does not check, so it is no mutant"
                    print(said[-600:])
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
    # The page and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
