//! **kiokun.com's word page, served by the development server** (track
//! `kiokun`, W6): `examples/kiokun-site`, built as `pw build` builds it, its
//! entries read by the read-only kiokun layer from the repository's sample
//! (ADR-0037). kiokun.com's own page is the reference: each test states
//! what it shows, from the file and line it is read from, with its control.

use super::*;

/// kiokun's program, `change` applied to its `app.pw`, built as `pw build`
/// builds it: its directory, and the build in it.
fn built_kiokun_with(change: fn(&str) -> String) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-").expect("a temporary directory");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web", "examples/kiokun-site"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    // kiokun's shard rule, in Pleris since ADR-0041, and the record it names.
    paths.push(root.join("examples/kiokun/Shards.pw"));
    paths.push(root.join("examples/kiokun/dictionary.pw"));
    let units: Vec<pw_core::check::Unit> = paths
        .into_iter()
        .map(|p| {
            let mut src = std::fs::read_to_string(&p).expect("read");
            if p.ends_with("examples/kiokun-site/app.pw") {
                src = change(&src);
            }
            pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path: p.display().to_string(),
                src,
            }
        })
        .collect();
    let build = pw_core::build::build(&units).expect("kiokun builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let out = dir.path().join("build");
    build.write(&out).expect("the build is written");
    (dir, out)
}

fn served_kiokun() -> Served {
    served_kiokun_with(|app| app.to_string())
}

fn served_kiokun_with(change: fn(&str) -> String) -> Served {
    let (dir, out) = built_kiokun_with(change);
    Served {
        server: Server::from_build(out.clone(), out).expect("served"),
        _dir: dir,
    }
}

/// The word page's path for `word`, its one segment percent-encoded.
fn path_of(word: &str) -> String {
    let mut out = String::from("/word/");
    for b in word.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The response to `GET path`, whole: its status line, headers and body,
/// without the renderer's part markers (`<!--pw:s0-->`), which a reader
/// does not see.
fn fetched(s: &Server, path: &str) -> String {
    let whole: String = fetched_as(s, path, Some("a"))
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    let mut out = String::with_capacity(whole.len());
    let mut rest = whole.as_str();
    while let Some(at) = rest.find("<!--pw:") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        rest = &rest[rest.find("-->").map_or(rest.len(), |e| e + 3)..];
    }
    out.push_str(rest);
    out
}

/// The section `id`'s markup, from its opening tag to its end.
fn section<'a>(html: &'a str, id: &str) -> &'a str {
    let at = html
        .find(&format!("<section id=\"{id}\""))
        .unwrap_or_else(|| panic!("no section {id}: {html}"));
    let end = at + html[at..].find("</section>").expect("its end");
    &html[at..end]
}

/// **The development server serves kiokun's program from kiokun's files**:
/// its contracts import `kiokun:data/…`, so the host chooses the kiokun
/// layer, which supplies `entries#read` and grants reading entries alone.
#[test]
fn kiokun_is_served_by_the_host_its_entries_read_from_kiokuns_files() {
    let s = served_kiokun();
    assert_eq!(s.data.grants(), vec!["database.read<Entry>"]);
    assert!(s.data.operations().contains("kiokun:data/entries#read"));
    let page = fetched(&s, &path_of("人"));
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "{page}");
    assert!(page.contains("<h1 id=\"headword\">人</h1>"), "{page}");
}

/// **Each language's words** (`[word]/+page.svelte:707-861`): a Chinese
/// reading `[pinyin]` `[jyutping]` with its numbered senses; a Japanese
/// word's written form, starred where common, its first reading and its
/// senses; a Korean word's hangul, `[hanja]`, part of speech and senses.
#[test]
fn the_word_page_shows_each_languages_words_as_kiokun_does() {
    let s = served_kiokun();
    let page = fetched(&s, &path_of("人"));
    let chinese = visible(section(&page, "chinese"));
    for shown in ["Chinese", "[rén]", "[jan4]", "person", "CL:個|个[gè],位[wèi]"] {
        assert!(chinese.contains(shown), "{shown}: {chinese}");
    }
    let japanese = section(&page, "japanese");
    assert!(japanese.contains("<span class=\"kana-pronunciation\">ひと</span>"), "{japanese}");
    // 1580640 is common, 1366420 is not: a star for one.
    assert!(japanese.contains("★"), "{japanese}");
    assert!(visible(japanese).contains("indicates nationality, race, origin, etc."), "{japanese}");
    // A part of speech is a label of its sense.
    assert!(japanese.contains("<span class=\"tag\">ctr</span>"), "{japanese}");
    let korean = visible(section(&page, "korean"));
    for shown in ["인", "[人]", "Affix", "A suffix used to mean a person."] {
        assert!(korean.contains(shown), "{shown}: {korean}");
    }
    let names = visible(section(&page, "names"));
    for shown in ["Japanese Names", "Jin", "given name", "unclassified name"] {
        assert!(names.contains(shown), "{shown}: {names}");
    }
}

