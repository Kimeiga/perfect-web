//! **TodoMVC, built and served** (track `todomvc`, docs/PARALLEL.md, W7): a
//! session's todos from the host's layer (`todomvc.rs`), the program's
//! commands over them, and the page they are shown on. Each test states one
//! rule of TodoMVC's spec, or one of the layer's, with its control.

use super::*;

const PAGE: &str = "todomvc.app.Todos";

/// The program, built as `pw build` builds it, and served.
fn served_todomvc() -> Served {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = tempfile::TempDir::with_prefix("pw-todomvc-").expect("a temporary directory");
    let mut units = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/todomvc",
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let src = std::fs::read_to_string(&p).expect("read");
            units.push(pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path: p.display().to_string(),
                src,
            });
        }
    }
    let build = pw_core::build::build(&units).expect("TodoMVC builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let out = dir.path().join("build");
    build.write(&out).expect("the build is written");
    Served {
        server: Server::from_build(out.clone(), out).expect("served"),
        _dir: dir,
    }
}

/// A command, as a browser sends it: its arguments as JSON, and the
/// interaction it is one press of.
fn press(
    s: &Server,
    command: &str,
    session: &str,
    args: &[serde_json::Value],
    interaction: &str,
) -> Result<Answered, String> {
    s.command_answer(
        &format!("todomvc.app.{command}"),
        session,
        args,
        Some(interaction),
    )
}

/// A command that committed.
fn committed(s: &Server, command: &str, session: &str, args: &[serde_json::Value], i: &str) {
    let answered = press(s, command, session, args, i).expect("runs");
    assert!(answered.committed, "{command}: {:?}", answered.result);
}

/// What `session` holds: each todo's id, title and whether it is completed,
/// oldest first, read from the layer as a query reads it.
fn held(s: &Server, session: &str) -> Vec<(String, String, bool)> {
    let ops = s.data.reads(session, None);
    let read = ops
        .get("todomvc:data/todos#all")
        .expect("the layer reads todos");
    let Ok(answer) = read(&[Val::String(session.to_string())]) else {
        panic!("the layer refused its own session's read");
    };
    let [Val::List(items)] = answer.as_slice() else {
        panic!("todos#all answered {answer:?}");
    };
    items
        .iter()
        .map(|item| {
            let Val::Record(fields) = item else {
                panic!("a todo is a record: {item:?}");
            };
            let field = |name: &str| {
                fields
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v)
                    .unwrap_or_else(|| panic!("a todo has `{name}`"))
            };
            let (Val::String(id), Val::String(title), Val::Bool(done)) =
                (field("id"), field("title"), field("completed"))
            else {
                panic!("a todo's fields: {fields:?}");
            };
            (id.clone(), title.clone(), *done)
        })
        .collect()
}

fn titles(s: &Server, session: &str) -> Vec<String> {
    held(s, session).into_iter().map(|(_, t, _)| t).collect()
}

/// The page `session` is served, as text.
fn page(s: &Server, session: &str) -> String {
    let (html, _, _, _) = s
        .serve_document_settled(session, PAGE, &Params::new(), &[])
        .expect("served");
    html
}

/// An element's opening tag, the first with `marker` in it.
fn tag_of<'a>(html: &'a str, marker: &str) -> &'a str {
    let at = html
        .find(marker)
        .unwrap_or_else(|| panic!("`{marker}` in {html}"));
    let start = html[..at].rfind('<').expect("its tag's start");
    &html[start..at + html[at..].find('>').expect("its tag's end") + 1]
}

