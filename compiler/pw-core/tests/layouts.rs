//! **A page is shown in its layout** (ADR-0303): the markup and the
//! bindings the pages that name it share, composed into each page as a view
//! is (ADR-0136), its parts and elements numbered first, so they are the same
//! on every page, and the page's markup placed in its `<slot />`.
//!
//! Each test states one part, with its control.

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

fn plan<'b>(b: &'b pw_core::build::Build, page: &str) -> &'b pw_core::page_values::PageValues {
    b.pages
        .iter()
        .find(|p| p.page == page)
        .and_then(|p| p.plan.as_ref().ok())
        .unwrap_or_else(|| panic!("no plan for `{page}`"))
}

/// The header the store's pages share, in module `t`: the reader's cart's
/// (the user's since track `store-accounts`, as the store's pages read it)
/// count, live.
const LAYOUT: &str = "user layout Shell {\n    \
    let cart = query Cart(current_user())\n\n    \
    view {\n        <header>\n            <a href=\"/\">Stores</a>\n            \
    <p>Cart: <span id=\"count\">{cart.line_count}</span></p>\n        </header>\n        \
    <main>\n            <slot />\n        </main>\n        \
    <footer><p>{cart.subtotal.display}</p></footer>\n    }\n}\n";

/// Module `t`: the layout, and two pages that name it, each with values of
/// its own.
fn program(layout: &str, pages: &str) -> String {
    format!(
        "module t\n\nimport store.page.{{ Store, Cart, Menu }}\nimport domain.{{ StoreId }}\n\
         import context.{{ current_user }}\n\n{layout}\n{pages}"
    )
}

const PAGES: &str = "user page A(id: StoreId) {\n    route  \"/a/{id}\"\n    \
    layout Shell\n    cache  private\n\n    let store = query Store(id)\n    \
    let cart = query Cart(current_user())\n\n    view {\n        \
    <title>{store.name}</title>\n        <h1>{store.name}</h1>\n        \
    <p>{cart.line_count} in your cart</p>\n    }\n}\n\n\
    user page B() {\n    route  \"/b\"\n    layout Shell\n    cache  private\n\n    \
    let cart = query Cart(current_user())\n\n    view {\n        \
    <title>Your cart</title>\n        <h1>Your cart</h1>\n        \
    {#each cart.lines as line (line.item_id)}<p>{line.name}</p>{/each}\n    }\n}\n";

/// A template as text: its markup, each part written `{#id}` where it is,
/// a block's arms inside it.
fn text(chunks: &[Chunk]) -> String {
    let mut out = String::new();
    for c in chunks {
        match c {
            Chunk::Static(s) => out.push_str(s),
            Chunk::Dynamic(p) => {
                out.push_str(&format!("{{#{}}}", p.id().map_or(u32::MAX, |i| i.0)));
                for inner in p.nested() {
                    out.push_str(&text(inner));
                }
            }
        }
    }
    out
}

