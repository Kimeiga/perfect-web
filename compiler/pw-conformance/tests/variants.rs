//! **A match over `Option`, a field read, and `Some`/`None` built**, run.
//!
//! The kiokun slice's lookup follows one redirect: `地図` is a stub whose
//! `redirect` names `地圖`. That needs the backend to read a variant's
//! discriminant, bind its payload, read a record field, build an option, and
//! join two arms' values: each new here, and each checked by running the
//! component, not by reading what it emitted.

use std::collections::BTreeMap;

use pw_conformance::{Calls, Runnable, compile, operation, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module dict

type Entry = Entry {
    key: String,
    redirect: Option<String>,
    gloss: String,
}

fn get(word: String) -> Option<Entry> !{ database.read<Entry> }
    host "kiokun:data/entries#get"

public query Lookup(word: String) -> Option<Entry> {
    match get(word) {
        Some(entry) => match entry.redirect {
            Some(target) => get(target),
            None => Some(entry),
        },
        None => None,
    }
}
"#;

fn entry(key: &str, redirect: Option<&str>, gloss: &str) -> Val {
    Val::Record(vec![
        ("key".into(), Val::String(key.into())),
        (
            "redirect".into(),
            Val::Option(redirect.map(|r| Box::new(Val::String(r.into())))),
        ),
        ("gloss".into(), Val::String(gloss.into())),
    ])
}

/// A data layer over `entries`, recording every lookup.
fn layer(calls: &Calls, entries: &[(&str, Val)]) -> BTreeMap<String, pw_host::engine::HostFn> {
    let table: BTreeMap<String, Val> = entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    BTreeMap::from([operation(calls, "kiokun:data/entries#get", move |args| {
        let [Val::String(word)] = args else {
            return Val::Option(None);
        };
        Val::Option(table.get(word).cloned().map(Box::new))
    })])
}

fn lookup() -> Runnable {
    Runnable::new(compile(&units(&[("dict.pw", PROGRAM)]), "dict.Lookup"))
}

fn some(v: Val) -> Vec<Val> {
    vec![Val::Option(Some(Box::new(v)))]
}

#[test]
fn a_stub_is_followed_to_the_entry_it_names() {
    let calls = Calls::default();
    let map = entry("地圖", None, "map");
    let ops = layer(
        &calls,
        &[
            ("地図", entry("地図", Some("地圖"), "")),
            ("地圖", map.clone()),
        ],
    );
    let out = lookup()
        .call(&ops, &[Val::String("地図".into())])
        .expect("runs");
    println!("地図 -> {out:?}");
    assert_eq!(out, some(map));
    let seen: Vec<Vec<Val>> = calls
        .lock()
        .unwrap()
        .iter()
        .map(|(_, a)| a.clone())
        .collect();
    assert_eq!(
        seen,
        [
            vec![Val::String("地図".into())],
            vec![Val::String("地圖".into())]
        ],
        "the stub, then the entry its redirect names"
    );
}

#[test]
fn an_entry_is_returned_as_itself() {
    // `None => Some(entry)`: a new option, built in the invocation region,
    // whose payload is the record read out of the first answer.
    let calls = Calls::default();
    let person = entry("人", None, "person; people");
    let ops = layer(&calls, &[("人", person.clone())]);
    let out = lookup()
        .call(&ops, &[Val::String("人".into())])
        .expect("runs");
    println!("人 -> {out:?}");
    assert_eq!(out, some(person));
    assert_eq!(calls.lock().unwrap().len(), 1, "one lookup, no redirect");
}

#[test]
fn a_missing_word_is_none() {
    let calls = Calls::default();
    let ops = layer(&calls, &[]);
    let out = lookup()
        .call(&ops, &[Val::String("無".into())])
        .expect("runs");
    assert_eq!(out, vec![Val::Option(None)]);
}

#[test]
fn a_stub_whose_target_is_missing_is_none() {
    let calls = Calls::default();
    let ops = layer(&calls, &[("地図", entry("地図", Some("地圖"), ""))]);
    let out = lookup()
        .call(&ops, &[Val::String("地図".into())])
        .expect("runs");
    assert_eq!(out, vec![Val::Option(None)]);
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn strings_survive_every_path_intact() {
    // Empty, long, and multi-byte: a pointer or a length off by one shows up
    // here as a different string, or as a trap.
    let long = "語".repeat(5_000);
    let calls = Calls::default();
    let ops = layer(
        &calls,
        &[
            ("", entry("", None, "")),
            ("長", entry("長", None, &long)),
            ("🀄", entry("🀄", Some("長"), "tile")),
        ],
    );
    let r = lookup();
    assert_eq!(
        r.call(&ops, &[Val::String("".into())]).expect("runs"),
        some(entry("", None, ""))
    );
    assert_eq!(
        r.call(&ops, &[Val::String("長".into())]).expect("runs"),
        some(entry("長", None, &long))
    );
    assert_eq!(
        r.call(&ops, &[Val::String("🀄".into())]).expect("runs"),
        some(entry("長", None, &long))
    );
}

// --- what the backend refuses, by name --------------------------------------

fn refused(program: &str, id: &str) -> String {
    let err = pw_core::backend::component::compile(&units(&[("m.pw", program)]), id)
        .map(|_| ())
        .expect_err("refused");
    println!("{id}: {err}");
    err
}

#[test]
fn a_match_that_misses_a_case_is_refused() {
    // Refused before it could become a component with no code for `None`:
    // either the checker's exhaustiveness or the lowering says so.
    let err = refused(
        "module m\n\ntype Doc = Doc {\n    s: String,\n}\n\nfn get(w: String) -> Option<String> !{ database.read<Doc> }\n    host \"m:d/e#get\"\n\npublic query Q(w: String) -> Option<String> {\n    match get(w) {\n        Some(x) => Some(x),\n    }\n}\n",
        "m.Q",
    );
    // The CHECKER refuses it, and names the missing case. Until 2026-09-25 it
    // did not: exhaustiveness read only a bare name annotated with a declared
    // sum type, and this refusal was the backend's. The backend's refusal
    // stays behind the checker's as a second layer.
    assert!(
        err.contains("[PW0305] match on `Option<String>` is not exhaustive"),
        "{err}"
    );
}

#[test]
fn a_nested_pattern_that_misses_a_case_is_refused() {
    // Refused by the backend by name until ADR-0060, which compiles a nested
    // pattern; the checker now reads it, and names what it misses.
    let err = refused(
        "module m\n\ntype Doc = Doc {\n    s: String,\n}\n\nfn get(w: String) -> Option<Option<String>> !{ database.read<Doc> }\n    host \"m:d/e#get\"\n\npublic query Q(w: String) -> Option<String> {\n    match get(w) {\n        Some(Some(x)) => Some(x),\n        None => None,\n    }\n}\n",
        "m.Q",
    );
    assert!(
        err.contains("[PW0305] match on `Option<Option<String>>` is not exhaustive"),
        "{err}"
    );
}

#[test]
fn a_match_over_a_list_is_refused_by_name() {
    let err = refused(
        "module m\n\ntype Doc = Doc {\n    s: String,\n}\n\nfn get(w: String) -> List<String> !{ database.read<Doc> }\n    host \"m:d/e#get\"\n\npublic query Q(w: String) -> Option<String> {\n    match get(w) {\n        Some(x) => Some(x),\n        None => None,\n    }\n}\n",
        "m.Q",
    );
    // The checker's, since ADR-0060: a list is taken apart by no pattern.
    assert!(
        err.contains("[PW0608] `Some` is not a constructor of `List<String>`"),
        "{err}"
    );
}
