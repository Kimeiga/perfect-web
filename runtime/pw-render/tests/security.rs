//! E7 task 2 gate 3 — the escaping security matrix.
//!
//! Architect ruling, 2026-08-06: *"This is probably the highest-risk part of
//! task 2."* The matrix it named, plus valid neighbours.
//!
//! # Every row has a mutation that makes it go red
//!
//! > Every favorable observation needs a nearby implementation or mutation that
//! > makes the exact measurement go red.
//!
//! A test asserting "the output contains no `<script>`" passes against a
//! renderer that outputs nothing. So each property is checked twice: once
//! against the real escaper, and once against a deliberately broken one that
//! must fail the same assertion. `the_matrix_can_fail` is that control, and it
//! is written so that a broken escaper cannot satisfy it by accident.

use pw_render::*;
use std::collections::BTreeMap;

/// One template with one hole, in a given context.
fn one_hole(context: Context) -> Template {
    let (before, part, after) = match context {
        Context::Text => (
            "<p>".to_string(),
            Part::Text {
                id: PartId(0),
                value: "v".into(),
                context,
            },
            "</p>".to_string(),
        ),
        Context::Attribute => (
            "<div ".to_string(),
            Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: "title".into(),
                value: "v".into(),
                context,
            },
            "></div>".to_string(),
        ),
        Context::Url => (
            "<a ".to_string(),
            Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: "href".into(),
                value: "v".into(),
                context,
            },
            ">x</a>".to_string(),
        ),
        Context::Style => (
            "<div ".to_string(),
            Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: "style".into(),
                value: "v".into(),
                context,
            },
            "></div>".to_string(),
        ),
        Context::RawHtml => (
            "<div>".to_string(),
            Part::RawHtml {
                id: PartId(0),
                value: "v".into(),
                capability: "unsafe.raw_html".into(),
            },
            "</div>".to_string(),
        ),
    };
    Template {
        path: "t.T".into(),
        name: "T".into(),
        params: vec!["v".into()],
        schema: "test".into(),
        chunks: vec![
            Chunk::Static(before),
            Chunk::Dynamic(part),
            Chunk::Static(after),
        ],
    }
}

/// The document with its part anchors removed.
///
/// The security matrix is about ESCAPING, and an anchor is neither escaped nor
/// escapable — it is bytes the renderer wrote. Leaving them in would make every
/// injection expectation change whenever an identity did.
fn visible(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find("<!--pw:") {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find("-->") else {
            break;
        };
        rest = &rest[i + j + 3..];
    }
    out.push_str(rest);
    out
}

fn render_hostile(context: Context, value: &str) -> String {
    let t = one_hole(context);
    let env = Env::new().set("v", Value::Text(value.to_string()));
    visible(&render(&t, &env, &[]).expect("renders"))
}

// --- the matrix ----------------------------------------------------------

#[test]
fn text_injection_becomes_inert_text() {
    let out = render_hostile(Context::Text, "<script>alert(1)</script>");
    assert!(!out.contains("<script"), "{out}");
    assert_eq!(out, "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>");
}

#[test]
fn an_attribute_breakout_stays_inside_one_attribute() {
    let out = render_hostile(Context::Attribute, "\" onclick=\"steal()");
    // Counted as QUOTE CHARACTERS, not as `=`. A first version counted `=` and
    // went red on correct output — `&quot; onclick=&quot;` is one attribute
    // value that happens to contain an equals sign, and the sign is not what
    // ends an attribute. The delimiters are.
    assert_eq!(
        out.matches('"').count(),
        2,
        "exactly two quote characters: the delimiters, and none from the \
         value — {out}"
    );
    assert!(!out.contains("onclick=\""), "{out}");
}

#[test]
fn an_unquoted_breakout_attempt_stays_inside_the_quotes() {
    // The renderer always emits double quotes, so the classic unquoted-value
    // attack — a space starting a new attribute — has nothing to break out of.
    // Asserted rather than assumed, because "we always quote" is a property of
    // the emitter that a change could remove.
    let out = render_hostile(Context::Attribute, "x onclick=alert(1)");
    assert!(out.starts_with("<div title=\""), "{out}");
    assert_eq!(
        out.matches('"').count(),
        2,
        "exactly one quoted value: {out}"
    );
}

