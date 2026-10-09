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
        "                Some(t) => Some(followed_from(t, f)),",
        "                Some(_) => Some(f),",
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
        "a Chinese word with empty forms shows no headword",
        APP,
        "{ if w.simp != \"\" { w.simp } else { word } }",
        "{ w.simp }",
    ),
    (
        "a label of kiokun's table is not read",
        APP,
        '        Some(l) => if l.text == "" { None } else { Some(l.text) },',
        "        Some(_) => None,",
    ),
    (
        "a code is looked up as written alone, as kiokun.com does",
        APP,
        "        None => match labelled(labels, kind, underscored(code)) {",
        "        None => match labelled(labels, kind, code) {",
    ),
    (
        "a form's note is looked up by its short code",
        APP,
        '        "rK" => "rkanji",',
        '        "rK" => "rK",',
    ),
    (
        "a part of speech is labelled from another table",
        APP,
        '    let pos = List.map(s.pos, c => label(labels, "pos", c))',
        '    let pos = List.map(s.pos, c => label(labels, "misc", c))',
    ),
    (
        "Mandarin keeps the readings the words do not read",
        APP,
        "    let read = List.filter(frequencies, p => List.any(words, w => w == p))",
        "    let read = frequencies",
    ),
    (
        "Mandarin skips the older sources",
        APP,
        "    if List.length(shown) > 0 { shown } else { if List.length(old) > 0 { old } else { words } }",
        "    if List.length(shown) > 0 { shown } else { words }",
    ),
    (
        "the gloss is the meaning before the keyword",
        APP,
        '    learner(if keyword != "" { keyword } else { if meaning != "" { meaning } else { gloss } })',
        '    learner(if meaning != "" { meaning } else { if keyword != "" { keyword } else { gloss } })',
    ),
    (
        "the gloss keeps its source markers",
        APP,
        '    String.trim(without(without(without(without(gloss, "(trad/jp)"), "(trad)"), "(simp)"), "(jp)"))',
        "    String.trim(gloss)",
    ),
    (
        "a marker is matched in its case alone",
        APP,
        "    match found_at(String.to_lower_ascii(text), marker, 0) {",
        "    match found_at(text, marker, 0) {",
    ),
    (
        "every entry has a character header",
        APP,
        "        shown: has_chinese | has_japanese,",
        "        shown: true,",
    ),
    (
        "an HSK level of 0 is shown",
        APP,
        '            Some(n) => if n != 0 { "HSK {n}" } else { "" },',
        '            Some(n) => "HSK {n}",',
    ),
    (
        "dictionary meanings are shown with no keyword",
        APP,
        '        Some(m) => if m.keyword != "" & m.lexical != "" { m.lexical } else { "" },',
        "        Some(m) => m.lexical,",
    ),
    (
        "a reading the file repeats is shown twice",
        APP,
        '(out, v) => if v == "" | List.any(out, s => s == v) { out }',
        '(out, v) => if v == "" { out }',
    ),
    (
        "KANJIDIC's flat readings are never read",
        LAYER,
        '        ("grouped", Val::Bool(groups.is_some())),',
        '        ("grouped", Val::Bool(true)),',
    ),
    (
        "a hanja reading is its romanization",
        LAYER,
        '        ("readings", texts(k, "readings", "hangul")),',
        '        ("readings", texts(k, "readings", "romanization")),',
    ),
    (
        "the forms' notes are not read from the table",
        LAYER,
        'const LABEL_KINDS: [&str; 5] = ["pos", "misc", "field", "dial", "head_info"];',
        'const LABEL_KINDS: [&str; 4] = ["pos", "misc", "field", "dial"];',
    ),
    (
        "an app without its label table is served with none",
        LAYER,
        '    let path = app.join("src/lib/japanese-labels.json");\n    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;',
        '    let path = app.join("src/lib/japanese-labels.json");\n    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());',
    ),
    (
        "a stub's card yields to its target's",
        APP,
        "    let primary = match stub.mnemonic {",
        "    let primary = match target.mnemonic {",
    ),
    (
        "a stub's traditional variants are not read",
        APP,
        "        Some(s) => if List.length(traditional_of(s)) > 1 { traditional_of(s) } else { none },",
        "        Some(s) => if List.length(traditional_of(s)) > 1 { none } else { none },",
    ),
    (
        "a Chinese word read twice is shown twice",
        APP,
        "fn more_chinese(into: List<ChineseWord>, more: List<ChineseWord>) -> List<ChineseWord> {\n    List.fold(more, into, (out, w) => if List.any(out, s => s.id == w.id) { out } else { List.concat(out, [w]) })",
        "fn more_chinese(into: List<ChineseWord>, more: List<ChineseWord>) -> List<ChineseWord> {\n    List.concat(into, more)",
    ),
    (
        "a simplified form is equivalent whatever its card teaches",
        APP,
        '            if source_concept != "" & source_concept == target_concept { Some(target) } else { None }',
        '            if source_concept != "" { Some(target) } else { None }',
    ),
    (
        "a concept keeps its case and spaces",
        APP,
        "    spaced(String.to_lower(learner(gloss)))",
        "    learner(gloss)",
    ),
    (
        "an equivalent form's words are not merged",
        APP,
        "        Some(t) => if t == canonical { merged_form(into, r) } else { into },",
        "        Some(_) => into,",
    ),
    (
        "a traditional form is not Hong Kong's",
        APP,
        '        if x.character != "" & traditional_role { add_role(t3, x.character, "hong-kong") } else { t3 }',
        "        t3",
    ),
    (
        "a simplified form is written as a traditional one",
        APP,
        '            if has_role(roles, "hong-kong") & List.length(roles) == 1 { "zh-HK" } else { if has_role(roles, "simplified") & !has_role(roles, "traditional") { "zh-Hans" } else { "zh-Hant" } }',
        '            if has_role(roles, "hong-kong") & List.length(roles) == 1 { "zh-HK" } else { "zh-Hant" }',
    ),
    (
        "a form without a card has no meaning",
        APP,
        "        meaning: if carded != \"\" { carded } else { gloss_of(glosses, r.character) },",
        "        meaning: carded,",
    ),
    (
        "a component gloss keeps its source marker",
        APP,
        "        Some(g) => learner(g.gloss),",
        "        Some(g) => g.gloss,",
    ),
    (
        "the batched read reads nothing",
        LAYER,
        "                    out.push(read_one(&r, &subdirectory, &file)?);",
        "                    out.push(Val::Option(None));",
    ),
    (
        "the app's glosses are read from another file",
        LAYER,
        '    let path = app.join("static/game_data/component_glosses.json");',
        '    let path = app.join("static/game_data/component_uses.json");',
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
