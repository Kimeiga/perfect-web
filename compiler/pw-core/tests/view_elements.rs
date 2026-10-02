//! **An element named with a capital letter is a view** (ADR-0072), and a
//! view used in another is composed where it is used (ADR-0136).
//!
//! Until 2026-09-26 nothing read such a tag, and `<Money value={p} />`, the
//! charter's own way to use one view in another (§8.1), passed `pw check` and
//! built as an unknown HTML element named `Money`. The view's markup was never
//! rendered, and its props were checked by nothing: a prop of the wrong type,
//! one left out, and one the view does not take all passed. ADR-0072 refused
//! every use (PW5020). Since 2026-10-02 a view composes, and its props are
//! checked as a call's arguments are: each parameter given, of its type, and
//! nothing else (PW0619), and once (PW0028, as any attribute). A tag that
//! names no view is still refused. Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(files: &[(&str, &str)]) -> Vec<String> {
    let owned: Vec<(String, String)> = files
        .iter()
        .map(|(n, s)| (n.to_string(), s.to_string()))
        .collect();
    check_sources(&owned)
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

const CHILD: &str = "view Child(n: Int) !{} {\n    <p>{n}</p>\n}\n";

fn one(src: &str) -> String {
    format!(
        "module t\n\n{CHILD}\nview V(k: Int, s: String) !{{}} {{\n    <div>\n        {src}\n    </div>\n}}\n"
    )
}

#[test]
fn a_view_used_in_a_view_composes() {
    let found = reported(&[("t.pw", &one("<Child n={k} />"))]);
    assert!(found.is_empty(), "{found:#?}");
    // The view's markup, written in place, is what composing it means.
    let found = reported(&[("t.pw", &one("<p>{k}</p>"))]);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_view_is_given_each_of_its_props_once_of_its_type() {
    // Each passed until 2026-09-26, and none was read (ADR-0072). A prop is a
    // value the page holds, a name or fields read from one, as a template's
    // every value is (ADR-0073).
    for (used, expected) in [
        ("<Child n={1} />", "PW5020 `<Child>`'s prop `n` is computed"),
        ("<Child n={s} />", "PW0605"),
        ("<Child />", "PW0619 `<Child>` is used without its prop `n`"),
        (
            "<Child n={k} m={k} />",
            "PW0619 `<Child>` takes no prop `m`",
        ),
        // An attribute written twice, on any element (PW0028).
        (
            "<Child n={k} n={k} />",
            "PW0028 `<Child>` is given `n` twice",
        ),
        (
            "<Child n=\"1\" />",
            "PW0619 `<Child>`'s prop `n` is given as text",
        ),
    ] {
        let found = reported(&[("t.pw", &one(used))]);
        assert_eq!(found.len(), 1, "{used}: {found:#?}");
        assert!(found[0].starts_with(expected), "{used}: {found:#?}");
    }
}

#[test]
fn a_view_imported_from_another_module_composes() {
    let ui = format!("module ui\n\npublic {CHILD}");
    let page = "module t\n\nimport ui.{ Child }\n\nview V(k: Int) !{} {\n    <div>\n        <Child n={k} />\n    </div>\n}\n";
    let found = reported(&[("ui.pw", &ui), ("t.pw", page)]);
    assert!(found.is_empty(), "{found:#?}");
    // Its props are checked across the import, as a call's arguments are.
    let wrong = page.replace("<Child n={k} />", "<Child />");
    let found = reported(&[("ui.pw", &ui), ("t.pw", &wrong)]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with("PW0619"), "{found:#?}");
}

#[test]
fn a_capitalised_tag_names_a_view() {
    let src = |used: &str| {
        format!(
            "module t\n\ntype Shape =\n    | Circle(Int)\n    | Empty\n\nview V() !{{}} {{\n    <div>\n        {used}\n    </div>\n}}\n"
        )
    };
    let found = reported(&[("t.pw", &src("<Chidl />"))]);
    assert_eq!(found, vec!["PW5020 `<Chidl>` names no view in scope"]);
    let found = reported(&[("t.pw", &src("<Shape />"))]);
    assert_eq!(
        found,
        vec!["PW5020 `<Shape>` names a declaration that is not a view"]
    );
    // A page renders, but is not composed into another: it is served.
    let page = src("<Q />").replace(
        "view V() !{} {",
        "page Q() {\n    view { <p>q</p> }\n}\n\nview V() !{} {",
    );
    let found = reported(&[("t.pw", &page)]);
    assert_eq!(
        found,
        vec!["PW5020 `<Q>` uses `Q` inside another view, and only a view is used as an element"]
    );
    // HTML, and a custom element, are written in lowercase.
    let found = reported(&[("t.pw", &src("<section><my-widget>x</my-widget></section>"))]);
    assert!(found.is_empty(), "{found:#?}");
}