/// **No todos: `.main` and `.footer` are hidden, not absent** (the spec's
/// "when there are no todos, `#main` and `#footer` should be hidden"); and
/// with one, shown. Hidden, because the official tests find them to assert
/// they are not visible.
#[test]
fn with_no_todos_main_and_footer_are_hidden_and_with_one_they_are_shown() {
    let s = served_todomvc();
    let empty = page(&s, "a");
    assert!(
        tag_of(&empty, "class=\"main\"").contains(" hidden"),
        "{empty}"
    );
    assert!(
        tag_of(&empty, "class=\"footer\"").contains(" hidden"),
        "{empty}"
    );
    committed(
        &s,
        "add",
        "a",
        &[serde_json::json!("buy some cheese")],
        "i-1",
    );
    let one = page(&s, "a");
    assert!(!tag_of(&one, "class=\"main\"").contains(" hidden"), "{one}");
    assert!(
        !tag_of(&one, "class=\"footer\"").contains(" hidden"),
        "{one}"
    );
    // The control: and with none again, hidden again.
    let (id, _, _) = held(&s, "a").remove(0);
    committed(&s, "destroy", "a", &[serde_json::json!(id)], "i-2");
    let none = page(&s, "a");
    assert!(
        tag_of(&none, "class=\"main\"").contains(" hidden"),
        "{none}"
    );
}

/// **New todos are appended to the bottom of the list**, each with its own
/// id, and the page shows them in that order.
#[test]
fn a_new_todo_is_appended_to_the_bottom_of_the_list() {
    let s = served_todomvc();
    for (n, title) in [
        "buy some cheese",
        "feed the cat",
        "book a doctors appointment",
    ]
    .into_iter()
    .enumerate()
    {
        committed(
            &s,
            "add",
            "a",
            &[serde_json::json!(title)],
            &format!("i-{n}"),
        );
    }
    assert_eq!(
        titles(&s, "a"),
        [
            "buy some cheese",
            "feed the cat",
            "book a doctors appointment"
        ]
    );
    let html = page(&s, "a");
    let at = |t: &str| {
        html.find(&format!(">{t}<"))
            .unwrap_or_else(|| panic!("{t}"))
    };
    assert!(at("buy some cheese") < at("feed the cat"));
    assert!(at("feed the cat") < at("book a doctors appointment"));
    // Ids are never reused: a destroyed todo's is not a later one's.
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    assert_eq!(ids.len(), 3);
    committed(&s, "destroy", "a", &[serde_json::json!(ids[2])], "i-9");
    committed(&s, "add", "a", &[serde_json::json!("another")], "i-10");
    let after: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    assert!(
        !ids.contains(after.last().expect("one")),
        "{ids:?} {after:?}"
    );
}

/// **A title is something, and at most what the program says**: the host
/// holds a browser's argument to `Title`'s invariant, so a forged empty or
/// overlong one is refused before anything runs. The controls: the longest
/// is added.
#[test]
fn the_host_holds_a_title_to_its_invariant() {
    let s = served_todomvc();
    for (what, title) in [("empty", String::new()), ("overlong", "x".repeat(1001))] {
        let refused = press(
            &s,
            "add",
            "a",
            &[serde_json::json!(title)],
            &format!("i-{what}"),
        );
        assert!(
            refused.is_err(),
            "{what}: {:?}",
            refused.map(|a| a.committed)
        );
    }
    assert_eq!(titles(&s, "a"), Vec::<String>::new());
    committed(
        &s,
        "add",
        "a",
        &[serde_json::json!("x".repeat(1000))],
        "i-ok",
    );
    assert_eq!(titles(&s, "a").len(), 1);
}

/// **The counter's words are the spec's**: `0 items left`, `1 item left`,
/// `2 items left`, the number in a `<strong>`; and it counts the active
/// todos, which completing one takes down.
#[test]
fn the_counter_says_items_left_by_the_spec() {
    let s = served_todomvc();
    let counter = |s: &Server| {
        let html = page(s, "a");
        let at = html.find("class=\"todo-count\"").expect("the counter");
        let start = html[..at].rfind('<').expect("its tag's start");
        let end = html[at..].find("</span>").expect("its end");
        visible(&html[start..at + end + "</span>".len()])
    };
    assert!(counter(&s).contains("0 items left"), "{}", counter(&s));
    committed(&s, "add", "a", &[serde_json::json!("one")], "i-1");
    assert!(counter(&s).contains("1 item left"), "{}", counter(&s));
    committed(&s, "add", "a", &[serde_json::json!("two")], "i-2");
    assert!(counter(&s).contains("2 items left"), "{}", counter(&s));
    // The number is wrapped in a `<strong>`.
    let html = page(&s, "a");
    let at = html.find("class=\"todo-count\"").expect("the counter");
    assert!(html[at..].contains("<strong>"), "{}", &html[at..at + 200]);
    // Completing one takes the count down.
    let (id, _, _) = held(&s, "a").remove(0);
    committed(&s, "toggle", "a", &[serde_json::json!(id)], "i-3");
    assert!(counter(&s).contains("1 item left"), "{}", counter(&s));
}

