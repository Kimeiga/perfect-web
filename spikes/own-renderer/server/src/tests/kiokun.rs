//! **kiokun.com's word page, served by the development server** (track
//! `kiokun`, W6): `examples/kiokun-site`, built as `pw build` builds it, its
//! entries read by the read-only kiokun layer from the repository's sample
//! (ADR-0037). kiokun.com's own page is the reference: each test states
//! what it shows, from the file and line it is read from, with its control.

use super::*;

/// kiokun's program, `change` applied to its `app.pw`, built as `pw build`
/// builds it: its directory, and the build in it. The program is compiled
/// once per test process for each distinct source (`BUILDS`), and written
/// into this test's own directory.
fn built_kiokun_with(change: fn(&str) -> String) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-").expect("a temporary directory");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/kiokun-site",
    ] {
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
    let sources: Vec<(String, String)> = paths
        .into_iter()
        .map(|p| {
            let mut src = std::fs::read_to_string(&p).expect("read");
            if p.ends_with("examples/kiokun-site/app.pw") {
                src = change(&src);
            }
            (p.display().to_string(), src)
        })
        .collect();
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        sources.hash(&mut h);
        h.finish()
    };
    let cell = BUILDS
        .get_or_init(Default::default)
        .lock()
        .expect("the builds")
        .entry(key)
        .or_default()
        .clone();
    let files = cell
        .get_or_init(|| {
            let units: Vec<pw_core::check::Unit> = sources
                .iter()
                .map(|(path, src)| pw_core::check::Unit {
                    hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
                    path: path.clone(),
                    src: src.clone(),
                })
                .collect();
            Arc::new(compiled(&units))
        })
        .clone();
    let out = dir.path().join("build");
    std::fs::create_dir_all(&out).expect("the build's directory");
    for (rel, bytes) in files.iter() {
        match bytes {
            None => std::fs::create_dir_all(out.join(rel)).expect("a directory"),
            Some(b) => std::fs::write(out.join(rel), b).expect("a built file"),
        }
    }
    (dir, out)
}

/// A build's files: each path relative to the build's directory, with its
/// bytes, or `None` for a directory.
type Files = Vec<(std::path::PathBuf, Option<Vec<u8>>)>;

/// **Each distinct program is compiled once per test process** (the
/// integrator's ruling of 2026-10-09 on e14-kiokun-word's time): the
/// build's files, as bytes, keyed by a hash of the sources compiled. Each
/// test still writes them into its own `TempDir` (ADR-0158), so nothing on
/// disk outlives its test; the cache holds bytes, not a `Build`. The map's
/// lock is held only to find or insert a key's cell, and each key compiles
/// in its own cell, so two programs do not wait for each other.
static BUILDS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u64, Cell>>> =
    std::sync::OnceLock::new();

/// One program's files, compiled by the first test to ask for them.
type Cell = Arc<std::sync::OnceLock<Arc<Files>>>;

/// Every file and directory under `dir`, relative to `base`, in name order.
fn files_under(base: &std::path::Path, dir: &std::path::Path, out: &mut Files) {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("the build's directory")
        .map(|e| e.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let rel = path
            .strip_prefix(base)
            .expect("under the build")
            .to_path_buf();
        if path.is_dir() {
            out.push((rel, None));
            files_under(base, &path, out);
        } else {
            out.push((rel, Some(std::fs::read(&path).expect("a built file"))));
        }
    }
}

/// `units` built as `pw build` builds them, its files read back from a
/// scratch directory dropped right after.
fn compiled(units: &[pw_core::check::Unit]) -> Files {
    let build = pw_core::build::build(units).expect("kiokun builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let scratch =
        tempfile::TempDir::with_prefix("pw-kiokun-build-").expect("a temporary directory");
    let out = scratch.path().join("build");
    build.write(&out).expect("the build is written");
    let mut files = Files::new();
    files_under(&out, &out, &mut files);
    files
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

/// The response to `GET path` as the server sent it, its bytes read to the
/// close and decoded once, as UTF-8 or not at all. `fetched_as` decodes
/// each read on its own, so a character a read boundary splits comes out as
/// U+FFFD: a reader's error, not the server's, and one that depends on
/// timing (on a loaded machine the oracle once found "に��める" for
/// "にしめる").
fn fetched_whole(s: &Server, path: &str) -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let at = listener.local_addr().expect("address");
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let (stream, _) = listener.accept().expect("accept");
            handle(s, stream);
        });
        let mut client = TcpStream::connect(at).expect("connect");
        client
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: t\r\nCookie: pw-session=a\r\n\r\n")
                    .as_bytes(),
            )
            .expect("request");
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).expect("read");
        String::from_utf8(bytes).expect("the response is UTF-8")
    })
}

/// The response to `GET path`, whole: its status line, headers and body,
/// without the renderer's part markers (`<!--pw:s0-->`), which a reader
/// does not see.
fn fetched(s: &Server, path: &str) -> String {
    let whole = fetched_whole(s, path);
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
    assert_eq!(
        s.data.grants(),
        vec![
            "database.read<Entry>",
            "database.read<Label>",
            "database.read<CharGloss>",
            "database.read<PitchReading>"
        ]
    );
    assert!(s.data.operations().contains("kiokun:data/entries#read"));
    assert!(s.data.operations().contains("kiokun:data/labels#japanese"));
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
    for shown in [
        "Chinese",
        "[rén]",
        "[jan4]",
        "person",
        "CL:個|个[gè],位[wèi]",
    ] {
        assert!(chinese.contains(shown), "{shown}: {chinese}");
    }
    let japanese = section(&page, "japanese");
    assert!(
        japanese.contains("<span class=\"kana-pronunciation\">ひと</span>"),
        "{japanese}"
    );
    // 1580640 is common, 1366420 is not: a star for one.
    assert!(japanese.contains("★"), "{japanese}");
    assert!(
        visible(japanese).contains("indicates nationality, race, origin, etc."),
        "{japanese}"
    );
    // A part of speech is a label of its sense.
    assert!(
        japanese.contains("<span class=\"tag\">ctr</span>"),
        "{japanese}"
    );
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
    for hidden in [
        "Strokes",
        "School grade",
        "Frequency rank",
        "Meaning in Korean",
    ] {
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
    assert!(
        japanese.contains("<div class=\"single-sense\">"),
        "{japanese}"
    );
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
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
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
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(dir.to_path_buf(), None));
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
    assert!(
        dir.path().join("47/a_b_b.json.deflate").exists(),
        "the file"
    );
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
    assert!(
        chinese.contains("[shì]") && chinese.contains("[si3]"),
        "{chinese}"
    );
    assert!(!chinese.contains("no-senses"), "{chinese}");
    // `simp / trad` where they differ.
    assert!(chinese.contains("试 / 試"), "{chinese}");
    let japanese = section(&page, "japanese");
    assert!(japanese.contains("rare-form"), "{japanese}");
    assert!(
        japanese.contains("<span class=\"info-tag\">(rK)</span>"),
        "{japanese}"
    );
    assert!(!japanese.contains("search-only-form"), "{japanese}");
    // A gloss's type, and glosses joined by "; ".
    assert!(
        visible(japanese).contains("(lit) trial; test"),
        "{japanese}"
    );
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
    assert_eq!(
        place("ba", "人"),
        Some(std::path::PathBuf::from("ba/人.json.deflate"))
    );
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
    let Val::List(names) = names else {
        panic!("a list")
    };
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

/// **A word too long to be a file name is no word of kiokun's**: a file
/// name is at most 255 bytes (`NAME_MAX` on macOS and Linux), so kiokun's
/// build wrote no `<file>.json.deflate` longer, and such a word is a 404, not
/// a read the file system refuses (ENAMETOOLONG, which Rust's `std` reports
/// as `InvalidFilename`, not `NotFound`). A word at the limit is read.
#[test]
fn a_word_too_long_to_be_a_file_name_is_not_found() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    // Every subdirectory, as a whole build has: a missing one would answer
    // `NotFound` before the name's length is read.
    for n in 0..256 {
        std::fs::create_dir_all(dir.path().join(format!("{n:02x}"))).expect("a subdirectory");
    }
    // `<242 a's>.json.deflate` is 255 bytes: the longest name there is.
    let longest = "a".repeat(242);
    write_entry(
        dir.path(),
        &longest,
        &format!(
            r#"{{"key":"{longest}","chinese_words":[{{"_id":"1","simp":"x","trad":"x","items":[{{"pinyin":"x","definitions":["the longest word"]}}]}}]}}"#
        ),
    );
    let s = served_kiokun_on(dir.path());
    let found = fetched(&s, &format!("/word/{longest}"));
    assert!(found.starts_with("HTTP/1.1 200 OK\r\n"), "{found}");
    assert!(found.contains("the longest word"), "{found}");
    // One byte more, and a hundred CJK characters (300 bytes).
    for word in ["a".repeat(243), "人".repeat(100)] {
        let page = fetched(&s, &path_of(&word));
        assert!(
            page.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "{}: {page}",
            word.len()
        );
    }
    use crate::kiokun::place;
    assert!(place("00", &longest).is_some());
    assert_eq!(place("00", &"a".repeat(243)), None);
}
/// The repository's sample (ADR-0037), as the layer reads it by default.
fn sample() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/kiokun/data/han-1char-3")
}

/// An app beside kiokun's files whose label table is `labels`, as
/// `sveltekit-app/src/lib/japanese-labels.json` is laid out.
fn app_with_labels(labels: &str) -> tempfile::TempDir {
    app_with(labels, "{}")
}

/// An app whose label table is `labels` and whose component glosses are
/// `glosses` (`static/game_data/component_glosses.json`).
fn app_with(labels: &str, glosses: &str) -> tempfile::TempDir {
    let app = tempfile::TempDir::with_prefix("pw-kiokun-app-").expect("a directory");
    std::fs::create_dir_all(app.path().join("src/lib")).expect("its lib");
    std::fs::write(app.path().join("src/lib/japanese-labels.json"), labels).expect("written");
    std::fs::create_dir_all(app.path().join("static/game_data")).expect("its game data");
    std::fs::write(
        app.path().join("static/game_data/component_glosses.json"),
        glosses,
    )
    .expect("written");
    app
}

