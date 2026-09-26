//! **An element named with a capital letter is a view** (ADR-0072).
//!
//! Until 2026-09-26 nothing read such a tag, and `<Money value={p} />`, the
//! charter's own way to use one view in another (§8.1), passed `pw check` and
//! built as an unknown HTML element named `Money`. The view's markup was never
//! rendered, and its props were checked by nothing: a prop of the wrong type,
//! one left out, and one the view does not take all passed. A view used in
//! another view is not compiled yet, so it is refused (PW5020), and so is a
//! tag that names no view. Each test states one case, with a control.

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
        "module t\n\n{CHILD}\nview V(k: Int) !{{}} {{\n    <div>\n        {src}\n    </div>\n}}\n"
    )
}

#[test]
fn a_view_used_in_a_view_is_refused() {
    for used in [
        "<Child n={k} />",
        // A prop of the wrong type, one left out, and one it does not take:
        // each passed, and none was read.
        "<Child n={\"a\"} />",
        "<Child />",
        "<Child n={k} m={k} />",
    ] {
        let found = reported(&[("t.pw", &one(used))]);
        assert_eq!(found.len(), 1, "{used}: {found:#?}");
        assert!(
            found[0].starts_with("PW5020 `<Child>` uses `Child` inside another view"),
            "{used}: {found:#?}"
        );
    }
    // The view's markup, written in place, is what composing it would mean.
    let found = reported(&[("t.pw", &one("<p>{k}</p>"))]);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_view_imported_from_another_module_is_refused() {
    let ui = format!("module ui\n\npublic {CHILD}");
    let page = "module t\n\nimport ui.{ Child }\n\nview V(k: Int) !{} {\n    <div>\n        <Child n={k} />\n    </div>\n}\n";
    let found = reported(&[("ui.pw", &ui), ("t.pw", page)]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("PW5020 `<Child>` uses `Child`"),
        "{found:#?}"
    );
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
    // HTML, and a custom element, are written in lowercase.
    let found = reported(&[("t.pw", &src("<section><my-widget>x</my-widget></section>"))]);
    assert!(found.is_empty(), "{found:#?}");
}