/// **Toggling one todo, and every todo**: a todo's checkbox toggles it, the
/// toggle-all takes every one to completed, or, where every one is, to
/// active. The toggle-all's checkbox is checked where there are todos and
/// none is active.
#[test]
fn toggling_one_and_toggling_all() {
    let s = served_todomvc();
    for (n, t) in ["one", "two", "three"].into_iter().enumerate() {
        committed(&s, "add", "a", &[serde_json::json!(t)], &format!("i-{n}"));
    }
    let done = |s: &Server| -> Vec<bool> { held(s, "a").into_iter().map(|(_, _, d)| d).collect() };
    let checked = |s: &Server| tag_of(&page(s, "a"), "id=\"toggle-all\"").contains(" checked");
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    // One.
    committed(&s, "toggle", "a", &[serde_json::json!(ids[1])], "t-1");
    assert_eq!(done(&s), [false, true, false]);
    assert!(!checked(&s));
    committed(&s, "toggle", "a", &[serde_json::json!(ids[1])], "t-2");
    assert_eq!(done(&s), [false, false, false]);
    // All: to completed, and checked.
    committed(&s, "toggle_all", "a", &[], "all-1");
    assert_eq!(done(&s), [true, true, true]);
    assert!(checked(&s));
    // Not checked once one is active again: it follows each todo.
    committed(&s, "toggle", "a", &[serde_json::json!(ids[0])], "t-3");
    assert_eq!(done(&s), [false, true, true]);
    assert!(!checked(&s));
    // With one active, the toggle-all completes the rest, and then none.
    committed(&s, "toggle_all", "a", &[], "all-2");
    assert_eq!(done(&s), [true, true, true]);
    committed(&s, "toggle_all", "a", &[], "all-3");
    assert_eq!(done(&s), [false, false, false]);
    // The control: with no todos it is not checked.
    assert!(!tag_of(&page(&s, "b"), "id=\"toggle-all\"").contains(" checked"));
}

/// **Clear completed removes the completed**, and the button is hidden where
/// none is.
#[test]
fn clear_completed_removes_the_completed_and_hides_with_none() {
    let s = served_todomvc();
    for (n, t) in ["one", "two", "three"].into_iter().enumerate() {
        committed(&s, "add", "a", &[serde_json::json!(t)], &format!("i-{n}"));
    }
    let clear = |s: &Server| tag_of(&page(s, "a"), "class=\"clear-completed\"").to_string();
    assert!(
        clear(&s).contains(" hidden"),
        "none completed: {}",
        clear(&s)
    );
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    committed(&s, "toggle", "a", &[serde_json::json!(ids[1])], "t-1");
    assert!(
        !clear(&s).contains(" hidden"),
        "one completed: {}",
        clear(&s)
    );
    committed(&s, "clear_completed", "a", &[], "c-1");
    assert_eq!(titles(&s, "a"), ["one", "three"]);
    assert!(clear(&s).contains(" hidden"), "again: {}", clear(&s));
}

