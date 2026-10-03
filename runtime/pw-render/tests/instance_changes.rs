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
