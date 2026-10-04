//! **What changed in one instance of a keyed loop, part by part** (ADR-0168).
//!
//! A host sets what a change reaches where it is: a text part's text, and an
//! attribute's value, so the instance keeps its nodes. Until 2026-10-03 only
//! text parts were set: an attribute that read a changed field kept its old
//! value, and the store's Add button kept the name of the item it was before.

use pw_render::*;

/// `{#each items as item (item.id)}<li title="{item.name}" data-x={item.note}
/// hidden={item.gone}><span>{item.name}</span>{#if item.new}<b>new</b>{/if}</li>{/each}`
fn page() -> Template {
    let st = |s: &str| Chunk::Static(s.to_string());
    Template {
        path: "t.T".into(),
        name: "T".into(),
        params: vec![],
        schema: "s".into(),
        chunks: vec![Chunk::Dynamic(Part::Each {
            id: PartId(0),
            collection: "items".into(),
            binding: "item".into(),
            key: Some("id".into()),
            body: vec![
                st("<li data-pw=\"0\" "),
                Chunk::Dynamic(Part::InterpolatedAttribute {
                    id: PartId(1),
                    owner: ElementId(0),
                    name: "aria-label".into(),
                    segments: vec![
                        Segment::Static("Add ".into()),
                        Segment::Value("item.name".into()),
                    ],
                    context: Context::Attribute,
                }),
                st(" "),
                Chunk::Dynamic(Part::Attribute {
                    id: PartId(2),
                    owner: ElementId(0),
                    name: "title".into(),
                    value: "item.note".into(),
                    context: Context::Attribute,
                }),
                st(" "),
                Chunk::Dynamic(Part::BooleanAttribute {
                    id: PartId(3),
                    owner: ElementId(0),
                    name: "hidden".into(),
                    value: "item.gone".into(),
                }),
                st("><span>"),
                Chunk::Dynamic(Part::Text {
                    id: PartId(4),
                    value: "item.name".into(),
                    context: Context::Text,
                }),
                st("</span>"),
                Chunk::Dynamic(Part::Conditional {
                    id: PartId(5),
                    value: "item.new".into(),
                    then: vec![st("<b>new</b>")],
                    otherwise: vec![],
                }),
                st("</li>"),
            ],
        })],
    }
}

fn item(name: &str, note: &str, gone: bool, new: bool) -> Value {
    Value::Record(
        [
            ("id".to_string(), Value::Text("a".into())),
            ("name".to_string(), Value::Text(name.into())),
            ("note".to_string(), Value::Text(note.into())),
            ("gone".to_string(), Value::Bool(gone)),
            ("new".to_string(), Value::Bool(new)),
        ]
        .into(),
    )
}

fn changes(was: &Value, now: &Value) -> Option<Vec<(PartId, InstanceChange)>> {
    instance_changes(&page(), PartId(0), was, now, &Env::new(), &[]).expect("derived")
}

#[test]
fn nothing_changed_is_no_change() {
    let a = item("Tea", "hot", false, false);
    assert_eq!(changes(&a, &a), Some(vec![]));
}

#[test]
fn a_renamed_item_sets_its_text_and_every_attribute_that_reads_the_name() {
    assert_eq!(
        changes(
            &item("Tea", "hot", false, false),
            &item("Green Tea", "hot", false, false)
        ),
        Some(vec![
            (
                PartId(1),
                InstanceChange::Attribute {
                    name: "aria-label".into(),
                    value: Some("Add Green Tea".into()),
                }
            ),
            (PartId(4), InstanceChange::Text("Green Tea".into())),
        ])
    );
}

#[test]
fn an_attribute_is_set_as_the_document_writes_it_and_a_boolean_one_comes_and_goes() {
    assert_eq!(
        changes(
            &item("Tea", "hot", false, false),
            &item("Tea", "\"very\" hot", true, false)
        ),
        Some(vec![
            (
                PartId(2),
                InstanceChange::Attribute {
                    name: "title".into(),
                    value: Some("&quot;very&quot; hot".into()),
                }
            ),
            (
                PartId(3),
                InstanceChange::Attribute {
                    name: "hidden".into(),
                    value: Some(String::new()),
                }
            ),
        ])
    );
    assert_eq!(
        changes(
            &item("Tea", "hot", true, false),
            &item("Tea", "hot", false, false)
        ),
        Some(vec![(
            PartId(3),
            InstanceChange::Attribute {
                name: "hidden".into(),
                value: None,
            }
        )])
    );
}

#[test]
fn a_block_that_changed_renders_the_instance_again() {
    assert_eq!(
        changes(
            &item("Tea", "hot", false, false),
            &item("Tea", "hot", false, true)
        ),
        None
    );
}

/// `{#each items as item (item.id)}<li><button on:press={…captures item.name}
/// on:focus={…captures item.id}>{item.name}</button></li>{/each}`
fn buttons() -> Template {
    let st = |s: &str| Chunk::Static(s.to_string());
    let handler = |id: u32, event: &str, captures: &[&str]| {
        Chunk::Dynamic(Part::Event {
            id: PartId(id),
            owner: ElementId(1),
            event: event.into(),
            handler: format!("h{id}"),
            name: format!("on_{event}"),
            captures: captures.iter().map(|c| c.to_string()).collect(),
            renames: Default::default(),
            modifiers: vec![],
        })
    };
    Template {
        path: "t.B".into(),
        name: "B".into(),
        params: vec![],
        schema: "s".into(),
        chunks: vec![Chunk::Dynamic(Part::Each {
            id: PartId(0),
            collection: "items".into(),
            binding: "item".into(),
            key: Some("id".into()),
            body: vec![
                st("<li><button data-pw=\"1\""),
                handler(1, "press", &["item.name"]),
                handler(2, "focus", &["item.id"]),
                st(">"),
                Chunk::Dynamic(Part::Text {
                    id: PartId(3),
                    value: "item.name".into(),
                    context: Context::Text,
                }),
                st("</button></li>"),
            ],
        })],
    }
}

/// **What a handler captures is set where it is** (ADR-0172): the attribute
/// the runtime reads when the handler runs, once for its element's handlers.
/// Until 2026-10-03 a change to it rendered the instance again, and a
/// renamed item's button lost its nodes, and focus.
#[test]
fn what_a_handler_captures_is_set_where_it_is() {
    let changes = |was: &Value, now: &Value| {
        instance_changes(&buttons(), PartId(0), was, now, &Env::new(), &[]).expect("derived")
    };
    assert_eq!(
        changes(
            &item("Tea", "hot", false, false),
            &item("Green Tea", "hot", false, false)
        ),
        Some(vec![
            (
                PartId(1),
                InstanceChange::Attribute {
                    name: "data-pw-captures".into(),
                    value: Some(
                        "{&quot;item&quot;:{&quot;id&quot;:&quot;a&quot;,\
                         &quot;name&quot;:&quot;Green Tea&quot;}}"
                            .into()
                    ),
                }
            ),
            (PartId(3), InstanceChange::Text("Green Tea".into())),
        ])
    );
    // What no handler captures changes nothing of the element's.
    assert_eq!(
        changes(
            &item("Tea", "hot", false, false),
            &item("Tea", "cold", false, false)
        ),
        Some(vec![])
    );
}