#[test]
fn a_javascript_url_is_replaced_rather_than_escaped() {
    for hostile in [
        "javascript:alert(1)",
        "JAVASCRIPT:alert(1)",
        "java\nscript:alert(1)",
        "  javascript:alert(1)",
        "&#106;avascript:alert(1)",
        "&#x6a;avascript:alert(1)",
        "vbscript:msgbox(1)",
        "data:text/html;base64,PHNjcmlwdD4=",
    ] {
        let out = render_hostile(Context::Url, hostile);
        assert!(
            out.contains("about:blank"),
            "`{hostile}` must not survive as a scheme: {out}"
        );
    }
}

#[test]
fn an_ordinary_url_is_not_mangled() {
    // The neighbour. Without it, the rule above is satisfied by a renderer
    // that returns `about:blank` for every href.
    let out = render_hostile(Context::Url, "/store/47?locale=en&tenant=acme");
    assert_eq!(out, "<a href=\"/store/47?locale=en&amp;tenant=acme\">x</a>");
    assert!(!out.contains("about:blank"));
}

#[test]
fn ampersands_are_encoded_in_both_contexts() {
    assert!(render_hostile(Context::Text, "a & b").contains("a &amp; b"));
    assert!(render_hostile(Context::Attribute, "a & b").contains("a &amp; b"));
}

#[test]
fn quotes_and_angles_are_encoded_where_they_matter() {
    let text = render_hostile(Context::Text, "< > \" '");
    assert!(text.contains("&lt;") && text.contains("&gt;"), "{text}");
    let attr = render_hostile(Context::Attribute, "< > \" '");
    assert!(attr.contains("&quot;") && attr.contains("&#39;"), "{attr}");
}

#[test]
fn a_style_value_that_executes_is_dropped() {
    let out = render_hostile(Context::Style, "width: expression(alert(1))");
    assert!(!out.contains("expression"), "{out}");
    // And the neighbour, in the same test: an ordinary declaration survives.
    let ok = render_hostile(Context::Style, "color: red");
    assert!(ok.contains("color: red"), "{ok}");
}

#[test]
fn unicode_passes_through_unchanged() {
    // Escaping must not be a whitelist. A renderer that emitted only ASCII
    // would satisfy every injection test above and be unusable.
    for s in ["café", "日本語", "🇯🇵", "Ω≈ç√", "\u{200B}zero-width"] {
        let out = render_hostile(Context::Text, s);
        assert!(out.contains(s), "`{s}` was altered: {out}");
    }
}

#[test]
fn lone_surrogates_cannot_reach_the_renderer() {
    // Rust `String` is UTF-8 by construction, so a lone surrogate cannot be
    // built — the boundary where malformed input would arrive is the decoder,
    // not this crate. Asserted rather than assumed, because "it cannot happen"
    // is the kind of claim that stops being true when a byte-oriented input
    // path is added.
    assert!(String::from_utf8(vec![0xED, 0xA0, 0x80]).is_err());
    // And the escaper is total over everything that CAN be built.
    let awkward = "\u{FFFD}\u{0}\u{1F}<&>";
    let out = render_hostile(Context::Text, awkward);
    assert!(out.contains("&lt;") && out.contains("&amp;"), "{out}");
}