/// **An edit is saved trimmed, and a title trimmed to nothing destroys the
/// todo** (the spec's "if it's empty the todo should instead be destroyed").
#[test]
fn an_edit_is_saved_trimmed_and_an_empty_one_destroys_the_todo() {
    let s = served_todomvc();
    for (n, t) in [
        "buy some cheese",
        "feed the cat",
        "book a doctors appointment",
    ]
    .into_iter()
    .enumerate()
    {
        committed(&s, "add", "a", &[serde_json::json!(t)], &format!("i-{n}"));
    }
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    committed(
        &s,
        "save_edit",
        "a",
        &[
            serde_json::json!(ids[1]),
            serde_json::json!("   buy some sausages    "),
        ],
        "e-1",
    );
    assert_eq!(
        titles(&s, "a"),
        [
            "buy some cheese",
            "buy some sausages",
            "book a doctors appointment"
        ]
    );
    // Its place and its state are the todo's own.
    assert_eq!(held(&s, "a")[1].0, ids[1]);
    // Nothing left of it: destroyed, whatever the whitespace.
    committed(
        &s,
        "save_edit",
        "a",
        &[serde_json::json!(ids[1]), serde_json::json!(" \t ")],
        "e-2",
    );
    assert_eq!(
        titles(&s, "a"),
        ["buy some cheese", "book a doctors appointment"]
    );
    // The control: an edit that is no longer than the text is kept as it is.
    committed(
        &s,
        "save_edit",
        "a",
        &[
            serde_json::json!(ids[0]),
            serde_json::json!("buy some cheese"),
        ],
        "e-3",
    );
    assert_eq!(titles(&s, "a")[0], "buy some cheese");
}

