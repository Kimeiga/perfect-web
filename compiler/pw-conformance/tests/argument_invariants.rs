//! **An argument of a type that contains itself is held as the component is
//! passed it** (ADR-XXXX, argument invariants; ADR-0194).
//!
//! Such a value crosses a component boundary as its nodes, and the
//! contract's paths into it are its nodes' (`*`, then the field or the
//! case). A host may give it nested, as the program holds it, and the call
//! makes it its nodes: so the call's own check of an export's arguments holds
//! the nodes, whichever form it was given. A value held nested would be read
//! by the wrong path: a record at the root refused for "no field `*`", a case
//! at the root passed over unread.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::{HostFn, Val};

const PROGRAM: &str = r#"module held

import List
import Map
import Set

opaque type Likes = Int where value >= 0

type Comment = Comment { text: String, likes: Likes, replies: List<Comment> }

type Tree =
    | Leaf(Likes)
    | Branch(List<Tree>)

fn count(c: Comment) -> Int {
    List.fold(c.replies, 1, (n, r) => n + count(r))
}

fn leaves(t: Tree) -> Int {
    match t {
        Leaf(_) => 1,
        Branch(ts) => List.fold(ts, 0, (n, x) => n + leaves(x)),
    }
}

public query Size(c: Comment) -> Int { count(c) }

public query Leaves(t: Tree) -> Int { leaves(t) }

public query Valued(m: Map<String, Likes>) -> Int { Map.size(m) }

public query Keyed(m: Map<Likes, String>) -> Int { Map.size(m) }

public query Counted(s: Set<Likes>) -> Int { Set.size(s) }
"#;

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("held.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, arg: Val) -> Result<Val, String> {
    let ops: BTreeMap<String, HostFn> = BTreeMap::new();
    r.call(&ops, &[arg]).map(|mut v| v.remove(0))
}

/// A comment as the program holds it: nested.
fn comment(text: &str, likes: i64, replies: Vec<Val>) -> Val {
    Val::Record(vec![
        ("text".into(), Val::String(text.into())),
        ("likes".into(), Val::S64(likes)),
        ("replies".into(), Val::List(replies)),
    ])
}

/// A comment's node, as a component passes it: its replies by index.
fn node(text: &str, likes: i64, replies: &[u32]) -> Val {
    Val::Record(vec![
        ("text".into(), Val::String(text.into())),
        ("likes".into(), Val::S64(likes)),
        (
            "replies".into(),
            Val::List(replies.iter().map(|i| Val::U32(*i)).collect()),
        ),
    ])
}

const LIKES: &str = "`held.Likes` holds `value >= 0`";

/// **A record at the root**: a thread of three, its second reply's likes
/// broken or not, given nested and as its nodes.
#[test]
fn a_record_that_contains_itself_is_held_as_its_nodes_however_it_is_given() {
    let size = compiled("held.Size");
    let thread = |likes: i64| {
        comment(
            "root",
            2,
            vec![comment("a", 0, vec![]), comment("b", likes, vec![])],
        )
    };
    let nodes = |likes: i64| {
        Val::List(vec![
            node("root", 2, &[1, 2]),
            node("a", 0, &[]),
            node("b", likes, &[]),
        ])
    };
    for arg in [thread(0), nodes(0), thread(7), nodes(7)] {
        assert_eq!(call(&size, arg.clone()), Ok(Val::S64(3)), "{arg:?}");
    }
    let said = format!("argument refused: value 1's `[2].likes` is -1, and {LIKES}");
    for arg in [thread(-1), nodes(-1)] {
        assert_eq!(call(&size, arg.clone()), Err(said.clone()), "{arg:?}");
    }
}

/// **A case at the root**: a branch of two leaves and a branch, one leaf's
/// likes broken or not, given nested and as its nodes.
#[test]
fn a_case_that_contains_itself_is_held_as_its_nodes_however_it_is_given() {
    let leaves = compiled("held.Leaves");
    let leaf = |likes: i64| Val::Variant("leaf".into(), Some(Box::new(Val::S64(likes))));
    let branch = |ts: Vec<Val>| Val::Variant("branch".into(), Some(Box::new(Val::List(ts))));
    let tree = |likes: i64| branch(vec![leaf(1), branch(vec![leaf(likes)])]);
    let indices = |is: &[u32]| {
        Val::Variant(
            "branch".into(),
            Some(Box::new(Val::List(
                is.iter().map(|i| Val::U32(*i)).collect(),
            ))),
        )
    };
    let nodes = |likes: i64| Val::List(vec![indices(&[1, 2]), leaf(1), indices(&[3]), leaf(likes)]);
    for arg in [tree(0), nodes(0), tree(4), nodes(4)] {
        assert_eq!(call(&leaves, arg.clone()), Ok(Val::S64(2)), "{arg:?}");
    }
    let said = format!("argument refused: value 1's `[3].leaf` is -2, and {LIKES}");
    for arg in [tree(-2), nodes(-2)] {
        assert_eq!(call(&leaves, arg.clone()), Err(said.clone()), "{arg:?}");
    }
}

/// **A map's entries and a set's items** (ADR-0057's `list<tuple<k, v>>`
/// and `list<t>`): a value, a key and an item each held where it is, the
/// entry by its index and the key or value by its place in the entry.
#[test]
fn a_maps_keys_and_values_and_a_sets_items_are_held() {
    let entry = |k: Val, v: Val| Val::Tuple(vec![k, v]);
    let text = |t: &str| Val::String(t.into());
    let valued = compiled("held.Valued");
    let values = |likes: i64| {
        Val::List(vec![
            entry(text("a"), Val::S64(1)),
            entry(text("b"), Val::S64(likes)),
        ])
    };
    assert_eq!(call(&valued, values(0)), Ok(Val::S64(2)));
    assert_eq!(
        call(&valued, values(-1)),
        Err(format!(
            "argument refused: value 1's `[1].1` is -1, and {LIKES}"
        ))
    );
    let keyed = compiled("held.Keyed");
    let keys = |likes: i64| {
        Val::List(vec![
            entry(Val::S64(likes), text("a")),
            entry(Val::S64(3), text("b")),
        ])
    };
    assert_eq!(call(&keyed, keys(0)), Ok(Val::S64(2)));
    assert_eq!(
        call(&keyed, keys(-5)),
        Err(format!(
            "argument refused: value 1's `[0].0` is -5, and {LIKES}"
        ))
    );
    let counted = compiled("held.Counted");
    let items = |likes: i64| Val::List(vec![Val::S64(likes), Val::S64(9)]);
    assert_eq!(call(&counted, items(0)), Ok(Val::S64(2)));
    assert_eq!(
        call(&counted, items(-3)),
        Err(format!(
            "argument refused: value 1's `[0]` is -3, and {LIKES}"
        ))
    );
}