/// **A code is shown by kiokun.com's label for it** (`japaneseLabels.ts`):
/// a part of speech and a form's note from their own tables; a code a table
/// lacks, as it is. kiokun.com's table spells `adj_na` where JMdict's code
/// is `adj-na`, and kiokun.com shows `adj-na`; this page shows the label the
/// table means (the integrator's ruling of 2026-10-09).
#[test]
fn a_code_is_shown_by_kiokuns_label_for_it() {
    let app = app_with_labels(
        r#"{"labels":{"pos":{"n":"noun","ctr":"counter","adj_na":"na adj."},
                      "misc":{"uk":"usually kana"},"field":{},"dial":{},
                      "head_info":{"rkanji":"rare","io":"irreg."}}}"#,
    );
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(sample(), Some(app.path())));
    let japanese = section(&fetched(&s, &path_of("人")), "japanese").to_string();
    assert!(
        japanese.contains("<span class=\"tag\">counter</span>"),
        "{japanese}"
    );
    assert!(
        japanese.contains("<span class=\"tag\">noun</span>"),
        "{japanese}"
    );
    assert!(
        !japanese.contains("<span class=\"tag\">ctr</span>"),
        "{japanese}"
    );
    // 空's senses carry `adj-na`, which the table spells `adj_na`.
    let sky = section(&fetched(&s, &path_of("空")), "japanese").to_string();
    assert!(sky.contains("<span class=\"tag\">na adj.</span>"), "{sky}");
    assert!(!sky.contains("<span class=\"tag\">adj-na</span>"), "{sky}");
    // A code in neither spelling is shown as it is: `adj-no`.
    assert!(sky.contains("<span class=\"tag\">adj-no</span>"), "{sky}");
    // A form's note, `rK` read as the table's `rkanji`.
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "稀",
        r#"{"key":"稀","japanese_words":[{"id":"j","kanji":[{"text":"稀","tags":["rK"],"common":false}],
            "kana":[{"text":"まれ","tags":[],"common":false}],"sense":[]}]}"#,
    );
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(
        dir.path().to_path_buf(),
        Some(app.path()),
    ));
    let rare = section(&fetched(&s, &path_of("稀")), "japanese").to_string();
    assert!(
        rare.contains("<span class=\"info-tag\">(rare)</span>"),
        "{rare}"
    );
    // Control: with no app, a code is shown as it is. A server of its own,
    // since a word's page is in the shared cache once read.
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(sample(), None));
    let bare = section(&fetched(&s, &path_of("人")), "japanese").to_string();
    assert!(bare.contains("<span class=\"tag\">ctr</span>"), "{bare}");
}

/// **An app named without its label table is refused**, not served with
/// codes where labels were meant.
#[test]
fn an_app_without_its_label_table_is_refused() {
    let empty = tempfile::TempDir::with_prefix("pw-kiokun-app-").expect("a directory");
    assert!(crate::kiokun::labels_of(empty.path()).is_err());
    assert!(crate::kiokun::app_of(empty.path()).is_err());
    // Its component glosses too: a label table alone is refused.
    std::fs::create_dir_all(empty.path().join("src/lib")).expect("its lib");
    std::fs::write(empty.path().join("src/lib/japanese-labels.json"), "{}").expect("written");
    assert!(crate::kiokun::labels_of(empty.path()).is_ok());
    assert!(crate::kiokun::app_of(empty.path()).is_err());
}

/// What a page renders, before the scripts that carry its blocks'
/// templates for the browser: those hold every block's markup, shown or not.
fn rendered(html: &str) -> &str {
    &html[..html.find("<script").unwrap_or(html.len())]
}

/// The character header's text.
fn header_of(html: &str) -> String {
    let start = html
        .find("<div id=\"character-header\"")
        .unwrap_or_else(|| panic!("no header: {html}"));
    let end = start + html[start..].find("<section").unwrap_or(html.len() - start);
    visible(&html[start..end])
}

/// The text of the element whose id is `id`.
fn text_of(html: &str, id: &str) -> String {
    let at = html
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no {id}: {html}"));
    let start = at + html[at..].find('>').expect("its tag's end") + 1;
    let end = start + html[start..].find("</").expect("its end");
    visible(&html[start..end])
}

/// **The character header** (`[word]/+page.svelte:572-690`): the learner
/// gloss with the HSK and former JLPT levels beside it, and the readings,
/// each labelled. 人's Mandarin is the frequency list's `rén` alone, since
/// its words read `rén` and not `ren`.
#[test]
fn the_character_header_shows_its_gloss_levels_and_readings() {
    let s = served_kiokun();
    let person = fetched(&s, &path_of("人"));
    assert_eq!(text_of(&person, "entry-gloss"), "person");
    let header = header_of(&person);
    for shown in [
        "HSK 1",
        "N4",
        "Mandarin",
        "rén",
        "Cantonese",
        "jan4",
        "Korean",
        "인",
    ] {
        assert!(header.contains(shown), "{shown}: {header}");
    }
    assert!(!header.contains("rén, ren"), "{header}");
    let music = fetched(&s, &path_of("樂"));
    // No keyword: the mnemonic's meaning.
    assert_eq!(text_of(&music, "entry-gloss"), "music");
    let header = header_of(&music);
    for shown in [
        "HSK 10",
        "N3",
        "lè, yuè",
        "lok6",
        "ガク、ラク、ゴウ",
        "たの.しい、たの.しむ、この.む",
        "악, 락, 요",
    ] {
        assert!(header.contains(shown), "{shown}: {header}");
    }
}

/// **The header's own rules**, each with its control: no header for an
/// entry with no Chinese or Japanese character; a source marker taken off
/// the gloss (`normalizeLearnerGloss`); the keyword before the meaning, and
/// the dictionary meanings shown beside a keyword; Mandarin from older
/// sources where the frequency list has none, and from the words where
/// neither has; a level of 0 not shown.
#[test]
fn the_character_headers_rules_hold() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "あ",
        r#"{"key":"あ","japanese_words":[{"id":"j","kanji":[],"kana":[{"text":"あ","tags":[],"common":true}],"sense":[]}]}"#,
    );
    write_entry(
        dir.path(),
        "甲",
        r#"{"key":"甲","chinese_char":{"gloss":"shell (trad/jp) armour (TRAD)","pinyinFrequencies":[],
              "oldPronunciations":[{"pinyin":"jiǎ"}],"cantonese":[],"statistics":{"hskLevel":0}},
            "semantic_mnemonic":{"mnemonic_keyword":"turtle","meaning":"shell","lexical_gloss":"first; shell"}}"#,
    );
    write_entry(
        dir.path(),
        "乙",
        r#"{"key":"乙","chinese_words":[{"_id":"1","simp":"乙","trad":"乙","items":[{"pinyin":"yǐ","definitions":["second"]}]}],
            "chinese_char":{"gloss":"second (simp)","pinyinFrequencies":[],"oldPronunciations":[]}}"#,
    );
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(
        dir.path().to_path_buf(),
        None,
    ));
    let kana = fetched(&s, &path_of("あ"));
    assert!(kana.starts_with("HTTP/1.1 200 OK\r\n"), "{kana}");
    assert!(!rendered(&kana).contains("character-header"), "{kana}");
    let shell = fetched(&s, &path_of("甲"));
    assert_eq!(text_of(&shell, "entry-gloss"), "turtle");
    let header = header_of(&shell);
    assert!(
        header.contains("Mnemonic keyword · Actual meanings: first; shell"),
        "{header}"
    );
    assert!(header.contains("jiǎ"), "{header}");
    assert!(!header.contains("HSK"), "{header}");
    let second = fetched(&s, &path_of("乙"));
    assert_eq!(text_of(&second, "entry-gloss"), "second");
    // A Chinese word whose forms are empty is shown as the word looked up
    // (`trad || simp || word`).
    write_entry(
        dir.path(),
        "丁",
        r#"{"key":"丁","chinese_words":[{"_id":"1","simp":"","trad":"","items":[{"pinyin":"dīng","definitions":["fourth"]}]}]}"#,
    );
    let fourth = fetched(&s, &path_of("丁"));
    let chinese = section(&fourth, "chinese");
    assert!(
        chinese.contains("<span class=\"chinese-word-text\">"),
        "{chinese}"
    );
    assert!(visible(chinese).contains("丁"), "{chinese}");
    assert!(!rendered(&second).contains("mnemonic-keyword"), "{second}");
    let header = header_of(&second);
    assert!(
        header.contains("Mandarin") && header.contains("yǐ"),
        "{header}"
    );
    // The markers, ignoring case, and the space before each.
    write_entry(
        dir.path(),
        "丙",
        r#"{"key":"丙","chinese_char":{"gloss":"third (trad/jp) bright (TRAD)  (Jp)","cantonese":["bing2","bing2"]},
            "japanese_char":{"misc":{},"readingMeaning":{"readings":[{"type":"ja_on","value":"ヘイ"}]}},
            "semantic_mnemonic":{"mnemonic_keyword":"","meaning":"","lexical_gloss":"fire"}}"#,
    );
    let third = fetched(&s, &path_of("丙"));
    assert_eq!(text_of(&third, "entry-gloss"), "third bright");
    let header = header_of(&third);
    // A reading once, however often the file lists it.
    assert!(
        header.contains("bing2") && !header.contains("bing2, bing2"),
        "{header}"
    );
    // KANJIDIC's older flat list, where the file has no groups.
    assert!(header.contains("ヘイ"), "{header}");
    // Dictionary meanings with no keyword are not shown as a keyword's.
    assert!(!rendered(&third).contains("mnemonic-keyword"), "{third}");
}

/// The header's written forms, each as `form-roles` shows it: its
/// character and its short roles.
fn forms_of(html: &str) -> Vec<String> {
    let header = &html[html
        .find("<div id=\"character-header\"")
        .expect("the header")..];
    let mut out = Vec::new();
    let mut rest = header;
    // An element's attributes may carry the renderer's own (`data-pw`), so
    // each is found by its class, not by its tag's opening.
    while let Some(at) = rest.find("class=\"character-specimen\"") {
        rest = &rest[at..];
        let character = text_of_class(rest, " lang=");
        let roles = &rest[rest.find("class=\"form-roles\"").expect("its roles")..];
        let hidden = &roles[roles.find("aria-hidden=\"true\"").expect("its short roles")..];
        let short = &hidden[hidden.find('>').expect("its tag's end") + 1..];
        let short = &short[..short.find('<').expect("its end")];
        out.push(format!("{character} {short}"));
        rest = &rest[1..];
    }
    out
}

/// The text inside the first element whose tag begins `open`.
fn text_of_class(html: &str, open: &str) -> String {
    let at = html.find(open).expect("the element");
    let start = at + html[at..].find('>').expect("its tag's end") + 1;
    let end = start + html[start..].find('<').expect("its end");
    html[start..end].to_string()
}

