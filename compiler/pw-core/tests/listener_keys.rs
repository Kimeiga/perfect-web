//! **A listener binds its entry's key** (ADR-0091).
//!
//! `invalidates_on InventoryChanged(id, _)` says which of an event's values
//! must equal which part of the declaration's key: the first must equal its
//! `id`, and the second may be anything. Until 2026-09-26 the value was text.
//! `InventoryChanged(id, item)`, with `item` naming nothing, checked, and so
//! did `StoreChanged(consumer)`, a consumer where the event carries a store.
//! Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(src: &str, says: &str) {
    let found = reported(src);
    assert_eq!(found.len(), 1, "{src}\n{found:#?}");
    assert!(found[0].contains(says), "{src}\n{found:#?}");
}

fn clean(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{src}\n{found:#?}");
}

/// An event with two values, and a materialization keyed by two, listening.
fn program(listens: &str) -> String {
    format!(
        "module t\n\n\
         event Moved(to: Int, why: String)\n\n\
         query Box(id: Int) -> Int !{{}} {{ id }}\n\n\
         materialize Panel(id: Int, note: String) {{\n    \
             placement      origin\n    \
             partition      private\n    \
             depends_on     Box(id)\n    \
             invalidates_on {listens}\n    \
             regenerate     on_invalidation\n}}\n"
    )
}

#[test]
fn a_listener_names_a_parameter_or_any_value() {
    clean(&program("Moved(id, note)"));
    clean(&program("Moved(id, _)"));
    clean(&program("Moved(_, _)"));
    one(
        &program("Moved(id, item)"),
        "PW5104 `Panel` listens for `Moved` with `item`, which is none of its parameters",
    );
    one(
        &program("Moved(id + 1, _)"),
        "PW5104 `Panel` listens for `Moved` with `id + 1`, which is none of its parameters",
    );
}

#[test]
fn a_listener_is_given_what_the_event_carries() {
    one(
        &program("Moved(note, _)"),
        "PW0605 argument 1 of `t.Moved` is declared `Int` and this is `String`",
    );
    one(
        &program("Moved(id)"),
        "PW0604 `t.Moved` declares 2 arguments and this call passes 1",
    );
    clean(&program("Moved(why = note, to = id)"));
}