/// What a page's template holds around its slot, and in it.
fn around(t: &Template) -> (String, String, String) {
    let all = text(&t.chunks);
    let (before, rest) = all
        .split_once(pw_core::layouts::SLOT_START)
        .unwrap_or_else(|| panic!("`{}` has no slot: {all}", t.path));
    let (inside, after) = rest
        .split_once(pw_core::layouts::SLOT_END)
        .unwrap_or_else(|| panic!("`{}`'s slot is not closed", t.path));
    (before.to_string(), inside.to_string(), after.to_string())
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

#[test]
fn a_page_is_shown_in_its_layout_whose_parts_are_numbered_first() {
    let src = program(LAYOUT, PAGES);
    assert_eq!(reported(&[("t.pw", &src)]), Vec::<String>::new());
    let b = built(&[("t.pw", &src)]);
    let (a, c) = (template(&b, "t.A"), template(&b, "t.B"));
    // The layout's markup, before the slot and after it, is the same on both
    // pages, its parts 0 and 1 in both.
    let (a_before, a_page, a_after) = around(a);
    let (c_before, c_page, c_after) = around(c);
    assert_eq!(a_before, c_before);
    assert_eq!(a_after, c_after);
    assert!(
        a_before.contains("<span id=\"count\">{#0}</span>"),
        "{a_before}"
    );
    assert!(
        a_after.contains("<footer><p>{#1}</p></footer>"),
        "{a_after}"
    );
    // Each page's own markup is in the slot, numbered after the layout's,
    // and its title after everything else (ADR-0183).
    assert!(a_page.contains("<h1>{#2}</h1>"), "{a_page}");
    assert!(c_page.contains("<h1>Your cart</h1>{#2}"), "{c_page}");
    for t in [a, c] {
        let ids: Vec<u32> = parts(&t.chunks)
            .iter()
            .filter_map(|p| p.id())
            .map(|i| i.0)
            .collect();
        let title = parts(&t.chunks)
            .iter()
            .find(|p| matches!(p, Part::Title { .. }))
            .and_then(|p| p.id())
            .map(|i| i.0);
        assert_eq!(title, ids.iter().max().copied(), "`{}`: {ids:?}", t.path);
    }
    // Control: a page that names no layout is as before, with no slot.
    let bare = PAGES.replace("    layout Shell\n", "");
    let b = built(&[("t.pw", &program(LAYOUT, &bare))]);
    let all = text(&template(&b, "t.A").chunks);
    assert!(!all.contains(pw_core::layouts::SLOT_START), "{all}");
    assert!(all.starts_with("<h1>{#0}</h1>"), "{all}");
}

#[test]
fn its_plan_reads_the_layouts_bindings_under_names_no_source_writes() {
    let b = built(&[("t.pw", &program(LAYOUT, PAGES))]);
    for page in ["t.A", "t.B"] {
        let plan = plan(&b, page);
        // The layout's binding first, as its markup is rendered first, under
        // a name with `~`, which no identifier holds; the page's own `cart`
        // beside it, read once each.
        let names: Vec<&str> = plan.bindings.iter().map(|b| b.binding.as_str()).collect();
        assert_eq!(names.first(), Some(&"cart~Shell"), "{page}: {names:?}");
        assert!(names.contains(&"cart"), "{page}: {names:?}");
        let theirs = &plan.bindings[0];
        assert_eq!(theirs.resource, "store.page.Cart");
        assert_eq!(theirs.args, ["current_user()"]);
        // Its parts read it by that name.
        let read: Vec<(u32, &str)> = plan
            .parts
            .iter()
            .map(|p| (p.part, p.binding.as_str()))
            .collect();
        assert!(read.contains(&(0, "cart~Shell")), "{page}: {read:?}");
        assert!(read.contains(&(1, "cart~Shell")), "{page}: {read:?}");
        // And the plan records the layout, the same for both pages.
        let layout = plan.layout.as_ref().expect("the layout is recorded");
        assert_eq!(layout.path, "t.Shell");
        assert_eq!((layout.parts, layout.elements), (2, 0));
    }
    assert_eq!(plan(&b, "t.A").layout, plan(&b, "t.B").layout);
    // Its schema is the layout's own markup's: another markup, another one.
    let changed = LAYOUT.replace("Stores</a>", "Every store</a>");
    let other = built(&[("t.pw", &program(&changed, PAGES))]);
    assert_ne!(
        plan(&other, "t.A").layout.as_ref().map(|l| &l.schema),
        plan(&b, "t.A").layout.as_ref().map(|l| &l.schema)
    );
    // Control: a page that names none records none.
    let bare = PAGES.replace("    layout Shell\n", "");
    let b = built(&[("t.pw", &program(LAYOUT, &bare))]);
    assert_eq!(plan(&b, "t.A").layout, None);
    assert!(
        plan(&b, "t.A")
            .bindings
            .iter()
            .all(|b| !b.binding.contains('~'))
    );
}

/// A layout with a signal of its own, a handler that sets it, and a block
/// it decides.
const MENU: &str = "user layout Shell {\n    \
    let cart = query Cart(current_user())\n    signal open: Bool = false\n\n    \
    view {\n        <header>\n            \
    <button type=\"button\" on:press={() => open = !open}>Menu</button>\n            \
    {#if open}<nav><a href=\"/b\">Your cart</a></nav>{/if}\n            \
    <p>Cart: <span id=\"count\">{cart.line_count}</span></p>\n        </header>\n        \
    <slot />\n    }\n}\n";

#[test]
fn a_layouts_signal_and_handler_are_its_pages_under_its_name() {
    let src = program(MENU, PAGES);
    assert_eq!(reported(&[("t.pw", &src)]), Vec::<String>::new());
    let b = built(&[("t.pw", &src)]);
    let (a, c) = (template(&b, "t.A"), template(&b, "t.B"));
    // The handler's element says which signal it sets, as a composed view's
    // does (ADR-0144), and the same on both pages.
    let (a_before, ..) = around(a);
    assert!(
        a_before.contains("data-pw-signals=\"{&quot;open&quot;:&quot;open~Shell&quot;}\""),
        "{a_before}"
    );
    assert_eq!(around(a).0, around(c).0);
    // The browser holds the signal by that name, at its first value.
    let plan = plan(&b, "t.A");
    let held: Vec<&str> = plan.signals.iter().map(|s| s.name.as_str()).collect();
    assert!(held.contains(&"open~Shell"), "{held:?}");
    // The handler is the layout's, one module whichever page holds it.
    let layout = b
        .handlers
        .iter()
        .filter(|h| h.declaration.ends_with("Shell"))
        .count();
    assert_eq!(
        layout,
        1,
        "{:?}",
        b.handlers
            .iter()
            .map(|h| &h.declaration)
            .collect::<Vec<_>>()
    );
    let events = |t: &Template| -> Vec<String> {
        parts(&t.chunks)
            .into_iter()
            .filter_map(|p| match p {
                Part::Event { handler, .. } => Some(handler.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(events(a), events(c));
    assert_eq!(events(a).len(), 1);
}

/// What `pw check` reports for a layout `layout`, and pages `pages`, in
/// module `t`: each diagnostic's code.
fn codes(layout: &str, pages: &str) -> Vec<String> {
    reported(&[("t.pw", &program(layout, pages))])
        .into_iter()
        .map(|d| d.split_once(' ').map_or(d.clone(), |(c, _)| c.to_string()))
        .collect()
}

#[test]
fn a_page_names_one_layout_and_a_layout_is_given_nothing() {
    // Control.
    assert_eq!(codes(LAYOUT, PAGES), Vec::<String>::new());
    // A layout that names nothing, or names a view.
    let none = PAGES.replace("layout Shell", "layout Nowhere");
    assert!(
        codes(LAYOUT, &none).contains(&"PW0352".to_string()),
        "{:?}",
        codes(LAYOUT, &none)
    );
    let view = format!("{LAYOUT}\nview Row() {{\n    <p>row</p>\n}}\n");
    let named_view = PAGES.replace("layout Shell", "layout Row");
    let said = reported(&[("t.pw", &program(&view, &named_view))]);
    assert!(
        said.iter()
            .any(|d| d.starts_with("PW0352") && d.contains("names a view")),
        "{said:?}"
    );
    // Two layouts.
    let twice = PAGES.replacen(
        "    layout Shell\n",
        "    layout Shell\n    layout Shell\n",
        1,
    );
    assert!(
        codes(LAYOUT, &twice).contains(&"PW0352".to_string()),
        "{:?}",
        codes(LAYOUT, &twice)
    );
    // A layout with parameters.
    let given = LAYOUT.replace("layout Shell {", "layout Shell(id: StoreId) {");
    assert!(
        codes(&given, PAGES).contains(&"PW0353".to_string()),
        "{:?}",
        codes(&given, PAGES)
    );
    // And with none, written `()`: control.
    let empty = LAYOUT.replace("layout Shell {", "layout Shell() {");
    assert_eq!(codes(&empty, PAGES), Vec::<String>::new());
}

#[test]
fn a_slot_is_its_layouts_once_at_the_top_and_empty() {
    let refused = |layout: &str, pages: &str, why: &str| {
        let said = codes(layout, pages);
        assert!(said.contains(&"PW5044".to_string()), "{why}: {said:?}");
    };
    // In a block, in a stream, with attributes, with content, twice.
    refused(
        &LAYOUT.replace("<slot />", "{#if cart.lines}<slot />{/if}"),
        PAGES,
        "in a block",
    );
    refused(
        &LAYOUT.replace(
            "<slot />",
            "<stream query={Cart(current_user())}><placeholder><p>…</p></placeholder>\
             <ready as={c}><slot /></ready><failed><p>-</p></failed></stream>",
        ),
        PAGES,
        "in a stream",
    );
    refused(
        &LAYOUT.replace("<slot />", "<slot name=\"main\" />"),
        PAGES,
        "with attributes",
    );
    refused(
        &LAYOUT.replace("<slot />", "<slot>x</slot>"),
        PAGES,
        "with content",
    );
    refused(
        &LAYOUT.replace("<slot />", "<slot /><slot />"),
        PAGES,
        "twice",
    );
    // In a page, and in a view.
    refused(
        LAYOUT,
        &PAGES.replace("<h1>Your cart</h1>", "<h1>Your cart</h1>\n        <slot />"),
        "in a page",
    );
    refused(
        &format!("{LAYOUT}\nview Row() {{\n    <p><slot /></p>\n}}\n"),
        PAGES,
        "in a view",
    );
    // None at all.
    let none = LAYOUT.replace("<slot />", "");
    assert!(
        codes(&none, PAGES).contains(&"PW5045".to_string()),
        "{:?}",
        codes(&none, PAGES)
    );
    // Control: one, inside an element.
    assert_eq!(codes(LAYOUT, PAGES), Vec::<String>::new());
}

#[test]
fn a_layout_takes_no_page_clause_and_no_other_declaration_names_one() {
    // A route, a cache, a placement or a layout of its own: the policy
    // table's (PW5105).
    for clause in [
        "route  \"/x\"",
        "cache  private",
        "placement origin",
        "layout Shell",
    ] {
        let with = LAYOUT.replace(
            "    let cart = query Cart(current_user())\n",
            &format!("    {clause}\n\n    let cart = query Cart(current_user())\n"),
        );
        let said = codes(&with, PAGES);
        assert!(said.contains(&"PW5105".to_string()), "{clause}: {said:?}");
    }
    // A view that names one.
    let view = format!("{LAYOUT}\nview Row() {{\n    layout Shell\n\n    <p>row</p>\n}}\n");
    assert!(
        codes(&view, PAGES).contains(&"PW5105".to_string()),
        "{:?}",
        codes(&view, PAGES)
    );
    // A title and a description are the page's (ADR-0183, ADR-0186).
    let titled = LAYOUT.replace("<header>", "<title>Shop</title>\n        <header>");
    let said = reported(&[("t.pw", &program(&titled, PAGES))]);
    assert!(
        said.iter()
            .any(|d| d.starts_with("PW5030") && d.contains("layout")),
        "{said:?}"
    );
}

#[test]
fn a_page_declares_at_least_its_layouts_audience() {
    // A page for anyone's eyes shown in a user's layout.
    let public = PAGES.replace("user page B()", "page B()").replace(
        "    let cart = query Cart(current_user())\n\n    view {\n        <title>Your cart",
        "\n    view {\n        <title>Your cart",
    );
    let public = public.replace(
        "{#each cart.lines as line (line.item_id)}<p>{line.name}</p>{/each}",
        "<p>Nothing here.</p>",
    );
    let public = public.replace(
        "    cache  private\n\n\n    view {\n        <title>Your cart",
        "\n    view {\n        <title>Your cart",
    );
    let said = reported(&[("t.pw", &program(LAYOUT, &public))]);
    assert!(said.iter().any(|d| d.starts_with("PW5046")), "{said:?}");
    // Control: a user's page shown in a layout for anyone.
    let open_layout = LAYOUT
        .replace("user layout Shell", "layout Shell")
        .replace("    let cart = query Cart(current_user())\n\n", "")
        .replace("{cart.line_count}", "-")
        .replace("{cart.subtotal.display}", "-");
    assert_eq!(codes(&open_layout, PAGES), Vec::<String>::new());
}

#[test]
fn what_a_layout_shows_is_its_pages_for_the_shared_cache() {
    // An unlabelled layout that reads a user's cart: a page that declares
    // a shared cache would hold every user's cart in it (PW5001), as a
    // read of its own would make it (charter §7.8).
    let unlabelled = LAYOUT.replace("user layout Shell", "layout Shell");
    let shared = "page C() {\n    route  \"/c\"\n    layout Shell\n    cache  shared\n\n    \
        view {\n        <title>About</title>\n        <h1>About</h1>\n    }\n}\n";
    let said = reported(&[("t.pw", &program(&unlabelled, shared))]);
    assert!(
        said.iter()
            .any(|d| d.starts_with("PW5001") && d.contains("`C`")),
        "{said:?}"
    );
    // Control: the same page, its layout reading nothing a user owns.
    let public = unlabelled
        .replace("    let cart = query Cart(current_user())\n\n", "")
        .replace("{cart.line_count}", "-")
        .replace("{cart.subtotal.display}", "-");
    let said = reported(&[("t.pw", &program(&public, shared))]);
    assert!(!said.iter().any(|d| d.starts_with("PW5001")), "{said:?}");
}

#[test]
fn a_page_reads_what_its_layout_reads_in_the_graph() {
    // The page `C` reads no cart of its own; its layout does, and a change to
    // the cart is the page's to be told of (ADR-0123).
    let about = "user page C() {\n    route  \"/c\"\n    layout Shell\n    cache  private\n\n    \
        view {\n        <title>About</title>\n        <h1>About</h1>\n    }\n}\n";
    let b = built(&[("t.pw", &program(LAYOUT, about))]);
    let reads: Vec<&str> = b
        .graph
        .edges
        .iter()
        .filter(|e| e.from == "t.C")
        .map(|e| e.to.as_str())
        .collect();
    assert!(reads.contains(&"store.page.Cart"), "{reads:?}");
    // Control: named in no layout, it reads nothing.
    let bare = about.replace("    layout Shell\n", "");
    let b = built(&[("t.pw", &program(LAYOUT, &bare))]);
    assert!(!b.graph.edges.iter().any(|e| e.from == "t.C"));
}

#[test]
fn layout_is_a_declaration_and_a_clause_only_where_a_name_follows() {
    use pw_syntax::SyntaxKind as K;
    let kinds = |src: &str| -> Vec<K> {
        parse_tree(src)
            .green
            .descendants()
            .map(|n| n.kind())
            .collect()
    };
    // `layout Shell { .. }` is a UI declaration, and `layout Shell` in a page
    // a clause.
    let decl = "layout Shell {\n    view {\n        <slot />\n    }\n}\n";
    assert!(kinds(decl).contains(&K::UiDecl), "{:?}", kinds(decl));
    let page = "page P() {\n    layout Shell\n\n    view {\n        <p>x</p>\n    }\n}\n";
    assert!(kinds(page).contains(&K::Policy), "{:?}", kinds(page));
    assert!(
        parse_tree(page).errors.is_empty(),
        "{:?}",
        parse_tree(page).errors
    );
    // Control: `layout.measure` written first in a body that takes clauses,
    // a view's, is an expression, an effect family's path, and no clause.
    // (A function's body takes none, so it would say nothing here.)
    let call = "view V() !{} {\n    layout.measure()\n    <p>x</p>\n}\n";
    assert!(!kinds(call).contains(&K::Policy), "{:?}", kinds(call));
    assert!(
        !kinds(call).contains(&K::UnknownPolicy),
        "{:?}",
        kinds(call)
    );
}

#[test]
fn a_layouts_computed_part_is_named_by_the_layout() {
    // A part a host computes is read by a path the compiler names
    // (ADR-0226): the layout's, so the same on every page.
    let counted = "fn counted(n: Int) -> String !{} {\n    if n == 1 {\n        \"1 item\"\n    \
        } else {\n        \"{n} items\"\n    }\n}\n";
    let layout = format!(
        "{counted}\n{}",
        LAYOUT.replace("{cart.subtotal.display}", "{counted(cart.line_count)}")
    );
    let src = program(&layout, PAGES);
    assert_eq!(reported(&[("t.pw", &src)]), Vec::<String>::new());
    let b = built(&[("t.pw", &src)]);
    let path = |page: &str| {
        plan(&b, page)
            .parts
            .iter()
            .find(|p| p.part == 1)
            .map(|p| p.path.clone())
    };
    assert_eq!(path("t.A").as_deref(), Some("#t.Shell~1"));
    assert_eq!(path("t.A"), path("t.B"));
}

/// A module of views that need a signal provided: `Open` sets `drawer`.
const UI: &str = "module ui\n\nsignal drawer: Bool\n\n\
    view Open() !{} {\n    <button type=\"button\" on:press={() => drawer = true}>Open</button>\n}\n";

#[test]
fn a_layout_provides_what_its_views_need_and_its_page_provides_it_nothing() {
    let with = |layout: &str, pages: &str| -> Vec<String> {
        let src = program(layout, pages).replace(
            "import context.{ current_user }\n",
            "import context.{ current_user }\nimport ui.{ drawer, Open }\n",
        );
        reported(&[("ui.pw", UI), ("t.pw", &src)])
    };
    let open = LAYOUT.replace(
        "<a href=\"/\">Stores</a>",
        "<a href=\"/\">Stores</a><Open />",
    );
    // Nothing provides it to the layout.
    let said = with(&open, PAGES);
    assert!(
        said.iter()
            .any(|d| d.starts_with("PW5305") && d.contains("`Shell`")),
        "{said:?}"
    );
    // A page that provides it gives the layout nothing.
    let given = PAGES.replace(
        "    let store = query Store(id)\n",
        "    provide drawer = false\n    let store = query Store(id)\n",
    );
    let said = with(&open, &given);
    assert!(
        said.iter()
            .any(|d| d.starts_with("PW5305") && d.contains("`Shell`")),
        "{said:?}"
    );
    // Control: the layout provides it, and its view sets the layout's.
    let provided = open.replace(
        "    let cart = query Cart(current_user())\n",
        "    let cart = query Cart(current_user())\n    provide drawer = false\n",
    );
    assert_eq!(with(&provided, PAGES), Vec::<String>::new());
    let src = program(&provided, PAGES).replace(
        "import context.{ current_user }\n",
        "import context.{ current_user }\nimport ui.{ drawer, Open }\n",
    );
    let b = built(&[("ui.pw", UI), ("t.pw", &src)]);
    let (before, ..) = around(template(&b, "t.A"));
    assert!(
        before.contains("data-pw-signals=\"{&quot;drawer&quot;:&quot;drawer~Shell&quot;}\""),
        "{before}"
    );
}

#[test]
fn a_layouts_count_is_speculated_with_the_pages() {
    // The store's page speculates on its cart when an item is added
    // (ADR-0122): its own count, and its layout's, so the page never shows
    // two counts while the command is out.
    let b = built(&[]);
    let store = b
        .speculations
        .iter()
        .find(|s| s.page == "store.page.StorePage")
        .expect("the store's page speculates");
    let pw_core::backend::wasm::Encoding::Encoded(module) = &store.module else {
        panic!("{:?}", store.module);
    };
    let bindings: Vec<&str> = module.bindings.iter().map(|b| b.binding.as_str()).collect();
    assert!(bindings.contains(&"cart"), "{bindings:?}");
    assert!(bindings.contains(&"cart~StoreLayout"), "{bindings:?}");
    // Both transitioned for each command that speculates on the cart.
    assert!(
        module
            .source
            .contains("binding: \"cart~StoreLayout\", transition"),
        "{}",
        module.source
    );
    assert!(
        module.source.contains("binding: \"cart\", transition"),
        "{}",
        module.source
    );
    // And the layout's count is a part it renders again.
    assert!(
        module.source.contains("\"cart~StoreLayout\": ["),
        "{}",
        module.source
    );
    // Control: the order's page, whose handlers speculate on nothing, has
    // no module, whatever its layout reads.
    assert!(
        !b.speculations
            .iter()
            .any(|s| s.page == "store.page.OrderPage")
    );
}
