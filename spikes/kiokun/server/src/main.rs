//! **The kiokun slice's host** (E10): one shard of kiokun.com's dictionary,
//! served by what `pw build` compiled from `examples/kiokun/`.
//!
//! ```text
//! GET /             the search page, empty
//! GET /search?q=    the compiled `Search`, rendered by `SearchPage`
//! GET /<word>       the compiled `Lookup`, rendered by `WordPage`: 404 for `None`
//! ```
//!
//! The components, contracts and templates are read from
//! `docs/evidence/E10/kiokun/`, the compiler's output that `evidence_is_current`
//! holds to the source. The shard is `examples/kiokun/data/han-1char-3/`, 28
//! real entries. `KIOKUN_DATA=/path/to/kiokun-data/output_dictionary` serves
//! the whole shard instead: 17,597 entries.

mod app;
mod data;
mod shard;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

pub const SHARD: &str = "han-1char-3";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The data directory: `KIOKUN_DATA`, or the committed sample.
fn data_dir() -> PathBuf {
    std::env::var_os("KIOKUN_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("examples/kiokun/data").join(SHARD))
}

fn load() -> Result<app::App, String> {
    load_from(&data_dir())
}

fn load_from(dir: &std::path::Path) -> Result<app::App, String> {
    app::App::load(&root().join("docs/evidence/E10/kiokun"), dir, SHARD)
}

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3160);
    let started = std::time::Instant::now();
    let app = Arc::new(load().unwrap_or_else(|e| {
        eprintln!("kiokun: {e}");
        std::process::exit(1);
    }));
    println!(
        "kiokun slice on {port}: shard {}, {} entries, {} search rows, loaded in {:.0} ms",
        app.index.shard.name,
        app.index.shard.entries.len(),
        app.index.rows.len(),
        started.elapsed().as_secs_f64() * 1e3
    );
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    for stream in listener.incoming().flatten() {
        let app = app.clone();
        std::thread::spawn(move || handle(&app, stream));
    }
}

