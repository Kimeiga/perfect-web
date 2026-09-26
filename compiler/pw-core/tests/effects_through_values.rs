//! **An effect is performed where its function is named** (ADR-0078).
//!
//! A declaration named as a value, `List.map(xs, stamp)` or
//! `let f = clock.now`, may be called wherever the value goes, so its effects
//! are performed where it is named, as a lambda's are where it is written.
//! Until 2026-09-26 only a call counted. In a view declared `!{}`,
//! `clock.now()` was refused and `() => clock.now()` was too, and each of these
//! passed:
//! - `let f = clock.now` then `f()`;
//! - a named function passed to a helper that calls it;
//! - a named function held in a record field and called through it;
//! - `List.map(xs, stamp)`: R-037's invariant, which held only for a lambda;
//! - a helper that declares no row and reads `el.offsetWidth`, or calls
//!   `clock.now` through a local: the rows of such helpers counted calls
//!   alone.
//!
//! Each test states one case, with a control.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

/// `code message | explanation` for each diagnostic on `t.pw`.
fn reported(src: &str) -> Vec<String> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                format!(
                    "{} {} | {}",
                    d.code,
                    d.message,
                    d.explanation.unwrap_or_default()
                )
            })
        })
        .collect()
}

fn module(decls: &str) -> String {
    format!(
        "module t\n\nimport clock\nimport List\nimport browser.{{ ElementRef }}\n\n\
         fn stamp(n: Int) -> Int !{{ clock.read }} {{ clock.now() + n }}\n\n\
         fn same(n: Int) -> Int !{{}} {{ n }}\n\n{decls}\n"
    )
}

/// Exactly one diagnostic, this code, on the view.
fn refused(decls: &str, code: &str) -> String {
    let found = reported(&module(decls));
    assert_eq!(found.len(), 1, "{decls}: {found:#?}");
    assert!(found[0].starts_with(code), "{decls}: {found:#?}");
    assert!(found[0].contains("`V`"), "{decls}: {found:#?}");
    found[0].clone()
}

fn clean(decls: &str) {
    let found = reported(&module(decls));
    assert!(found.is_empty(), "{decls}: {found:#?}");
}

#[test]
fn a_function_named_as_a_value_performs_its_effects_where_it_is_named() {
    let routes = [
        // A local bound to it: a platform function, and a program's own.
        "view V() !{} {\n    let f = clock.now\n    <p>{f()}</p>\n}",
        "view V() !{} {\n    let f = stamp\n    <p>{f(1)}</p>\n}",
        // A helper that calls what it is given.
        "fn apply(f: fn(Int) -> Int) -> Int !{} { f(1) }\n\n\
         view V() !{} {\n    <p>{apply(stamp)}</p>\n}",
        // A record field that holds it.
        "type Box = Box { f: fn(Int) -> Int }\n\n\
         view V() !{} {\n    let b = Box { f: stamp }\n    <p>{b.f(1)}</p>\n}",
        // A generic helper's callback (R-037's invariant).
        "view V(xs: List<Int>) !{} {\n    let ys = List.map(xs, stamp)\n    <p>x</p>\n}",
    ];
    for route in routes {
        refused(route, "PW0400");
    }
    // The same routes, carrying a function that performs nothing.
    for route in &routes[1..] {
        clean(&route.replace("stamp", "same"));
    }
}

#[test]
fn the_diagnostic_says_it_was_named_as_a_value() {
    let d = refused(
        "view V(xs: List<Int>) !{} {\n    let ys = List.map(xs, stamp)\n    <p>x</p>\n}",
        "PW0400",
    );
    assert!(d.contains("`stamp` is named as a value here"), "{d}");
}

#[test]
fn a_helper_without_a_row_carries_what_it_performs() {
    // A member it reads: a geometry read, which a view may not do.
    refused(
        "fn width(el: ElementRef) -> Float {\n    el.offsetWidth\n}\n\n\
         view V(el: ElementRef) !{} {\n    let w = width(el)\n    <p>x</p>\n}",
        "PW0401",
    );
    // A function it calls through a local.
    refused(
        "fn read() -> Int {\n    let f = clock.now\n    f()\n}\n\n\
         view V() !{} {\n    let n = read()\n    <p>x</p>\n}",
        "PW0400",
    );
    clean(
        "fn read() -> Int {\n    let f = same\n    f(1)\n}\n\n\
         view V() !{} {\n    let n = read()\n    <p>x</p>\n}",
    );
}

/// A function named in an event attribute runs when the event arrives, not
/// while the view renders: its effects are the handler's, as a lambda's in
/// the same place are.
#[test]
fn a_function_named_in_a_handler_is_the_handlers() {
    clean(
        "fn go() -> () !{ clock.read } { let n = clock.now() }\n\n\
         view V() !{} {\n    <button type=\"button\" on:press={go}>Go</button>\n}",
    );
}

/// A binding in scope is its own value, whatever declaration shares its name.
#[test]
fn a_binding_named_like_a_function_is_its_value() {
    clean("view V() !{} {\n    let stamp = 5\n    <p>{stamp}</p>\n}");
}