/// **The header's written forms** (`buildCharacterHeaderForms`): 樂 is
/// traditional, Hong Kong's and Korean; 乐, its simplified form; 楽, its
/// Japanese form; and the hanja table's form, U+F914, the compatibility
/// ideograph, a code point of its own and so a form of its own, as
/// kiokun.com shows it. Each form's meaning is its card's, else kiokun.com's
/// component gloss, its source marker removed.
#[test]
fn the_header_shows_each_written_form_with_its_roles() {
    let app = app_with("{}", r#"{"乐":"music (simp)","楽":"fun (jp)"}"#);
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(sample(), Some(app.path())));
    let music = fetched(&s, &path_of("樂"));
    assert_eq!(
        forms_of(&music),
        ["樂 Trad · HK · KR", "乐 Simp", "楽 JP", "\u{F914} KR"]
    );
    let header = header_of(&music);
    for meaning in ["music", "fun"] {
        assert!(header.contains(meaning), "{meaning}: {header}");
    }
    assert!(
        !header.contains("(simp)") && !header.contains("(jp)"),
        "{header}"
    );
    assert!(
        music.contains("aria-label=\"樂: Traditional / Hong Kong / Korean\""),
        "{music}"
    );
    assert!(music.contains("lang=\"zh-Hans\">乐</div>"), "{music}");
    assert!(music.contains("lang=\"ja\">楽</div>"), "{music}");
}

/// **A stub's page shows its target's forms and its own** (the related
/// forms, `+page.ts:236-280`): 谚 is 諺's simplified form, read as the
/// related form it is. 諺 is the hanja table's character too.
#[test]
fn a_stubs_page_shows_its_targets_forms_and_its_own() {
    let s = served_kiokun();
    let proverb = fetched(&s, &path_of("谚"));
    assert_eq!(forms_of(&proverb), ["諺 Trad · HK · JP · KR", "谚 Simp"]);
}

/// **A stub's card stays the page's** (`+page.ts:438-462`): the stub's
/// meaning heads the page, not its target's; with no card of its own, the
/// target's.
#[test]
fn a_stubs_card_heads_the_page_it_names() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "乙",
        r#"{"key":"乙","chinese_char":{"char":"乙"},
            "semantic_mnemonic":{"character":"乙","meaning":"target meaning"}}"#,
    );
    write_entry(
        dir.path(),
        "甲",
        r#"{"key":"甲","redirect":"乙","semantic_mnemonic":{"character":"甲","meaning":"stub meaning"}}"#,
    );
    write_entry(dir.path(), "丙", r#"{"key":"丙","redirect":"乙"}"#);
    let s = served_kiokun_on(dir.path());
    assert_eq!(
        text_of(&fetched(&s, &path_of("甲")), "entry-gloss"),
        "stub meaning"
    );
    assert_eq!(
        text_of(&fetched(&s, &path_of("丙")), "entry-gloss"),
        "target meaning"
    );
}

/// **A stub of several traditional forms shows each one's Chinese words**
/// (`+page.ts:464-492`), once by id; a stub of one shows its target's alone.
#[test]
fn a_stub_of_several_traditional_forms_shows_each_ones_words() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    let word = |key: &str, id: &str, def: &str| {
        format!(
            r#"{{"key":"{key}","chinese_words":[{{"_id":"{id}","simp":"当","trad":"{key}","items":[{{"pinyin":"dāng","definitions":["{def}"]}}]}}]}}"#
        )
    };
    write_entry(dir.path(), "當", &word("當", "a", "to serve as"));
    write_entry(dir.path(), "噹", &word("噹", "b", "a clang"));
    write_entry(
        dir.path(),
        "当",
        r#"{"key":"当","redirect":"當","chinese_char":{"char":"当","tradVariants":["當","噹"]}}"#,
    );
    write_entry(
        dir.path(),
        "档",
        r#"{"key":"档","redirect":"當","chinese_char":{"char":"档","tradVariants":["當"]}}"#,
    );
    let s = served_kiokun_on(dir.path());
    let both = visible(section(&fetched(&s, &path_of("当")), "chinese"));
    assert!(
        both.contains("to serve as") && both.contains("a clang"),
        "{both}"
    );
    // 當 is read twice, as the target and as a variant: its word once.
    assert_eq!(both.matches("to serve as").count(), 1, "{both}");
    let one = visible(section(&fetched(&s, &path_of("档")), "chinese"));
    assert!(
        one.contains("to serve as") && !one.contains("a clang"),
        "{one}"
    );
}

/// **An equivalent simplified form's words are shown on its traditional
/// page** (`mergeEquivalentFormData`): 后 is the simplified form of 後 alone
/// and their cards teach one concept, so 後's page shows 后's words; 余,
/// whose card teaches another, keeps its own.
#[test]
fn an_equivalent_forms_words_are_shown_on_the_canonical_page() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "後",
        r#"{"key":"後","chinese_char":{"char":"後","simpVariants":["后","余"]},
            "chinese_words":[{"_id":"t","simp":"后","trad":"後","items":[{"pinyin":"hòu","definitions":["behind"]}]}],
            "contains":[{"w":"後","d":"behind"}],
            "contained_in_chinese":[{"w":"後面","p":"hòu miàn","d":"behind","fr":10},{"w":"然後","p":"rán hòu","d":"then"}]}"#,
    );
    write_entry(
        dir.path(),
        "后",
        r#"{"key":"后","simplified_form_of":"後","chinese_char":{"char":"后","tradVariants":["後"]},
            "semantic_mnemonic":{"character":"后","meaning":"Back"},
            "semantic_mnemonic_variants":[{"character":"後","meaning":"back "}],
            "chinese_words":[{"_id":"s","simp":"后","trad":"后","items":[{"pinyin":"hòu","definitions":["queen"]}]}],
            "contains":[{"w":"后","p":"hòu","d":"queen"},{"w":"後","d":"behind"}],
            "contained_in_chinese":[{"w":"後面","p":"hòu mian","d":"x"},{"w":"后来","p":"hòu lái","d":"afterwards","fr":20}]}"#,
    );
    write_entry(
        dir.path(),
        "余",
        r#"{"key":"余","simplified_form_of":"後","chinese_char":{"char":"余","tradVariants":["後"]},
            "semantic_mnemonic":{"character":"余","meaning":"surplus"},
            "semantic_mnemonic_variants":[{"character":"後","meaning":"back"}],
            "chinese_words":[{"_id":"r","simp":"余","trad":"余","items":[{"pinyin":"yú","definitions":["surplus"]}]}],
            "contained_in_chinese":[{"w":"多余","p":"duō yú","d":"surplus"}]}"#,
    );
    let s = served_kiokun_on(dir.path());
    let page = fetched(&s, &path_of("後"));
    let back = visible(section(&page, "chinese"));
    assert!(back.contains("behind") && back.contains("queen"), "{back}");
    assert!(!back.contains("surplus"), "{back}");
    // Its lists too (`+page.ts:198-217`): Appears in by form, 後面 once,
    // ranked; Contains by form and readings, 後 once, and 后/後 one card,
    // since the page's forms are 后 and 後. 余's are not merged.
    let html = page.split("<script").next().unwrap_or_default();
    assert_eq!(
        appears_in_column(html, "chinese-words"),
        [
            "後面 | hòu miàn |  | behind",
            "后来 | hòu lái |  | afterwards",
            "然後 | rán hòu |  | then"
        ]
    );
    assert_eq!(contains_in(html), ["后/後 | hòu |  |  |  |  | queen"]);
}

/// **A simplified form that equals its one traditional form in meaning is
/// moved to that form's page, for good** (`[word]/+page.ts:415-425`,
/// ADR-0295). The cases are kiokun's own entries' deciding fields:
/// - 宁 is moved to 寧, the address's query kept, and 寧's page is shown,
///   not moved again;
/// - 余 is moved to 餘: its traditional forms are 余 and 餘, and its own is
///   not counted;
/// - 后 keeps its page, as 後 does: its card teaches another concept
///   (kiokun.com keeps 后/後 apart);
/// - 甲, whose file's character is 乙's and names 甲 as its traditional
///   form, is not moved to itself (`canonicalTarget !== word`).
#[test]
fn an_equivalent_simplified_form_is_moved_to_its_traditional_page() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "宁",
        r#"{"key":"宁","simplified_form_of":"寧","chinese_char":{"char":"宁","tradVariants":["寧"]},
            "semantic_mnemonic":{"character":"宁","meaning":"calm"},
            "semantic_mnemonic_variants":[{"character":"寧","meaning":"calm"}]}"#,
    );
    write_entry(
        dir.path(),
        "寧",
        r#"{"key":"寧","chinese_char":{"char":"寧","simpVariants":["宁"]}}"#,
    );
    write_entry(
        dir.path(),
        "余",
        r#"{"key":"余","simplified_form_of":"餘","chinese_char":{"char":"余","tradVariants":["余","餘"],"simpVariants":["余"]},
            "semantic_mnemonic":{"character":"余","meaning":"excess"},
            "semantic_mnemonic_variants":[{"character":"餘","meaning":"excess"}]}"#,
    );
    write_entry(
        dir.path(),
        "餘",
        r#"{"key":"餘","chinese_char":{"char":"餘","simpVariants":["余"]}}"#,
    );
    write_entry(
        dir.path(),
        "後",
        r#"{"key":"後","chinese_char":{"char":"後","simpVariants":["后"]}}"#,
    );
    write_entry(
        dir.path(),
        "后",
        r#"{"key":"后","simplified_form_of":"後","chinese_char":{"char":"后","tradVariants":["後"]},
            "semantic_mnemonic":{"character":"后","meaning":"empress"},
            "semantic_mnemonic_variants":[{"character":"後","meaning":"behind"}]}"#,
    );
    write_entry(
        dir.path(),
        "甲",
        r#"{"key":"甲","simplified_form_of":"甲","chinese_char":{"char":"乙","tradVariants":["甲"]},
            "semantic_mnemonic":{"character":"乙","meaning":"shell"},
            "semantic_mnemonic_variants":[{"character":"甲","meaning":"shell"}]}"#,
    );
    let s = served_kiokun_on(dir.path());
    let moved = fetched(&s, &path_of("宁"));
    assert!(
        moved.starts_with("HTTP/1.1 308 Permanent Redirect\r\n"),
        "{moved}"
    );
    // 寧 is E5 AF A7 in UTF-8.
    assert!(
        moved.contains("\r\nlocation: /word/%E5%AF%A7\r\n"),
        "{moved}"
    );
    let kept = fetched(&s, &format!("{}?q=1", path_of("宁")));
    assert!(
        kept.contains("\r\nlocation: /word/%E5%AF%A7?q=1\r\n"),
        "{kept}"
    );
    let moved = fetched(&s, &path_of("余"));
    assert!(
        moved.contains("\r\nlocation: /word/%E9%A4%98\r\n"),
        "{moved}"
    );
    for word in ["寧", "餘", "后", "後", "甲"] {
        let page = fetched(&s, &path_of(word));
        assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "{word}: {page}");
    }
}

