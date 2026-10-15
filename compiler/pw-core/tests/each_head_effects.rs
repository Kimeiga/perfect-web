//! **An `{#each}`'s head is read as what it is: expressions the template
//! evaluates** (ADR-XXXX, the each-head-effects track).
//!
//! The list a loop runs over and the key that tells its rows apart are
//! evaluated when the view renders, as a text hole, an attribute or an
//! `{#if}`'s condition is. Until this ADR they were kept as text (ADR-0242's
//! "not claimed"), so no walker of a view's expressions saw them, and on
//! 2026-10-15 each of these checked clean:
//! - a view declared `!{}` reading the database in its loop's list,
//!   `{#each items() as item}`, where the same call in a `let`, a text hole,
//!   an `{#if}` or an attribute is PW0401 (charter §7.4);
//! - the same through a member, `{#each store.items as item}`, where `items`
//!   reads the database (a member read is a call, ADR-0048): `pw build`
//!   compiled it too;
//! - the same in a loop's key, `(item.label)`, and in a nested loop;
//! - a secret rendered through a computed list, `{#each [key] as k}`, where
//!   the same list in a `let` is PW5003;
//! - a member's read in a text hole, `{item.label}`, where the loop's rows
//!   are a computed list's: they had no type, so no member was found.
//!
//! And two readings the text gave wrongly: a dialog in a loop over a
//! module's signal was "shown by nothing", and Marko wrote a list read from
//! a parameter as a name of its own.
//!
//! Each test states one case, with its control; `a_loop_over_what_a_view_
//! may_read_is_accepted` holds the neighbours that must stay accepted, so the
//! fix does not over-refuse.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

/// Each diagnostic on `t.pw`, as its code and the text its primary span
/// covers: where it is reported is part of what is claimed.
fn reported(src: &str) -> Vec<(String, String)> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let at = src.get(d.primary_span.clone()).unwrap_or("?").to_string();
                (d.code.to_string(), at)
            })
        })
        .collect()
}

fn one(code: &str, at: &str) -> Vec<(String, String)> {
    vec![(code.to_string(), at.to_string())]
}

const NOTHING: Vec<(String, String)> = Vec::new();

/// A module whose `items()` reads the database, and a view `Menu` with
/// `{markup}` as its body.
fn reading(markup: &str) -> String {
    format!(
        "module t\n\n\
         type Item = Item {{ name: String }}\n\n\
         fn items() -> List<Item> !{{ database.read<Item> }}\n    host \"shop:data/items#all\"\n\n\
         view Menu(given: List<Item>) !{{}} {{\n{markup}\n}}\n"
    )
}

#[test]
fn an_effectful_call_in_an_each_head_is_refused_at_the_head() {
    let src = reading(
        "    <ul>\n        {#each items() as item (item.name)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>",
    );
    assert_eq!(reported(&src), one("PW0401", "items()"));
    // The control: the loop over what the view is given.
    assert_eq!(reported(&src.replace("items() as", "given as")), NOTHING);
    // And the same call in a `let`, which was refused before: one reading.
    let let_ = reading(
        "    let all = items()\n    <ul>\n        {#each all as item (item.name)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>",
    );
    assert_eq!(reported(&let_), one("PW0401", "items()"));
}

/// A module with a `Store` whose member `items` is `{row}`, and a view over
/// one store's items with `{markup}` as its body.
fn member(row: &str, markup: &str) -> String {
    format!(
        "module t\n\n\
         type Item = Item {{ name: String }}\n\
         type Store = Store {{ id: Int }}\n\n\
         fn items(s: Store) -> List<Item> {row}\n\n\
         fn label(i: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n\
         view Menu(store: Store, given: List<Item>) !{{}} {{\n{markup}\n}}\n"
    )
}

const READS: &str = "!{ database.read<Item> }\n    host \"shop:data/items#of\"";
const PURE: &str = "!{} {\n    []\n}";

