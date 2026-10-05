//! **A value of a type that contains itself, as its nodes** (ADR-0205).
//!
//! On the browser's wire such a value is `{ "$graph": [node, ...] }`: node 0
//! the value, each value of the type inside a node `{ "$node": k }` for a
//! later node. The renderer builds it from its last node to its first, and
//! holds it, clones it, compares it and drops it with stacks on the heap, so
//! a value is as deep as its data and the JSON as shallow as its type.

use pw_render::Value;
use serde_json::json;

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
    assert_eq!(
        Value::from_wire(&graph).expect("a tree"),
        Value::from_wire(&nested).expect("a value")
    );
    // A node held where a case holds it, `Some(next)`.
    let chain = json!({ "$graph": [
        { "n": 1, "next": { "$case": "Some", "value": { "$node": 1 } } },
        { "n": 2, "next": { "$case": "None" } },
    ]});
    let nested = json!({ "n": 1, "next": { "$case": "Some", "value":
        { "n": 2, "next": { "$case": "None" } } } });
    assert_eq!(
        Value::from_wire(&chain).expect("a tree"),
        Value::from_wire(&nested).expect("a value")
    );
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
    assert_eq!(
        Value::from_wire(&outer).expect("a tree"),
        Value::from_wire(&nested).expect("a value")
    );
}

#[test]
fn a_graph_that_is_not_a_tree_is_refused() {
    let refused = |j: serde_json::Value| Value::from_wire(&j).expect_err("refused");
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
    // A chain 100,000 nodes deep, as JSON three values deep: read, cloned,
    // compared and dropped on a thread with 256 KiB of stack, an eighth of
    // a server worker's.
    const DEEP: usize = 100_000;
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
    let deepest = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let value = Value::from_wire(&graph).expect("a tree");
            let copy = value.clone();
            assert_eq!(value, copy);
            // The last node, as deep as the chain.
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
            // Differing at the bottom only: found unequal.
            let mut other = value.clone();
            let mut cursor = &mut other;
            loop {
                let Value::Record(fields) = cursor else {
                    unreachable!()
                };
                if !matches!(fields.get("next"), Some(Value::List(next)) if !next.is_empty()) {
                    fields.insert("n".into(), Value::Int(-1));
                    break;
                }
                let Some(Value::List(next)) = fields.get_mut("next") else {
                    unreachable!()
                };
                cursor = &mut next[0];
            }
            assert_ne!(value, other);
            depth
        })
        .expect("spawns")
        .join()
        .expect("returns");
    assert_eq!(deepest, DEEP - 1);
}

#[test]
fn a_copy_is_the_value_and_its_own() {
    // Branching, so a copy whose children came back in another order, or a
    // comparison that read one side's alone, is found.
    let tree = json!({ "id": 1, "tags": ["x", "y"], "replies": [
        { "id": 2, "tags": [], "replies": [{ "id": 4, "tags": ["z"], "replies": [] }] },
        { "id": 3, "tags": ["w"], "replies": [] },
    ], "state": { "$case": "Some", "value": { "seen": true } } });
    let value = Value::from_wire(&tree).expect("a value");
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
            json!({ "$case": "Some", "value": 1 }),
            json!({ "$case": "None" }),
        ),
        (
            json!({ "$case": "Ok", "value": 1 }),
            json!({ "$case": "Err", "value": 1 }),
        ),
        (json!("1"), json!(1)),
    ] {
        let (x, y) = (
            Value::from_wire(&a).expect("a value"),
            Value::from_wire(&b).expect("a value"),
        );
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
    // What no browser's module writes for such a type, and the renderer
    // reads all the same: 50,000 lists, one inside another, read on a
    // thread with 256 KiB of stack. The JSON is leaked, not dropped:
    // serde_json drops its own values by recursion.
    const DEEP: usize = 50_000;
    let mut j = json!(0);
    for _ in 0..DEEP {
        j = serde_json::Value::Array(vec![j]);
    }
    let j: &'static serde_json::Value = Box::leak(Box::new(j));
    let depth = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let v = Value::from_wire(j).expect("a value");
            let mut at = &v;
            let mut depth = 0;
            while let Value::List(items) = at {
                at = &items[0];
                depth += 1;
            }
            assert_eq!(at, &Value::Int(0));
            depth
        })
        .expect("spawns")
        .join()
        .expect("returns");
    assert_eq!(depth, DEEP);
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
        Value::from_wire(&json!([1, "a", true])).expect("a value"),
        Value::List(vec![
            Value::Int(1),
            Value::Text("a".into()),
            Value::Bool(true)
        ])
    );
    assert_eq!(
        Value::from_wire(&json!({ "a": 1, "b": "x", "c": [2, 3] })).expect("a value"),
        record(&[
            ("a", Value::Int(1)),
            ("b", Value::Text("x".into())),
            ("c", Value::List(vec![Value::Int(2), Value::Int(3)])),
        ])
    );
    assert_eq!(
        Value::from_wire(&json!({ "$case": "some", "value": [4, 5] })).expect("a value"),
        Value::Variant {
            case: "some".into(),
            payload: Some(Box::new(Value::List(vec![Value::Int(4), Value::Int(5)]))),
        }
    );
}
