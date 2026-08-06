//! E7 task 2 gates 2, 5 and 6 — dynamic values, determinism, and refusal.
//!
//! Each property is paired with a mutation that must break it, per the standing
//! discipline:
//!
//! > Every favorable observation needs a nearby implementation or mutation that
//! > makes the exact measurement go red.

use pw_render::*;
use std::collections::BTreeMap;

/// The document with its part anchors removed.
///
/// Used where the assertion is about CONTENT. The anchors have their own
/// tests: mixing the two would make every content expectation change whenever
/// an identity did, and then nobody would read them.
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

fn record(fields: &[(&str, Value)]) -> Value {
    let mut m = BTreeMap::new();
    for (k, v) in fields {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Record(m)
}

fn t(chunks: Vec<Chunk>) -> Template {
    Template {
        path: "t.T".into(),
        name: "T".into(),
        params: vec![],
        schema: "test".into(),
        chunks,
    }
}

// --- gate 2: dynamic values render correctly -----------------------------

#[test]
fn a_text_expression_renders_its_value() {
    let tpl = t(vec![
        Chunk::Static("<h1>".into()),
        Chunk::Dynamic(Part::Text {
            id: PartId(0),
            value: "name".into(),
            context: Context::Text,
        }),
        Chunk::Static("</h1>".into()),
    ]);
    let env = Env::new().set("name", Value::Text("Blue Bottle".into()));
    assert_eq!(
        visible(&render(&tpl, &env, &[]).unwrap()),
        "<h1>Blue Bottle</h1>"
    );
}

#[test]
fn an_attribute_renders_as_a_quoted_value() {
    let tpl = t(vec![
        Chunk::Static("<div ".into()),
        Chunk::Dynamic(Part::Attribute {
            id: PartId(0),
            owner: ElementId(0),
            name: "class".into(),
            value: "kind".into(),
            context: Context::Attribute,
        }),
        Chunk::Static("></div>".into()),
    ]);
    let env = Env::new().set("kind", Value::Text("card wide".into()));
    assert_eq!(
        visible(&render(&tpl, &env, &[]).unwrap()),
        "<div class=\"card wide\"></div>"
    );
}

#[test]
fn a_false_boolean_attribute_is_absent_not_empty() {
    // `disabled=""` is disabled. A renderer that emitted the attribute with an
    // empty value would produce a control nobody can use, and the markup would
    // look almost right.
    let tpl = t(vec![
        Chunk::Static("<input ".into()),
        Chunk::Dynamic(Part::BooleanAttribute {
            id: PartId(0),
            owner: ElementId(0),
            name: "disabled".into(),
            value: "locked".into(),
        }),
        Chunk::Static(">".into()),
    ]);
    let off = Env::new().set("locked", Value::Bool(false));
    assert_eq!(visible(&render(&tpl, &off, &[]).unwrap()), "<input>");

    let on = Env::new().set("locked", Value::Bool(true));
    assert_eq!(
        visible(&render(&tpl, &on, &[]).unwrap()),
        "<input disabled>"
    );
}

#[test]
fn a_conditional_region_renders_one_branch() {
    let tpl = t(vec![Chunk::Dynamic(Part::Conditional {
        id: PartId(0),
        value: "signed_in".into(),
        then: vec![Chunk::Static("<p>welcome</p>".into())],
        otherwise: vec![Chunk::Static("<p>sign in</p>".into())],
    })]);
    let yes = Env::new().set("signed_in", Value::Bool(true));
    let no = Env::new().set("signed_in", Value::Bool(false));
    assert_eq!(visible(&render(&tpl, &yes, &[]).unwrap()), "<p>welcome</p>");
    assert_eq!(visible(&render(&tpl, &no, &[]).unwrap()), "<p>sign in</p>");
}

#[test]
fn a_missing_else_renders_nothing_rather_than_failing() {
    let tpl = t(vec![Chunk::Dynamic(Part::Conditional {
        id: PartId(0),
        value: "show".into(),
        then: vec![Chunk::Static("<p>x</p>".into())],
        otherwise: vec![],
    })]);
    let no = Env::new().set("show", Value::Bool(false));
    assert_eq!(visible(&render(&tpl, &no, &[]).unwrap()), "");
}

#[test]
fn an_each_region_renders_once_per_element_with_the_binding_in_scope() {
    let tpl = t(vec![
        Chunk::Static("<ul>".into()),
        Chunk::Dynamic(Part::Each {
            id: PartId(0),
            collection: "items".into(),
            binding: "item".into(),
            key: Some("id".into()),
            body: vec![
                Chunk::Static("<li>".into()),
                Chunk::Dynamic(Part::Text {
                    id: PartId(0),
                    value: "item.name".into(),
                    context: Context::Text,
                }),
                Chunk::Static("</li>".into()),
            ],
        }),
        Chunk::Static("</ul>".into()),
    ]);
    let env = Env::new().set(
        "items",
        Value::List(vec![
            record(&[
                ("id", Value::Int(1)),
                ("name", Value::Text("Espresso".into())),
            ]),
            record(&[
                ("id", Value::Int(2)),
                ("name", Value::Text("Cortado".into())),
            ]),
        ]),
    );
    assert_eq!(
        visible(&render(&tpl, &env, &[]).unwrap()),
        "<ul><li>Espresso</li><li>Cortado</li></ul>"
    );
}

#[test]
fn an_empty_collection_renders_the_surrounding_markup_and_no_items() {
    let tpl = t(vec![
        Chunk::Static("<ul>".into()),
        Chunk::Dynamic(Part::Each {
            id: PartId(0),
            collection: "items".into(),
            binding: "item".into(),
            key: None,
            body: vec![Chunk::Static("<li>x</li>".into())],
        }),
        Chunk::Static("</ul>".into()),
    ]);
    let env = Env::new().set("items", Value::List(vec![]));
    assert_eq!(visible(&render(&tpl, &env, &[]).unwrap()), "<ul></ul>");
}

#[test]
fn a_nested_template_renders_in_place_with_its_own_arguments() {
    let child = Template {
        path: "t.Card".into(),
        name: "Card".into(),
        params: vec!["label".into()],
        schema: "test".into(),
        chunks: vec![
            Chunk::Static("<span>".into()),
            Chunk::Dynamic(Part::Text {
                id: PartId(0),
                value: "label".into(),
                context: Context::Text,
            }),
            Chunk::Static("</span>".into()),
        ],
    };
    let parent = t(vec![
        Chunk::Static("<div>".into()),
        Chunk::Dynamic(Part::Component {
            id: PartId(0),
            path: "t.Card".into(),
            args: vec![("label".into(), "title".into())],
        }),
        Chunk::Static("</div>".into()),
    ]);
    let env = Env::new().set("title", Value::Text("Menu".into()));
    assert_eq!(
        visible(&render(&parent, &env, std::slice::from_ref(&child)).unwrap()),
        "<div><span>Menu</span></div>"
    );

    // A child does NOT see the parent's other values. Scope is by argument, so
    // a template cannot depend on what happened to be in scope where it was
    // used — which is what makes it reusable and what makes its inputs
    // checkable.
    let leaky = Template {
        chunks: vec![Chunk::Dynamic(Part::Text {
            id: PartId(0),
            value: "title".into(),
            context: Context::Text,
        })],
        ..child
    };
    assert_eq!(
        render(&parent, &env, &[leaky]),
        Err(Blocked::MissingValue {
            path: "title".into()
        })
    );
}

// --- gate 5: deterministic output ----------------------------------------

#[test]
fn identical_ir_and_values_produce_identical_bytes() {
    let tpl = t(vec![
        Chunk::Static("<article ".into()),
        Chunk::Dynamic(Part::Attribute {
            id: PartId(0),
            owner: ElementId(0),
            name: "id".into(),
            value: "id".into(),
            context: Context::Attribute,
        }),
        Chunk::Static(" ".into()),
        Chunk::Dynamic(Part::Attribute {
            id: PartId(0),
            owner: ElementId(0),
            name: "class".into(),
            value: "kind".into(),
            context: Context::Attribute,
        }),
        Chunk::Static(">".into()),
        Chunk::Dynamic(Part::Each {
            id: PartId(0),
            collection: "items".into(),
            binding: "item".into(),
            key: Some("id".into()),
            body: vec![Chunk::Dynamic(Part::Text {
                id: PartId(0),
                value: "item.name".into(),
                context: Context::Text,
            })],
        }),
        Chunk::Static("</article>".into()),
    ]);
    let env = Env::new()
        .set("id", Value::Text("a".into()))
        .set("kind", Value::Text("card".into()))
        .set(
            "items",
            Value::List(vec![
                record(&[("name", Value::Text("x".into()))]),
                record(&[("name", Value::Text("y".into()))]),
            ]),
        );

    let first = render(&tpl, &env, &[]).unwrap();
    for _ in 0..64 {
        assert_eq!(render(&tpl, &env, &[]).unwrap(), first);
    }
    assert_eq!(
        visible(&first),
        "<article id=\"a\" class=\"card\">xy</article>"
    );
}

#[test]
fn the_determinism_check_can_fail() {
    // Architect ruling: "determinism test → deliberately randomize attribute
    // ordering → test must go red." Two IRs that differ only in attribute
    // ORDER must produce different bytes — which is what makes the assertion
    // above about the renderer rather than about the assertion.
    let attrs = |first: &str, second: &str| {
        t(vec![
            Chunk::Static("<div ".into()),
            Chunk::Dynamic(Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: first.into(),
                value: "v".into(),
                context: Context::Attribute,
            }),
            Chunk::Static(" ".into()),
            Chunk::Dynamic(Part::Attribute {
                id: PartId(0),
                owner: ElementId(0),
                name: second.into(),
                value: "v".into(),
                context: Context::Attribute,
            }),
            Chunk::Static("></div>".into()),
        ])
    };
    let env = Env::new().set("v", Value::Text("x".into()));
    let a = render(&attrs("id", "class"), &env, &[]).unwrap();
    let b = render(&attrs("class", "id"), &env, &[]).unwrap();
    assert_ne!(
        a, b,
        "reordered attributes produced identical bytes, so the determinism \
         assertion is not observing order at all"
    );
}

