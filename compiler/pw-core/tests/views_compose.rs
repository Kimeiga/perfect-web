//! **A view used in another is written where it is used** (ADR-0136).
//!
//! Until 2026-10-02 a view used in another, `<MenuRow item={item} />`, was
//! refused (PW5020, ADR-0072): no page could be built from views. A view now
//! composes. Its markup is lowered in place, in the page's one numbering, and
//! each of its parameters reads as the path its prop gives. A handler written
//! in a view keeps its own identity and module, and the template says where
//! the page holds what it captures. What composition writes into a page is
//! checked where the page gives it: whose value a handler captures, and
//! whether it is a signal.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::template_ir::{Chunk, Part, Template};
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

fn units(files: &[(&str, &str)]) -> Vec<Unit> {
    let mut sources = library();
    sources.extend(files.iter().map(|(n, s)| (n.to_string(), s.to_string())));
    sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect()
}

/// What `pw check` reports in `files`, each as `code message`.
fn reported(files: &[(&str, &str)]) -> Vec<String> {
    let mut sources = library();
    sources.extend(files.iter().map(|(n, s)| (n.to_string(), s.to_string())));
    let mine: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| mine.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn built(files: &[(&str, &str)]) -> pw_core::build::Build {
    let b = pw_core::build::build(&units(files)).expect("builds");
    assert!(b.refusals().is_empty(), "{:#?}", b.refusals());
    b
}

fn template<'b>(b: &'b pw_core::build::Build, path: &str) -> &'b Template {
    b.templates
        .iter()
        .find(|t| t.path == path)
        .unwrap_or_else(|| panic!("no template `{path}`"))
}

/// Every part of a template, in document order, nested ones included.
fn parts(chunks: &[Chunk]) -> Vec<&Part> {
    let mut out = Vec::new();
    for c in chunks {
        if let Chunk::Dynamic(p) = c {
            out.push(p);
            for inner in p.nested() {
                out.extend(parts(inner));
            }
        }
    }
    out
}

/// `views`, each `public`, in a module of their own that sees the store's
/// types: what a page of module `t` imports.
fn ui(views: &str) -> String {
    format!(
        "module ui\n\nimport domain.{{ Store, MenuItem, PositiveInt }}\n\
         import store.page.{{ add_to_cart, increase_in_cart }}\n\n{}",
        views.replace("view ", "public view ")
    )
}

/// A store's items as one list, for a page over them: the store's own `Menu`
/// is grouped by category since ADR-0181.
const ITEMS: &str = "import Menus\nimport Events.{ MenuChanged }\n\
    import domain.{ StoreError, MenuItem }\n\n\
    public query Items(id: StoreId) -> Result<List<MenuItem>, StoreError>\n    \
    freshness      5.minutes\n    consistency    snapshot\n    cache          shared\n    \
    key            id\n    invalidates_on MenuChanged(id)\n    timeout        2.seconds\n\
    {\n    Menus.for_store(id)\n}\n\n";

/// A page over the store's menu, in module `t`, whose view holds `markup`,
/// using the views `used` imports from `ui`.
fn page(used: &[&str], markup: &str) -> String {
    let import = match used {
        [] => String::new(),
        _ => format!("import ui.{{ {} }}\n", used.join(", ")),
    };
    format!(
        "module t\n\nimport store.page.{{ Store, Cart }}\nimport domain.{{ StoreId }}\n\
         import context.{{ current_user }}\n{import}{items}\n\
         page P(id: StoreId) {{\n    cache private\n\n    \
         let store = query Store(id)\n    let menu = query Items(id)\n    \
         let cart = query Cart(current_user())\n\n    \
         view {{\n        <main>\n{markup}\n        </main>\n    }}\n}}\n",
        items = ITEMS
    )
}

const LINE: &str =
    "view MenuLine(item: MenuItem) !{} {\n    <li><span>{item.name}</span></li>\n}\n";