/// The text of the first element whose class is `class` in `html`,
/// unescaped, or empty where there is none.
fn class_text(html: &str, class: &str) -> String {
    match html.find(&format!("class=\"{class}\"")) {
        Some(at) => unescaped(&text_of_class(&html[at..], &format!("class=\"{class}\""))),
        None => String::new(),
    }
}

/// The pieces of `html` that begin where `open` does, each to the next.
fn pieces<'a>(html: &'a str, open: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(open) {
        rest = &rest[at + 1..];
        let end = rest.find(open).unwrap_or(rest.len());
        out.push(&rest[..end]);
    }
    out
}

/// **Contains as the page shows it** (`Contains.svelte:146-195`): each card,
/// shown first or behind the disclosure, as `text | pinyin | Jyutping |
/// Japanese | on'yomi | Korean | definition`; none where the page has no
/// section.
fn contains_in(html: &str) -> Vec<String> {
    if !html.contains("<section id=\"contains\"") {
        return Vec::new();
    }
    pieces(section(html, "contains"), "class=\"character-card\"")
        .into_iter()
        .map(|card| {
            let jp = match card.find("class=\"japanese-reading kunyomi-reading\"") {
                Some(at) => class_text(&card[at..], "japanese-reading kunyomi-reading"),
                None => class_text(card, "japanese-reading"),
            };
            format!(
                "{} | {} | {} | {} | {} | {} | {}",
                class_text(card, "character"),
                class_text(card, "chinese-reading"),
                class_text(card, "cantonese-reading"),
                jp,
                class_text(card, "japanese-onyomi-reading"),
                class_text(card, "korean-reading"),
                class_text(card, "definition"),
            )
        })
        .collect()
}

/// **One column of Appears in as the page shows it**
/// (`AppearsIn.svelte:160-290`): each word, shown first or behind the
/// disclosure, as `word | reading | Jyutping | definition`, a common
/// Japanese word's star before it; none where the page has no such column.
fn appears_in_column(html: &str, id: &str) -> Vec<String> {
    // The heading's id is a hole, so the renderer marks the element before
    // it (`data-pw`): it is found by the id alone.
    let Some(at) = html.find(&format!(" id=\"{id}\"")) else {
        return Vec::new();
    };
    let column = &html[at..];
    let end = column
        .find("class=\"column\"")
        .or_else(|| column.find("</section>"))
        .unwrap_or(column.len());
    pieces(&column[..end], "class=\"word-card\"")
        .into_iter()
        .map(|card| {
            let word_at = card.find("class=\"word-text\"").expect("the word");
            let word_end = card[word_at..]
                .find("class=\"pronunciation\"")
                .or_else(|| card[word_at..].find("class=\"definition-row\""))
                .map_or(card.len(), |e| word_at + e);
            let word = unescaped(&visible(&format!("<{}", &card[word_at..word_end])));
            format!(
                "{} | {} | {} | {}",
                word,
                class_text(card, "pronunciation"),
                class_text(card, "cantonese-pronunciation"),
                class_text(card, "definition"),
            )
        })
        .collect()
}

/// One list of the oracle's answer: its length where the oracle kept only
/// part of it (the CI fixture's trim), else none; and the items it kept,
/// each with its place, in the forms the page readers give.
type Wanted = (Option<usize>, Vec<(usize, String)>);

/// The oracle's answer for a word: each list, whole or trimmed.
fn oracle_lists(answer: &serde_json::Value) -> Vec<(String, Wanted)> {
    let s = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    let card = |c: &serde_json::Value| {
        format!(
            "{} | {} | {} | {} | {} | {} | {}",
            s(c, "text"),
            s(c, "p"),
            s(c, "ct"),
            s(c, "jp"),
            s(c, "jo"),
            s(c, "kr"),
            s(c, "definition")
        )
    };
    let word = |w: &serde_json::Value| {
        let star = if w["c"].as_bool() == Some(true) {
            "⭐"
        } else {
            ""
        };
        format!(
            "{star}{} | {} | {} | {}",
            s(w, "w"),
            s(w, "reading"),
            s(w, "ct"),
            s(w, "d")
        )
    };
    let list = |v: &serde_json::Value, shown: &dyn Fn(&serde_json::Value) -> String| -> Wanted {
        match v.as_array() {
            Some(items) => (None, items.iter().map(shown).enumerate().collect()),
            None => (
                Some(v["length"].as_u64().expect("a trimmed list's length") as usize),
                v["kept"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| (i["at"].as_u64().expect("its place") as usize, shown(i)))
                    .collect(),
            ),
        }
    };
    vec![
        ("contains".to_string(), list(&answer["contains"], &card)),
        ("chinese-words".to_string(), list(&answer["chinese"], &word)),
        (
            "japanese-words".to_string(),
            list(&answer["japanese"], &word),
        ),
        ("korean-words".to_string(), list(&answer["korean"], &word)),
    ]
}

/// Does the page's list hold what the oracle kept: the whole list, or a
/// trimmed list's length and each kept item at its place?
fn holds(got: &[String], (length, kept): &Wanted) -> bool {
    got.len() == length.unwrap_or(kept.len()) && kept.iter().all(|(at, w)| got.get(*at) == Some(w))
}

/// **Contains and Appears in against kiokun.com's answers**: each word's
/// lists, as the page shows them in order, beside the oracle's. No
/// difference is named for them; each is listed.
fn contains_held_to_oracle(s: &Server, oracle: &serde_json::Value) -> Vec<String> {
    let answers = oracle["answers"].as_object().expect("the oracle's answers");
    let mut differ: Vec<String> = Vec::new();
    for (word, answer) in answers {
        let page = fetched(s, &path_of(word));
        if !page.starts_with("HTTP/1.1 200") {
            differ.push(format!(
                "{word}: {}",
                page.lines().next().unwrap_or_default()
            ));
            continue;
        }
        let html = page.split("<script").next().unwrap_or_default();
        for (list, want) in oracle_lists(answer) {
            let got = if list == "contains" {
                contains_in(html)
            } else {
                appears_in_column(html, &list)
            };
            if !holds(&got, &want) {
                differ.push(format!(
                    "{word} {list}:\n  kiokun.com: {want:?}\n  rewrite:    {got:?}"
                ));
            }
        }
    }
    differ
}

/// **Contains and Appears in against kiokun.com's own code** (local: `just
/// e14-kiokun-contains` runs `orderContainsWords`, `rankAppearsIn` and the
/// page's and `Contains.svelte`'s own functions, copied from `KIOKUN_APP`,
/// over a stated sample of `KIOKUN_DATA`, and names its answers in
/// `KIOKUN_CONTAINS_ORACLE`).
#[test]
#[ignore = "reads kiokun.com's own answers, made locally by `just e14-kiokun-contains`"]
fn contains_and_appears_in_match_kiokuns_own_on_a_sample() {
    let path = std::env::var_os("KIOKUN_CONTAINS_ORACLE").expect("KIOKUN_CONTAINS_ORACLE");
    let oracle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the answers")).expect("JSON");
    let s = served_kiokun();
    let differ = contains_held_to_oracle(&s, &oracle);
    println!(
        "compared: {} words (stride {}, {} entries read)",
        oracle["answered"], oracle["stride"], oracle["read"]
    );
    println!("named differences: {{}}");
    println!("unnamed differences: {}", differ.len());
    for d in differ.iter().take(30) {
        println!("difference: {d}");
    }
    assert!(differ.is_empty(), "{} unnamed difference(s)", differ.len());
}

/// **Contains** (`Contains.svelte`, `contains-order.ts`): a multi-character
/// word's previews, broken down breadth first (学生 + 会, then 学 and 生),
/// then the rest, the longer and earlier first. Where the word has forms of
/// one length (学生会/學生會), the one-character previews at one place are
/// one card written with each form's character (会/會, 学/學). A card's
/// definition is the first of its characters' component glosses, its marker
/// taken off ("assemble (TRAD)" is 會's, the card's second form), else the
/// preview's; an on'yomi the same as the reading is not shown. The expected
/// cards are kiokun.com's own answers for these entries (`contains.mjs`).
#[test]
fn contains_breaks_the_word_down_and_cards_its_forms() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "学生会",
        r#"{"key":"学生会","chinese_words":[{"_id":"1","simp":"学生会","trad":"學生會","items":[{"pinyin":"xué shēng huì","definitions":["student union"]}]}],
            "contains":[{"w":"会","p":"huì","ct":"wui5","d":"can"},{"w":"学生","p":"xué shēng","d":"student","fr":120},
            {"w":"會","p":"huì","d":"meeting","fr":30},{"w":"学","p":"xué","jp":"まなぶ","jo":"ガク","kr":"학","d":"learn"},
            {"w":"生会","d":""},{"w":"生","p":"shēng","jp":"せい","jo":"せい","d":"life","c":true},
            {"w":"學","p":"xué","d":"study"},{"w":"x","d":"unrelated"},{"w":"學生","p":"xué shēng","d":"pupil"}]}"#,
    );
    let app = app_with(
        "{}",
        r#"{"学":"to study (simp)","會":"assemble (TRAD)","生":""}"#,
    );
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(
        dir.path().to_path_buf(),
        Some(app.path()),
    ));
    let page = fetched(&s, &path_of("学生会"));
    let html = page.split("<script").next().unwrap_or_default();
    assert_eq!(
        contains_in(html),
        [
            "学生 | xué shēng |  |  |  |  | student",
            "会/會 | huì | wui5 |  |  |  | assemble",
            "学/學 | xué |  | まなぶ | ガク | 학 | to study",
            "生 | shēng |  | せい |  |  | life",
            "生会 |  |  |  |  |  | ",
            "學生 | xué shēng |  |  |  |  | pupil",
            "x |  |  |  |  |  | unrelated",
        ]
    );
    // 学's on'yomi differs from its reading: both shown, the reading as the
    // kun'yomi. Each card links to its word's page.
    let study = section(html, "contains");
    assert!(
        study.contains("aria-label=\"Japanese kun’yomi: まなぶ\""),
        "{study}"
    );
    assert!(
        study.contains("href=\"/word/%E5%AD%A6%E7%94%9F\""),
        "{study}"
    );
    // Seven cards: none behind the disclosure.
    assert!(!study.contains("<details"), "{study}");
    // Where nothing breaks a form down, the previews tie on where they sit
    // in the form chosen (the first, 丁乙丙, where neither is), and the
    // word's own order decides: 甲 is in 甲乙丙, 戊 is not
    // (`sortContainsWords`), whatever the file's order.
    write_entry(
        dir.path(),
        "甲乙丙",
        r#"{"key":"甲乙丙","chinese_words":[{"_id":"1","simp":"丁乙丙","trad":"丁乙丙","items":[{"pinyin":"x","definitions":["x"]}]}],
            "contains":[{"w":"戊","d":"fifth"},{"w":"甲","d":"first"}]}"#,
    );
    let tie = fetched(&s, &path_of("甲乙丙"));
    assert_eq!(
        contains_in(tie.split("<script").next().unwrap_or_default()),
        ["丁/甲 |  |  |  |  |  | first", "戊 |  |  |  |  |  | fifth"]
    );
    // Control: a word whose file has no previews, 人 in the repository's
    // sample, has no section.
    let person = fetched(&served_kiokun(), &path_of("人"));
    assert!(person.starts_with("HTTP/1.1 200"), "{person}");
    assert!(!person.contains("<section id=\"contains\""), "{person}");
}

