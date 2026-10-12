//! **A page's canonical address** (ADR-XXXX): written into the document's
//! head by its host, the route of the page it names with each `{name}`
//! filled as a link's hole is, absolute with the deployment's origin, and
//! Open Graph's `og:url` from it; nothing in the body.

use pw_render::*;

/// `<main>…</main>`, then the address `/word/{word}/{form}`: the entry's
/// word, and a form written as text.
fn page() -> Template {
    Template {
        path: "t.P".into(),
        name: "P".into(),
        params: vec![],
        schema: "s".into(),
        chunks: vec![
            Chunk::Static("<main><h1>Word</h1></main>".into()),
            Chunk::Dynamic(Part::Canonical {
                id: PartId(0),
                route: "/word/{word}/{form}".into(),
                args: vec![
                    ("word".into(), TitlePiece::Value("entry.word".into())),
                    ("form".into(), TitlePiece::Text("plain form".into())),
                ],
            }),
        ],
    }
}

fn entry(word: &str) -> Env {
    Env::new().set(
        "entry",
        Value::Record([("word".to_string(), Value::Text(word.into()))].into()),
    )
}

const ORIGIN: Option<&str> = Some("https://kiokun.example");

#[test]
fn the_address_is_the_head_s_absolute_and_nothing_in_the_body() {
    // 食べる is E9 A3 9F E3 81 B9 E3 82 8B in UTF-8, each byte escaped as a
    // link's hole is; a space too.
    let address = "https://kiokun.example/word/%E9%A3%9F%E3%81%B9%E3%82%8B/plain%20form";
    assert_eq!(
        head_canonical(&page(), &entry("食べる"), ORIGIN).expect("rendered"),
        format!(
            "<link rel=\"canonical\" href=\"{address}\">\n\
             <meta property=\"og:url\" content=\"{address}\">\n"
        )
    );
    // A trailing `/` on the origin is not doubled.
    assert!(
        head_canonical(&page(), &entry("a"), Some("https://kiokun.example/"))
            .expect("rendered")
            .contains("href=\"https://kiokun.example/word/a/plain%20form\"")
    );
    assert_eq!(
        render(&page(), &entry("a"), &[]).expect("rendered"),
        "<main><h1>Word</h1></main>"
    );
    let kinds: Vec<(String, Anchor)> = page()
        .manifest()
        .into_iter()
        .map(|e| (e.kind, e.anchor))
        .collect();
    assert_eq!(kinds, [("canonical".to_string(), Anchor::Document)]);
    // A page that states none has none, origin or not.
    let none = Template {
        chunks: vec![Chunk::Static("<main></main>".into())],
        ..page()
    };
    assert_eq!(
        head_canonical(&none, &Env::new(), None).expect("rendered"),
        ""
    );
}

#[test]
fn a_value_is_escaped_whole_and_never_splits_the_path() {
    // A `/`, a `?` and a `#` in a value stay in its segment, and a quote
    // cannot leave the attribute.
    let written = head_canonical(&page(), &entry("a/b?c#d\"e"), ORIGIN).expect("rendered");
    assert!(
        written.contains("href=\"https://kiokun.example/word/a%2Fb%3Fc%23d%22e/plain%20form\""),
        "{written}"
    );
}

#[test]
fn an_address_is_refused_without_an_origin_or_a_segment() {
    // The host's origin, never a request's: a host with none cannot write
    // one.
    assert!(matches!(
        head_canonical(&page(), &entry("a"), None),
        Err(Blocked::UnrepresentedConstruct { reason, .. }) if reason.contains("public origin")
    ));
    // A value no segment carries, as `navigate`'s address refuses one.
    for word in ["", ".", ".."] {
        assert!(
            matches!(
                head_canonical(&page(), &entry(word), ORIGIN),
                Err(Blocked::UnrepresentedConstruct { reason, .. })
                    if reason.contains("no segment of an address")
            ),
            "{word:?}"
        );
    }
    // A value the page is not given, and one that is HTML.
    assert!(matches!(
        head_canonical(&page(), &Env::new(), ORIGIN),
        Err(Blocked::MissingValue { path }) if path == "entry.word"
    ));
    let raw = Env::new().set(
        "entry",
        Value::Record(
            [(
                "word".to_string(),
                Value::Raw {
                    html: "<b>a</b>".into(),
                    capability: "Html".into(),
                },
            )]
            .into(),
        ),
    );
    assert!(matches!(
        head_canonical(&page(), &raw, ORIGIN),
        Err(Blocked::UnrepresentedConstruct { reason, .. }) if reason.contains("is HTML")
    ));
}