#[test]
fn a_view_is_written_where_it_is_used() {
    let views = ui(LINE);
    let composed = page(
        &["MenuLine"],
        "            <ul>{#each menu as item (item.id)}<MenuLine item={item} />{/each}</ul>",
    );
    let inlined = page(
        &[],
        "            <ul>{#each menu as item (item.id)}<li><span>{item.name}</span></li>{/each}</ul>",
    );
    let files = [("ui.pw", views.as_str()), ("t.pw", composed.as_str())];
    assert_eq!(reported(&files), Vec::<String>::new());
    let a = built(&files);
    let b = built(&[("t.pw", &inlined)]);
    // One document: the same chunks, the same parts, the same schema.
    assert_eq!(template(&a, "t.P"), template(&b, "t.P"));
    // The view keeps a template of its own.
    assert!(a.templates.iter().any(|t| t.path == "ui.MenuLine"));
}

#[test]
fn a_view_used_twice_has_parts_of_its_own_each_time() {
    let views = ui("view Name(s: Store) !{} {\n    <span>{s.name}</span>\n}\n");
    let src = page(
        &["Name"],
        "            <h1><Name s={store} /></h1>\n            <p><Name s={store} /></p>",
    );
    let b = built(&[("ui.pw", &views), ("t.pw", &src)]);
    let found: Vec<(u32, String)> = template(&b, "t.P")
        .manifest()
        .into_iter()
        .map(|e| (e.id.0, e.value))
        .collect();
    assert_eq!(
        found,
        [(0, "store.name".to_string()), (1, "store.name".to_string())]
    );
}

#[test]
fn a_handler_in_a_view_captures_what_the_page_gave_it() {
    let views = ui("view AddButton(entry: MenuItem) !{} {\n    \
         <button type=\"button\" on:press={() => { let _added = increase_in_cart(entry.id) }}>More</button>\n}\n");
    // The page shows the cart `increase_in_cart` speculates on (ADR-0120).
    let src = page(
        &["AddButton"],
        "            <ul>{#each menu as item (item.id)}<li><AddButton entry={item} /></li>{/each}</ul>\n\
         \x20           <p>{cart.line_count}</p>",
    );
    let files = [("ui.pw", views.as_str()), ("t.pw", src.as_str())];
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = built(&files);
    let t = template(&b, "t.P");
    let Some(Part::Event {
        handler,
        captures,
        renames,
        ..
    }) = parts(&t.chunks)
        .into_iter()
        .find(|p| matches!(p, Part::Event { .. }))
    else {
        panic!("an event part: {t:#?}");
    };
    // The view's handler: its identity, its module, its own names.
    assert!(
        b.handlers.iter().any(|h| matches!(
            &h.module,
            pw_core::backend::wasm::Encoding::Encoded(m) if m.identity == *handler
        )),
        "a module for `{handler}`"
    );
    assert_eq!(captures, &["entry.id"]);
    // And where the page holds `entry`: its loop's `item`.
    assert_eq!(
        renames.iter().collect::<Vec<_>>(),
        [(&"entry".to_string(), &"item".to_string())]
    );

    // Control: a page whose name is the view's has nothing to rename.
    let same = src
        .replace("as item (item.id)", "as entry (entry.id)")
        .replace("<AddButton entry={item} />", "<AddButton entry={entry} />");
    let b = built(&[("ui.pw", &views), ("t.pw", &same)]);
    let t = template(&b, "t.P");
    assert!(
        parts(&t.chunks)
            .into_iter()
            .any(|p| matches!(p, Part::Event { renames, .. } if renames.is_empty())),
        "{t:#?}"
    );
}

/// `Outer`'s template, where `Tags` is used inside a loop over `dishes`.
fn tags(tags_view: &str, used: &str) -> Template {
    let src = format!(
        "module t\n\ntype Tag = Tag {{ name: String }}\n\
         type Dish = Dish {{ title: String, tags: List<Tag>, extra: Option<Tag> }}\n\n{tags_view}\n\
         view Outer(dishes: List<Dish>) !{{}} {{\n    <div>{used}</div>\n}}\n"
    );
    assert_eq!(reported(&[("t.pw", &src)]), Vec::<String>::new(), "{src}");
    let hirs: Vec<pw_core::hir::Hir> = vec![lower_file(&src, &parse_tree(&src).green)];
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    pw_core::template_ir::build(&refs)
        .into_iter()
        .find(|t| t.path == "t.Outer")
        .expect("Outer")
}

