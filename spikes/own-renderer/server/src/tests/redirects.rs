//! **A page answers a redirect where its address is another address of the
//! page** (ADR-XXXX): `redirect_on Type.Case permanent` (or `temporary`),
//! and a query the page reads answering the case, are answered 308 (or 307)
//! to the page's route filled with what the case carries, the address's
//! query kept, kept by no cache.
//!
//! kiokun's word page is the program: kiokun.com answers 308 to the one
//! traditional form a simplified character equals in meaning
//! (`[word]/+page.ts:419-425`), and the program declares
//! `KiokunError.Moved(String)` for it. Here its query is made to answer the
//! case for words the tests choose, so each answer is the host's alone.

use super::*;

/// kiokun's program, `change` applied to its `app.pw`, built as `pw build`
/// builds it, and served from the repository's sample.
fn served_with(change: fn(&str) -> String) -> Served {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = tempfile::TempDir::with_prefix("pw-redirect-").expect("a temporary directory");
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
    Served {
        server: Server::from_build(out.clone(), out).expect("served"),
        _dir: dir,
    }
}

/// The word query's answer, as written in `app.pw`.
const ANSWER: &str = "    match page {\n        Some(l) => Ok(shown(word, l, labels, glosses)),\n        None => Err(NotFound),\n    }\n";

/// The query answering `Moved` for the words the tests choose: `old` is
/// 魚's, `same` its own, and `dots` an address no segment can carry.
fn moving(app: &str) -> String {
    assert_eq!(app.matches(ANSWER).count(), 1, "the word query's answer");
    app.replace(
        ANSWER,
        "    if word == \"old\" {\n        Err(Moved(\"魚\"))\n    } else {\n        \
         if word == \"same\" {\n            Err(Moved(\"same\"))\n        } else {\n            \
         if word == \"dots\" {\n                Err(Moved(\"..\"))\n            } else {\n\
         \x20               match page {\n                    \
         Some(l) => Ok(shown(word, l, labels, glosses)),\n                    \
         None => Err(NotFound),\n                }\n            }\n        }\n    }\n",
    )
}

/// The page's clause, `how` the move is.
fn declared(app: &str, how: &str) -> String {
    let clause = "    not_found_on KiokunError.NotFound\n";
    assert_eq!(app.matches(clause).count(), 1, "the page's clause");
    app.replace(
        clause,
        &format!("{clause}    redirect_on  KiokunError.Moved {how}\n"),
    )
}

fn permanent(app: &str) -> String {
    declared(&moving(app), "permanent")
}

fn temporary(app: &str) -> String {
    declared(&moving(app), "temporary")
}

/// The response to `GET path`, whole.
fn fetched(s: &Server, path: &str) -> String {
    fetched_as(s, path, Some("a"))
        .into_iter()
        .map(|(_, c)| c)
        .collect()
}

/// The word page's plan, as the build wrote it.
fn word_plan(s: &Server) -> &serde_json::Value {
    let mut pages = s
        .plans
        .iter()
        .filter(|(page, _)| page.ends_with(".WordPage"));
    let (_, plan) = pages.next().expect("the word page's plan");
    assert!(pages.next().is_none(), "one word page");
    plan
}

/// A response's head, up to its blank line.
fn head(response: &str) -> &str {
    response.split("\r\n\r\n").next().unwrap_or_default()
}

#[test]
fn a_page_at_another_address_of_the_page_is_moved_there_for_good() {
    let s = served_with(permanent);
    let moved = fetched(&s, "/word/old");
    let h = head(&moved);
    assert!(h.starts_with("HTTP/1.1 308 Permanent Redirect\r\n"), "{h}");
    // The route filled with what the case carries, encoded as a link's hole
    // is: 魚 is E9 AD 9A in UTF-8.
    assert!(h.contains("\r\nlocation: /word/%E9%AD%9A\r\n"), "{h}");
    // Kept by no cache: a heuristically cacheable 308 would outlive the
    // data that decided it.
    assert!(h.contains("cache-control: private, no-store\r\n"), "{h}");
    // And a note with a link, for a reader that does not follow it.
    assert!(
        moved.contains("<a href=\"/word/%E9%AD%9A\">/word/%E9%AD%9A</a>"),
        "{moved}"
    );
    // The address's query is kept, as kiokun.com keeps it.
    let kept = fetched(&s, "/word/old?from=x&y=%20z");
    assert!(
        head(&kept).contains("\r\nlocation: /word/%E9%AD%9A?from=x&y=%20z\r\n"),
        "{kept}"
    );
    // Control: the address it moves to is the page's.
    assert!(fetched(&s, "/word/%E9%AD%9A").starts_with("HTTP/1.1 200 OK\r\n"));
    // Control: a word that names nothing is still not found.
    assert!(fetched(&s, "/word/no-such-word-in-kiokun").starts_with("HTTP/1.1 404 Not Found\r\n"));
}

#[test]
fn a_temporary_move_is_answered_307() {
    let s = served_with(temporary);
    let h = fetched(&s, "/word/old");
    assert!(h.starts_with("HTTP/1.1 307 Temporary Redirect\r\n"), "{h}");
    assert!(
        head(&h).contains("\r\nlocation: /word/%E9%AD%9A\r\n"),
        "{h}"
    );
}

#[test]
fn a_move_to_itself_or_to_no_segment_is_the_programs_fault() {
    let s = served_with(permanent);
    let same = fetched(&s, "/word/same");
    assert!(
        same.starts_with("HTTP/1.1 500 Internal Server Error\r\n"),
        "{same}"
    );
    assert!(same.contains("moves `same` to itself"), "{same}");
    let dots = fetched(&s, "/word/dots");
    assert!(
        dots.starts_with("HTTP/1.1 500 Internal Server Error\r\n"),
        "{dots}"
    );
    assert!(dots.contains("`..` is no segment of an address"), "{dots}");
}

#[test]
fn a_case_the_page_does_not_name_is_a_failure_like_any_other() {
    // The query answers `Moved`, and the page says nothing of it: the host
    // cannot read it as a move, and the page cannot be shown now.
    let s = served_with(moving);
    assert!(!word_plan(&s).to_string().contains("redirect"));
    let failed = fetched(&s, "/word/old");
    assert!(
        failed.starts_with("HTTP/1.1 503 Service Unavailable\r\n"),
        "{failed}"
    );
}

#[test]
fn the_plan_carries_the_move_on_the_binding_that_can_answer_it() {
    let s = served_with(permanent);
    let plan = word_plan(&s);
    let bindings = plan["bindings"].as_array().expect("bindings");
    let word = bindings
        .iter()
        .find(|b| b["binding"] == "page")
        .expect("the word's binding");
    assert_eq!(
        word["redirect"],
        serde_json::json!({ "case": "moved", "permanent": true }),
        "{plan}"
    );
}