fn handle(app: &app::App, mut stream: TcpStream) {
    let mut request = String::new();
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    if reader.read_line(&mut request).is_err() {
        return;
    }
    // Drain the headers; nothing here reads them.
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
    }
    let mut parts = request.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let (status, body) = route(app, method, target);
    let head = format!(
        "HTTP/1.1 {status} {}\r\ncontent-type: text/html; charset=utf-8\r\n\
         content-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
        reason(status),
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

/// One request's status and body.
fn route(app: &app::App, method: &str, target: &str) -> (u16, String) {
    if method != "GET" {
        return (405, "method not allowed".into());
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let page = match path {
        "/" => app.search_page(""),
        "/search" => {
            let q = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("q="))
                .unwrap_or_default();
            match decode(q, true) {
                Some(q) => app.search_page(&q),
                None => return (400, "the query is not valid percent-encoded UTF-8".into()),
            }
        }
        "/favicon.ico" => return (404, String::new()),
        // One path segment is one word. A word holding `/` arrives as `%2F`:
        // its file name escapes it (`shard::file_name`), so no word reaches
        // outside kiokun's directory.
        word => match word
            .strip_prefix('/')
            .filter(|w| !w.contains('/'))
            .and_then(|w| decode(w, false))
        {
            Some(w) if !w.is_empty() => app.entry_page(&w),
            _ => return (400, "not a word".into()),
        },
    };
    match page {
        Ok(p) => (p.status, p.html),
        Err(e) => (500, format!("kiokun: {e}")),
    }
}

/// Percent-decoding into UTF-8, with `+` as a space in a query. `None` for a
/// malformed escape or bytes that are not UTF-8: a request is refused rather
/// than guessed at.
fn decode(s: &str, query: bool) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' if query => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_host::engine::Val;

    /// The committed sample, whatever `KIOKUN_DATA` says: these tests are
    /// about the sample's 28 entries, and the whole shard has its own test.
    fn app() -> app::App {
        load_from(&root().join("examples/kiokun/data").join(SHARD)).expect("the slice loads")
    }

    /// The page without its part anchors (`<!--pw:s3-->`), for reading text.
    fn bare(html: &str) -> String {
        html.split("<!--")
            .map(|p| p.split_once("-->").map_or(p, |(_, rest)| rest))
            .collect()
    }

    fn field<'a>(v: &'a Val, name: &str) -> &'a Val {
        let Val::Record(fields) = v else {
            panic!("not a record: {v:?}")
        };
        &fields.iter().find(|(k, _)| k == name).expect(name).1
    }

    /// Strings a shard rule must agree on: every script the data has, and
    /// the edges of the hash.
    fn generated_words(n: usize) -> Vec<String> {
        const POOLS: [&str; 5] = [
            "人水火山川日月木金土大小中上下",
            "ひらがなカタカナ",
            "abcxyzABC",
            "𠀀𪜀丽😀",
            "·-、 ",
        ];
        let mut state: u64 = 0x5eed;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        (0..n)
            .map(|_| {
                let len = (next() % 7) as usize;
                (0..len)
                    .map(|_| {
                        let pool: Vec<char> = POOLS[(next() % POOLS.len() as u64) as usize]
                            .chars()
                            .collect();
                        pool[(next() % pool.len() as u64) as usize]
                    })
                    .collect()
            })
            .collect()
    }

    /// **The compiled shard rule is kiokun's** (ADR-0041): `shards.Place`
    /// and `shards.Places`, compiled from `examples/kiokun/Shards.pw`, against
    /// the Rust rule, on every word of the sample and on generated words.
    /// 2,500 words make `Places` answer in three calls.
    #[test]
    fn the_compiled_shard_rule_is_kiokuns() {
        let a = app();
        let mut words: Vec<String> = a.index.shard.entries.keys().cloned().collect();
        words.extend(generated_words(2_500));
        words.push(String::new());
        let batched = a.places(&words).expect("Places runs");
        assert_eq!(batched.len(), words.len());
        for (i, w) in words.iter().enumerate() {
            let rule = (shard::shard_of(w), shard::subdirectory(w));
            assert_eq!(batched[i], rule, "Places, {w:?}");
            if i < 600 {
                assert_eq!(a.place(w).expect("Place runs"), rule, "Place, {w:?}");
            }
        }
        println!(
            "place: {} words, the compiled rule and kiokun's agree",
            words.len()
        );
    }

    /// **kiokun's file-name escape** (`create_safe_filename`): what it
    /// writes, and a name several words share, read by the key its file
    /// records. A lookup of another of those words does not take that file.
    #[test]
    fn an_escaped_file_name_is_read_by_its_key() {
        assert_eq!(shard::file_name("人"), "人.json.deflate");
        assert_eq!(
            shard::file_name("a/b\\c:d*e?f\"g<h>i|j\u{7}k"),
            "a_b_c_d_e_f_g_h_i_j_k.json.deflate"
        );
        // One file name, and, since 31 is -1 modulo 32 and `_` is `/` + 48,
        // one subdirectory: only the key tells these two words apart.
        assert_eq!(shard::file_name("a/b_c"), shard::file_name("a_b/c"));
        assert_eq!(shard::subdirectory("a/b_c"), shard::subdirectory("a_b/c"));

        let dir = std::env::temp_dir().join(format!("kiokun-escape-{}", std::process::id()));
        let sample = root().join("examples/kiokun/data").join(SHARD);
        for sub in std::fs::read_dir(&sample).expect("sample").flatten() {
            if sub.path().is_dir() {
                let to = dir.join(sub.file_name());
                std::fs::create_dir_all(&to).expect("temp");
                for f in std::fs::read_dir(sub.path()).expect("sub").flatten() {
                    std::fs::copy(f.path(), to.join(f.file_name())).expect("copy");
                }
            }
        }
        let deflate = |json: &str| miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 6);
        let sub = dir.join(shard::subdirectory("a/b_c"));
        std::fs::create_dir_all(&sub).expect("temp");
        let file = sub.join(shard::file_name("a/b_c"));
        std::fs::write(&file, deflate(r#"{"key":"a/b_c"}"#)).expect("write");
        let a = load_from(&dir).expect("the sample and one more file load");
        assert_eq!(a.index.shard.entries.len(), 28);
        assert_eq!(shard::word_of(&file), Ok(Some("a/b_c".to_string())));
        assert_eq!(route(&a, "GET", "/a%2Fb_c").0, 200);
        assert_eq!(route(&a, "GET", "/a_b%2Fc").0, 404, "the file is `a/b_c`'s");

        // A name its file's key does not escape to is not kiokun's layout.
        let odd = sub.join("z_z.json.deflate");
        std::fs::write(&odd, deflate(r#"{"key":"zz"}"#)).expect("write");
        assert!(shard::word_of(&odd).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Queries a ranking must agree on, from an index: every word, reading
    /// and stub key, their prefixes, the words of every definition, and the
    /// same with case and surrounding space changed.
    fn queries(index: &data::Index) -> Vec<String> {
        let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for r in &index.rows {
            out.insert(r.word.clone());
            out.insert(r.reading.clone());
            out.insert(r.pronunciation.clone());
            if let Some(first) = r.word.chars().next() {
                out.insert(first.to_string());
            }
            for w in r.words.iter().take(3) {
                out.insert(w.clone());
                out.insert(w.chars().take(2).collect());
                out.insert(w.to_uppercase());
            }
        }
        for key in index.aliases.keys() {
            out.insert(key.clone());
            out.insert(format!(" {key}\u{3000}"));
        }
        out.extend(["", "   ", "zzzz-no-such-word", "Person", "PERSON"].map(String::from));
        out.into_iter().collect()
    }

    /// **The compiled ranking is kiokun's** (ADR-0041): `kiokun.page.Search`,
    /// compiled from Pleris, against kiokun.com's ranking in Rust
    /// (`Index::search`), hit for hit and field for field.
    #[test]
    fn the_compiled_search_is_kiokuns_ranking() {
        let a = app();
        let qs = queries(&a.index);
        let mut hits = 0;
        for q in &qs {
            let compiled = a.search(q).expect("Search runs");
            let reference = a.index.search(q, 20);
            assert_eq!(compiled, reference, "{q:?}");
            hits += compiled.len();
        }
        println!(
            "search: {} queries, {hits} hits, the compiled ranking and kiokun's agree",
            qs.len()
        );
    }

    #[test]
    fn the_shard_rule_is_kiokuns() {
        // The research's check against kiokun's own build output: `人` is in
        // `han-1char-3/ba`, and `你好` in subdirectory `1d`.
        assert_eq!(shard::shard_of("人"), "han-1char-3");
        assert_eq!(shard::subdirectory("人"), "ba");
        assert_eq!(shard::subdirectory("你好"), "1d");
        assert_eq!(shard::shard_of("好き"), shard::shard_of("好き"));
        assert!(
            shard::shard_of("好き").starts_with("han-1char-"),
            "kana are not Han"
        );
        assert!(shard::shard_of("hello").starts_with("non-han-"));
    }

    #[test]
    fn the_sample_is_one_shard() {
        let a = app();
        println!(
            "sample: {} entries, {} rows, {} aliases",
            a.index.shard.entries.len(),
            a.index.rows.len(),
            a.index.aliases.len()
        );
        assert_eq!(a.index.shard.entries.len(), 28);
        assert!(
            a.index
                .shard
                .entries
                .keys()
                .all(|w| shard::shard_of(w) == SHARD)
        );
        assert_eq!(a.index.aliases.len(), 4, "谚 贪 攒 缢");
    }

    #[test]
    fn the_compiled_lookup_follows_the_shards_redirects() {
        let a = app();
        let person = a.lookup("人").expect("runs").expect("an entry");
        assert_eq!(field(&person, "key"), &Val::String("人".into()));
        // `谚` is the simplified form; its file redirects to `諺`, and the
        // COMPONENT followed it.
        let proverb = a.lookup("谚").expect("runs").expect("an entry");
        assert_eq!(field(&proverb, "key"), &Val::String("諺".into()));
        assert_eq!(field(&proverb, "redirect"), &Val::Option(None));
        assert!(a.lookup("無").expect("runs").is_none(), "not in this shard");
    }

    #[test]
    fn search_ranks_as_kiokun_ranks() {
        let a = app();
        let first = |q: &str| {
            let hits = a.search(q).expect("runs");
            println!(
                "{q}: {} hits, first {:?}",
                hits.len(),
                hits.first().map(|h| field(h, "id").clone())
            );
            hits.first().map(|h| field(h, "id").clone())
        };
        // One hit per entry, as kiokun groups them. A definition equal to the
        // query scores 1000.
        assert_eq!(first("person"), Some(Val::String("人".into())));
        // Toneless pinyin equal to the query, 900.
        assert_eq!(first("ren"), Some(Val::String("人".into())));
        // The word itself, 1000.
        assert_eq!(first("人"), Some(Val::String("人".into())));
        // A simplified stub searches as the entry it names, 900.
        assert_eq!(first("谚"), Some(Val::String("諺".into())));
        // Kana against the reading, 875.
        assert_eq!(first("ひと"), Some(Val::String("人".into())));
        assert_eq!(first("zzzz-no-such-word"), None);
        assert!(
            a.search("person").unwrap().len() <= 20,
            "the query asks for twenty"
        );
    }

    #[test]
    fn the_pages_are_the_compiled_templates_with_no_script() {
        let a = app();
        let (status, html) = route(&a, "GET", "/%E4%BA%BA");
        assert_eq!(status, 200);
        assert!(html.contains("<h1 id=\"headword\">"), "{html}");
        assert!(html.contains("人"), "{html}");
        assert!(html.contains("rén"), "pinyin: {html}");
        assert!(!html.contains("<script"), "a static page");
        // Korean, names and the character, each from kiokun's own entry.
        let text = bare(&html);
        assert!(
            text.contains("<span class=\"hangul\">인</span>"),
            "Korean: {html}"
        );
        assert!(text.contains("Hitozaki"), "a name: {html}");
        assert!(
            text.contains("<dd class=\"strokes\">2</dd>"),
            "strokes: {html}"
        );
        assert!(text.contains("<dd class=\"grade\">1</dd>"), "grade: {html}");

        let (status, html) = route(&a, "GET", "/%E8%B0%9A");
        assert_eq!(status, 200, "the redirect, followed");
        assert!(html.contains("諺"), "{html}");

        // `WordPage`'s `{:None}` arm (ADR-0042).
        let (status, html) = route(&a, "GET", "/%E7%84%A1");
        assert_eq!(status, 404);
        assert!(
            bare(&html).contains("<h1 id=\"headword\">無</h1>"),
            "{html}"
        );
        assert!(html.contains("There is no entry for it here."), "{html}");
        assert!(!html.contains("pinyin"), "one arm renders: {html}");

        let (status, html) = route(&a, "GET", "/search?q=person");
        assert_eq!(status, 200);
        // `/{hit.target}`: the entry's path, its word one URI component.
        assert!(html.contains("href=\"/%E4%BA%BA\""), "{html}");
        assert!(
            html.contains("value=\"person\""),
            "the query stays in the box: {html}"
        );
        assert!(!html.contains("id=\"nothing\""), "{html}");
        assert!(!html.contains("<script"));

        // `{:else if q}`: said when a query found nothing, not on the empty
        // page.
        let (_, html) = route(&a, "GET", "/search?q=zzzz");
        assert!(bare(&html).contains("No entry matches zzzz."), "{html}");
        assert!(!html.contains("id=\"hits\""), "{html}");
        let (_, html) = route(&a, "GET", "/");
        assert!(
            !html.contains("id=\"nothing\"") && !html.contains("id=\"hits\""),
            "{html}"
        );

        assert_eq!(route(&a, "GET", "/%FF").0, 400, "not UTF-8");
        assert_eq!(route(&a, "GET", "/人/人").0, 400, "two segments");
        assert_eq!(route(&a, "GET", "/a%2Fb").0, 404, "a word with `/`");
        assert_eq!(route(&a, "POST", "/人").0, 405);
    }

    /// **Every file kiokun has**: the compiled shard rule against the Rust one,
    /// on every word in a kiokun-data checkout's `output_dictionary`, and
    /// every word found in the subdirectory the compiled rule names. A word is
    /// its file's name, or its file's key where the name was escaped.
    #[test]
    #[ignore = "reads a kiokun-data checkout named by KIOKUN_DATA"]
    fn the_compiled_shard_rule_places_every_word_kiokun_has() {
        let dir = PathBuf::from(std::env::var_os("KIOKUN_DATA").expect("KIOKUN_DATA"));
        let a = app();
        let (mut words, mut misplaced, mut escaped) = (0usize, Vec::new(), 0usize);
        let mut shards: std::collections::BTreeMap<String, usize> = Default::default();
        let started = std::time::Instant::now();
        for sub in std::fs::read_dir(&dir).expect("output_dictionary") {
            let sub = sub.expect("entry").path();
            if !sub.is_dir() {
                continue;
            }
            let here = sub.file_name().unwrap().to_string_lossy().to_string();
            let mut found = Vec::new();
            for file in std::fs::read_dir(&sub).expect("subdirectory") {
                let path = file.expect("file").path();
                // A name with `_` is read by its file's key: kiokun's escape
                // loses the word (`shard::file_name`).
                let Some(word) = shard::word_of(&path).expect("a word") else {
                    continue;
                };
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                assert_eq!(name, shard::file_name(&word), "kiokun's escape");
                escaped += usize::from(name != format!("{word}.json.deflate"));
                found.push(word);
            }
            let placed = a.places(&found).expect("Places runs");
            for (word, (shard, subdirectory)) in found.into_iter().zip(placed) {
                assert_eq!(
                    (shard.clone(), subdirectory.clone()),
                    (shard::shard_of(&word), shard::subdirectory(&word)),
                    "{word:?}"
                );
                if subdirectory != here {
                    misplaced.push(word);
                }
                *shards.entry(shard).or_default() += 1;
                words += 1;
            }
        }
        println!(
            "every word: {words} words in {} shards, the compiled rule and kiokun's agree, \
             {escaped} file names escaped, {} found outside the subdirectory it names, \
             in {:.0} s",
            shards.len(),
            misplaced.len(),
            started.elapsed().as_secs_f64()
        );
        assert!(
            misplaced.is_empty(),
            "{:?}",
            &misplaced[..misplaced.len().min(10)]
        );
        assert_eq!(shards.len(), 28, "{shards:?}");
    }

    /// **The compiled ranking on the whole shard**: every query `queries`
    /// derives from 17,597 entries, against kiokun's ranking in Rust.
    #[test]
    #[ignore = "reads a kiokun-data checkout named by KIOKUN_DATA"]
    fn the_compiled_search_is_kiokuns_ranking_on_the_whole_shard() {
        assert!(std::env::var_os("KIOKUN_DATA").is_some(), "set KIOKUN_DATA");
        let a = load().expect("the whole shard loads");
        let qs = queries(&a.index);
        let (mut hits, mut times) = (0, Vec::with_capacity(qs.len()));
        for q in &qs {
            let t = std::time::Instant::now();
            let compiled = a.search(q).expect("Search runs");
            times.push(t.elapsed());
            assert_eq!(compiled, a.index.search(q, 20), "{q:?}");
            hits += compiled.len();
        }
        times.sort();
        let at = |f: f64| times[((times.len() - 1) as f64 * f) as usize].as_secs_f64() * 1e3;
        println!(
            "whole shard: {} queries, {hits} hits, the compiled ranking and kiokun's agree",
            qs.len()
        );
        println!(
            "whole shard: compiled Search per query p50 {:.2} ms, p99 {:.2} ms, max {:.2} ms",
            at(0.5),
            at(0.99),
            at(1.0)
        );
    }

    /// **The whole shard**: 17,597 real entries from a kiokun-data checkout,
    /// every one looked up by the compiled `Lookup` and rendered by the compiled
    /// `WordPage`, and every in-shard redirect followed. `just e10-kiokun`
    /// runs it with `KIOKUN_DATA` set, in release.
    #[test]
    #[ignore = "reads a kiokun-data checkout named by KIOKUN_DATA"]
    fn every_entry_in_the_whole_shard_looks_up_and_renders() {
        assert!(
            std::env::var_os("KIOKUN_DATA").is_some(),
            "set KIOKUN_DATA to kiokun-data's output_dictionary"
        );
        let started = std::time::Instant::now();
        let a = load().expect("the whole shard loads");
        let loaded = started.elapsed();
        let words: Vec<String> = a.index.shard.entries.keys().cloned().collect();
        let (mut rendered, mut followed, mut failures) = (0, 0, Vec::new());
        let mut lookups = Vec::with_capacity(words.len());
        for w in &words {
            let t = std::time::Instant::now();
            let found = a.lookup(w);
            lookups.push(t.elapsed());
            match found {
                Ok(Some(entry)) => {
                    if let Some(target) = a.index.aliases.get(w) {
                        assert_eq!(field(&entry, "key"), &Val::String(target.clone()), "{w}");
                        followed += 1;
                    }
                }
                Ok(None) => {
                    // A stub whose target is in another shard: this host has
                    // only this shard, and says so.
                    assert!(a.index.aliases.contains_key(w), "{w} vanished");
                    continue;
                }
                Err(e) => {
                    failures.push(format!("{w}: {e}"));
                    continue;
                }
            }
            match a.entry_page(w) {
                Ok(p) if p.status == 200 => rendered += 1,
                Ok(p) => failures.push(format!("{w}: status {}", p.status)),
                Err(e) => failures.push(format!("{w}: {e}")),
            }
        }
        lookups.sort();
        let q = |f: f64| lookups[((lookups.len() - 1) as f64 * f) as usize].as_secs_f64() * 1e6;
        let search: Vec<(String, f64)> = ["person", "ren", "水", "ひと", "water", "谚"]
            .iter()
            .map(|term| {
                let t = std::time::Instant::now();
                let hits = a.search(term).expect("searches");
                (
                    format!("{term} ({} hits)", hits.len()),
                    t.elapsed().as_secs_f64() * 1e3,
                )
            })
            .collect();
        println!(
            "whole shard: {} entries, {} search rows, loaded in {:.0} ms",
            words.len(),
            a.index.rows.len(),
            loaded.as_secs_f64() * 1e3
        );
        println!(
            "whole shard: {rendered} rendered, {followed} redirects followed, {} not in this shard, {} failures",
            words.len() - rendered - failures.len(),
            failures.len()
        );
        println!(
            "whole shard: compiled Lookup per word p50 {:.1} µs, p99 {:.1} µs, max {:.1} µs",
            q(0.5),
            q(0.99),
            q(1.0)
        );
        for (term, ms) in &search {
            println!("whole shard: compiled Search {term}: {ms:.1} ms");
        }
        for f in failures.iter().take(10) {
            println!("whole shard: FAILED {f}");
        }
        assert!(failures.is_empty(), "{} entries failed", failures.len());
        assert_eq!(words.len(), 17_597);

        // Every word whose file name kiokun escaped, all in other shards,
        // through the route: `/` arrives as `%2F`.
        let percent = |w: &str| -> String {
            w.bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                        (b as char).to_string()
                    }
                    b => format!("%{b:02X}"),
                })
                .collect()
        };
        let mut escaped = Vec::new();
        for sub in std::fs::read_dir(data_dir())
            .expect("output_dictionary")
            .flatten()
        {
            for f in std::fs::read_dir(sub.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                if f.file_name().to_string_lossy().contains('_') {
                    escaped.push(shard::word_of(&f.path()).expect("a key").expect("a word"));
                }
            }
        }
        for w in &escaped {
            let (status, html) = route(&a, "GET", &format!("/{}", percent(w)));
            assert_eq!(status, 200, "{w}");
            assert!(html.contains(w.as_str()), "{w}: {html}");
        }
        println!(
            "whole shard: {} words kiokun escaped in a file name, each served from its file",
            escaped.len()
        );
        assert_eq!(escaped.len(), 33);
    }
}
