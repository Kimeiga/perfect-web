#!/usr/bin/env python3
"""Mutation controls for kiokun.com's word page on the development server
(track `kiokun`, W6).

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one rule of the word page, in the program
(`examples/kiokun-site/app.pw`), in the read-only kiokun layer
(`spikes/own-renderer/server/src/kiokun.rs`), or at the host's seam that
chooses it, and at least one of the server's kiokun tests
(`server/src/tests/kiokun.rs`) must then fail or not build.

Run from the repository root; `just e14-kiokun-word` records the output.
The sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = ROOT / "examples/kiokun-site/app.pw"
LAYER = ROOT / "spikes/own-renderer/server/src/kiokun.rs"
HOST = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a file answers for any word whose name it has",
        APP,
        "        Some(e) => if e.key == word { Some(e) } else { None },",
        "        Some(e) => Some(e),",
    ),
    (
        "a stub's redirect is read and not shown",
        APP,
        "                Some(t) => Ok(shown(word, t)),",
        "                Some(t) => Ok(shown(word, e)),",
    ),
    (
        "kiokun's file names escape control characters alone",
        APP,
        "    if unsafe_mark | control { 95 } else { c }",
        "    if control { 95 } else { c }",
    ),
    (
        "a Chinese reading with no senses is shown",
        APP,
        "    List.filter(all, s => List.length(s.definitions) > 0)",
        "    all",
    ),
    (
        "a search-only form is shown",
        APP,
        "    let shown = List.filter(items, h => !List.any(h.tags, t => t == hidden))",
        "    let shown = items",
    ),
    (
        "kiokun's placeholder definition is shown",
        APP,
        '    let kept = List.filter(w.definitions, d => d != "" & d != "Sentence")',
        '    let kept = List.filter(w.definitions, d => d != "")',
    ),
    (
        "one sense is numbered as many are",
        APP,
        "        many: List.length(w.senses) > 1,",
        "        many: List.length(w.senses) > 0,",
    ),
    (
        "a name's kind is shown as its code",
        APP,
        '        "given" => "given name",',
        '        "given" => "given",',
    ),
    (
        "a reading's Jyutping is its pinyin",
        APP,
        "[{c.jyutping}]",
        "[{c.pinyin}]",
    ),
    (
        "any subdirectory is read",
        LAYER,
        "    let hex = subdirectory.len() == 2\n        && subdirectory",
        "    let hex = true\n        || subdirectory",
    ),
    (
        "a file name may hold a separator",
        LAYER,
        " && !file.contains(['/', '\\\\', '\\0']);",
        ";",
    ),
    (
        "a name too long for the file system is read",
        LAYER,
        "    let fits = name.len() <= NAME_MAX;",
        "    let fits = true;",
    ),
    (
        "a repeated id keys two records",
        LAYER,
        '        } else {\n            format!("{id}:{n}")\n        };',
        "        } else {\n            id.clone()\n        };",
    ),
    (
        "the layer promises no strong reads, which the program states",
        LAYER,
        '            reads: BTreeSet::from(["strong".to_string()]),',
        "            reads: BTreeSet::new(),",
    ),
    (
        "the host does not choose the kiokun layer",
        HOST,
        '            .any(|i| i.interface.starts_with("kiokun:"))',
        '            .any(|i| i.interface.starts_with("kiokun-never:"))',
    ),
]

TESTS = [["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "kiokun"]]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
