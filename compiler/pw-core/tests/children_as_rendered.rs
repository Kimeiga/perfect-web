//! **An element holds only the children HTML permits, as the page holds
//! them** (ADR-0204, PW5012).
//!
//! A block's rows and branches are the element's children, and a view used
//! there is the elements it renders at its top. Until 2026-10-05 only the
//! elements written directly inside were read: a `<div>` in an `{#each}`
//! inside a `<ul>` passed, and `<ul><Item /></ul>` was refused by the view's
//! name though `Item` renders an `<li>`.

use pw_core::check::check_sources;

fn reported(files: &[(&str, &str)]) -> Vec<String> {
    let sources: Vec<(String, String)> = files
        .iter()
        .map(|(n, s)| (n.to_string(), s.to_string()))
        .collect();
    check_sources(&sources)
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// Everything a one-file program is told: the nesting refusal alone, or
/// nothing.
fn nesting(src: &str) -> Vec<String> {
    reported(&[("t.pw", src)])
}

/// A module with `Item` rendering `item`, used in a `<ul>` by `List`.
fn listed(item: &str) -> String {
    format!(
        "module t\n\ntype I = I {{ id: Int, on: Bool }}\n\n\
         view Item(i: I) !{{}} {{\n    {item}\n}}\n\n\
         view List(is: List<I>, i: I) !{{}} {{\n    <ul><Item i={{i}} /></ul>\n}}\n"
    )
}

#[test]
fn a_view_is_the_elements_it_renders() {
    // An `<li>`: a list's child, though the tag written is `<Item>`.
    assert_eq!(nesting(&listed("<li>one</li>")), Vec::<String>::new());
    // A `<div>`: named as what it renders.
    assert_eq!(
        nesting(&listed("<div>one</div>")),
        ["PW5012 `<Item>` renders `<div>`, which is not permitted as a child of `<ul>`"]
    );
}

#[test]
fn a_blocks_rows_and_branches_are_the_elements_children() {
    let each = "module t\n\ntype I = I { id: Int, on: Bool }\n\n\
        view List(is: List<I>) !{} {\n    <ul>{#each is as i (i.id)}<div>row</div>{/each}</ul>\n}\n";
    assert_eq!(
        nesting(each),
        ["PW5012 `<div>` is not permitted as a child of `<ul>`"]
    );
    // Each branch of an `{#if}`, the second wrong alone.
    let branches = each.replace(
        "{#each is as i (i.id)}<div>row</div>{/each}",
        "{#each is as i (i.id)}{#if i.on}<li>on</li>{:else}<p>off</p>{/if}{/each}",
    );
    assert_eq!(
        nesting(&branches),
        ["PW5012 `<p>` is not permitted as a child of `<ul>`"]
    );
    // Rows that are the list's own: none.
    let rows = each.replace("<div>row</div>", "<li>row</li>");
    assert_eq!(nesting(&rows), Vec::<String>::new());
}

#[test]
fn a_view_is_read_through_its_blocks_and_the_views_it_uses() {
    // `Item` renders `Line` in one branch, and `Line` a `<div>`.
    let src = listed("{#if i.on}<li>on</li>{:else}<Line i={i} />{/if}").replace(
        "view List(",
        "view Line(i: I) !{} {\n    <div>off</div>\n}\n\nview List(",
    );
    assert_eq!(
        nesting(&src),
        ["PW5012 `<Item>` renders `<div>`, which is not permitted as a child of `<ul>`"]
    );
    // With `Line` an `<li>`, nothing.
    assert_eq!(
        nesting(&src.replace("<div>off</div>", "<li>off</li>")),
        Vec::<String>::new()
    );
}

#[test]
fn a_view_from_another_module_is_read_where_it_is_declared() {
    let d = "module d\n\ntype I = I { id: Int, on: Bool }\n";
    // `Cells` renders `Cell`, which `t` does not import: it is read where
    // `Cells` is declared.
    let ui = "module ui\n\nimport d.{ I }\n\n\
        view Row(i: I) !{} {\n    <tr><td>row</td></tr>\n}\n\n\
        view Cell(i: I) !{} {\n    <td>cell</td>\n}\n\n\
        view Cells(i: I) !{} {\n    <Cell i={i} />\n}\n";
    let t = |used: &str| {
        format!(
            "module t\n\nimport d.{{ I }}\nimport ui.{{ Row, Cells }}\n\n\
             view Table(i: I) !{{}} {{\n    <table><tbody>{used}</tbody></table>\n}}\n"
        )
    };
    let nested = |used: &str| reported(&[("d.pw", d), ("ui.pw", ui), ("t.pw", &t(used))]);
    assert_eq!(nested("<Row i={i} />"), Vec::<String>::new());
    assert_eq!(
        nested("<Cells i={i} />"),
        ["PW5012 `<Cells>` renders `<td>`, which is not permitted as a child of `<tbody>`"]
    );
}

#[test]
fn a_view_that_contains_itself_is_read_once() {
    // `Tree` renders an `<li>` whose list holds `Tree` again: read once, and
    // what it renders is an `<li>`.
    let src = "module t\n\ntype N = N { id: Int, kids: List<N> }\n\n\
        view Tree(n: N) !{} {\n    <li><ul>{#each n.kids as k (k.id)}<Tree n={k} />{/each}</ul></li>\n}\n\n\
        view Root(n: N) !{} {\n    <ul><Tree n={n} /></ul>\n}\n";
    assert_eq!(nesting(src), Vec::<String>::new());
    // And one whose top holds itself, through a block, ends.
    let top = src.replace(
        "<li><ul>{#each n.kids as k (k.id)}<Tree n={k} />{/each}</ul></li>",
        "<div>leaf</div>{#each n.kids as k (k.id)}<Tree n={k} />{/each}",
    );
    assert_eq!(
        nesting(&top),
        ["PW5012 `<Tree>` renders `<div>`, which is not permitted as a child of `<ul>`"]
    );
}