// --- gate 6: no invalid state silently disappears ------------------------

#[test]
fn a_blocked_part_refuses_the_render() {
    let tpl = t(vec![
        Chunk::Static("<main>".into()),
        Chunk::Dynamic(Part::Blocked {
            reason: "the block directive `{#await x}` has no representation".into(),
            at: "{#await x}".into(),
        }),
        Chunk::Static("</main>".into()),
    ]);
    let r = render(&tpl, &Env::new(), &[]);
    assert!(
        matches!(r, Err(Blocked::UnrepresentedConstruct { .. })),
        "got {r:?}"
    );
}

#[test]
fn the_blocked_check_can_fail() {
    // Architect ruling: "blocked-state test → deliberately turn Blocked into
    // empty output → test must go red."
    //
    // The mutation is a renderer that skips what it does not understand, which
    // is the natural implementation and the one this design exists to reject.
    fn lenient(chunks: &[Chunk]) -> String {
        chunks
            .iter()
            .map(|c| match c {
                Chunk::Static(s) => s.clone(),
                Chunk::Dynamic(_) => String::new(),
            })
            .collect()
    }
    let tpl = t(vec![
        Chunk::Static("<main>".into()),
        Chunk::Dynamic(Part::Blocked {
            reason: "unrepresented".into(),
            at: "x".into(),
        }),
        Chunk::Static("</main>".into()),
    ]);
    assert_eq!(
        lenient(&tpl.chunks),
        "<main></main>",
        "the lenient renderer produces a page that LOOKS correct"
    );
    assert!(
        render(&tpl, &Env::new(), &[]).is_err(),
        "and the real one must refuse it"
    );
}