/// **Appears in** (`AppearsIn.svelte`, `appears-in-order.ts`): each
/// language's words that contain this one. Chinese words before names
/// (pinyin that begins with a capital, its zero-width spaces not counted),
/// each by frequency rank, a rank of 0 counted as none; Japanese common
/// words first; Korean as the file lists them. A word read and defined as
/// another is shown once; one without a form, not at all. Ten are shown,
/// and the rest behind a disclosure that counts them. The expected words
/// are kiokun.com's own answers for this entry (`contains.mjs`).
#[test]
fn appears_in_ranks_each_languages_words_ten_at_a_time() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "子",
        "{\"key\":\"子\",\"contained_in_chinese\":[\
         {\"w\":\"孩子\",\"p\":\"hái zi\",\"d\":\"child\",\"fr\":900},{\"w\":\"老子\",\"p\":\"Lǎo zǐ\",\"d\":\"Laozi\",\"fr\":5},\
         {\"w\":\"儿子\",\"p\":\"ér zi\",\"d\":\"son\",\"fr\":300},{\"w\":\"儿子\",\"p\":\"ér\u{200b}zi\",\"d\":\"son \"},\
         {\"w\":\"子弹\",\"p\":\"zǐ dàn\",\"ct\":\"zi2 daan2\",\"d\":\"bullet\"},{\"w\":\"孙子\",\"p\":\"\u{200b}Sūn\u{200b}zǐ\",\"d\":\"Sunzi\"},\
         {\"w\":\"桌子\",\"p\":\"zhuō zi\",\"d\":\"table\",\"fr\":0},{\"w\":\"\",\"p\":\"x\",\"d\":\"nothing\"},\
         {\"w\":\"椅子\",\"p\":\"yǐ zi\",\"d\":\"chair\",\"fr\":1200},{\"w\":\"瓶子\",\"p\":\"píng zi\",\"d\":\"bottle\",\"fr\":2000},\
         {\"w\":\"房子\",\"p\":\"fáng zi\",\"d\":\"house\",\"fr\":700},{\"w\":\"日子\",\"p\":\"rì zi\",\"d\":\"day\",\"fr\":400},\
         {\"w\":\"样子\",\"p\":\"yàng zi\",\"d\":\"look\",\"fr\":350},{\"w\":\"妻子\",\"p\":\"qī zi\",\"d\":\"wife\",\"fr\":800}],\
         \"contained_in_japanese\":[{\"w\":\"子供\",\"jp\":\"こども\",\"d\":\"child\",\"fr\":50},\
         {\"w\":\"帽子\",\"jp\":\"ぼうし\",\"d\":\"hat\",\"c\":true,\"fr\":900},{\"w\":\"様子\",\"jp\":\"ようす\",\"d\":\"state\",\"c\":true,\"fr\":100},\
         {\"w\":\"子音\",\"p\":\"zǐ yīn\",\"d\":\"consonant\"}],\
         \"contained_in_korean\":[{\"w\":\"자손\",\"kr\":\"자손\",\"d\":\"descendant\",\"fr\":9},{\"w\":\"모자\",\"d\":\"hat\",\"fr\":1}]}",
    );
    let s = served_kiokun_on(dir.path());
    let page = fetched(&s, &path_of("子"));
    let html = page.split("<script").next().unwrap_or_default();
    assert_eq!(
        appears_in_column(html, "chinese-words"),
        [
            "儿子 | ér zi |  | son",
            "样子 | yàng zi |  | look",
            "日子 | rì zi |  | day",
            "房子 | fáng zi |  | house",
            "妻子 | qī zi |  | wife",
            "孩子 | hái zi |  | child",
            "椅子 | yǐ zi |  | chair",
            "瓶子 | píng zi |  | bottle",
            "子弹 | zǐ dàn | zi2 daan2 | bullet",
            "桌子 | zhuō zi |  | table",
            "老子 | Lǎo zǐ |  | Laozi",
            "孙子 | \u{200b}Sūn\u{200b}zǐ |  | Sunzi",
        ]
    );
    assert_eq!(
        appears_in_column(html, "japanese-words"),
        [
            "⭐様子 | ようす |  | state",
            "⭐帽子 | ぼうし |  | hat",
            "子供 | こども |  | child",
            "子音 | zǐ yīn |  | consonant",
        ]
    );
    assert_eq!(
        appears_in_column(html, "korean-words"),
        ["자손 | 자손 |  | descendant", "모자 |  |  | hat"]
    );
    // Twelve Chinese words: ten shown, two behind the disclosure.
    let chinese = &html[html.find(" id=\"chinese-words\"").expect("the column")..];
    let chinese = &chinese[..chinese.find("class=\"column\"").unwrap_or(chinese.len())];
    let disclosed = &chinese[chinese.find("<details").expect("the disclosure")..];
    assert_eq!(chinese.matches("class=\"word-card\"").count(), 12);
    assert_eq!(disclosed.matches("class=\"word-card\"").count(), 2);
    assert!(disclosed.contains(">2 more items<"), "{disclosed}");
    // Three columns.
    assert!(html.contains("word-columns three-columns"), "{html}");
}

/// **Contains and Appears in against kiokun.com's answers for the
/// repository's sample** (`kiokun-oracle/contains-sample.json`, written by
/// `just e14-kiokun-contains`): what CI holds. The sample's words are single
/// characters, with no Contains; its Appears in lists are kiokun's own, up
/// to 200 words each. The fixture keeps each list's length and its first 20
/// and last 5 words, each with its place (the integrator's ruling of
/// 2026-10-10): the head, where the order shows, and the tail, where the
/// unranked words and the names go. The whole lists are held locally.
#[test]
fn contains_and_appears_in_match_kiokuns_answers_for_the_sample() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kiokun-oracle/contains-sample.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON");
    assert!(
        fixture["kiokun_commit"]
            .as_str()
            .is_some_and(|c| c.len() == 40),
        "{fixture}"
    );
    let oracle = &fixture["oracle"];
    assert_eq!(
        oracle["trimmed"],
        "each list's length, and its first 20 and last 5 items, each with its place",
        "{fixture}"
    );
    let chinese = &oracle["answers"]["人"]["chinese"];
    assert!(
        chinese["length"].as_u64().is_some_and(|n| n > 25),
        "{fixture}"
    );
    assert_eq!(
        chinese["kept"].as_array().map(Vec::len),
        Some(25),
        "{fixture}"
    );
    let s = served_kiokun();
    let differ = contains_held_to_oracle(&s, oracle);
    assert!(differ.is_empty(), "{differ:#?}");
}

/// **Word pages with long lists, timed** (local: it reads the owner's
/// kiokun-data checkout through `KIOKUN_DATA`; run in release by `just
/// e14-kiokun-contains`). Each word in `KIOKUN_TIME_WORDS` is asked for
/// `KIOKUN_TIME_REPEAT` times (30 by default), each time of a server just
/// started from the one build, since the word's page is kept in the shared
/// cache once read: the time from the request to the response's last byte.
/// `KIOKUN_SAMPLE_APP` names another `app.pw` to serve, for a baseline.
#[test]
#[ignore = "reads a kiokun-data checkout named by KIOKUN_DATA; a measurement, run by `just e14-kiokun-contains`"]
fn word_pages_with_long_lists_are_timed() {
    assert!(
        std::env::var_os("KIOKUN_DATA").is_some(),
        "KIOKUN_DATA must name a kiokun-data checkout's output_dictionary"
    );
    let words = std::env::var("KIOKUN_TIME_WORDS").expect("KIOKUN_TIME_WORDS");
    let repeat: usize = std::env::var("KIOKUN_TIME_REPEAT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(30);
    let (_dir, out) = built_kiokun_with(|app| match std::env::var_os("KIOKUN_SAMPLE_APP") {
        Some(other) => std::fs::read_to_string(other).expect("KIOKUN_SAMPLE_APP"),
        None => app.to_string(),
    });
    for word in words.split(',') {
        let mut times: Vec<f64> = Vec::new();
        let mut page = String::new();
        for _ in 0..repeat {
            let s = Server::from_build(out.clone(), out.clone()).expect("served");
            let at = std::time::Instant::now();
            page = fetched_as(&s, &path_of(word), Some("timed"))
                .into_iter()
                .map(|(_, c)| c)
                .collect();
            times.push(at.elapsed().as_secs_f64() * 1e3);
        }
        assert!(page.starts_with("HTTP/1.1 200"), "{word}: {page}");
        times.sort_by(|a, b| a.partial_cmp(b).expect("a time"));
        let html = page.split("<script").next().unwrap_or_default();
        println!(
            "word {word}: {} bytes; Contains {}, Chinese {}, Japanese {}, Korean {}; per request, ms: p50 {:.1}, p90 {:.1}, max {:.1} ({repeat} requests)",
            page.len(),
            contains_in(html).len(),
            appears_in_column(html, "chinese-words").len(),
            appears_in_column(html, "japanese-words").len(),
            appears_in_column(html, "korean-words").len(),
            quantile(&times, 0.5),
            quantile(&times, 0.9),
            quantile(&times, 1.0)
        );
    }
}

/// **EDRDG's acknowledgement on each page that shows its data**: the
/// licence of JMdict, JMnedict and KANJIDIC2 asks a web dictionary to
/// acknowledge the files "on each screen display"
/// (https://www.edrdg.org/edrdg/licence.html, checked 2026-10-10). 人's page
/// shows JMdict's words and KANJIDIC2's character, and says so at its foot,
/// linking each to its project and the Group to its licence; a page with
/// Chinese words alone does not. kiokun.com's pages carry none.
#[test]
fn a_page_with_edrdg_data_acknowledges_the_group() {
    let person = fetched(&served_kiokun(), &path_of("人"));
    let html = person.split("<script").next().unwrap_or_default();
    let foot = &html[html
        .find("<footer class=\"data-sources\"")
        .expect("the acknowledgement")..];
    let text = visible(foot);
    assert!(
        text.contains("These files are the property of the Electronic Dictionary Research and Development Group, and are used in conformance with the Group's licence."),
        "{text}"
    );
    for link in [
        "https://www.edrdg.org/wiki/index.php/JMdict-EDICT_Dictionary_Project",
        "https://www.edrdg.org/wiki/index.php/KANJIDIC_Project",
        "https://www.edrdg.org/edrdg/licence.html",
    ] {
        assert!(foot.contains(&format!("href=\"{link}\"")), "{foot}");
    }
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "电脑",
        r#"{"key":"电脑","chinese_words":[{"_id":"1","simp":"电脑","trad":"電腦","items":[{"pinyin":"diàn nǎo","definitions":["computer"]}]}]}"#,
    );
    let computer = fetched(&served_kiokun_on(dir.path()), &path_of("电脑"));
    assert!(computer.starts_with("HTTP/1.1 200"), "{computer}");
    assert!(!computer.contains("data-sources"), "{computer}");
}