#[test]
fn a_list_read_through_an_effectful_member_is_refused() {
    let markup = "    <ul>\n        {#each store.items as item (item.name)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>";
    assert_eq!(
        reported(&member(READS, markup)),
        one("PW0401", "store.items")
    );
    // The control: the same member, pure.
    assert_eq!(reported(&member(PURE, markup)), NOTHING);
}

#[test]
fn an_effect_in_an_each_key_is_refused_at_the_key() {
    let markup = "    <ul>\n        {#each given as item (item.label)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>";
    assert_eq!(reported(&member(PURE, markup)), one("PW0401", "item.label"));
    // The control: a key read from the row's own field.
    assert_eq!(
        reported(&member(
            PURE,
            &markup.replace("(item.label)", "(item.name)")
        )),
        NOTHING
    );
    // And the same member in a text hole, which was refused before.
    let hole = "    <ul>\n        {#each given as item (item.name)}\n            <li>{item.label}</li>\n        {/each}\n    </ul>";
    assert_eq!(reported(&member(PURE, hole)), one("PW0401", "item.label"));
}

#[test]
fn an_effect_in_a_nested_each_head_is_refused() {
    let src = "module t\n\n\
               type Item = Item { name: String }\n\
               type Group = Group { title: String }\n\n\
               fn items(g: Group) -> List<Item> !{ database.read<Item> }\n    host \"shop:data/items#of\"\n\n\
               view Menu(groups: List<Group>) !{} {\n    <ul>\n        {#each groups as g (g.title)}\n            <li>\n                {#each g.items as item (item.name)}\n                    <p>{item.name}</p>\n                {/each}\n            </li>\n        {/each}\n    </ul>\n}\n";
    assert_eq!(reported(src), one("PW0401", "g.items"));
    // The control: the inner loop over a pure member.
    let pure = src.replace(
        "!{ database.read<Item> }\n    host \"shop:data/items#of\"",
        "!{} {\n    []\n}",
    );
    assert_eq!(reported(&pure), NOTHING);
}

/// A view given a secret and a list of them, with `{markup}` as its body.
fn secret(markup: &str) -> String {
    format!(
        "module t\n\n\
         import capability.{{ Secret, Payments }}\n\
         import List\n\n\
         view Keys(key: Secret<Payments>, keys: List<Secret<Payments>>, names: List<String>) !{{}} {{\n{markup}\n}}\n"
    )
}

#[test]
fn a_secret_reached_through_a_computed_list_is_refused() {
    let loop_over = |list: &str| {
        secret(&format!(
            "    <ul>\n        {{#each {list} as k (k)}}\n            <li>{{k}}</li>\n        {{/each}}\n    </ul>"
        ))
    };
    // A list written as a name was refused before; computed, it was not.
    for list in ["keys", "[key]", "List.reverse(keys)"] {
        let found = reported(&loop_over(list));
        assert!(
            found.iter().any(|(c, at)| c == "PW5003" && at == "k"),
            "`{{#each {list} as k}}` renders a secret: {found:?}"
        );
    }
    // The control: a public list, written and computed.
    for list in ["names", "List.reverse(names)"] {
        assert_eq!(reported(&loop_over(list)), NOTHING, "{list}");
    }
}

/// **The neighbours stay accepted**: a loop over a query's value, over a pure
/// function's result, and over a pure member.
#[test]
fn a_loop_over_what_a_view_may_read_is_accepted() {
    let query = "module t\n\n\
                 type Item = Item { name: String }\n\n\
                 fn all_items() -> List<Item> !{ database.read<Item> }\n    host \"shop:data/items#all\"\n\n\
                 public query Items() -> List<Item> {\n    all_items()\n}\n\n\
                 page Menu() {\n    route \"/menu\"\n\n    let items = query Items()\n\n    view {\n        <title>Menu</title>\n        <main>\n            <ul>\n                {#each items as item (item.name)}\n                    <li>{item.name}</li>\n                {/each}\n            </ul>\n        </main>\n    }\n}\n";
    assert_eq!(reported(query), NOTHING);
    let pure = "module t\n\n\
                import List\n\n\
                type Item = Item { name: String }\n\n\
                view Menu(items: List<Item>) !{} {\n    <ul>\n        {#each List.reverse(items) as item (item.name)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>\n}\n";
    assert_eq!(reported(pure), NOTHING);
    let markup = "    <ul>\n        {#each store.items as item (item.name)}\n            <li>{item.name}</li>\n        {/each}\n    </ul>";
    assert_eq!(reported(&member(PURE, markup)), NOTHING);
}