/// **What the slice showed and kiokun.com does not is not shown**
/// (docs/PARALLEL.md, "W6's inventory, answered", Q8): the stroke count,
/// school grade, frequency rank, the Korean character's meanings and a
/// Korean word's pronunciation wait for the owner's ruling.
#[test]
fn what_kiokun_com_does_not_show_is_not_shown() {
    let s = served_kiokun();
    let page = visible(&fetched(&s, &path_of("人")));
    for hidden in ["Strokes", "School grade", "Frequency rank", "Meaning in Korean"] {
        assert!(!page.contains(hidden), "{hidden}: {page}");
    }
}

/// **A sense list is numbered where there is more than one**
/// (`Definitions.svelte`): one sense is shown alone, many as a list.
#[test]
fn one_japanese_sense_is_not_numbered_and_many_are() {
    let s = served_kiokun();
    let page = fetched(&s, &path_of("人"));
    let japanese = section(&page, "japanese");
    assert!(japanese.contains("<div class=\"single-sense\">"), "{japanese}");
    assert!(japanese.contains("<ol class=\"sense-list\">"), "{japanese}");
}

/// **A stub's `redirect` is followed** (`[word]/+page.ts:438-462`): 谚 is
/// the simplified form of 諺, and its file holds only the redirect, so its
/// page shows 諺's words under the word asked for.
#[test]
fn a_stubs_redirect_is_followed_to_the_entry_it_names() {
    let s = served_kiokun();
    let stub = fetched(&s, &path_of("谚"));
    assert!(stub.starts_with("HTTP/1.1 200 OK\r\n"), "{stub}");
    assert!(stub.contains("<h1 id=\"headword\">谚</h1>"), "{stub}");
    let target = fetched(&s, &path_of("諺"));
    assert_eq!(
        visible(section(&stub, "chinese")),
        visible(section(&target, "chinese")),
        "the stub shows the entry it names"
    );
}

/// **A word kiokun has no entry for is not found** (`[word]/+page.ts:407`):
/// 404, and no word's content.
#[test]
fn a_word_kiokun_lacks_is_not_found() {
    let s = served_kiokun();
    let absent = fetched(&s, &path_of("no-such-word-in-kiokun"));
    assert!(absent.starts_with("HTTP/1.1 404 Not Found\r\n"), "{absent}");
    assert!(!absent.contains("<section"), "{absent}");
    // Control: a word it has is found.
    assert!(fetched(&s, &path_of("魚")).starts_with("HTTP/1.1 200 OK\r\n"));
}

/// Writes `json` as `word`'s file in `dir`, where kiokun's build would:
/// the subdirectory its hash names, its name escaped
/// (`create_safe_filename`). Written here in Rust, as the reference the
/// program's own rule is held to.
fn write_entry(dir: &std::path::Path, word: &str, json: &str) {
    let hash = word
        .chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)));
    let sub = format!("{:02x}", hash & 0xff);
    let name: String = word
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    std::fs::create_dir_all(dir.join(&sub)).expect("a subdirectory");
    std::fs::write(
        dir.join(sub).join(format!("{name}.json.deflate")),
        miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 6),
    )
    .expect("written");
}

/// kiokun's program, its entries read from `dir`.
fn served_kiokun_on(dir: &std::path::Path) -> Served {
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(dir.to_path_buf()));
    s
}

/// **A file answers only for the word it records** (ADR-0041): kiokun
/// escapes `/`, `|`, `"` and the rest to `_`, and its subdirectory reads only
/// a hash's low eight bits, so two words can share one file: `a/b/b` and
/// `a|b"b` are both `47/a_b_b.json.deflate`. The file records which word it
/// is, and answers for that word alone.
#[test]
fn a_file_answers_only_for_the_word_it_records() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "a/b/b",
        r#"{"key":"a/b/b","chinese_words":[{"_id":"1","simp":"a/b/b","trad":"a/b/b","items":[{"pinyin":"x","definitions":["the word a/b/b"]}]}]}"#,
    );
    assert!(dir.path().join("47/a_b_b.json.deflate").exists(), "the file");
    let s = served_kiokun_on(dir.path());
    let found = fetched(&s, "/word/a%2Fb%2Fb");
    assert!(found.starts_with("HTTP/1.1 200 OK\r\n"), "{found}");
    assert!(found.contains("the word a/b/b"), "{found}");
    let other = fetched(&s, "/word/a%7Cb%22b");
    assert!(other.starts_with("HTTP/1.1 404 Not Found\r\n"), "{other}");
}

