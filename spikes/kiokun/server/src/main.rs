//! **The kiokun slice's host** (E10): one shard of kiokun.com's dictionary,
//! served by what `pw build` compiled from `examples/kiokun/`.
//!
//! ```text
//! GET /             the search page, empty
//! GET /search?q=    the compiled `Search`, rendered by `SearchPage`
//! GET /<word>       the compiled `Lookup`: `EntryPage`, or 404 with `NotFound`
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
    let shard = shard::Shard::load(dir, SHARD)?;
    app::App::load(
        &root().join("docs/evidence/E10/kiokun"),
        data::Index::build(shard),
    )
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
        word => match decode(word.trim_start_matches('/'), false) {
            Some(w) if !w.is_empty() && !w.contains('/') => app.entry_page(&w),
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

    fn field<'a>(v: &'a Val, name: &str) -> &'a Val {
        let Val::Record(fields) = v else {
            panic!("not a record: {v:?}")
        };
        &fields.iter().find(|(k, _)| k == name).expect(name).1
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

        let (status, html) = route(&a, "GET", "/%E8%B0%9A");
        assert_eq!(status, 200, "the redirect, followed");
        assert!(html.contains("諺"), "{html}");

        let (status, html) = route(&a, "GET", "/%E7%84%A1");
        assert_eq!(status, 404);
        assert!(html.contains("This shard has no entry for it."), "{html}");

        let (status, html) = route(&a, "GET", "/search?q=person");
        assert_eq!(status, 200);
        // Relative, from `/search`: the entry at `/人`.
        assert!(html.contains("href=\"人\""), "{html}");
        assert!(
            html.contains("value=\"person\""),
            "the query stays in the box: {html}"
        );
        assert!(!html.contains("<script"));

        assert_eq!(route(&a, "GET", "/%FF").0, 400, "not UTF-8");
        assert_eq!(route(&a, "POST", "/人").0, 405);
    }

    /// **The whole shard**: 17,597 real entries from a kiokun-data checkout,
    /// every one looked up by the compiled `Lookup` and rendered by the compiled
    /// `EntryPage`, and every in-shard redirect followed. `just e10-kiokun`
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
    }
}