#[test]
fn a_missing_value_is_an_error_not_an_empty_string() {
    let tpl = t(vec![Chunk::Dynamic(Part::Text {
        id: PartId(0),
        value: "absent".into(),
        context: Context::Text,
    })]);
    assert_eq!(
        render(&tpl, &Env::new(), &[]),
        Err(Blocked::MissingValue {
            path: "absent".into()
        })
    );
}

#[test]
fn a_value_with_no_text_form_is_refused_rather_than_stringified() {
    // A list in text position has no rendering. `[object Object]` is what a
    // renderer produces when it decides to have an answer for everything.
    let tpl = t(vec![Chunk::Dynamic(Part::Text {
        id: PartId(0),
        value: "items".into(),
        context: Context::Text,
    })]);
    let env = Env::new().set("items", Value::List(vec![Value::Int(1)]));
    assert!(render(&tpl, &env, &[]).is_err());
}

#[test]
fn an_unknown_component_is_refused_rather_than_skipped() {
    let tpl = t(vec![Chunk::Dynamic(Part::Component {
        id: PartId(0),
        path: "t.Missing".into(),
        args: vec![],
    })]);
    assert_eq!(
        render(&tpl, &Env::new(), &[]),
        Err(Blocked::UnknownComponent {
            path: "t.Missing".into()
        })
    );
}

