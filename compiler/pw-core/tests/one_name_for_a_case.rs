//! **A case has one name where values are rendered: its WIT case's**
//! (ADR-0206).
//!
//! The browser's wire names a case as its WIT case is: `some`, `none`,
//! `ok`, `err`, and `circle` for a declared `Circle` (ADR-0061, ADR-0130).
//! The build writes a signal's first value so, and a handler sets one so.
//! The template named the language's four `Some`, `None`, `Ok` and `Err`, as
//! a host's query value did. So a page matching on an `Option` or a `Result`
//! a signal holds found no arm for it.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;
use serde_json::json;

const PAGE: &str = r#"module m

type Shape =
    | Circle(Int)
    | Empty

page P() {
    cache private

    signal picked: Option<Int> = None
    signal done: Result<Int, String> = Ok(1)
    signal shape: Shape = Shape.Circle(2)

    view {
        <main>
            {#match picked}
            {:Some(n)}<p id="some">{n}</p>
            {:None}<p id="none">none</p>
            {/match}
            {#match done}
            {:Ok(v)}<p id="ok">{v}</p>
            {:Err(e)}<p id="err">{e}</p>
            {/match}
            {#match shape}
            {:Circle(r)}<p id="circle">{r}</p>
            {:Empty}<p id="empty">empty</p>
            {/match}
            <button type="button" on:press={() => picked = Some(3)}>Pick</button>
        </main>
    }
}
"#;

fn unit() -> Unit {
    Unit {
        path: "m.pw".into(),
        hir: lower_file(PAGE, &parse_tree(PAGE).green),
        src: PAGE.to_string(),
    }
}

/// The page, rendered with each signal at `values`, or else at the first
/// value the build writes for it.
fn rendered(values: &[(&str, serde_json::Value)]) -> Result<String, pw_render::Blocked> {
    let u = unit();
    let templates: Vec<pw_render::Template> = pw_core::template_ir::build(&[&u.hir])
        .into_iter()
        .map(|t| serde_json::from_value(serde_json::to_value(t).expect("writes")).expect("reads"))
        .collect();
    let page = templates
        .iter()
        .find(|t| t.path == "m.P")
        .expect("the page");
    let compiled = pw_core::backend::signals::compile(&[u]).expect("checks");
    let first = compiled[0].signals.as_ref().expect("data");
    let mut env = pw_render::Env::new();
    for s in first {
        let given = values.iter().find(|(n, _)| *n == s.name).map(|(_, v)| v);
        let value = pw_render::Value::from_wire(given.unwrap_or(&s.initial)).expect("a value");
        env = env.set(&s.name, value);
    }
    pw_render::render(page, &env, &templates)
}

/// The text of the paragraphs a render shows, markers aside.
fn shown(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<p id=\"") {
        let tail = &rest[at..];
        let end = tail.find("</p>").expect("closed");
        let mut text = String::new();
        let mut inner = &tail[..end];
        inner = &inner[inner.find('>').expect("opened") + 1..];
        while let Some(i) = inner.find("<!--") {
            text.push_str(&inner[..i]);
            inner = &inner[i + inner[i..].find("-->").expect("closed") + 3..];
        }
        text.push_str(inner);
        out.push(format!(
            "{}:{text}",
            &tail[7..tail[7..].find('"').expect("id") + 7]
        ));
        rest = &tail[end..];
    }
    out
}

#[test]
fn a_page_matches_the_cases_its_signals_hold() {
    // At the first values the build writes.
    let html = rendered(&[]).expect("renders");
    assert_eq!(shown(&html), ["none:none", "ok:1", "circle:2"], "{html}");
    // As a handler sets them, as the browser's wire writes a case.
    let html = rendered(&[
        ("picked", json!({ "$case": "some", "value": 3 })),
        ("done", json!({ "$case": "err", "value": "late" })),
        ("shape", json!({ "$case": "empty" })),
    ])
    .expect("renders");
    assert_eq!(
        shown(&html),
        ["some:3", "err:late", "empty:empty"],
        "{html}"
    );
}

#[test]
fn the_build_writes_each_case_by_its_wit_name() {
    let compiled = pw_core::backend::signals::compile(&[unit()]).expect("checks");
    let first: Vec<(String, serde_json::Value)> = compiled[0]
        .signals
        .as_ref()
        .expect("data")
        .iter()
        .map(|s| (s.name.clone(), s.initial.clone()))
        .collect();
    assert_eq!(
        first,
        [
            ("picked".to_string(), json!({ "$case": "none" })),
            ("done".to_string(), json!({ "$case": "ok", "value": 1 })),
            (
                "shape".to_string(),
                json!({ "$case": "circle", "value": 2 })
            ),
        ]
    );
}
