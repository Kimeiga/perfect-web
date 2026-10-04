//! **A page's metadata** (ADR-0186): written into the document's head by its
//! host, from the page's values, escaped for an attribute; nothing in the
//! body.

use pw_render::*;

/// `<main>…</main>`, then a description from the store's name and an Open
/// Graph title.
fn page() -> Template {
    Template {
        path: "t.P".into(),
        name: "P".into(),
        params: vec![],
        schema: "s".into(),
        chunks: vec![
            Chunk::Static("<main><h1>Store</h1></main>".into()),
            Chunk::Dynamic(Part::Meta {
                id: PartId(0),
                attribute: "name".into(),
                key: "description".into(),
                content: vec![
                    TitlePiece::Value("store.name".into()),
                    TitlePiece::Text(", and \"its\" menu".into()),
                ],
            }),
            Chunk::Dynamic(Part::Meta {
                id: PartId(1),
                attribute: "property".into(),
                key: "og:title".into(),
                content: vec![TitlePiece::Value("store.name".into())],
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
fn metadata_is_the_head_s_escaped_and_nothing_in_the_body() {
    let env = store("Tom & Jerry's");
    assert_eq!(
        head_metadata(&page(), &env).expect("rendered"),
        "<meta name=\"description\" content=\"Tom &amp; Jerry&#39;s, and &quot;its&quot; menu\">\n\
         <meta property=\"og:title\" content=\"Tom &amp; Jerry&#39;s\">\n"
    );
    assert_eq!(
        render(&page(), &env, &[]).expect("rendered"),
        "<main><h1>Store</h1></main>"
    );
    let kinds: Vec<(String, Anchor)> = page()
        .manifest()
        .into_iter()
        .map(|e| (e.kind, e.anchor))
        .collect();
    assert_eq!(
        kinds,
        [
            ("meta".to_string(), Anchor::Document),
            ("meta".to_string(), Anchor::Document)
        ]
    );
}

#[test]
fn metadata_reads_what_the_page_is_given_or_is_refused() {
    assert!(matches!(
        head_metadata(&page(), &Env::new()),
        Err(Blocked::MissingValue { path }) if path == "store.name"
    ));
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
        head_metadata(&page(), &raw),
        Err(Blocked::UnrepresentedConstruct { .. })
    ));
    // A template that states none writes none.
    let mut bare = page();
    bare.chunks.truncate(1);
    assert_eq!(
        head_metadata(&bare, &store("Blue Bottle")).expect("rendered"),
        ""
    );
}

#[test]
fn the_binary_writes_a_page_s_metadata_into_its_head() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let temp = tempfile::TempDir::with_prefix("pw-metadata-").expect("a temporary directory");
    let out = temp.path().join("out");
    let template = serde_json::json!([{
        "path": "t.P", "name": "P", "schema": "s", "params": [],
        "chunks": [
            { "chunk": "static", "value": "<main><h1>Terms</h1></main>" },
            { "chunk": "dynamic", "value": {
                "part": "meta", "id": 0, "attribute": "name", "key": "description",
                "content": [{ "piece": "text", "value": "Terms & conditions" }],
            }},
        ],
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
    let page = std::fs::read_to_string(out.join("P.html")).expect("written");
    let head = page.split("</head>").next().expect("a head");
    assert!(
        head.contains("<meta name=\"description\" content=\"Terms &amp; conditions\">"),
        "{page}"
    );
    assert!(
        !page
            .split("</head>")
            .nth(1)
            .expect("a body")
            .contains("<meta"),
        "{page}"
    );
}
