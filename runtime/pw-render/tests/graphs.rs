//! **A value of a type that contains itself, as its nodes** (ADR-0205).
//!
//! On the browser's wire such a value is `{ "$graph": [node, ...] }`: node 0
//! the value, each value of the type inside a node `{ "$node": k }` for a
//! later node. The renderer builds it from its last node to its first, and
//! holds it, clones it, compares it and drops it with stacks on the heap, so
//! a value is as deep as its data and the JSON as shallow as its type.

use pw_render::Value;
use serde_json::json;

#[path = "graphs/stack_probe.rs"]
mod stack_probe;

fn wire_value(json: &serde_json::Value) -> Value {
    let value = Value::from_wire(json);
    assert!(value.is_ok(), "the valid wire value is accepted: {value:?}");
    value.unwrap()
}

fn small_value_contract() {
    // Establish bounded semantic neighbors before allocating deep stress data.
    let graph = json!({ "$graph": [{ "n": 1, "next": { "$node": 1 } }, { "n": 2 }] });
    let parsed = Value::from_wire(&graph);
    assert!(
        parsed.is_ok(),
        "the small valid tree is accepted: {parsed:?}"
    );
    assert_ne!(Value::Int(1), Value::Int(2));
    let parsed = parsed.unwrap();
    assert!(matches!(&parsed, Value::Record(fields)
        if matches!(fields.get("n"), Some(Value::Int(1)))
        && matches!(fields.get("next"), Some(Value::Record(next))
            if matches!(next.get("n"), Some(Value::Int(2))))));
}

/// A comment, nested, as JSON writes a value of no such type.
fn comment(id: i64, replies: Vec<serde_json::Value>) -> serde_json::Value {
    json!({ "id": id, "replies": replies })
}

#[test]
fn a_graph_is_the_value_its_nodes_make() {
    // 1 holds 2 and 3; 3 holds 4. Level order, each node's own run.
    let graph = json!({ "$graph": [
        { "id": 1, "replies": [{ "$node": 1 }, { "$node": 2 }] },
        { "id": 2, "replies": [] },
        { "id": 3, "replies": [{ "$node": 3 }] },
        { "id": 4, "replies": [] },
    ]});
    let nested = comment(
        1,
        vec![comment(2, vec![]), comment(3, vec![comment(4, vec![])])],
    );
    assert_eq!(wire_value(&graph), wire_value(&nested));
    // A node held where a case holds it, `Some(next)`.
    let chain = json!({ "$graph": [
        { "n": 1, "next": { "$case": "some", "value": { "$node": 1 } } },
        { "n": 2, "next": { "$case": "none" } },
    ]});
    let nested = json!({ "n": 1, "next": { "$case": "some", "value":
        { "n": 2, "next": { "$case": "none" } } } });
    assert_eq!(wire_value(&chain), wire_value(&nested));
    // Another type's graph inside a node is its own: its `$node`s are its.
    let inner = json!({ "$graph": [
        { "id": 1, "replies": [{ "$node": 1 }] },
        { "id": 2, "replies": [] },
    ]});
    let outer = json!({ "$graph": [
        { "tag": "a", "thread": inner, "kids": [{ "$node": 1 }] },
        { "tag": "b", "thread": inner, "kids": [] },
    ]});
    let thread = comment(1, vec![comment(2, vec![])]);
    let nested = json!({ "tag": "a", "thread": thread, "kids": [
        { "tag": "b", "thread": thread, "kids": [] }
    ]});
    assert_eq!(wire_value(&outer), wire_value(&nested));
}

#[test]
fn a_graph_that_is_not_a_tree_is_refused() {
    let refused = |j: serde_json::Value| {
        let result = Value::from_wire(&j);
        assert!(result.is_err(), "the malformed tree is refused");
        result.unwrap_err()
    };
    // A node held by one before it, or by itself: a cycle.
    assert!(
        refused(json!({ "$graph": [{ "next": [] }, { "back": { "$node": 0 } }] }))
            .contains("not a later node")
    );
    assert!(refused(json!({ "$graph": [{ "me": { "$node": 0 } }] })).contains("not a later node"));
    // Held twice: two parents.
    assert!(
        refused(json!({ "$graph": [
            { "a": { "$node": 1 }, "b": { "$node": 1 } },
            { "x": 1 },
        ]}))
        .contains("held twice")
    );
    // Held by none.
    assert!(refused(json!({ "$graph": [{ "x": 1 }, { "x": 2 }] })).contains("no other holds"));
    // Past the graph, no graph, no node, and no index.
    assert!(refused(json!({ "$graph": [{ "a": { "$node": 5 } }] })).contains("not a later node"));
    assert!(refused(json!({ "a": { "$node": 1 } })).contains("outside any `$graph`"));
    assert!(refused(json!({ "$graph": [] })).contains("no node"));
    assert!(refused(json!({ "$graph": [{ "a": { "$node": "1" } }] })).contains("not an index"));
    assert!(refused(json!({ "$graph": 3 })).contains("not a list of nodes"));
}

