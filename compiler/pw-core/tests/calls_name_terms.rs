//! **A call names a term** (ADR-0087).
//!
//! `resolve::Namespace` says what each kind of declaration is for. A view, a
//! component, a page and a materialization are rendered, an event is emitted,
//! and an effect is performed; none is called. Until 2026-09-26 the rule for
//! a bare call accepted any name that resolved in any namespace, so each of
//! these checked, and every analysis answered for it as for a call to
//! nothing: no effects, no label, no type. `fn f() -> Int { Badge(1) }`
//! checked. Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(src: &str, says: &str) {
    let found = reported(src);
    assert_eq!(found.len(), 1, "{src}\n{found:#?}");
    assert!(found[0].contains(says), "{src}\n{found:#?}");
}

fn clean(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{src}\n{found:#?}");
}

const UI: &str = "module t\n\n\
    view Badge(n: Int) !{} {\n    <span>{n}</span>\n}\n\n\
    page Home() {\n    placement origin\n\n    view {\n        <main>home</main>\n    }\n}\n\n\
    fn count(n: Int) -> Int !{} { n }\n";

#[test]
fn a_call_names_no_view_and_no_page() {
    one(
        &format!("{UI}\nfn f() -> Int !{{}} {{\n    let v = Badge(1)\n    1\n}}\n"),
        "PW0027 `Badge` is a view, which a call cannot name",
    );
    // The value it was said to produce had no type, so this checked too.
    one(
        &format!("{UI}\nfn f() -> Int !{{}} {{ Badge(1) }}\n"),
        "PW0027 `Badge` is a view",
    );
    one(
        &format!("{UI}\nfn f() -> Int !{{}} {{\n    let p = Home()\n    1\n}}\n"),
        "PW0027 `Home` is a page",
    );
    clean(&format!("{UI}\nfn f() -> Int !{{}} {{ count(1) }}\n"));
}

#[test]
fn a_call_names_no_event() {
    let src = "module t\n\nevent Changed(n: Int)\n\n\
               fn f(n: Int) -> Int !{} {\n    let e = Changed(n)\n    1\n}\n";
    one(
        src,
        "PW0027 `Changed` is an event, which a call cannot name",
    );
    // The wrong arity changed nothing: it was related by nothing either.
    one(
        &src.replace("Changed(n)", "Changed(n, 2)"),
        "PW0027 `Changed` is an event",
    );
    clean(&src.replace("let e = Changed(n)", "let e = n"));
}

/// An effect's name is exported to every unit (`prelude Effect`), so it is
/// visible, and still is not a term.
#[test]
fn a_call_names_no_effect() {
    let effects = "module fx\n\nprelude Effect\n\neffect ping {\n    placement origin\n    capability ping\n}\n";
    let src = "module t\n\nfn f() -> Int !{} {\n    let x = ping()\n    1\n}\n";
    let found: Vec<String> = check_sources(&[
        ("fx.pw".to_string(), effects.to_string()),
        ("t.pw".to_string(), src.to_string()),
    ])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
    .collect();
    assert_eq!(
        found,
        vec!["PW0027 `ping` is an effect, which a call cannot name"],
        "{found:#?}"
    );
}

/// Each namespace declares a name once, so a function or a type may share a
/// view's: a call names the function, or builds the type.
#[test]
fn a_term_or_a_type_that_shares_the_name_is_what_a_call_names() {
    clean(
        "module t\n\nview Tally(n: Int) !{} {\n    <span>{n}</span>\n}\n\n\
         fn Tally(n: Int) -> Int !{} { n }\n\n\
         fn f() -> Int !{} { Tally(1) }\n",
    );
    clean(
        "module t\n\ntype Point = Point { x: Int }\n\n\
         view Point(n: Int) !{} {\n    <span>{n}</span>\n}\n\n\
         fn f() -> Int !{} {\n    let p = Point(1)\n    p.x\n}\n",
    );
}

/// A name some scope binds is the value it holds (ADR-0066): a parameter
/// that shares a view's name is called, and the view is not.
#[test]
fn a_binding_that_shares_the_name_is_its_value() {
    clean(&format!(
        "{UI}\nfn f(Badge: fn(Int) -> Int) -> Int !{{}} {{ Badge(1) }}\n"
    ));
    clean(&format!(
        "{UI}\nfn f() -> Int !{{}} {{\n    let Badge = count\n    Badge(1)\n}}\n"
    ));
}
