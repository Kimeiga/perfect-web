//! **`{:else}`, `{#match}` and interpolated attributes, rendered** (ADR-0042).
//!
//! The IR decides what each part is. These tests hold the renderer to it:
//! exactly one branch of a conditional and one arm of a match render, an
//! arm's payload is bound to its name, and a value inside a URL is one URI
//! component whatever it holds.

use pw_render::*;

fn template(chunks: Vec<Chunk>) -> Template {
    Template {
        path: "t.T".into(),
        name: "T".into(),
        params: vec![],
        schema: "s".into(),
        chunks,
    }
}

fn st(s: &str) -> Chunk {
    Chunk::Static(s.to_string())
}

fn text(id: u32, value: &str) -> Chunk {
    Chunk::Dynamic(Part::Text {
        id: PartId(id),
        value: value.into(),
        context: Context::Text,
    })
}

fn some(v: Value) -> Value {
    Value::Variant {
        case: "Some".into(),
        payload: Some(Box::new(v)),
    }
}

fn none() -> Value {
    Value::Variant {
        case: "None".into(),
        payload: None,
    }
}

fn record(fields: &[(&str, &str)]) -> Value {
    Value::Record(
        fields
            .iter()
            .map(|(k, v)| (k.to_string(), Value::Text(v.to_string())))
            .collect(),
    )
}

/// `{#match found}{:Some(h)}<p>{h.word}</p>{:None}<p>-</p>{/match}`
fn found_page() -> Template {
    template(vec![Chunk::Dynamic(Part::Match {
        id: PartId(0),
        value: "found".into(),
        arms: vec![
            Arm {
                case: "Some".into(),
                binding: Some("h".into()),
                body: vec![st("<p>"), text(1, "h.word"), st("</p>")],
            },
            Arm {
                case: "None".into(),
                binding: None,
                body: vec![st("<p>-</p>")],
            },
        ],
    })])
}

#[test]
fn a_match_renders_the_arm_its_case_names_with_the_payload_bound() {
    let t = found_page();
    let hit = Env::new().set("found", some(record(&[("word", "人")])));
    assert_eq!(
        render(&t, &hit, &[]).unwrap(),
        "<!--pw:s0--><p><!--pw:s1-->人<!--pw:e1--></p><!--pw:e0-->"
    );
    let nothing = Env::new().set("found", none());
    assert_eq!(
        render(&t, &nothing, &[]).unwrap(),
        "<!--pw:s0--><p>-</p><!--pw:e0-->"
    );
}

#[test]
fn a_part_inside_an_arm_can_be_rendered_alone() {
    // `render_part` finds parts through `Part::nested`, as every walk does.
    let t = found_page();
    let env = Env::new().set("h", record(&[("word", "水")]));
    assert_eq!(
        render_part(&t, PartId(1), &env, &[]).unwrap(),
        "<!--pw:s1-->水<!--pw:e1-->"
    );
    let manifest: Vec<u32> = t.manifest().iter().map(|e| e.id.0).collect();
    assert_eq!(manifest, vec![0, 1]);
}

#[test]
fn a_match_refuses_what_it_cannot_take_apart() {
    let t = found_page();
    for (env, why) in [
        (
            Env::new().set("found", Value::Text("x".into())),
            "not a variant",
        ),
        (
            Env::new().set(
                "found",
                Value::Variant {
                    case: "Ok".into(),
                    payload: None,
                },
            ),
            "no arm",
        ),
        (
            Env::new().set(
                "found",
                Value::Variant {
                    case: "Some".into(),
                    payload: None,
                },
            ),
            "no payload",
        ),
        (Env::new(), "no value"),
    ] {
        assert!(render(&t, &env, &[]).is_err(), "{why}");
    }
}

#[test]
fn a_conditional_renders_exactly_one_branch_and_refuses_a_variant() {
    let t = template(vec![Chunk::Dynamic(Part::Conditional {
        id: PartId(0),
        value: "a".into(),
        then: vec![st("A")],
        otherwise: vec![st("B")],
    })]);
    assert_eq!(
        render(&t, &Env::new().set("a", Value::Bool(true)), &[]).unwrap(),
        "<!--pw:s0-->A<!--pw:e0-->"
    );
    assert_eq!(
        render(&t, &Env::new().set("a", Value::Bool(false)), &[]).unwrap(),
        "<!--pw:s0-->B<!--pw:e0-->"
    );
    // `Some(false)` is not a condition: `{#match}` takes it apart.
    let err = render(&t, &Env::new().set("a", some(Value::Bool(false))), &[]).unwrap_err();
    assert!(
        matches!(err, Blocked::UnrepresentedConstruct { .. }),
        "{err}"
    );
}

/// `<a href="/words/{w}" title="see {w}">`
fn link() -> Template {
    template(vec![
        st("<a data-pw=\"0\" "),
        Chunk::Dynamic(Part::InterpolatedAttribute {
            id: PartId(0),
            owner: ElementId(0),
            name: "href".into(),
            segments: vec![
                Segment::Static("/words/".into()),
                Segment::Value("w".into()),
            ],
            context: Context::Url,
        }),
        st(" "),
        Chunk::Dynamic(Part::InterpolatedAttribute {
            id: PartId(1),
            owner: ElementId(0),
            name: "title".into(),
            segments: vec![Segment::Static("see ".into()), Segment::Value("w".into())],
            context: Context::Attribute,
        }),
        st(">x</a>"),
    ])
}

#[test]
fn a_value_in_a_url_is_one_component_whatever_it_holds() {
    let html = render(
        &link(),
        &Env::new().set("w", Value::Text("길/에 ?x#y%\"<人&".into())),
        &[],
    )
    .unwrap();
    assert_eq!(
        html,
        "<a data-pw=\"0\" href=\"/words/%EA%B8%B8%2F%EC%97%90%20%3Fx%23y%25%22%3C%E4%BA%BA%26\" \
         title=\"see 길/에 ?x#y%&quot;&lt;人&amp;\">x</a>"
    );
}

#[test]
fn a_value_cannot_give_a_url_a_scheme_or_another_segment() {
    // The control: what a raw interpolation would have produced. Each would
    // change what the link names; none survives the encoding.
    for hostile in [
        "javascript:alert(1)",
        "../../admin",
        "x?admin=1",
        "x#top",
        "//evil.example",
    ] {
        let html = render(
            &link(),
            &Env::new().set("w", Value::Text(hostile.into())),
            &[],
        )
        .unwrap();
        let href = html
            .split("href=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let tail = href.strip_prefix("/words/").expect("the author's prefix");
        assert!(
            tail.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)),
            "{hostile} -> {href}"
        );
        assert!(
            !format!("/words/{hostile}").eq(href),
            "{hostile} was not encoded"
        );
    }
}

#[test]
fn an_interpolated_attribute_refuses_a_missing_value() {
    let err = render(&link(), &Env::new(), &[]).unwrap_err();
    assert_eq!(err, Blocked::MissingValue { path: "w".into() });
}

#[test]
fn a_variant_has_no_text_form() {
    let t = template(vec![text(0, "v")]);
    assert!(render(&t, &Env::new().set("v", some(Value::Text("x".into()))), &[]).is_err());
}