/// **A command for a todo that is not there is not found, and changes
/// nothing**: destroyed in another tab, or never there.
#[test]
fn a_todo_that_is_not_there_is_not_found() {
    let s = served_todomvc();
    committed(&s, "add", "a", &[serde_json::json!("one")], "i-1");
    for (n, (command, args)) in [
        ("toggle", vec![serde_json::json!("t99")]),
        ("destroy", vec![serde_json::json!("t99")]),
        (
            "save_edit",
            vec![serde_json::json!("t99"), serde_json::json!("x")],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let answered = press(&s, command, "a", &args, &format!("n-{n}")).expect("runs");
        assert!(!answered.committed, "{command}");
        assert!(
            format!("{:?}", answered.result).contains("not-found"),
            "{command}: {:?}",
            answered.result
        );
    }
    assert_eq!(titles(&s, "a"), ["one"]);
}

/// **A session's todos are its own**: another session sees none of them and
/// changes none of them, by a page, by a command naming an id, and by the
/// layer, which holds a command to its own session.
#[test]
fn a_sessions_todos_are_its_own() {
    let s = served_todomvc();
    committed(&s, "add", "a", &[serde_json::json!("alice's")], "i-1");
    committed(&s, "add", "b", &[serde_json::json!("bob's")], "i-2");
    assert_eq!(titles(&s, "a"), ["alice's"]);
    assert_eq!(titles(&s, "b"), ["bob's"]);
    assert!(!page(&s, "b").contains("alice's"));
    // An id is a session's own counter, so alice's first todo and bob's are
    // both `t1`: bob's command on `t1` is on bob's todo, and alice's is
    // untouched.
    let alices = held(&s, "a")[0].0.clone();
    let bobs = held(&s, "b")[0].0.clone();
    assert_eq!(alices, bobs, "ids are the session's own counter");
    committed(&s, "toggle", "b", &[serde_json::json!(bobs)], "i-3");
    assert!(!held(&s, "a")[0].2, "alice's is untouched");
    assert!(held(&s, "b")[0].2);
    committed(&s, "destroy", "b", &[serde_json::json!(bobs)], "i-4");
    assert_eq!(titles(&s, "a"), ["alice's"]);
    assert_eq!(titles(&s, "b"), Vec::<String>::new());
    // And the layer: an operation that names another session than the one
    // it runs for is refused, a read as a write.
    let ops = s.data.reads("a", None);
    let read = ops.get("todomvc:data/todos#all").expect("a read");
    assert!(read(&[Val::String("b".to_string())]).is_err());
    let staged = s.data.begin("a");
    let staged_ops = staged.ops();
    let add = staged_ops.get("todomvc:data/todos#add").expect("a write");
    assert!(add(&[Val::String("b".to_string()), Val::String("x".to_string())]).is_err());
    assert!(add(&[Val::String("a".to_string()), Val::String("x".to_string())]).is_ok());
}

/// **A session holds at most `MAX_TODOS`**: the next is `full`, and nothing
/// is staged for it. Staged in one transaction, as a command's writes are.
#[test]
fn a_session_holds_at_most_a_bounded_list() {
    let s = served_todomvc();
    let staged = s.data.begin("a");
    let ops = staged.ops();
    let add = ops.get("todomvc:data/todos#add").expect("a write");
    let added = |n: usize| {
        add(&[
            Val::String("a".to_string()),
            Val::String(format!("todo {n}")),
        ])
    };
    for n in 0..crate::todomvc::MAX_TODOS {
        let answer = added(n).expect("runs");
        assert!(
            matches!(answer.as_slice(), [Val::Result(Ok(_))]),
            "todo {n}: {answer:?}"
        );
    }
    let answer = added(crate::todomvc::MAX_TODOS).expect("runs");
    assert!(
        format!("{answer:?}").contains("full"),
        "the next one is full: {answer:?}"
    );
}

/// **A fresh session's id is not a stranger's**: the page is served for a
/// session that has never added, empty, and `Set-Cookie`'s session is the
/// host's alone to make: the layer keeps no list for a session until it
/// commits one.
#[test]
fn a_session_with_no_todos_has_no_list_kept() {
    let s = served_todomvc();
    let _ = page(&s, "never-added");
    assert_eq!(held(&s, "never-added"), Vec::new());
}

/// **The model filters** (the spec: "the todo list should be filtered on a
/// model level"): the page's read for a filter holds the todos it shows, and
/// the counts of all of them. Read as the browser reads it again when the
/// fragment changes (ADR-0152).
#[test]
fn the_model_filters_by_the_pages_key() {
    let s = served_todomvc();
    for (n, t) in [
        "buy some cheese",
        "feed the cat",
        "book a doctors appointment",
    ]
    .into_iter()
    .enumerate()
    {
        committed(&s, "add", "a", &[serde_json::json!(t)], &format!("i-{n}"));
    }
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    committed(&s, "toggle", "a", &[serde_json::json!(ids[1])], "t-1");
    let doc = {
        let _ = page(&s, "a");
        latest(&s.pending.lock().expect("pending"), "a")
    };
    let removed = |set: &str| set.matches("RemoveInstance").count();
    let inserted =
        |set: &str| set.matches("InsertBefore").count() + set.matches("InsertAfter").count();
    let read = |filter: &str, seq: u64| -> String {
        let key = BTreeMap::from([
            ("filter".to_string(), serde_json::json!(filter)),
            ("editing".to_string(), serde_json::json!("")),
        ]);
        let before = sets_of(&s, &doc).len();
        let outcome = s
            .read_keyed("a", "board", seq, doc.1, &key, &|| false)
            .expect("read");
        assert!(matches!(outcome, KeyOutcome::Applied), "{filter}");
        let sets = sets_of(&s, &doc);
        assert!(sets.len() > before, "{filter}: a patch set was sent");
        format!("{:?}", sets.last().expect("a set"))
    };
    // Only what changes is sent: under `active` the completed row is taken
    // out, and nothing is added.
    let active = read("active", 1);
    assert_eq!(
        (removed(&active), inserted(&active)),
        (1, 0),
        "the completed todo goes: {active}"
    );
    // From active to completed: both active rows go, and the completed one,
    // as a row of its own (completed, checked), is shown.
    let completed = read("completed", 2);
    assert_eq!(
        (removed(&completed), inserted(&completed)),
        (2, 1),
        "{completed}"
    );
    assert!(completed.contains("feed the cat"), "{completed}");
    assert!(completed.contains("class=\\\"completed\\\""), "{completed}");
    // And back to all: the two active rows return, around the one kept.
    let all = read("all", 3);
    assert_eq!((removed(&all), inserted(&all)), (0, 2), "{all}");
    assert!(all.contains("buy some cheese"), "{all}");
    assert!(all.contains("book a doctors appointment"), "{all}");
    assert!(
        !all.contains("feed the cat"),
        "the completed row was kept: {all}"
    );
}

/// **The page's key names the row being edited**: the model says which row
/// shows its field, so the editing mode is the page's signal, never the
/// model's, and a read for another key shows none.
#[test]
fn the_pages_key_names_the_row_being_edited() {
    let s = served_todomvc();
    committed(&s, "add", "a", &[serde_json::json!("one")], "i-1");
    committed(&s, "add", "a", &[serde_json::json!("two")], "i-2");
    let ids: Vec<String> = held(&s, "a").into_iter().map(|(i, _, _)| i).collect();
    let _ = page(&s, "a");
    let doc = latest(&s.pending.lock().expect("pending"), "a");
    let key = |editing: &str| {
        BTreeMap::from([
            ("filter".to_string(), serde_json::json!("all")),
            ("editing".to_string(), serde_json::json!(editing)),
        ])
    };
    let outcome = s
        .read_keyed("a", "board", 1, doc.1, &key(&ids[1]), &|| false)
        .expect("read");
    assert!(matches!(outcome, KeyOutcome::Applied));
    let editing = format!("{:?}", sets_of(&s, &doc).last().expect("a set"));
    // The second row is shown again, as the row being edited: its class, and
    // its field holding the title, focused where it appears.
    assert!(editing.contains("class=\\\"editing\\\""), "{editing}");
    assert!(editing.contains("class=\\\"edit\\\""), "{editing}");
    assert!(editing.contains("autofocus value=\\\"two\\\""), "{editing}");
    assert!(
        !editing.contains("value=\\\"one\\\""),
        "only that row: {editing}"
    );
    // The control: for a key that names none, the row is shown again as it
    // was, with no field.
    let outcome = s
        .read_keyed("a", "board", 2, doc.1, &key(""), &|| false)
        .expect("read");
    assert!(matches!(outcome, KeyOutcome::Applied));
    let none = format!("{:?}", sets_of(&s, &doc).last().expect("a set"));
    assert!(!none.contains("class=\\\"edit\\\""), "{none}");
    assert!(!none.contains("class=\\\"editing\\\""), "{none}");
    assert!(none.contains("class=\\\"\\\""), "the row, plain: {none}");
}

/// **The page asks for what the spec's markup has**: the template's classes,
/// which the official tests' selectors are written over.
#[test]
fn the_page_has_the_templates_markup() {
    let s = served_todomvc();
    committed(&s, "add", "a", &[serde_json::json!("one")], "i-1");
    let html = page(&s, "a");
    for marker in [
        "class=\"todoapp\"",
        "class=\"header\"",
        "class=\"new-todo\"",
        "class=\"main\"",
        "class=\"toggle-all\"",
        "class=\"todo-list\"",
        "class=\"toggle\"",
        "class=\"destroy\"",
        "class=\"footer\"",
        "class=\"todo-count\"",
        "class=\"filters\"",
        "class=\"clear-completed\"",
        "class=\"info\"",
    ] {
        assert!(html.contains(marker), "{marker} in {html}");
    }
    // The new-todo input is focused when the page is loaded, "preferably by
    // using the `autofocus` input attribute".
    assert!(
        tag_of(&html, "class=\"new-todo\"").contains(" autofocus"),
        "{html}"
    );
    // The filters' three links, the default selected.
    assert!(html.contains("href=\"#/\""), "{html}");
    assert!(html.contains("href=\"#/active\""), "{html}");
    assert!(html.contains("href=\"#/completed\""), "{html}");
    assert!(
        tag_of(&html, "href=\"#/\"").contains("class=\"selected\""),
        "{html}"
    );
    // And TodoMVC's own stylesheets, which this host serves.
    assert!(html.contains("/todomvc-app.css"), "{html}");
}