/// Each part's kind and what it reads, binds or carries, in document order.
fn shape(t: &Template) -> Vec<String> {
    parts(&t.chunks)
        .into_iter()
        .map(|p| match p {
            Part::Text { value, .. } => format!("text {value}"),
            Part::Each {
                collection,
                binding,
                ..
            } => format!("each {collection} as {binding}"),
            Part::Match { value, arms, .. } => format!(
                "match {value}: {}",
                arms.iter()
                    .map(|a| a.binding.clone().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            other => other.kind().to_string(),
        })
        .collect()
}

#[test]
fn a_name_the_view_binds_hides_nothing_it_was_given() {
    let view = "view Tags(item: Dish) !{} {\n    \
                <ul>{#each item.tags as entry (entry.name)}<li>{entry.name} of {item.title}</li>{/each}</ul>\n}\n";
    // The page's `entry` is given as `item`, and the view binds `entry`
    // itself: written in place as it is, `{item.title}` would read the tag's.
    let t = tags(
        view,
        "{#each dishes as entry (entry.title)}<Tags item={entry} />{/each}",
    );
    assert_eq!(
        shape(&t),
        [
            "each dishes as entry",
            "each entry.tags as entry~1",
            "text entry~1.name",
            "text entry.title",
        ]
    );
    // Control: where nothing is hidden, nothing is renamed.
    let t = tags(
        view,
        "{#each dishes as dish (dish.title)}<Tags item={dish} />{/each}",
    );
    assert_eq!(
        shape(&t),
        [
            "each dishes as dish",
            "each dish.tags as entry",
            "text entry.name",
            "text dish.title",
        ]
    );
    // An arm's name, the same way.
    let arm = "view Extra(item: Dish) !{} {\n    \
               <p>{#match item.extra}{:Some(entry)}{entry.name} on {item.title}{:None}none{/match}</p>\n}\n";
    let t = tags(
        arm,
        "{#each dishes as entry (entry.title)}<Extra item={entry} />{/each}",
    );
    assert_eq!(
        shape(&t),
        [
            "each dishes as entry",
            "match entry.extra: entry~1,",
            "text entry~1.name",
            "text entry.title",
        ]
    );
    // A name the view binds over its own parameter's is the view's.
    let own = "view Own(item: Dish) !{} {\n    \
               <ul>{#each item.tags as item (item.name)}<li>{item.name}</li>{/each}</ul>\n}\n";
    let t = tags(
        own,
        "{#each dishes as dish (dish.title)}<Own item={dish} />{/each}",
    );
    assert_eq!(
        shape(&t),
        [
            "each dishes as dish",
            "each dish.tags as item",
            "text item.name"
        ]
    );
}

#[test]
fn a_page_plans_a_composed_view_as_its_own_markup() {
    let views = ui("view Header(s: Store) !{} {\n    <h1>{s.name}</h1>\n}\n");
    let composed = page(&["Header"], "            <Header s={store} />");
    let inlined = page(&[], "            <h1>{store.name}</h1>");
    let plan = |src: &str| {
        let b = built(&[("ui.pw", &views), ("t.pw", src)]);
        b.pages
            .into_iter()
            .find(|p| p.page == "t.P")
            .expect("a plan")
            .plan
            .expect("planned")
    };
    let (a, b) = (plan(&composed), plan(&inlined));
    assert_eq!(a.parts, b.parts);
    assert_eq!(a.parts[0].path, "store.name");
}

/// The store's page, its cart's count and its Add buttons written `inline`,
/// or as views from `ui`.
fn cart_count_page(inline: bool) -> (String, String) {
    // The store's cart is a user's, `UserCart`, since track `store-accounts`.
    let views = format!(
        "{}\nimport UserCarts.{{ UserCart }}\n\n\
         public view CartCount(c: UserCart) !{{}} {{\n    <p id=\"cart-count\">{{c.line_count}}</p>\n}}\n\n\
         public view AddButton(entry: MenuItem) !{{}} {{\n    \
         <button type=\"button\" on:press={{() => {{ let _added = add_to_cart(entry, PositiveInt(1)) }}}}>Add</button>\n}}\n",
        ui("")
    );
    let (count, add) = match inline {
        true => (
            "<p id=\"cart-count\">{cart.line_count}</p>",
            "<button type=\"button\" on:press={() => { let _added = add_to_cart(item, PositiveInt(1)) }}>Add</button>",
        ),
        false => ("<CartCount c={cart} />", "<AddButton entry={item} />"),
    };
    let imports = match inline {
        true => "import store.page.{ add_to_cart }\nimport domain.{ PositiveInt }\n",
        false => "import ui.{ CartCount, AddButton }\n",
    };
    let page = format!(
        "module t\n\nimport store.page.{{ Cart }}\nimport domain.{{ StoreId }}\n\
         import context.{{ current_user }}\n{imports}{items}\n\
         page P(id: StoreId) {{\n    cache private\n\n    \
         let menu = query Items(id)\n    let cart = query Cart(current_user())\n\n    \
         view {{\n        <main>\n            {count}\n            \
         <ul>{{#each menu as item (item.id)}}<li>{add}</li>{{/each}}</ul>\n        </main>\n    }}\n}}\n",
        items = ITEMS
    );
    (views, page)
}

#[test]
fn a_view_s_part_and_handler_are_the_page_s_to_speculate() {
    // ADR-0122: pressing Add moves the count before the server answers. A
    // page's speculation counted the handlers declared in the page alone, so
    // an Add button written in a view, or a count shown by one, would have
    // been a declared optimistic update that did nothing.
    let module = |inline: bool| {
        let (views, page) = cart_count_page(inline);
        let b = built(&[("ui.pw", &views), ("t.pw", &page)]);
        let s = b
            .speculations
            .iter()
            .find(|s| s.page == "t.P")
            .unwrap_or_else(|| panic!("no speculation for `t.P` (inline: {inline})"));
        match &s.module {
            pw_core::backend::wasm::Encoding::Encoded(m) => m.clone(),
            other => panic!("{other}"),
        }
    };
    let (inline, composed) = (module(true), module(false));
    assert_eq!(composed.commands, inline.commands);
    assert_eq!(composed.bindings, inline.bindings);
    let parts = |source: &str| {
        source
            .split("export const parts = {")
            .nth(1)
            .and_then(|r| r.split("};").next())
            .map(|p| p.replace(char::is_whitespace, ""))
            .unwrap_or_default()
    };
    assert_eq!(parts(&composed.source), parts(&inline.source));
    assert!(
        parts(&inline.source).contains("\"cart\":{\"0\":f"),
        "{}",
        inline.source
    );
}

#[test]
fn what_cannot_be_composed_is_refused_by_name() {
    for (views, used, expected) in [
        (
            "view Own(n: Int) !{} {\n    let m = n\n    <p>{m}</p>\n}\n",
            "<Own n={k} />",
            "PW5020 `<Own>` declares values of its own",
        ),
        (
            "view Loop(n: Int) !{} {\n    <p><Loop n={n} /></p>\n}\n",
            "<Loop n={k} />",
            "PW5020 `<Loop>` contains itself",
        ),
        (
            "view A(n: Int) !{} {\n    <p><B n={n} /></p>\n}\nview B(n: Int) !{} {\n    <p><A n={n} /></p>\n}\n",
            "<A n={k} />",
            "PW5020 `<A>` contains itself",
        ),
        (
            "page Q() {\n    view { <p>q</p> }\n}\n",
            "<Q />",
            "PW5020 `<Q>` uses `Q` inside another view, and only a view is used as an element",
        ),
    ] {
        let src =
            format!("module t\n\n{views}\nview V(k: Int) !{{}} {{\n    <div>{used}</div>\n}}\n");
        let found = reported(&[("t.pw", &src)]);
        assert!(
            found.iter().any(|d| d.starts_with(expected)),
            "{used}: {found:#?}"
        );
    }
}

#[test]
fn a_view_that_cannot_compose_is_reported_where_it_is_used() {
    // `Outer` composes; `Inner`, which it uses, does not. The problem is
    // `Inner`'s, reported where `Inner` is used, and not again at each use of
    // `Outer` around it.
    let src = "module t\n\nview Inner(n: Int) !{} {\n    let m = n\n    <p>{m}</p>\n}\n\n\
               view Outer(n: Int) !{} {\n    <div><Inner n={n} /></div>\n}\n\n\
               view V(k: Int) !{} {\n    <main><Outer n={k} /><Outer n={k} /></main>\n}\n";
    let found = reported(&[("t.pw", src)]);
    assert_eq!(
        found,
        [
            "PW5020 `<Inner>` declares values of its own, and a view composed into another holds \
          its markup and its signals alone"
        ],
        "{found:#?}"
    );
}

/// R-030's program (`boundary_matrix.rs`): a `Cart` is the session's,
/// because only a `session query` makes one. A page declared `head`, with
/// `policy` in its body, whose handler captures its `cart`, written `inline`
/// or in a `session view`, which may hold it.
fn cart_page(inline: bool, head: &str, policy: &str) -> String {
    let button = "<button on:press={resumable(captures = { cart }) => cart.line_count}>go</button>";
    let (view, used) = match inline {
        true => (String::new(), button.to_string()),
        false => (
            format!("session view Summary(cart: Cart) !{{}} {{\n    {button}\n}}\n"),
            "<Summary cart={cart} />".to_string(),
        ),
    };
    format!(
        "module t\n\ntype Cart = Cart {{ line_count: Int }}\ntype CartError = CartError {{ why: String }}\n\
         opaque type SessionId = String\n\n\
         session query Basket(s: SessionId) -> Result<Cart, CartError>\n    cache private\n{{\n    todo\n}}\n\n\
         {view}\n{head} {{\n    {policy}\n    view {{ {used} }}\n}}\n"
    )
}

#[test]
fn what_a_view_captures_is_checked_where_it_is_given() {
    let codes = |src: &str| -> Vec<String> {
        let mut found: Vec<String> = reported(&[("t.pw", src)])
            .into_iter()
            .map(|d| d.split(' ').next().unwrap_or_default().to_string())
            .collect();
        found.sort();
        found
    };
    // The session's value, into a document private to nobody it names
    // (PW5018), and into the shared shell (PW5007): each refused where the
    // view is used, as the same handler written in the page is. The view is
    // `session` and may hold it itself, so its own check passes; what goes
    // into the page is the page's to decide.
    for (policy, expected) in [("cache private", "PW5018"), ("", "PW5007")] {
        let head = "page Checkout(cart: Cart)";
        let inline = codes(&cart_page(true, head, policy));
        assert!(inline.contains(&expected.to_string()), "{inline:?}");
        let composed = cart_page(false, head, policy);
        assert_eq!(codes(&composed), inline, "{composed}");
    }
    // Control: a page private to its session may hold it, either way.
    for inline in [true, false] {
        let src = cart_page(inline, "session page Checkout(cart: Cart)", "cache private");
        assert_eq!(codes(&src), Vec::<String>::new(), "{src}");
    }
    // Through the view's own loop: its handler captures an item of the list
    // it was given, and only a session query makes such a list.
    let lines = |capture: &str, reads: &str| {
        format!(
            "module t\n\ntype Line = Line {{ n: Int }}\ntype CartError = CartError {{ why: String }}\n\
             opaque type SessionId = String\n\n\
             session query Basket(s: SessionId) -> Result<List<Line>, CartError>\n    cache private\n{{\n    todo\n}}\n\n\
             session view Lines(lines: List<Line>) !{{}} {{\n    \
             <ul>{{#each lines as line (line.n)}}<li><button on:press={{resumable({capture}) => {reads}}}>go</button></li>{{/each}}</ul>\n}}\n\n\
             page Checkout(lines: List<Line>) {{\n    cache private\n    view {{ <Lines lines={{lines}} /> }}\n}}\n"
        )
    };
    let found = codes(&lines("captures = { line }", "line.n"));
    assert!(found.contains(&"PW5018".to_string()), "{found:?}");
    // Control: a handler that captures nothing of the list writes nothing of
    // it into the page.
    assert_eq!(codes(&lines("", "1")), Vec::<String>::new());
}

#[test]
fn a_signal_a_view_shows_is_live_as_the_page_s_own() {
    // ADR-0130: the browser renders again exactly the parts a signal decides.
    // Shown through a view, the part is the page's: in the same place, read
    // by the same path, at top level and inside a block the signal decides.
    let src = |inline: bool| {
        let (views, title, help) = match inline {
            true => ("", "<h1>{greeting}</h1>", "<p>{greeting} {presses}</p>"),
            false => (
                "view Title(text: String) !{} {\n    <h1>{text}</h1>\n}\n\
                 view Help(text: String, n: Int) !{} {\n    <p>{text} {n}</p>\n}\n",
                "<Title text={greeting} />",
                "<Help text={greeting} n={presses} />",
            ),
        };
        format!(
            "module t\n\ntype Panel = Shut | Open\n\n{views}\n\
             page P() {{\n    cache private\n\n    signal panel: Panel = Panel.Shut\n    \
             signal greeting: String = \"Hello\"\n    signal presses: Int = 0\n\n    \
             view {{\n        <main>{title}\
             <button type=\"button\" on:press={{() => presses = presses + 1}}>Press</button>\
             <button type=\"button\" on:press={{() => panel = Panel.Open}}>Open</button>\
             {{#match panel}}{{:Shut}}<p>shut</p>{{:Open}}{help}{{/match}}</main>\n    }}\n}}\n"
        )
    };
    let plan = |inline: bool| {
        let src = src(inline);
        assert_eq!(reported(&[("t.pw", &src)]), Vec::<String>::new(), "{src}");
        built(&[("t.pw", &src)])
            .pages
            .into_iter()
            .find(|p| p.page == "t.P")
            .expect("a plan")
            .plan
            .expect("planned")
    };
    let (inline, composed) = (plan(true), plan(false));
    assert_eq!(composed.live, inline.live);
    let kinds: Vec<(&str, &str)> = inline
        .live
        .iter()
        .map(|l| (l.kind.as_str(), l.path.as_str()))
        .collect();
    // The block is rendered again for `panel`, and the parts in it that read
    // the other two are set in place (ADR-0142).
    assert_eq!(
        kinds,
        [
            ("text", "greeting"),
            ("match", "panel"),
            ("text", "greeting"),
            ("text", "presses")
        ]
    );
    assert_eq!(inline.live[1].reads, ["panel"]);
}

#[test]
fn a_signal_a_view_captures_is_refused() {
    let src = |view: &str| {
        format!(
            "module t\n\n{view}\npage P() {{\n    cache private\n\n    signal count: Int = 0\n\n    \
             view {{\n        <main><button type=\"button\" on:press={{() => count = count + 1}}>Up</button>\
             <Count n={{count}} /></main>\n    }}\n}}\n"
        )
    };
    // A handler in the view captures `n`: it would read the count the page
    // was rendered with, however often it was pressed since.
    let captured = "view Count(n: Int) !{} {\n    \
                    <button type=\"button\" on:press={() => log(n)}>Log</button>\n}\n";
    let found = reported(&[("t.pw", &src(captured))]);
    assert!(
        found.iter().any(|d| d
            .starts_with("PW5301 `count` is a signal, and `<Count>`'s handler captures it as `n`")),
        "{found:#?}"
    );
    // Control: a view that shows the signal reads it where the browser reads
    // it again.
    let shown = "view Count(n: Int) !{} {\n    <p>{n}</p>\n}\n";
    let found = reported(&[("t.pw", &src(shown))]);
    assert!(found.is_empty(), "{found:#?}");
}