#[test]
fn a_value_as_deep_as_its_data_is_held_without_recursion() {
    small_value_contract();
    const DEEP: usize = 100_000;
    let observation = stack_probe::observe(
        "a_value_as_deep_as_its_data_is_held_without_recursion",
        3,
        || {
            let nodes: Vec<serde_json::Value> = (0..DEEP)
                .map(|k| {
                    if k + 1 < DEEP {
                        json!({ "n": k, "next": [{ "$node": k + 1 }] })
                    } else {
                        json!({ "n": k, "next": [] })
                    }
                })
                .collect();
            let graph = json!({ "$graph": nodes });
            let value = Value::from_wire(&graph).expect("the deep valid tree");
            let copy = value.clone();
            let equal = value == copy;
            let mut at = &copy;
            let mut depth = 0;
            while let Value::Record(fields) = at {
                match fields.get("next") {
                    Some(Value::List(next)) if !next.is_empty() => {
                        at = &next[0];
                        depth += 1;
                    }
                    _ => break,
                }
            }
            let mut other = value.clone();
            let mut cursor = &mut other;
            loop {
                let Value::Record(fields) = cursor else {
                    panic!("invalid deep record")
                };
                if !matches!(fields.get("next"), Some(Value::List(next)) if !next.is_empty()) {
                    fields.insert("n".into(), Value::Int(-1));
                    break;
                }
                let Some(Value::List(next)) = fields.get_mut("next") else {
                    panic!("invalid deep next")
                };
                cursor = &mut next[0];
            }
            let different = value != other;
            (
                vec![depth, usize::from(equal), usize::from(different)],
                vec![value, copy, other],
            )
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let Some(observation) = observation else {
        return;
    };
    assert!(
        !observation.overflow,
        "the exact deep drop fits its 256KiB stack: {}",
        observation.output
    );
    assert_eq!(
        observation.values,
        vec![DEEP - 1, 1, 1],
        "{}",
        observation.output
    );
}

#[test]
fn a_copy_is_the_value_and_its_own() {
    // Branching, so a copy whose children came back in another order, or a
    // comparison that read one side's alone, is found.
    let tree = json!({ "id": 1, "tags": ["x", "y"], "replies": [
        { "id": 2, "tags": [], "replies": [{ "id": 4, "tags": ["z"], "replies": [] }] },
        { "id": 3, "tags": ["w"], "replies": [] },
    ], "state": { "$case": "some", "value": { "seen": true } } });
    let value = wire_value(&tree);
    let copy = value.clone();
    assert_eq!(copy, value);
    assert_eq!(format!("{copy:?}"), format!("{value:?}"));
    // Its own: a change to the copy is not the value's.
    let mut changed = copy.clone();
    let Value::Record(fields) = &mut changed else {
        unreachable!()
    };
    fields.insert("id".into(), Value::Int(9));
    assert_ne!(changed, value);
    assert_eq!(copy, value);
    // Each kind differs from another, and from itself changed.
    for (a, b) in [
        (json!("a"), json!("b")),
        (json!(true), json!(false)),
        (json!(1), json!(2)),
        (json!([1, 2]), json!([2, 1])),
        (json!([1]), json!([1, 1])),
        (json!({ "a": 1 }), json!({ "b": 1 })),
        (
            json!({ "$case": "some", "value": 1 }),
            json!({ "$case": "none" }),
        ),
        (
            json!({ "$case": "ok", "value": 1 }),
            json!({ "$case": "err", "value": 1 }),
        ),
        (json!("1"), json!(1)),
    ] {
        let (x, y) = (wire_value(&a), wire_value(&b));
        assert_ne!(x, y, "{a} {b}");
        assert_eq!(x.clone(), x, "{a}");
    }
    let raw = |html: &str| Value::Raw {
        html: html.into(),
        capability: "c".into(),
    };
    assert_ne!(raw("<b>"), raw("<i>"));
    assert_eq!(raw("<b>").clone(), raw("<b>"));
}

#[test]
fn nested_json_as_deep_as_its_data_is_read_without_recursion() {
    small_value_contract();
    const DEEP: usize = 50_000;
    let observation = stack_probe::observe(
        "nested_json_as_deep_as_its_data_is_read_without_recursion",
        2,
        || {
            let mut json = json!(0);
            for _ in 0..DEEP {
                json = serde_json::Value::Array(vec![json]);
            }
            // serde_json itself drops recursively; that is outside this contract.
            let json: &'static serde_json::Value = Box::leak(Box::new(json));
            let value = Value::from_wire(json).expect("the deep valid JSON");
            let mut at = &value;
            let mut depth = 0;
            while let Value::List(items) = at {
                at = &items[0];
                depth += 1;
            }
            let leaf = at == &Value::Int(0);
            (vec![depth, usize::from(leaf)], vec![value])
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let Some(observation) = observation else {
        return;
    };
    assert!(
        !observation.overflow,
        "the exact nested drop fits its 256KiB stack: {}",
        observation.output
    );
    assert_eq!(observation.values, vec![DEEP, 1], "{}", observation.output);
}

#[test]
fn the_reader_keeps_each_item_and_field_in_its_place() {
    // Against values built here, not read: a reader that put a list's items
    // or a record's values elsewhere reads both sides of a comparison so.
    let record = |fields: &[(&str, Value)]| {
        Value::Record(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    };
    assert_eq!(
        wire_value(&json!([1, "a", true])),
        Value::List(vec![
            Value::Int(1),
            Value::Text("a".into()),
            Value::Bool(true)
        ])
    );
    assert_eq!(
        wire_value(&json!({ "a": 1, "b": "x", "c": [2, 3] })),
        record(&[
            ("a", Value::Int(1)),
            ("b", Value::Text("x".into())),
            ("c", Value::List(vec![Value::Int(2), Value::Int(3)])),
        ])
    );
    assert_eq!(
        wire_value(&json!({ "$case": "some", "value": [4, 5] })),
        Value::Variant {
            case: "some".into(),
            payload: Some(Box::new(Value::List(vec![Value::Int(4), Value::Int(5)]))),
        }
    );
}
