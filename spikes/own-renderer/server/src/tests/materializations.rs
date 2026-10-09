//! **A materialization kept, and served** (ADR-0277): the store's page reads
//! its menu counted, `MenuLine(id)`, which reads `MenuSize(id)`, which reads
//! the menu. The host keeps each, every reader's; makes the chain again when
//! the menu changes, the count before the line and each once; tells each
//! open document that reads an entry made again, and no other; and serves
//! the last good value of what could not be made again. Each test states one
//! case, with its control.

use super::*;

const SIZE: &str = "store.page.MenuSize";
const LINE: &str = "store.page.MenuLine";

/// What the materializer did with the chain's entries for store `id`, in
/// order: each made, or asked for and already current.
fn chain_work(s: &Server, id: &str) -> Vec<String> {
    s.materializer
        .trace()
        .into_iter()
        .filter_map(|t| match t {
            pw_materialize::Trace::Regenerated { entry, .. } => Some(("made", entry)),
            pw_materialize::Trace::AlreadyCurrent { entry } => Some(("current", entry)),
            _ => None,
        })
        .filter_map(|(what, entry)| {
            [SIZE, LINE]
                .into_iter()
                .find(|p| entry.contains(&format!("{p}({id})")))
                .map(|p| format!("{what} {p}"))
        })
        .collect()
}

/// The line store `id`'s value is now, as the host keeps it.
fn line(s: &Server, id: &str) -> String {
    match s.materialized(LINE, &[Val::String(id.to_string())]) {
        Ok(Val::String(line)) => line,
        other => panic!("the line: {other:?}"),
    }
}

/// The flat white, added before the cortado: `MenuChanged(47)`.
fn add_a_flat_white(s: &Server) {
    s.broadcast_menu(MenuOp::Insert {
        id: "flat-white".to_string(),
        name: "Flat White".to_string(),
        at: Some("cortado".to_string()),
        before: true,
    })
    .expect("the menu changes");
}

/// Whether session `a`'s `document` was told an entry it reads was made
/// again: the frame `tell_kept` sends it, which reads the document again. A
/// document read again whose page did not change is sent no operation, so
/// its operations alone cannot say whether it was told.
fn told(s: &Server, document: u64) -> bool {
    s.pending.lock().expect("pending")[&("a".to_string(), document)]
        .frames
        .iter()
        .any(|(_, f)| {
            matches!(
                f,
                StreamFrame::ResourceChanged { entry, .. } if *entry == session_documents("a")
            )
        })
}

#[test]
fn a_page_is_served_what_a_chain_derives_every_readers() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let (html, _) = s.serve_store_document("a", STORE_ID).expect("served");
    assert!(
        visible(&html).contains("3 items in 1 section"),
        "{}",
        visible(&html)
    );
    // Each made once, the count first, since the line reads it.
    assert_eq!(
        chain_work(&s, STORE_ID),
        [format!("made {SIZE}"), format!("made {LINE}")]
    );
    // The control: another reader is served what was kept, and nothing is
    // made again.
    let (html, _) = s.serve_store_document("b", STORE_ID).expect("served");
    assert!(visible(&html).contains("3 items in 1 section"));
    assert_eq!(chain_work(&s, STORE_ID).len(), 2);
}

#[test]
fn a_change_makes_the_chain_again_in_its_order_and_tells_its_readers() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let (_, document) = s.serve_store_document("a", STORE_ID).expect("served");
    let before = chain_work(&s, STORE_ID).len();
    add_a_flat_white(&s);
    // The count, then the line from it, each once: nothing asked for again,
    // as a line made first would ask for the count it had made inside it.
    assert_eq!(
        chain_work(&s, STORE_ID)[before..],
        [format!("made {SIZE}"), format!("made {LINE}")]
    );
    assert_eq!(line(&s, STORE_ID), "4 items in 1 section");
    // The document that reads it, told its text.
    assert!(
        operations_for(&s, "a", document).iter().any(|o| o.operation
            == PatchOp::ReplaceText {
                text: "4 items in 1 section".to_string()
            }),
        "{:?}",
        operations_for(&s, "a", document)
    );
}

#[test]
fn a_document_that_reads_another_key_is_told_nothing() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let (_, blue) = s.serve_store_document("a", STORE_ID).expect("served");
    let (_, harbor) = s.serve_store_document("a", "48").expect("served");
    let made = chain_work(&s, "48").len();
    add_a_flat_white(&s);
    // Store 48's line reads store 48's count, which `MenuChanged(47)` does not
    // reach: neither made again, nor its document told, though the session
    // holds both.
    assert_eq!(chain_work(&s, "48").len(), made);
    assert!(!told(&s, harbor), "store 48's document was told");
    assert!(
        operations_for(&s, "a", harbor).is_empty(),
        "{:?}",
        operations_for(&s, "a", harbor)
    );
    // The control: store 47's, in the same session, is.
    assert!(told(&s, blue), "store 47's document was not told");
    assert!(!operations_for(&s, "a", blue).is_empty());
}

#[test]
fn what_could_not_be_made_again_is_served_its_last_good_value() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    s.serve_store_document("a", STORE_ID).expect("served");
    // Every making of the count fails, while the fault holds.
    s.materializer_faults
        .lock()
        .expect("faults")
        .insert(SIZE.to_string());
    add_a_flat_white(&s);
    // `fallback last_known_good`: the count it had, and the line from it.
    assert_eq!(line(&s, STORE_ID), "3 items in 1 section");
    assert!(
        s.materializer.trace().iter().any(|t| matches!(
            t,
            pw_materialize::Trace::ServedLastKnownGood { entry } if entry.contains(SIZE)
        )),
        "the last good count was served"
    );
    // The control: the fault gone, the next change makes it again.
    s.materializer_faults.lock().expect("faults").remove(SIZE);
    s.broadcast_menu(MenuOp::Rename {
        id: "espresso".to_string(),
        name: "Espresso Doppio".to_string(),
    })
    .expect("the menu changes");
    assert_eq!(line(&s, STORE_ID), "4 items in 1 section");
}

#[test]
fn an_entry_made_again_with_its_value_tells_no_one() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let (_, document) = s.serve_store_document("a", STORE_ID).expect("served");
    let before = chain_work(&s, STORE_ID).len();
    s.broadcast_menu(MenuOp::Rename {
        id: "espresso".to_string(),
        name: "Espresso Doppio".to_string(),
    })
    .expect("the menu changes");
    // Made again, since the menu changed, to the value it had: a rename
    // counts the same.
    assert_eq!(
        chain_work(&s, STORE_ID)[before..],
        [format!("made {SIZE}"), format!("made {LINE}")]
    );
    assert_eq!(line(&s, STORE_ID), "3 items in 1 section");
    // So its document is not read again: it has the menu's own frames, and
    // no telling of the session's documents. Until it was told only on a
    // change, each rename read every open store page again, and a thousand
    // of them outlasted the idle window.
    assert!(!told(&s, document), "the document was told");
    let frames = &s.pending.lock().expect("pending")[&("a".to_string(), document)].frames;
    // The control: the menu's own patch reached it.
    assert!(
        frames
            .iter()
            .any(|(_, f)| matches!(f, StreamFrame::Patch(_) | StreamFrame::PatchSet(_))),
        "{frames:?}"
    );
}