#[test]
fn raw_html_requires_the_capability_and_is_not_a_flag_on_a_string() {
    let t = one_hole(Context::RawHtml);

    // An ordinary string reaching a raw part without the grant is refused.
    let plain = Env::new().set("v", Value::Text("<b>bold</b>".into()));
    assert_eq!(
        render(&t, &plain, &[]),
        Err(Blocked::UnauthorisedRawHtml {
            capability: "unsafe.raw_html".into()
        })
    );

    // With the grant, and only with it, the bytes are emitted.
    let granted = plain.clone().grant("unsafe.raw_html");
    assert_eq!(
        visible(&render(&t, &granted, &[]).unwrap()),
        "<div><b>bold</b></div>"
    );

    // A `Raw` VALUE in ordinary text position still needs its capability: the
    // authorisation belongs to the value, not to the position it lands in.
    let text = Template {
        chunks: vec![Chunk::Dynamic(Part::Text {
            id: PartId(0),
            value: "v".into(),
            context: Context::Text,
        })],
        ..one_hole(Context::Text)
    };
    let raw = Env::new().set(
        "v",
        Value::Raw {
            html: "<b>x</b>".into(),
            capability: "unsafe.raw_html".into(),
        },
    );
    assert!(matches!(
        render(&text, &raw, &[]),
        Err(Blocked::UnauthorisedRawHtml { .. })
    ));
    assert_eq!(
        visible(&render(&text, &raw.grant("unsafe.raw_html"), &[]).unwrap()),
        "<b>x</b>"
    );
}

// --- the control ---------------------------------------------------------

/// A deliberately broken escaper, and the same assertions run against it.
///
/// Architect ruling: *"escaping test → deliberately disable attribute escaping
/// → test must go red."* Without this, every assertion above is satisfied by a
/// renderer that emits nothing at all.
///
/// The break is applied to the OUTPUT rather than by editing the crate, because
/// a test that required editing the crate to run would not run.
#[test]
fn the_matrix_can_fail() {
    fn unescaped(context: Context, value: &str) -> String {
        // What the renderer would produce with escaping removed.
        match context {
            Context::Text => format!("<p>{value}</p>"),
            Context::Attribute => format!("<div title=\"{value}\"></div>"),
            Context::Url => format!("<a href=\"{value}\">x</a>"),
            Context::Style => format!("<div style=\"{value}\"></div>"),
            Context::RawHtml => format!("<div>{value}</div>"),
        }
    }

    /// A property of rendered output, as a predicate the real renderer holds
    /// and an unescaped one must not.
    type Holds = fn(&str) -> bool;
    /// The row: what it is called, where the value sits, the hostile input.
    type Row = (&'static str, Context, &'static str, Holds);

    let rows: Vec<Row> = vec![
        (
            "text injection",
            Context::Text,
            "<script>alert(1)</script>",
            |o| !o.contains("<script"),
        ),
        (
            "attribute breakout",
            Context::Attribute,
            "\" onclick=\"steal()",
            |o| o.matches('"').count() == 2,
        ),
        ("javascript url", Context::Url, "javascript:alert(1)", |o| {
            o.contains("about:blank")
        }),
        (
            "style expression",
            Context::Style,
            "width: expression(alert(1))",
            |o| !o.contains("expression"),
        ),
    ];

    for (name, context, hostile, holds) in rows {
        assert!(
            holds(&render_hostile(context, hostile)),
            "{name}: the real renderer must hold the property"
        );
        assert!(
            !holds(&unescaped(context, hostile)),
            "{name}: the assertion passes against an UNESCAPED renderer, so it \
             is not measuring escaping"
        );
    }
}

#[test]
fn a_record_field_in_a_hostile_position_is_escaped_the_same_way() {
    // Values reached through a field, not only values set at the top level:
    // an escaping rule that applied to one and not the other would be a hole
    // exactly where loop bodies put their data.
    let t = one_hole(Context::Attribute);
    let mut fields = BTreeMap::new();
    fields.insert("title".to_string(), Value::Text("\" onclick=\"x".into()));
    let env = Env::new().set("item", Value::Record(fields));
    let t = Template {
        chunks: vec![
            Chunk::Static("<div ".into()),
            Chunk::Dynamic(Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: "title".into(),
                value: "item.title".into(),
                context: Context::Attribute,
            }),
            Chunk::Static("></div>".into()),
        ],
        ..t
    };
    let out = visible(&render(&t, &env, &[]).unwrap());
    assert!(!out.contains("onclick=\""), "{out}");
}