/// **What kiokun.com leaves out, left out**, each with its control: a
/// Chinese reading with no senses (`+page.svelte:713-716`), a Japanese form
/// JMdict marks search-only (`sK`, `WordEntry.svelte:19`), and a Korean
/// definition that is kiokun's placeholder "Sentence" (`+page.svelte:808`).
#[test]
fn what_kiokun_com_leaves_out_is_left_out() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "試",
        r#"{"key":"試",
            "chinese_words":[{"_id":"c","simp":"试","trad":"試","items":[
                {"pinyin":"shì","jyutping":"si3","definitions":["to test"]},
                {"pinyin":"no-senses","definitions":[]}]}],
            "japanese_words":[{"id":"j","kanji":[
                {"text":"試","tags":[],"common":true},
                {"text":"search-only-form","tags":["sK"],"common":false},
                {"text":"rare-form","tags":["rK"],"common":false}],
              "kana":[{"text":"し","tags":[],"common":true}],
              "sense":[{"gloss":[{"text":"trial","type":"lit"},{"text":"test"}],
                        "info":[],"partOfSpeech":["n"],"field":[],"misc":[],"dialect":[]}]}],
            "korean_words":[{"id":"k","hangul":"시","hanja":"試","pos":"Noun",
              "definitions":[{"text":"Sentence"},{"text":"a test"}]}]}"#,
    );
    let s = served_kiokun_on(dir.path());
    let page = fetched(&s, "/word/%E8%A9%A6");
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "{page}");
    let chinese = visible(section(&page, "chinese"));
    assert!(chinese.contains("[shì]") && chinese.contains("[si3]"), "{chinese}");
    assert!(!chinese.contains("no-senses"), "{chinese}");
    // `simp / trad` where they differ.
    assert!(chinese.contains("试 / 試"), "{chinese}");
    let japanese = section(&page, "japanese");
    assert!(japanese.contains("rare-form"), "{japanese}");
    assert!(japanese.contains("<span class=\"info-tag\">(rK)</span>"), "{japanese}");
    assert!(!japanese.contains("search-only-form"), "{japanese}");
    // A gloss's type, and glosses joined by "; ".
    assert!(visible(japanese).contains("(lit) trial; test"), "{japanese}");
    let korean = visible(section(&page, "korean"));
    assert!(korean.contains("a test"), "{korean}");
    assert!(!korean.contains("Sentence"), "{korean}");
}

/// **The layer reads one file of kiokun's, and nothing else**: a
/// subdirectory is two lowercase hexadecimal digits and a name one path
/// segment, so no name the program passes reaches outside kiokun's files.
#[test]
fn the_layer_reads_no_path_but_one_of_kiokuns_files() {
    use crate::kiokun::place;
    assert_eq!(place("ba", "人"), Some(std::path::PathBuf::from("ba/人.json.deflate")));
    for (sub, file) in [
        ("..", "人"),
        ("b", "人"),
        ("BA", "人"),
        ("ba/..", "人"),
        ("ba", ".."),
        ("ba", "."),
        ("ba", ""),
        ("ba", "../ba/人"),
        ("ba", "a\\b"),
        ("ba", "a\0b"),
    ] {
        assert_eq!(place(sub, file), None, "{sub:?} {file:?}");
    }
}

/// **A record listed twice is shown twice, each keyed once**: kiokun.com
/// shows every record its build lists, and a page keys each list by id, so
/// a repeated id is `<id>:<n>`.
#[test]
fn a_record_listed_twice_is_shown_twice_each_keyed_once() {
    let json: serde_json::Value = serde_json::from_str(
        "{\"key\":\"x\",\"japanese_names\":[\
         {\"id\":\"5\",\"kanji\":[],\"kana\":[{\"text\":\"a\"}],\"translation\":[]},\
         {\"id\":\"5\",\"kanji\":[],\"kana\":[{\"text\":\"a\"}],\"translation\":[]},\
         {\"id\":\"5\",\"kanji\":[],\"kana\":[{\"text\":\"b\"}],\"translation\":[]}]}",
    )
    .expect("json");
    let Val::Record(fields) = crate::kiokun::entry(&json) else {
        panic!("a record")
    };
    let names = &fields.iter().find(|(k, _)| k == "names").expect("names").1;
    let Val::List(names) = names else { panic!("a list") };
    let ids: Vec<String> = names
        .iter()
        .map(|n| match n {
            Val::Record(f) => match &f[0].1 {
                Val::String(id) => id.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(ids, ["5", "5:1", "5:2"]);
}