/// The value at fraction `q` of sorted `values` (nearest rank).
fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

/// **A sample of the whole dictionary, served and timed** (local: it reads
/// the owner's kiokun-data checkout through `KIOKUN_DATA`, and
/// `KIOKUN_APP` where it is set). Every `KIOKUN_SAMPLE_EVERY`th file (256 by
/// default) of every subdirectory, in name order, is looked up by the word
/// its file records, through the server's own HTTP path, one request at a
/// time on a fresh connection: its status, and the time from the request to
/// the response's last byte. Run in release by `just e14-kiokun-sample`.
///
/// Every word a file records has a page, or is moved to one (308):
/// anything else is a defect, named.
#[test]
#[ignore = "reads a kiokun-data checkout named by KIOKUN_DATA; a measurement, run by `just e14-kiokun-sample`"]
fn a_sample_of_the_whole_dictionary_is_served_and_timed() {
    let Some(root) = std::env::var_os("KIOKUN_DATA").map(std::path::PathBuf::from) else {
        panic!("KIOKUN_DATA must name a kiokun-data checkout's output_dictionary");
    };
    let every: usize = std::env::var("KIOKUN_SAMPLE_EVERY")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(256);
    let started = std::time::Instant::now();
    // `KIOKUN_SAMPLE_APP` names another `app.pw` to serve, an earlier
    // milestone's, for a baseline.
    let (_dir, out) = built_kiokun_with(|app| match std::env::var_os("KIOKUN_SAMPLE_APP") {
        Some(other) => std::fs::read_to_string(other).expect("KIOKUN_SAMPLE_APP"),
        None => app.to_string(),
    });
    let s = Server::from_build(out.clone(), out.clone()).expect("served");
    let word_component = std::fs::metadata(out.join("components/kiokun.site.Word.wasm"))
        .expect("the Word component")
        .len();
    println!(
        "server built in {:.1} s (the program compiled and loaded); the Word component {} bytes; KIOKUN_APP {}",
        started.elapsed().as_secs_f64(),
        word_component,
        if std::env::var_os("KIOKUN_APP").is_some() {
            "set"
        } else {
            "not set"
        }
    );
    // Every file, in name order, every `every`th of them.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut subs: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
        .expect("KIOKUN_DATA is a directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();
    let mut total = 0usize;
    for sub in subs {
        let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&sub)
            .expect("a subdirectory")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".json.deflate"))
            .collect();
        names.sort();
        total += names.len();
        files.extend(names.into_iter().step_by(every.max(1)));
    }
    println!("files: {total}; sampled: {} (every {every}th)", files.len());
    let mut times: Vec<f64> = Vec::new();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut defects: Vec<String> = Vec::new();
    let mut slowest: Vec<(f64, String)> = Vec::new();
    for file in &files {
        let raw = std::fs::read(file).expect("the file");
        let json: serde_json::Value = serde_json::from_slice(
            &miniz_oxide::inflate::decompress_to_vec(&raw).expect("raw DEFLATE"),
        )
        .expect("JSON");
        let word = json["key"].as_str().unwrap_or_default().to_string();
        let at = std::time::Instant::now();
        let page: String = fetched_as(&s, &path_of(&word), Some("sample"))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let ms = at.elapsed().as_secs_f64() * 1e3;
        let status = page.lines().next().unwrap_or_default().to_string();
        *statuses.entry(status.clone()).or_default() += 1;
        times.push(ms);
        slowest.push((ms, word.clone()));
        // A move is kiokun.com's (ADR-0295), held to its own code by
        // `just e14-kiokun-moves`.
        if !status.starts_with("HTTP/1.1 200") && !status.starts_with("HTTP/1.1 308") {
            let body = page.split("\r\n\r\n").nth(1).unwrap_or_default();
            defects.push(format!(
                "{word}: {status}: {}",
                body.chars().take(300).collect::<String>()
            ));
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).expect("a time"));
    slowest.sort_by(|a, b| b.0.partial_cmp(&a.0).expect("a time"));
    for (status, n) in &statuses {
        println!("status: {status}: {n}");
    }
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    println!(
        "per request, ms: mean {mean:.1}, p50 {:.1}, p90 {:.1}, p99 {:.1}, max {:.1}",
        quantile(&times, 0.5),
        quantile(&times, 0.9),
        quantile(&times, 0.99),
        quantile(&times, 1.0)
    );
    for (ms, word) in slowest.iter().take(5) {
        println!("slowest: {word}: {ms:.1} ms");
    }
    println!("defects: {}", defects.len());
    for d in defects.iter().take(20) {
        println!("defect: {d}");
    }
    assert!(defects.is_empty(), "{} page(s) not served", defects.len());
}

/// The content of the head's `<meta>` whose `attribute` is `value`.
fn meta(html: &str, attribute: &str, value: &str) -> String {
    let at = html
        .find(&format!("{attribute}=\"{value}\""))
        .unwrap_or_else(|| panic!("no meta {value}: {html}"));
    let tag = &html[html[..at].rfind('<').expect("its tag")..];
    let tag = &tag[..tag.find('>').expect("its end")];
    let content = &tag[tag.find("content=\"").expect("its content") + 9..];
    content[..content.find('"').expect("its end")].to_string()
}

fn title(html: &str) -> String {
    let start = html.find("<title>").expect("a title") + 7;
    html[start..start + html[start..].find("</title>").expect("its end")].to_string()
}

/// **The page's description of itself** (`buildDictionarySeo`): 人's title
/// is the word and its first three meanings, the learner gloss first, and
/// its description says so; Open Graph and Twitter repeat them.
#[test]
fn the_page_describes_itself_as_kiokun_com_does() {
    let s = served_kiokun();
    let page = fetched(&s, &path_of("人"));
    assert_eq!(title(&page), "人 — person, man, people | Kiokun");
    let described = "人 means person, man, people. Character readings, definitions, examples, \
                     and learning tools across Chinese, Japanese, and Korean.";
    assert_eq!(meta(&page, "name", "description"), described);
    assert_eq!(
        meta(&page, "property", "og:title"),
        "人 — person, man, people | Kiokun"
    );
    assert_eq!(meta(&page, "property", "og:description"), described);
    assert_eq!(
        meta(&page, "name", "twitter:title"),
        "人 — person, man, people | Kiokun"
    );
    assert_eq!(meta(&page, "name", "robots"), "index, follow");
    // A stub's title names its other forms: 谚 (諺).
    let stub = fetched(&s, &path_of("谚"));
    assert!(title(&stub).starts_with("谚 (諺) — "), "{}", title(&stub));
}

/// **The definitions it names, as `definitionFragments` makes them**: tags
/// out, split at `;` and `；`, a leading `2)` off, a classifier and a
/// one-letter piece left out; and a title past 68 UTF-16 units cut to 67,
/// trimmed, with `…`.
#[test]
fn the_description_takes_kiokuns_fragments_and_length() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "卯",
        r#"{"key":"卯","chinese_char":{"char":"卯"},
            "chinese_words":[{"_id":"1","simp":"卯","trad":"卯","items":[{"pinyin":"mǎo",
              "definitions":["x","CL:個","<b>bold</b>  one; 2) two；three"]}]}]}"#,
    );
    write_entry(
        dir.path(),
        "辰",
        r#"{"key":"辰","chinese_char":{"char":"辰","gloss":"a gloss long enough that the title it heads runs past the limit"}}"#,
    );
    let s = served_kiokun_on(dir.path());
    assert_eq!(
        title(&fetched(&s, &path_of("卯"))),
        "卯 — bold one, two, three | Kiokun"
    );
    let whole = "辰 — a gloss long enough that the title it heads runs past the limit | Kiokun";
    let cut: Vec<u16> = whole.encode_utf16().take(67).collect();
    let expected = format!("{}…", String::from_utf16_lossy(&cut).trim_end());
    assert_eq!(title(&fetched(&s, &path_of("辰"))), expected);
    // A character past U+FFFF is two units, as JavaScript counts it.
    write_entry(
        dir.path(),
        "𠀀",
        r#"{"key":"𠀀","chinese_char":{"char":"𠀀","gloss":"a gloss long enough that the title it heads runs past the limit"}}"#,
    );
    let whole = "𠀀 — a gloss long enough that the title it heads runs past the limit | Kiokun";
    let cut: Vec<u16> = whole.encode_utf16().take(67).collect();
    let expected = format!("{}…", String::from_utf16_lossy(&cut).trim_end());
    assert_eq!(title(&fetched(&s, &path_of("𠀀"))), expected);
    // 68 code points and 69 units: JavaScript cuts it, and so does this.
    write_entry(
        dir.path(),
        "𡀀",
        r#"{"key":"𡀀","chinese_char":{"char":"𡀀","gloss":"a gloss of fifty-five units, just what this case needs!"}}"#,
    );
    let whole = "𡀀 — a gloss of fifty-five units, just what this case needs! | Kiokun";
    assert_eq!(
        (whole.chars().count(), whole.encode_utf16().count()),
        (68, 69)
    );
    let cut: Vec<u16> = whole.encode_utf16().take(67).collect();
    let expected = format!("{}…", String::from_utf16_lossy(&cut).trim_end());
    assert_eq!(title(&fetched(&s, &path_of("𡀀"))), expected);
}