// --- E7-R: part identity in the document ---------------------------------

#[test]
fn a_range_part_is_delimited_by_its_own_anchors() {
    // The encoding a range needs and an attribute cannot give it: a part that
    // renders NOTHING still has a place to reappear.
    let tpl = t(vec![
        Chunk::Static("<ul>".into()),
        Chunk::Dynamic(Part::Each {
            id: PartId(4),
            collection: "items".into(),
            binding: "item".into(),
            key: None,
            body: vec![Chunk::Static("<li>x</li>".into())],
        }),
        Chunk::Static("</ul>".into()),
    ]);
    let empty = Env::new().set("items", Value::List(vec![]));
    assert_eq!(
        render(&tpl, &empty, &[]).unwrap(),
        "<ul><!--pw:s4--><!--pw:e4--></ul>",
        "an empty range keeps its boundaries, or the runtime has nowhere to \
         insert the first item"
    );
}

#[test]
fn a_static_region_carries_no_identity_markup() {
    // The invariant that makes "without making static HTML noisy" mean
    // something: only dynamic and resumable regions pay identity overhead.
    let tpl = t(vec![Chunk::Static(
        "<h1>Store</h1><p>Open until 10</p>".into(),
    )]);
    let out = render(&tpl, &Env::new(), &[]).unwrap();
    assert_eq!(out, "<h1>Store</h1><p>Open until 10</p>");
    assert!(!out.contains("pw:"), "{out}");
    assert!(!out.contains("data-pw"), "{out}");
}

#[test]
fn an_element_local_part_anchors_on_the_element_not_on_a_comment() {
    // Three dynamic things on one element cost ONE identity, not three comment
    // pairs. `data-pw` is written by the IR's static chunk, so the renderer
    // emits nothing extra for the parts themselves.
    let tpl = t(vec![
        Chunk::Static("<button data-pw=\"7\" ".into()),
        Chunk::Dynamic(Part::Attribute {
            id: PartId(1),
            owner: ElementId(7),
            name: "title".into(),
            value: "label".into(),
            context: Context::Attribute,
        }),
        Chunk::Static(" ".into()),
        Chunk::Dynamic(Part::BooleanAttribute {
            id: PartId(2),
            owner: ElementId(7),
            name: "disabled".into(),
            value: "locked".into(),
        }),
        Chunk::Static(">Add</button>".into()),
    ]);
    let env = Env::new()
        .set("label", Value::Text("Add to cart".into()))
        .set("locked", Value::Bool(true));
    let out = render(&tpl, &env, &[]).unwrap();
    assert_eq!(
        out,
        "<button data-pw=\"7\" title=\"Add to cart\" disabled>Add</button>"
    );
    assert!(
        !out.contains("pw:s"),
        "no comment anchors for element parts: {out}"
    );
}

#[test]
fn an_event_part_emits_nothing_because_behaviour_is_not_markup() {
    // The element already carries `data-pw`, which is what the runtime needs to
    // find it. Emitting anything more would be inventing a second encoding of
    // the same fact.
    let tpl = t(vec![
        Chunk::Static("<button data-pw=\"3\">".into()),
        Chunk::Dynamic(Part::Event {
            id: PartId(9),
            owner: ElementId(3),
            event: "press".into(),
            handler: "add_to_cart".into(),
        }),
        Chunk::Static("Add</button>".into()),
    ]);
    assert_eq!(
        render(&tpl, &Env::new(), &[]).unwrap(),
        "<button data-pw=\"3\">Add</button>"
    );
}