/// **A computed list's rows have its elements' type**, where what computes
/// it names no type parameter: so a member read from a row is found, and
/// what it performs is the view's. Until 2026-10-15 only a path's rows were
/// typed, and `{item.label}`, a text hole every walker read, performed
/// nothing in a loop over `all(given)`.
#[test]
fn a_member_read_from_a_computed_lists_row_is_found() {
    let view = |hole: &str| {
        format!(
            "module t\n\n\
             type Item = Item {{ name: String }}\n\n\
             fn label(i: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n\
             fn all(given: List<Item>) -> List<Item> !{{}} {{\n    given\n}}\n\n\
             view Menu(given: List<Item>) !{{}} {{\n    <ul>\n        \
             {{#each all(given) as item (item.name)}}\n            <li>{{{hole}}}</li>\n        \
             {{/each}}\n    </ul>\n}}\n"
        )
    };
    assert_eq!(reported(&view("item.label")), one("PW0401", "item.label"));
    // The control: a field of the row.
    assert_eq!(reported(&view("item.name")), NOTHING);
}

/// Each diagnostic on `t.pw`, as its code and its message.
fn said(src: &str) -> Vec<String> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// **A loop over a module's signal shows what it renders** (ADR-0141,
/// ADR-0144), as a loop over a view's own signal does: the list is read as
/// a block's subject is. Until 2026-10-15 its first name was looked up among
/// the view's own bindings, and a dialog in a loop over a module's signal
/// was refused as shown by nothing.
#[test]
fn a_dialog_in_a_loop_over_a_modules_signal_is_shown_by_it() {
    let notes = |list: &str, close: &str| {
        format!(
            "module t\n\nsignal notes: List<String>\n\n\
             view Notes(given: List<String>) !{{}} {{\n    <ul>\n        \
             {{#each {list} as n (n)}}\n            \
             <li><dialog aria-label=\"Note\"{close}><p>{{n}}</p></dialog></li>\n        \
             {{/each}}\n    </ul>\n}}\n"
        )
    };
    assert_eq!(
        said(&notes("notes", "")),
        vec![
            "PW5303 this `<dialog>` is shown while `notes` renders it, and its closing is heard \
             by nothing"
                .to_string()
        ]
    );
    // Its closing heard, it is the browser's modal dialog.
    assert_eq!(
        said(&notes("notes", " on:close={() => notes = []}")),
        Vec::<String>::new()
    );
    // The control: a loop over a list no signal holds shows nothing.
    let given = said(&notes("given", ""));
    assert_eq!(given.len(), 1, "{given:?}");
    assert!(
        given[0].starts_with("PW5303 this `<dialog>` is shown by nothing"),
        "{given:?}"
    );
}

/// **Marko reads a loop's list as any expression** (ADR-0017): one read from
/// a parameter is read from `input`, as every other use of one is. Until
/// 2026-10-15 the list was text, and `store.items` was written as a name of
/// its own, which the template does not have.
#[test]
fn marko_reads_a_list_from_a_parameter_from_input() {
    let src = "module m\n\ntype Item = Item { id: String, name: String }\n\
               type Store = Store { items: List<Item> }\n\n\
               view V(store: Store) !{} {\n    <ul>{#each store.items as item (item.id)}<li>{item.name}</li>{/each}</ul>\n}\n";
    let parsed = pw_syntax::parse_tree(src);
    let out = pw_core::marko::render_module(
        &pw_core::lower::lower_file(src, &parsed.green),
        "test.pw",
        &Default::default(),
    );
    assert!(out.skipped.is_empty(), "{:?}", out.skipped);
    let (_, text) = &out.files[0];
    assert!(
        text.contains(r#"<for|item| of=input.store.items by="id">"#),
        "{text}"
    );
}