/// HTML's five escapes read back.
fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

/// **The page head against kiokun.com's answers** (the integrator's ruling
/// on oracles, 2026-10-09): each word's served title and description beside
/// the oracle's. An answer the oracle marked with a named difference holds
/// the rewrite's text for it, and is counted by its name; any other
/// difference is named here and fails. Returns the count of each named
/// difference and the differences found.
fn held_to_oracle(
    s: &Server,
    oracle: &serde_json::Value,
) -> (BTreeMap<String, usize>, Vec<String>) {
    let answers = oracle["answers"].as_object().expect("the oracle's answers");
    let mut named: BTreeMap<String, usize> = BTreeMap::new();
    let mut differ: Vec<String> = Vec::new();
    for (word, answer) in answers {
        for name in answer["named"].as_array().into_iter().flatten() {
            *named
                .entry(name.as_str().unwrap_or_default().to_string())
                .or_default() += 1;
        }
        let page = fetched(s, &path_of(word));
        let (want_title, want_description) = (
            answer["title"].as_str().unwrap_or_default(),
            answer["description"].as_str().unwrap_or_default(),
        );
        if !page.starts_with("HTTP/1.1 200") {
            differ.push(format!(
                "{word}: {}",
                page.lines().next().unwrap_or_default()
            ));
            continue;
        }
        let got_title = unescaped(&title(&page));
        let got_description = unescaped(&meta(&page, "name", "description"));
        if got_title != want_title {
            differ.push(format!(
                "{word}: title\n  kiokun.com: {want_title}\n  rewrite:    {got_title}"
            ));
        }
        if got_description != want_description {
            differ.push(format!(
                "{word}: description\n  kiokun.com: {want_description}\n  rewrite:    {got_description}"
            ));
        }
    }
    (named, differ)
}

/// **The page head against kiokun.com's own `buildDictionarySeo`** (local:
/// `just e14-kiokun-seo` runs kiokun.com's `seo.ts`, copied from
/// `KIOKUN_APP`, over a stated sample of `KIOKUN_DATA`, and names its
/// answers in `KIOKUN_SEO_ORACLE`).
#[test]
#[ignore = "reads kiokun.com's own answers, made locally by `just e14-kiokun-seo`"]
fn the_page_head_matches_kiokuns_own_on_a_sample() {
    let path = std::env::var_os("KIOKUN_SEO_ORACLE").expect("KIOKUN_SEO_ORACLE");
    let oracle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the answers")).expect("JSON");
    let s = served_kiokun();
    let (named, differ) = held_to_oracle(&s, &oracle);
    println!(
        "compared: {} words (stride {}, {} entries read)",
        oracle["answered"], oracle["stride"], oracle["read"]
    );
    println!("named differences: {named:?}");
    println!("unnamed differences: {}", differ.len());
    for d in differ.iter().take(30) {
        println!("difference: {d}");
    }
    assert!(differ.is_empty(), "{} unnamed difference(s)", differ.len());
}

/// **The page head against kiokun.com's answers for the repository's sample**
/// (`spikes/own-renderer/kiokun-oracle/seo-sample.json`, written by `just
/// e14-kiokun-seo` with its command and both commits): what CI holds, since
/// it has neither kiokun.com's code nor the owner's data.
#[test]
fn the_page_head_matches_kiokuns_answers_for_the_sample() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kiokun-oracle/seo-sample.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON");
    assert!(
        fixture["kiokun_commit"]
            .as_str()
            .is_some_and(|c| c.len() == 40),
        "{fixture}"
    );
    let oracle = &fixture["oracle"];
    assert!(
        oracle["answered"].as_u64().unwrap_or_default() > 0,
        "{fixture}"
    );
    let s = served_kiokun_on(&sample());
    let (_, differ) = held_to_oracle(&s, oracle);
    assert!(differ.is_empty(), "{differ:#?}");
}

/// Each example a section shows, in order: its text and its translation,
/// joined by ` / `.
fn examples_in(section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(at) = rest.find("class=\"sense-example\"") {
        rest = &rest[at + 1..];
        let source = text_of_class(rest, "class=\"sense-example-source\"");
        let end = rest.find("class=\"sense-example\"").unwrap_or(rest.len());
        let this = &rest[..end];
        let translation = this
            .find("class=\"sense-example-translation\"")
            .map(|t| text_of_class(&this[t..], "class=\"sense-example-translation\""))
            .unwrap_or_default();
        out.push(format!("{source} / {translation}"));
    }
    out
}

/// **A sense's examples** (`SenseExampleList.svelte`): those with a text;
/// the first shown, and the rest behind a disclosure that counts them; a
/// Chinese sense's from the record for its text, written in its simplified
/// form; a Korean sense's and a Korean word's own; each with its
/// translation where it has one.
#[test]
fn a_senses_examples_are_shown_first_and_the_rest_disclosed() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "例",
        r#"{"key":"例",
            "chinese_words":[{"_id":"1","simp":"例","trad":"例","items":[{"pinyin":"lì",
              "definitions":["example","rule"],
              "definitionExamples":[
                {"definition":"example","examples":[
                  {"simp":"举例","trad":"舉例","en":"to give an example"},
                  {"simp":"","trad":"例外","en":"an exception"},
                  {"simp":"","trad":"","en":"no text"}]},
                {"definition":"another","examples":[{"simp":"不","en":"not shown"}]}]}]}],
            "korean_words":[{"id":"k","hangul":"례","definitions":[
                {"text":"example","examples":[{"korean":"예를 들면","translation":"for example"}]}],
              "examples":[{"korean":"례","translation":""}]}]}"#,
    );
    let s = served_kiokun_on(dir.path());
    let page = fetched(&s, &path_of("例"));
    let chinese = section(&page, "chinese");
    assert_eq!(
        examples_in(chinese),
        ["举例 / to give an example", "例外 / an exception"]
    );
    assert!(visible(chinese).contains("Examples"), "{chinese}");
    assert!(
        chinese.contains("Show 1 more examples for this definition"),
        "{chinese}"
    );
    assert!(!chinese.contains("not shown"), "{chinese}");
    let korean = section(&page, "korean");
    assert_eq!(examples_in(korean), ["예를 들면 / for example", "례 / "]);
    assert!(visible(korean).contains("Example"), "{korean}");
}

/// **A Japanese sense's examples** (`japaneseExamplesForSense`): its
/// Japanese sentence, by `lang` or `land`, with its English; one without a
/// Japanese text is left out.
#[test]
fn a_japanese_senses_examples_are_its_japanese_sentences() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        "見",
        r#"{"key":"見","japanese_words":[{"id":"j","kanji":[{"text":"見","tags":[],"common":true}],
            "kana":[{"text":"み","tags":[],"common":true}],
            "sense":[{"gloss":[{"text":"look"}],"info":[],"partOfSpeech":[],"field":[],"misc":[],"dialect":[],
              "examples":[
                {"sentences":[{"lang":"jpn","text":"見て"},{"lang":"eng","text":"Look."}]},
                {"sentences":[{"land":"jpn","text":"見た"}]},
                {"sentences":[{"lang":"eng","text":"English alone"}]}]}]}]}"#,
    );
    let s = served_kiokun_on(dir.path());
    let japanese = section(&fetched(&s, &path_of("見")), "japanese").to_string();
    assert_eq!(examples_in(&japanese), ["見て / Look.", "見た / "]);
}

/// **The Japanese examples against kiokun.com's answers**: each word's
/// examples, as its Japanese section shows them in order, beside the
/// oracle's. No difference is named for them; each is listed.
fn examples_held_to_oracle(s: &Server, oracle: &serde_json::Value) -> Vec<String> {
    let answers = oracle["answers"].as_object().expect("the oracle's answers");
    let mut differ: Vec<String> = Vec::new();
    for (word, answer) in answers {
        let want: Vec<String> = answer
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| e.as_str().unwrap_or_default().to_string())
            .collect();
        let page = fetched(s, &path_of(word));
        if !page.starts_with("HTTP/1.1 200") {
            differ.push(format!(
                "{word}: {}",
                page.lines().next().unwrap_or_default()
            ));
            continue;
        }
        let got: Vec<String> = if want.is_empty() && !page.contains("<section id=\"japanese\"") {
            Vec::new()
        } else {
            examples_in(section(&page, "japanese"))
                .iter()
                .map(|e| unescaped(e))
                .collect()
        };
        if got != want {
            differ.push(format!(
                "{word}:\n  kiokun.com: {want:?}\n  rewrite:    {got:?}"
            ));
        }
    }
    differ
}

/// **The Japanese examples against kiokun.com's own
/// `japaneseExamplesForSense`** (local: `just e14-kiokun-examples` runs it,
/// copied from `KIOKUN_APP`, over a stated sample of `KIOKUN_DATA`, and names
/// its answers in `KIOKUN_EXAMPLES_ORACLE`).
#[test]
#[ignore = "reads kiokun.com's own answers, made locally by `just e14-kiokun-examples`"]
fn the_japanese_examples_match_kiokuns_own_on_a_sample() {
    let path = std::env::var_os("KIOKUN_EXAMPLES_ORACLE").expect("KIOKUN_EXAMPLES_ORACLE");
    let oracle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the answers")).expect("JSON");
    let s = served_kiokun();
    let differ = examples_held_to_oracle(&s, &oracle);
    println!(
        "compared: {} words (stride {}, {} entries read)",
        oracle["answered"], oracle["stride"], oracle["read"]
    );
    println!("named differences: {{}}");
    println!("unnamed differences: {}", differ.len());
    for d in differ.iter().take(30) {
        println!("difference: {d}");
    }
    assert!(differ.is_empty(), "{} unnamed difference(s)", differ.len());
}

