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
        '    let kept = List.filter(w.senses, s => s.text != "" & s.text != "Sentence")',
        '    let kept = List.filter(w.senses, s => s.text != "")',
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
        "a length is counted in code points, not UTF-16 units",
        APP,
        "(n, c) => n + if c > 65535 { 2 } else { 1 })",
        "(n, c) => n + 1)",
    ),
    (
        "a definition keeps its markup",
        APP,
        "    String.trim(spaced(String.from_codepoints(untagged(String.codepoints(value)))))",
        "    String.trim(spaced(value))",
    ),
    (
        "a long title is not cut",
        APP,
        '    if units(text) <= length { text } else { "{kept}…" }',
        "    text",
    ),
    (
        "a definition is not split at the full-width semicolon",
        APP,
        "(s, c) => if c == 59 | c == 65307 {",
        "(s, c) => if c == 59 {",
    ),
    (
        "a numbered definition keeps its number",
        APP,
        "    if digits > 0 & after {",
        "    if false {",
    ),
    (
        "a classifier and a one-letter piece are named",
        APP,
        "    List.filter(unique_clean(all), v => units(v) > 1 & units(v) <= 96 & !classifier(v))",
        "    List.filter(unique_clean(all), v => units(v) > 0 & units(v) <= 96)",
    ),
    (
        "the learner gloss does not lead the meanings",
        APP,
        "    let shown = List.take(unique_clean(List.concat([gloss], definitions_of(e))), 5)",
        "    let shown = List.take(unique_clean(definitions_of(e)), 5)",
    ),
    (
        "the title names no other form",
        APP,
        '    let suffix = String.join(List.take(List.filter(forms, f => f != word), 3), "・")',
        '    let suffix = String.join(List.take(List.filter(forms, f => f != word), 0), "・")',
    ),
    (
        "a pitch shard hashes code points",
        APP,
        "    if c > 65535 { [55296 + (c - 65536) / 1024, 56320 + (c - 65536) % 1024] } else { [c] }",
        "    [c]",
    ),
    (
        "a reading's own accent is never read",
        APP,
        "    match List.find(readings, r => r.reading == reading) {",
        "    match List.find(readings, r => false) {",
    ),
    (
        "a small kana is a mora of its own",
        APP,
        "            Some(next) => if small_kana(next) {",
        "            Some(next) => if false {",
    ),
    (
        "a fall after the last mora is 中高",
        APP,
        '    if accent == 0 { "平板" } else { if accent == 1 { "頭高" } else { if accent == count { "尾高" } else { "中高" } } }',
        '    if accent == 0 { "平板" } else { if accent == 1 { "頭高" } else { "中高" } }',
    ),
    (
        "the morae after the fall stay high",
        APP,
        "(a > 1 & List.length(out) > 0 & List.length(out) < a)",
        "(a > 1 & List.length(out) > 0)",
    ),
    (
        "a word's readings are read in sorted order",
        LAYER,
        "                Ok(Readings(out))",
        "                out.sort_by(|a, b| a.0.cmp(&b.0));\n                Ok(Readings(out))",
    ),
    (
        "a null accent is 0",
        LAYER,
        "                    out.push((reading, accent.and_then(|a| a.as_i64())));",
        "                    out.push((reading, accent.map(|_| 0)));",
    ),
    (
        "a Chinese example is written in its traditional form",
        APP,
        'Example { text: if e.simp != "" { e.simp } else { e.trad }, translation: e.en }',
        "Example { text: e.trad, translation: e.en }",
    ),
    (
        "two examples are shown before the disclosure",
        APP,
        "        first: List.take(all, 1),",
        "        first: List.take(all, 2),",
    ),
    (
        "an example without a text is shown",
        APP,
        '    let usable = List.filter(items, e => e.text != "")',
        "    let usable = items",
    ),
    (
        "a Japanese sentence's `land` is not read",
        APP,
        'List.find(e.sentences, t => t.land == "jpn" | t.lang == "jpn")',
        'List.find(e.sentences, t => t.lang == "jpn")',
    ),
    (
        "the host does not choose the kiokun layer",
        HOST,
        '            .any(|i| i.interface.starts_with("kiokun:"))',
        '            .any(|i| i.interface.starts_with("kiokun-never:"))',
    ),
    (
        "a word's own file never moves it",
        APP,
        "    let moved = moved_from(first, word)\n",
        "    let moved = moved_from(None, word)\n",
    ),
    (
        "a word is moved to itself",
        APP,
        "            Some(t) => if t != word { Some(t) } else { None },\n",
        "            Some(t) => Some(t),\n",
    ),
    (
        "a move is temporary",
        APP,
        "    redirect_on  KiokunError.Moved permanent\n",
        "    redirect_on  KiokunError.Moved temporary\n",
    ),
    (
        "the page names no move",
        APP,
        "    redirect_on  KiokunError.Moved permanent\n",
        "",
    ),
    (
        "a character's own form counts among its traditional forms",
        APP,
        '    let traditional = List.filter(traditional_of(e), v => v != "" & v != source)\n',
        '    let traditional = List.filter(traditional_of(e), v => v != "")\n',
    ),
    (
        "Contains keeps the file's order",
        APP,
        "    let sorted = List.sort_by(e.contains, (a, b) => by_place(word, a, b))\n",
        "    let sorted = e.contains\n",
    ),
    (
        "Contains is never broken down",
        APP,
        "    List.concat(placed, List.sort_by(rest, (a, b) => by_remaining(chosen.form, a, b)))\n",
        "    List.sort_by(candidates, (a, b) => by_remaining(chosen.form, a, b))\n",
    ),
    (
        "a breakdown of more pieces is better",
        APP,
        "            List.length(candidate) < List.length(now)\n",
        "            List.length(candidate) > List.length(now)\n",
    ),
    (
        "a breakdown of worse words is better",
        APP,
        "                quality_of(candidate) > quality_of(now)\n",
        "                quality_of(candidate) < quality_of(now)\n",
    ),
    (
        "the whole word may be one piece of itself",
        APP,
        "    if stop > end | (cursor == start & stop == end) | !matches_at(form, c.chars, cursor) {\n",
        "    if stop > end | !matches_at(form, c.chars, cursor) {\n",
    ),
    (
        "the rest are not ordered longest first",
        APP,
        "        List.length(b.chars) - List.length(a.chars)\n",
        "        List.length(a.chars) - List.length(b.chars)\n",
    ),
    (
        "a word's forms are never carded together",
        APP,
        "    if List.length(forms) < 2 | length == 0 | List.any(forms, f => List.length(f) != length) {\n",
        "    if true {\n",
    ),
    (
        "a card's definition is never a component gloss",
        APP,
        "        definition: contains_definition(card, glosses),\n",
        "        definition: card.candidate.item.d,\n",
    ),
    (
        "a gloss keeps its source marker",
        APP,
        '    if marker == "" { t } else { String.trim(String.slice(t, 0, String.length(t) - String.length(marker))) }\n',
        "    t\n",
    ),
    (
        "an on'yomi the same as the reading is shown",
        APP,
        '    let jo = if p.jo != p.jp { p.jo } else { "" }\n',
        "    let jo = p.jo\n",
    ),
    (
        "glosses are not read for Contains' previews",
        APP,
        "List.map(l.data.contains, p => p.w)",
        "List.map(List.take(l.data.contains, 0), p => p.w)",
    ),
    (
        "Chinese names are not put last",
        APP,
        "    let first = if chinese { proper } else { common }\n",
        "    let first = if chinese { 0 } else { common }\n",
    ),
    (
        "Japanese common words are not put first",
        APP,
        "    let first = if chinese { proper } else { common }\n",
        "    let first = if chinese { proper } else { 0 }\n",
    ),
    (
        "words are not ranked by frequency",
        APP,
        "        if rank_of(a.item) != rank_of(b.item) { if rank_of(a.item) < rank_of(b.item) { -1 } else { 1 } } else { a.index - b.index }\n",
        "        a.index - b.index\n",
    ),
    (
        "a rank of 0 is the first",
        APP,
        "        Some(n) => if n > 0 { n } else { 1000000000 },\n",
        "        Some(n) => n,\n",
    ),
    (
        "a word read and defined as another is shown twice",
        APP,
        "List.any(out, q => q.w == p.w & reading_of(q) == reading_of(p) & String.trim(q.d) == String.trim(p.d))",
        "false",
    ),
    (
        "a zero-width space is a letter",
        APP,
        "c != 8203 & c != 8204",
        "c != 8204",
    ),
    (
        "Korean words are ranked",
        APP,
        '    if language == "korean" {\n        unique\n',
        '    if language == "none" {\n        unique\n',
    ),
    (
        "every Appears in word is shown at once",
        APP,
        '        first: List.take(shown, 10),\n        rest: List.drop(shown, 10),\n        more: "{List.length(shown) - 10} more items",\n',
        '        first: shown,\n        rest: List.drop(shown, 1000),\n        more: "{List.length(shown) - 10} more items",\n',
    ),
    (
        "an equivalent form's Appears in is not merged",
        APP,
        "        in_chinese: more_previews(into.in_chinese, related.in_chinese),\n",
        "        in_chinese: into.in_chinese,\n",
    ),
    (
        "an equivalent form's Contains is not merged",
        APP,
        "        contains: more_contains(into.contains, related.contains),\n",
        "        contains: into.contains,\n",
    ),
    (
        "a merged word is shown twice",
        APP,
        '    List.fold(more, into, (out, p) => if p.w == "" | List.any(out, s => s.w == p.w) { out } else { List.concat(out, [p]) })\n',
        '    List.fold(more, into, (out, p) => if p.w == "" { out } else { List.concat(out, [p]) })\n',
    ),
    (
        "a preview's common flag is not read",
        LAYER,
        '                        Val::Bool(p.get("c").and_then(|c| c.as_bool()) == Some(true)),\n',
        "                        Val::Bool(false),\n",
    ),
    (
        "a preview's rank is not read",
        LAYER,
        '                    ("fr", maybe_int(p.get("fr"))),\n',
        '                    ("fr", maybe_int(None)),\n',
    ),
]

# Four test threads, not one per core. Each distinct program is compiled
# once per test process and its files written into each test's own TempDir
# (the integrator's ruling of 2026-10-09; ADR-0158 holds). Measured back to
# back with /usr/bin/time -l on 12 cores at a load near 16, the tests alone
# take 76.0 s and 2.71 GB at one thread per core, or 99.0 s and 2.39 GB on
# four (before the cache: 92.8 s and 3.61 GB, or 138.7 s and 2.09 GB). In
# a whole run at one thread per core, ADR-0292's 4 GiB bound still stopped
# the test process in 19 of 69 mutants' runs; on four threads, a whole run
# before the cache stopped none.
TESTS = [
    [
        "cargo",
        "test",
        "--quiet",
        "--locked",
        "-p",
        "pw-dev-server",
        "--",
        "kiokun",
        "--test-threads=4",
    ]
]


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
