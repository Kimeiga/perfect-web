//! **A page's title** (ADR-0183): written into the document's head by its
//! host, from the page's values, and nowhere in its body.
//!
//! The text is collapsed and trimmed as a browser's `document.title` reads
//! it, so the title a page is served with and the title a change sets are
//! one text.

use pw_render::*;

/// `<title>{store.name}, open now</title><main><h1>{store.name}</h1></main>`
fn page(pieces: Vec<TitlePiece>) -> Template {
    let st = |s: &str| Chunk::Static(s.to_string());
    Template {
        path: "t.P".into(),
        name: "P".into(),
        params: vec![],
        schema: "s".into(),
        chunks: vec![
            st("<main><h1>"),
            Chunk::Dynamic(Part::Text {
                id: PartId(0),
                value: "store.name".into(),
                context: Context::Text,
            }),
            st("</h1></main>"),
            Chunk::Dynamic(Part::Title {
                id: PartId(1),
                pieces,
            }),
        ],
    }
}

fn store(name: &str) -> Env {
    Env::new().set(
        "store",
        Value::Record([("name".to_string(), Value::Text(name.into()))].into()),
    )
}

#[test]
fn a_title_is_the_page_s_text_and_values_and_nothing_in_its_body() {
    let t = page(vec![
        TitlePiece::Value("store.name".into()),
        TitlePiece::Text(", open now".into()),
    ]);
    let env = store("Blue Bottle");
    assert_eq!(
        title_text(&t, &env).expect("rendered"),
        Some("Blue Bottle, open now".to_string())
    );
    let body = render(&t, &env, &[]).expect("rendered");
    assert_eq!(
        body,
        "<main><h1><!--pw:s0-->Blue Bottle<!--pw:e0--></h1></main>"
    );
    // The manifest lists it, anchored to the document.
    let entry = t
        .manifest()
        .into_iter()
        .find(|e| e.kind == "title")
        .expect("listed");
    assert_eq!((entry.id, entry.anchor), (PartId(1), Anchor::Document));
    assert_eq!(entry.value, "store.name");
}

#[test]
fn a_title_is_text_as_a_browser_reads_it() {
    // Whitespace collapsed and trimmed, as `document.title` reads it; and
    // not escaped: its host escapes it where it writes it.
    let t = page(vec![
        TitlePiece::Text("\n   ".into()),
        TitlePiece::Value("store.name".into()),
        TitlePiece::Text("  \t& more\n".into()),
    ]);
    assert_eq!(
        title_text(&t, &store("Tom  <Jerry>")).expect("rendered"),
        Some("Tom <Jerry> & more".to_string())
    );
}

#[test]
fn a_title_reads_what_the_page_is_given_or_is_refused() {
    let t = page(vec![TitlePiece::Value("store.name".into())]);
    assert!(matches!(
        title_text(&t, &Env::new()),
        Err(Blocked::MissingValue { path }) if path == "store.name"
    ));
    // HTML is no title.
    let raw = Env::new().set(
        "store",
        Value::Record(
            [(
                "name".to_string(),
                Value::Raw {
                    html: "<b>Blue</b>".into(),
                    capability: "unsafe.raw_html".into(),
                },
            )]
            .into(),
        ),
    );
    assert!(matches!(
        title_text(&t, &raw),
        Err(Blocked::UnrepresentedConstruct { .. })
    ));
    // A template that states no title: its host names the page.
    let mut untitled = page(vec![]);
    untitled.chunks.pop();
    assert_eq!(
        title_text(&untitled, &store("Blue Bottle")).expect("rendered"),
        None
    );
}

/// `pw-render`'s document for one template, rendered with no values.
fn document(chunks: serde_json::Value) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let temp = tempfile::TempDir::with_prefix("pw-titles-").expect("a temporary directory");
    let out = temp.path().join("out");
    let template = serde_json::json!([{
        "path": "t.P", "name": "P", "schema": "s", "params": [], "chunks": chunks,
    }]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pw-render"))
        .args(["--out", out.to_str().expect("a path")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pw-render runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(template.to_string().as_bytes())
        .expect("the IR");
    let done = child.wait_with_output().expect("finishes");
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    std::fs::read_to_string(out.join("P.html")).expect("written")
}

#[test]
fn a_page_s_document_is_titled_as_the_page_states_and_a_view_s_by_its_name() {
    let main = serde_json::json!({ "chunk": "static", "value": "<main><h1>Terms</h1></main>" });
    let titled = document(serde_json::json!([main, { "chunk": "dynamic", "value": {
        "part": "title", "id": 0,
        "pieces": [{ "piece": "text", "value": "Terms & conditions" }],
    }}]));
    assert!(
        titled.contains("<title>Terms &amp; conditions</title>"),
        "{titled}"
    );
    assert_eq!(
        titled.matches("<title>").count(),
        1,
        "in the head alone: {titled}"
    );
    let untitled = document(serde_json::json!([main]));
    assert!(untitled.contains("<title>P</title>"), "{untitled}");
}