/// **The Japanese examples against kiokun.com's answers for the
/// repository's sample** (`kiokun-oracle/examples-sample.json`, written by
/// `just e14-kiokun-examples`): what CI holds.
#[test]
fn the_japanese_examples_match_kiokuns_answers_for_the_sample() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kiokun-oracle/examples-sample.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON");
    assert!(
        fixture["kiokun_commit"]
            .as_str()
            .is_some_and(|c| c.len() == 40),
        "{fixture}"
    );
    let oracle = &fixture["oracle"];
    assert!(
        oracle["answered"].as_u64().unwrap_or_default() > 0,
        "{fixture}"
    );
    let s = served_kiokun_on(&sample());
    let differ = examples_held_to_oracle(&s, oracle);
    assert!(differ.is_empty(), "{differ:#?}");
}

/// **The moves against kiokun.com's answers**: each word's response beside
/// the oracle's, a 308 to the word it names, whose own page is shown, or
/// the page itself. No difference is named for them; each is listed.
fn moves_held_to_oracle(s: &Server, oracle: &serde_json::Value) -> Vec<String> {
    let answers = oracle["answers"].as_object().expect("the oracle's answers");
    let mut differ: Vec<String> = Vec::new();
    for (word, answer) in answers {
        let page = fetched(s, &path_of(word));
        let head = page.split("\r\n\r\n").next().unwrap_or_default();
        let status = head.lines().next().unwrap_or_default();
        // A move's target is a page, not another move: no chain.
        let held = match answer.as_str() {
            Some(to) => {
                status == "HTTP/1.1 308 Permanent Redirect"
                    && head.contains(&format!("\r\nlocation: {}\r\n", path_of(to)))
                    && fetched(s, &path_of(to)).starts_with("HTTP/1.1 200 OK\r\n")
            }
            None => status == "HTTP/1.1 200 OK",
        };
        if !held {
            let location = head
                .lines()
                .find(|l| l.starts_with("location: "))
                .unwrap_or_default();
            differ.push(format!(
                "{word}:\n  kiokun.com: {answer}\n  rewrite:    {status} {location}"
            ));
        }
    }
    differ
}

/// **The moves against kiokun.com's own `equivalentTraditionalTarget`**
/// (local: `just e14-kiokun-moves` runs it, copied from `KIOKUN_APP`, over a
/// stated sample of `KIOKUN_DATA`, and names its answers in
/// `KIOKUN_MOVES_ORACLE`).
#[test]
#[ignore = "reads kiokun.com's own answers, made locally by `just e14-kiokun-moves`"]
fn the_moves_match_kiokuns_own_on_a_sample() {
    let path = std::env::var_os("KIOKUN_MOVES_ORACLE").expect("KIOKUN_MOVES_ORACLE");
    let oracle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the answers")).expect("JSON");
    let s = served_kiokun();
    let differ = moves_held_to_oracle(&s, &oracle);
    println!(
        "compared: {} words ({} candidates, {} controls, stride {}; {} one-character entries read); {} move",
        oracle["answered"],
        oracle["candidates"],
        oracle["controls"],
        oracle["stride"],
        oracle["read"],
        oracle["moved"]
    );
    println!("named differences: {{}}");
    println!("unnamed differences: {}", differ.len());
    for d in differ.iter().take(30) {
        println!("difference: {d}");
    }
    assert!(differ.is_empty(), "{} unnamed difference(s)", differ.len());
}

/// **The moves against kiokun.com's answers for the repository's sample**
/// (`kiokun-oracle/moves-sample.json`, written by `just e14-kiokun-moves`):
/// what CI holds. The sample moves nothing; 面 is the case that matters,
/// the simplified form of 麵 with two traditional forms, one of them
/// itself, and no card.
#[test]
fn the_moves_match_kiokuns_answers_for_the_sample() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kiokun-oracle/moves-sample.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON");
    assert!(
        fixture["kiokun_commit"]
            .as_str()
            .is_some_and(|c| c.len() == 40),
        "{fixture}"
    );
    let oracle = &fixture["oracle"];
    assert!(
        oracle["answers"]
            .as_object()
            .is_some_and(|a| a.contains_key("面")),
        "{fixture}"
    );
    let s = served_kiokun();
    let differ = moves_held_to_oracle(&s, oracle);
    assert!(differ.is_empty(), "{differ:#?}");
}

/// The pitch shard a word is in, as `PitchAccent.svelte` hashes it: over
/// UTF-16 units, the reference the program's rule is held to.
fn pitch_shard_of(word: &str) -> String {
    let h = word
        .encode_utf16()
        .fold(0u32, |h, u| h.wrapping_mul(31).wrapping_add(u32::from(u)));
    format!("{:02x}", h & 0xff)
}

/// Writes `json` as pitch shard `shard` of `app`.
fn write_pitch(app: &std::path::Path, shard: &str, json: &str) {
    std::fs::create_dir_all(app.join("static/pitch")).expect("the pitch directory");
    std::fs::write(app.join(format!("static/pitch/{shard}.json")), json).expect("written");
}

/// The pitch a Japanese word shows: each mora, `^` where it is high, and the
/// pattern's name; "" where it shows none.
fn pitches_in(section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(at) = rest.find("class=\"pronunciation-slot\"") {
        rest = &rest[at + 1..];
        let end = rest
            .find("class=\"pronunciation-slot\"")
            .unwrap_or(rest.len());
        let this = &rest[..end];
        let Some(display) = this.find("class=\"pitch-display\"") else {
            out.push(String::new());
            continue;
        };
        let mut shown = String::new();
        let mut morae = &this[display..];
        while let Some(m) = morae.find("class=\"pitch-mora") {
            let high = morae[m..].starts_with("class=\"pitch-mora high\"");
            let text = text_of_class(&morae[m..], "class=\"pitch-mora");
            shown.push_str(&if high { format!("^{text}") } else { text });
            morae = &morae[m + 1..];
        }
        shown.push(' ');
        shown.push_str(&text_of_class(this, "class=\"pitch-label\""));
        out.push(shown);
    }
    out
}

/// **Pitch accent** (`PitchAccent.svelte`), from kiokun.com's pitch data
/// through `KIOKUN_APP`: the accent of the word's reading where the data has
/// it, else of its first reading, in the data's order; each mora high or
/// low by the accent; the pattern named 平板, 頭高, 中高 or 尾高; a small kana
/// joined to the mora before it; nothing where the accent is `null` or the
/// word is not in the data.
#[test]
fn a_japanese_word_shows_its_pitch_accent() {
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    let word = |key: &str, words: &str| format!(r#"{{"key":"{key}","japanese_words":[{words}]}}"#);
    let jword = |id: &str, kanji: &str, kana: &str| {
        format!(
            r#"{{"id":"{id}","kanji":[{{"text":"{kanji}","tags":[],"common":false}}],"kana":[{{"text":"{kana}","tags":[],"common":false}}],"sense":[]}}"#
        )
    };
    write_entry(dir.path(), "話", &word("話", &jword("1", "話", "はなし")));
    write_entry(dir.path(), "弟", &word("弟", &jword("2", "弟", "おとうと")));
    write_entry(
        dir.path(),
        "今日",
        &word("今日", &jword("3", "今日", "きょう")),
    );
    write_entry(
        dir.path(),
        "人",
        &word(
            "人",
            &format!(
                "{},{},{}",
                jword("4", "人", "ひと"),
                jword("5", "人", "じん"),
                jword("8", "人", "にん")
            ),
        ),
    );
    write_entry(dir.path(), "無", &word("無", &jword("6", "無", "む")));
    write_entry(dir.path(), "空", &word("空", &jword("7", "空", "そら")));
    let app = app_with("{}", "{}");
    write_pitch(app.path(), &pitch_shard_of("話"), r#"{"話":{"はなし":3}}"#);
    write_pitch(
        app.path(),
        &pitch_shard_of("弟"),
        r#"{"弟":{"おとうと":2}}"#,
    );
    write_pitch(
        app.path(),
        &pitch_shard_of("今日"),
        r#"{"今日":{"きょう":1}}"#,
    );
    // じん is not there: the first reading's, in the file's order (`ひと`
    // before `にん`, though `にん` sorts first). にん is, with its own.
    write_pitch(
        app.path(),
        &pitch_shard_of("人"),
        r#"{"人":{"ひと":0,"にん":1}}"#,
    );
    write_pitch(app.path(), &pitch_shard_of("無"), r#"{"無":{"む":null}}"#);
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(
        dir.path().to_path_buf(),
        Some(app.path()),
    ));
    let pitch = |w: &str| pitches_in(section(&fetched(&s, &path_of(w)), "japanese"));
    assert_eq!(pitch("話"), ["は^な^し 尾高"]);
    assert_eq!(pitch("弟"), ["お^とうと 中高"]);
    assert_eq!(pitch("今日"), ["^きょう 頭高"]);
    assert_eq!(pitch("人"), ["ひ^と 平板", "じ^ん 平板", "^にん 頭高"]);
    assert_eq!(pitch("無"), [""]);
    assert_eq!(pitch("空"), [""]);
}

/// **A pitch shard is the hash over UTF-16 units**, not the entries' shard
/// rule over code points: a word past U+FFFF is in another shard by each.
#[test]
fn a_pitch_shard_is_its_words_hash_over_utf16_units() {
    let word = "𠀋";
    let by_points = format!(
        "{:02x}",
        word.chars()
            .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)))
            & 0xff
    );
    assert_ne!(pitch_shard_of(word), by_points);
    let dir = tempfile::TempDir::with_prefix("pw-kiokun-data-").expect("a directory");
    write_entry(
        dir.path(),
        word,
        r#"{"key":"𠀋","japanese_words":[{"id":"1","kanji":[{"text":"𠀋","tags":[],"common":false}],"kana":[{"text":"じょう","tags":[],"common":false}],"sense":[]}]}"#,
    );
    let app = app_with("{}", "{}");
    write_pitch(app.path(), &pitch_shard_of(word), r#"{"𠀋":{"じょう":0}}"#);
    write_pitch(app.path(), &by_points, r#"{"𠀋":{"じょう":1}}"#);
    let mut s = served_kiokun();
    s.server.data = Arc::new(crate::kiokun::KiokunData::at(
        dir.path().to_path_buf(),
        Some(app.path()),
    ));
    let page = fetched(&s, &path_of(word));
    assert_eq!(pitches_in(section(&page, "japanese")), ["じょ^う 平板"]);
}
