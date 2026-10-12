//! **A name means the binding it is written in** (ADR-0063), in every
//! analysis: one a template, a `let` or a lambda binds is that binding,
//! whatever a module or an import declares by the same spelling. Each test
//! states one case, with a control.

use pw_core::check::check_sources;

fn program(src: &[(&str, &str)]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
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
    for (name, text) in src {
        out.push((name.to_string(), text.to_string()));
    }
    out
}

/// Each diagnostic reported in the test's own files, as `CODE message`.
fn reported(src: &[(&str, &str)]) -> Vec<String> {
    let own: Vec<&str> = src.iter().map(|(n, _)| *n).collect();
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| own.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A module whose function `estimate` reads the clock.
const ESTIMATES: &str = "module est\n\nimport clock\n\n\
    fn estimate() -> Int !{ clock.wall } {\n    clock.now_wall()\n}\n";

/// A page built once that streams a query's answer and names it `name`
/// where it is ready: a page built once may not read the clock, so a part
/// charged `fn estimate`'s row is refused.
fn streamed(name: &str) -> String {
    format!(
        "module t\n\nimport est.{{ estimate }}\n\n\
         query Waited() -> Int\n    freshness   0.seconds\n    consistency snapshot\n    \
         cache       shared\n    delivery    streamed\n    timeout     2.seconds\n{{\n    1\n}}\n\n\
         page P() {{\n    placement build\n    cache     shared\n\n    view {{\n        <main>\n            \
         <stream query={{Waited()}}>\n                <placeholder><p>Waiting</p></placeholder>\n                \
         <ready as={{{name}}}><p>{{{name}}}</p></ready>\n                \
         <failed><p>Not now</p></failed>\n            </stream>\n        </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_stream_parts_name_is_the_answer_not_a_namesake_function() {
    // W8's finding (2026-10-09): `as={estimate}` was charged the imported
    // `fn estimate`'s row. A binding's name changes nothing a check says:
    // the page named `estimate` is reported as the page named `waited` is.
    let named = |name: &str| {
        reported(&[("est.pw", ESTIMATES), ("t.pw", &streamed(name))])
            .into_iter()
            .map(|d| d.replace(name, "NAME"))
            .collect::<Vec<_>>()
    };
    let shadowing = named("estimate");
    assert!(
        !shadowing.iter().any(|d| d.contains("clock.wall")),
        "{shadowing:#?}"
    );
    assert_eq!(shadowing, named("waited"));
}

/// A page built once that calls a local function value named `found`, where
/// a module function `found` reads the clock.
fn built_calling(local: &str) -> String {
    format!(
        "module t\n\nimport clock\n\n\
         fn found() -> Int !{{ clock.wall }} {{\n    clock.now_wall()\n}}\n\n\
         page P() {{\n    placement build\n    cache     shared\n\n    \
         let {local} = fn() 1\n    let n = {local}()\n    view {{ <main><p>{{n}}</p></main> }}\n}}\n"
    )
}

#[test]
fn a_call_through_a_local_is_not_charged_its_namesakes_row() {
    // ADR-0283's limitation: a local `found` that shadows `fn found` was
    // charged `fn found`'s row, and a page built once refused for it.
    let clock = |found: &[String]| found.iter().any(|d| d.contains("clock.wall"));
    let found = reported(&[("t.pw", &built_calling("found"))]);
    assert!(!clock(&found), "{found:#?}");
    // The control: the module's own `found`, called, is charged.
    let calls_the_module = "module t\n\nimport clock\n\n\
         fn found() -> Int !{ clock.wall } {\n    clock.now_wall()\n}\n\n\
         page P() {\n    placement build\n    cache     shared\n\n    \
         let n = found()\n    view { <main><p>{n}</p></main> }\n}\n";
    let found = reported(&[("t.pw", calls_the_module)]);
    assert!(clock(&found), "{found:#?}");
}

/// A shared query whose body calls `mine`: the module's, which reads the
/// asking session's cart, or a local of that name, which reads nothing.
fn shared_calling(local: bool) -> String {
    let own = if local {
        "    let mine = fn() Ok(Cart { lines: [] })\n"
    } else {
        ""
    };
    format!(
        "module t\n\nimport Carts\nimport context.{{ current_session }}\n\
         import domain.{{ Cart, CartError }}\n\n\
         fn mine() -> Result<Cart, CartError> {{\n    Carts.current(current_session())\n}}\n\n\
         public query Shown() -> Result<Cart, CartError>\n    freshness   0.seconds\n    \
         consistency snapshot\n    cache       shared\n{{\n{own}    mine()\n}}\n"
    )
}

#[test]
fn a_call_through_a_local_reads_nothing_of_its_namesakes() {
    // Privacy's reads, as the effects' calls: a local `mine` reads what its
    // value reads, never the module's `fn mine`, the session's cart.
    let session = |found: &[String]| found.iter().any(|d| d.contains("Session<SessionId>"));
    let found = reported(&[("t.pw", &shared_calling(true))]);
    assert!(!session(&found), "{found:#?}");
    // The control: the module's own `mine`, called, reads the session's.
    let found = reported(&[("t.pw", &shared_calling(false))]);
    assert!(session(&found), "{found:#?}");
}

/// A page whose button's handler is `handler`, beside the command
/// `add_to_cart`.
fn handled(handler: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport Resources.{{ Menu }}\n\
         import context.{{ current_session }}\n\
         import domain.{{ StoreId, MenuItem, MenuItemId, PositiveInt, InteractionId, Cart, \
         CartError }}\n\n\
         command add_to_cart(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
         requires      SignedIn\n    idempotent_by InteractionId\n{{\n    \
         Carts.add(current_session(), item, quantity)\n}}\n\n\
         page ShopPage(id: StoreId) {{\n    placement origin\n    cache private\n\n    \
         let menu = query Menu(id)\n\n    view {{\n        <ul>\n            \
         {{#each menu as item (item.id)}}\n                <li>\n                    \
         <button type=\"button\" on:press={{{handler}}}>Add</button>\n                \
         </li>\n            {{/each}}\n        </ul>\n    }}\n}}\n"
    )
}

#[test]
fn a_handlers_call_through_a_local_is_no_commands_request() {
    // A command's request is performed by the command, at the origin, and
    // what its call computes there is not the browser's. A local named
    // `add_to_cart` is no command: the cart it reads, in its argument, is
    // read in the browser, which cannot grant it.
    let browser = |found: &[String]| found.iter().any(|d| d.contains("database.read<Carts>"));
    let found = reported(&[(
        "t.pw",
        &handled(
            "resumable(captures = { item }) => { let add_to_cart = fn(cart, n) n\n    \
             let _n = add_to_cart(Carts.current(current_session()), 1) }",
        ),
    )]);
    assert!(browser(&found), "{found:#?}");
    // The control: the same read, not passed to a call at all, is reported
    // as the browser's alike.
    let found = reported(&[(
        "t.pw",
        &handled(
            "resumable(captures = { item }) => { let _cart = Carts.current(current_session()) }",
        ),
    )]);
    assert!(browser(&found), "{found:#?}");
}

/// A shared query whose body calls `mine`: the module's `private fn mine`,
/// a value only its reader may see, or a local of that name.
fn shared_private(local: bool) -> String {
    let own = if local { "    let mine = fn() 2\n" } else { "" };
    format!(
        "module t\n\nprivate fn mine() -> Int {{\n    1\n}}\n\n\
         public query Shown() -> Int\n    freshness   0.seconds\n    \
         consistency snapshot\n    cache       shared\n{{\n{own}    mine()\n}}\n"
    )
}

#[test]
fn a_call_through_a_local_is_labelled_as_the_local() {
    // A declaration's label joins the labels its body's calls declare: a
    // local `mine` is not the module's `private fn mine`.
    let private = |found: &[String]| found.iter().any(|d| d.starts_with("PW5001"));
    let found = reported(&[("t.pw", &shared_private(true))]);
    assert!(!private(&found), "{found:#?}");
    // The control: the module's, called, makes the value its reader's.
    let found = reported(&[("t.pw", &shared_private(false))]);
    assert!(private(&found), "{found:#?}");
}

/// Each diagnostic's explanation, in the test's own files.
fn explained(src: &[(&str, &str)]) -> Vec<String> {
    let own: Vec<&str> = src.iter().map(|(n, _)| *n).collect();
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| own.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds.into_iter().filter_map(|d| d.explanation))
        .collect()
}

#[test]
fn a_labels_cause_is_what_was_read_not_a_locals_namesake() {
    // The cause a shared cache's refusal names is the read that makes the
    // value a session's: the cart, not a local `mine` called first, though a
    // `fn mine` that reads the cart is the module's.
    let src = "module t\n\nimport Carts\nimport context.{ current_session }\n\
               import domain.{ Cart, CartError }\n\n\
               fn mine() -> Result<Cart, CartError> {\n    Carts.current(current_session())\n}\n\n\
               public query Shown() -> Result<Cart, CartError>\n    freshness   0.seconds\n    \
               consistency snapshot\n    cache       shared\n{\n    \
               let mine = fn() Ok(Cart { lines: [] })\n    let _first = mine()\n    \
               Carts.current(current_session())\n}\n";
    let found = explained(&[("t.pw", src)]);
    assert!(
        found.iter().any(|e| e.contains("The cause chain")),
        "{found:#?}"
    );
    assert!(!found.iter().any(|e| e.contains("`mine`")), "{found:#?}");
}

// **The page plan** (the second part): a name a template reads is its
// binding's, not the page's binding of its spelling (ADR-0277's limitation).

/// The example store's sources, its `app.pw` changed by `change`, built.
fn store_built(change: impl Fn(&str) -> String) -> pw_core::build::Build {
    store_built_with(change, &[])
}

/// The example store with `files` beside it, built and refused nothing.
fn built_with(files: &[(&str, &str)]) -> pw_core::build::Build {
    let b = store_built_with(|app| app.to_string(), files);
    assert!(b.refusals().is_empty(), "{:#?}", b.refusals());
    b
}

fn store_built_with(
    change: impl Fn(&str) -> String,
    files: &[(&str, &str)],
) -> pw_core::build::Build {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut units = Vec::new();
    let mut add = |path: std::path::PathBuf, src: String| {
        units.push(pw_core::check::Unit {
            path: path.display().to_string(),
            hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
            src,
        });
    };
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
            let src = std::fs::read_to_string(&p).expect("read");
            let src = if p.ends_with("store/app.pw") {
                change(&src)
            } else {
                src
            };
            add(p, src);
        }
    }
    let domain = root.join("examples/domain.pw");
    let src = std::fs::read_to_string(&domain).expect("domain.pw");
    add(domain, src);
    for (name, src) in files {
        add(name.into(), src.to_string());
    }
    pw_core::build::build(&units).expect("builds")
}

fn store_plan(b: &pw_core::build::Build) -> &pw_core::page_values::PageValues {
    b.pages
        .iter()
        .find(|p| p.page == "store.page.StorePage")
        .map(|p| {
            p.plan
                .as_ref()
                .unwrap_or_else(|e| panic!("not planned: {e}"))
        })
        .expect("the store page")
}

/// Each `{#each}` in the store page's template, as (collection, binding).
fn store_loops(b: &pw_core::build::Build) -> Vec<(String, String)> {
    loops(b, "store.page.StorePage")
}

/// Each `{#each}` in the template at `path`, as (collection, binding).
fn loops(b: &pw_core::build::Build, path: &str) -> Vec<(String, String)> {
    use pw_core::template_ir::{Chunk, Part};
    fn walk(chunks: &[Chunk], out: &mut Vec<(String, String)>) {
        for c in chunks {
            let Chunk::Dynamic(p) = c else { continue };
            if let Part::Each {
                collection,
                binding,
                ..
            } = p
            {
                out.push((collection.clone(), binding.clone()));
            }
            for region in p.nested() {
                walk(region, out);
            }
        }
    }
    let t = b
        .templates
        .iter()
        .find(|t| t.path == path)
        .unwrap_or_else(|| panic!("no template `{path}`"));
    let mut out = Vec::new();
    walk(&t.chunks, &mut out);
    out
}

const LINE: &str = "    let line = query MenuLine(id)\n";
const LINE_SHOWN: &str = "<p id=\"menu-line\">{line}</p>";

/// The store, its page's binding of `MenuLine` named `name`.
fn menu_line_named(name: &str) -> impl Fn(&str) -> String + '_ {
    move |app: &str| {
        assert_eq!(app.matches(LINE).count(), 1, "the binding's anchor");
        assert_eq!(app.matches(LINE_SHOWN).count(), 1, "the part's anchor");
        app.replace(LINE, &format!("    let {name} = query MenuLine(id)\n"))
            .replace(LINE_SHOWN, &format!("<p id=\"menu-line\">{{{name}}}</p>"))
    }
}

#[test]
fn a_loops_name_is_the_loops_beside_a_page_binding_of_its_spelling() {
    // Found 2026-10-08 (ADR-0277): the store's page bound `line` beside the
    // cart's `{#each cart.lines as line}`, the plan read the loop's
    // `line.name` as the binding's, and the build refused. The binding was
    // named `summary` until 2026-10-10; the control is that name.
    for name in ["summary", "line"] {
        let b = store_built(menu_line_named(name));
        assert!(b.refusals().is_empty(), "{name}: {:#?}", b.refusals());
        let plan = store_plan(&b);
        // The page's part reads the page's binding, the menu's line.
        let part = plan
            .parts
            .iter()
            .find(|p| p.path == name)
            .unwrap_or_else(|| panic!("{name}: no part reads it: {:#?}", plan.parts));
        assert_eq!(part.binding, name);
        let bound = plan
            .bindings
            .iter()
            .find(|x| x.binding == name)
            .unwrap_or_else(|| panic!("{name}: not bound"));
        assert!(bound.resource.ends_with(".MenuLine"), "{}", bound.resource);
        // And the loop's rows read the loop's item, by a name no part
        // outside it reads.
        let loops = store_loops(&b);
        let (_, binding) = loops
            .iter()
            .find(|(c, _)| c == "cart.lines")
            .unwrap_or_else(|| panic!("{name}: no loop over the cart's lines: {loops:?}"));
        assert!(
            !plan.bindings.iter().any(|x| &x.binding == binding),
            "{name}: the loop binds `{binding}`, a page binding's name"
        );
        assert!(
            !plan.parts.iter().any(|p| p.binding == *binding),
            "{name}: a part outside the loop reads `{binding}`"
        );
    }
}

/// `app` with `change` made to the store page's declaration alone: the cart
/// page writes some of the same markup.
fn in_store_page(app: &str, change: impl Fn(&str) -> String) -> String {
    let start = app.find("session page StorePage(").expect("the store page");
    let end = start + app[start..].find("\n}\n").expect("its end") + 3;
    format!(
        "{}{}{}",
        &app[..start],
        change(&app[start..end]),
        &app[end..]
    )
}

const COUNT: &str = "<span id=\"cart-count\">{cart.line_count}</span></p>";

/// The store page with `markup` after the cart's count.
fn after_count(markup: &str) -> impl Fn(&str) -> String + '_ {
    move |app: &str| {
        in_store_page(app, |page| {
            assert_eq!(page.matches(COUNT).count(), 1, "the count's anchor");
            page.replace(COUNT, &format!("{COUNT}\n{markup}"))
        })
    }
}

#[test]
fn an_inner_loops_name_is_the_inner_loops() {
    // `{#each section.items as section}` binds the item where the outer loop
    // bound the section: inside it, `section` is the item, and its list is
    // read from the outer one's. Planned right before 2026-10-10, each walk
    // taking the nearest loop; the inner one's name is now its own as well.
    let b = store_built(after_count(
        "<ul id=\"nested\">{#each menu as section (section.category.id)}\
         {#each section.items as section (section.id)}\
         <li>{section.name} {section.price.display}</li>{/each}{/each}</ul>",
    ));
    assert!(b.refusals().is_empty(), "{:#?}", b.refusals());
    let loops = store_loops(&b);
    let (_, inner) = loops
        .iter()
        .filter(|(c, _)| c == "section.items")
        .find(|(_, b)| b != "item")
        .unwrap_or_else(|| panic!("no nested loop: {loops:?}"));
    assert_ne!(inner, "section", "{loops:?}");
    // Its row's member read is the item's, in the menu's sections' items.
    let plan = store_plan(&b);
    assert!(
        plan.rows.iter().any(|r| r.collection == "menu.*.items"
            && r.binding == *inner
            && r.path == format!("{inner}.price.display")),
        "{:#?}",
        plan.rows
    );
}

#[test]
fn a_stream_parts_name_is_the_answer_beside_a_page_binding_of_its_spelling() {
    // `<ready as={line}>` beside the page's `line`: the delivery estimate's
    // range is read from the answer, not from the menu's line.
    const READY: &str = "<ready as={estimate}><p>Delivery in {estimate.min_minutes} to \
                         {estimate.max_minutes} min</p></ready>";
    let b = store_built(|app: &str| {
        let app = menu_line_named("line")(app);
        assert_eq!(app.matches(READY).count(), 1, "the ready part's anchor");
        app.replace(
            READY,
            "<ready as={line}><p>Delivery in {line.min_minutes} to {line.max_minutes} \
             min</p></ready>",
        )
    });
    assert!(b.refusals().is_empty(), "{:#?}", b.refusals());
    let plan = store_plan(&b);
    assert!(
        !plan.parts.iter().any(|p| p.path.contains("min_minutes")),
        "a part outside the stream reads the range: {:#?}",
        plan.parts
    );
}

const NOTICE: &str = "    signal notice: String = \"\"\n";

#[test]
fn a_loops_name_is_the_loops_beside_a_signal_of_its_spelling() {
    // The page's signal `notice`, and a loop that binds `notice`: inside the
    // loop it is the line, which the browser does not hold, and a handler
    // there that reads it reads the line, not the signal.
    let b = store_built(|app: &str| {
        let app = after_count(
            "<ul id=\"notices\">{#each cart.lines as notice (notice.item_id)}\
             <li>{notice.name}<button type=\"button\" \
             on:press={resumable(captures = { notice }) => told = notice.name}>Tell</button>\
             </li>{/each}</ul>",
        )(app);
        in_store_page(&app, |page| {
            assert_eq!(page.matches(NOTICE).count(), 1, "the signal's anchor");
            page.replace(NOTICE, &format!("{NOTICE}    signal told: String = \"\"\n"))
        })
    });
    assert!(b.refusals().is_empty(), "{:#?}", b.refusals());
    let plan = store_plan(&b);
    assert!(
        !plan.live.iter().any(|l| l.path.contains("name")),
        "the loop's line is read as the signal: {:#?}",
        plan.live
    );
    let loops = store_loops(&b);
    assert!(
        loops
            .iter()
            .any(|(c, b)| c == "cart.lines" && b != "notice" && b != "line"),
        "{loops:?}"
    );
    // No element tells the browser the handler's `notice` is a signal.
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "store.page.StorePage")
        .expect("the store page's template");
    let markup = serde_json::to_string(&t.chunks).expect("chunks serialize");
    assert!(
        !markup.contains("data-pw-signals"),
        "a handler's `notice` is told to be a signal: {markup}"
    );
}

/// Module `t`: a view that lists a cart's lines, `as line`, and a page that
/// binds `line` and shows the view.
const VIEWED: &str = "module t\n\nimport store.page.{ Store, Cart }\n\
    import domain.{ StoreId, CartLine }\nimport context.{ current_session }\n\n\
    view Lines(lines: List<CartLine>) !{} {\n    \
    <ul>{#each lines as line (line.item_id)}<li>{line.quantity.count}</li>{/each}</ul>\n}\n\n\
    session page P(id: StoreId) {\n    route \"/p/{id}\"\n    cache private\n\n    \
    let line = query Store(id)\n    let cart = query Cart(current_session())\n\n    \
    view {\n        <title>{line.name}</title>\n        <h1>{line.name}</h1>\n        \
    <Lines lines={cart.lines} />\n    }\n}\n";

#[test]
fn a_views_loop_is_its_own_beside_a_page_binding_of_its_spelling() {
    // A view composed into the page (ADR-0136) reads its loop's `line`;
    // the page's `line` is a store, which has no quantity.
    let b = built_with(&[("t.pw", VIEWED)]);
    let loops = loops(&b, "t.P");
    assert!(
        loops.iter().any(|(c, b)| c == "cart.lines" && b != "line"),
        "{loops:?}"
    );
}

/// Module `t`: a layout that lists the session's cart's lines, `as line`,
/// itself and through a view, and two pages shown in it, one that binds
/// `line` and one that does not.
const LAID_OUT: &str = "module t\n\nimport store.page.{ Store, Cart }\n\
    import domain.{ StoreId, CartLine }\nimport context.{ current_session }\n\n\
    view Totals(lines: List<CartLine>) !{} {\n    \
    <ol>{#each lines as line (line.item_id)}<li>{line.total.display}</li>{/each}</ol>\n}\n\n\
    session layout Shell {\n    let cart = query Cart(current_session())\n\n    \
    view {\n        <header><ul>{#each cart.lines as line (line.item_id)}\
    <li>{line.quantity.count}</li>{/each}</ul><Totals lines={cart.lines} /></header>\n        \
    <main>\n            <slot />\n        </main>\n    }\n}\n\n\
    session page A(id: StoreId) {\n    route  \"/a/{id}\"\n    layout Shell\n    \
    cache  private\n\n    let line = query Store(id)\n\n    view {\n        \
    <title>{line.name}</title>\n        <h1>{line.name}</h1>\n    }\n}\n\n\
    session page B(id: StoreId) {\n    route  \"/b/{id}\"\n    layout Shell\n    \
    cache  private\n\n    let store = query Store(id)\n\n    view {\n        \
    <title>{store.name}</title>\n        <h1>{store.name}</h1>\n    }\n}\n";

#[test]
fn a_layouts_loop_is_the_same_on_every_page() {
    // The page `A` binds `line`, `B` does not: the layout's loop is its own
    // on both, and its markup the same, which a soft navigation compares.
    let b = built_with(&[("t.pw", LAID_OUT)]);
    let layout = |page: &str| {
        b.pages
            .iter()
            .find(|p| p.page == page)
            .and_then(|p| p.plan.as_ref().ok())
            .and_then(|p| p.layout.clone())
            .unwrap_or_else(|| panic!("`{page}` is shown in no layout"))
    };
    assert_eq!(layout("t.A"), layout("t.B"));
    for page in ["t.A", "t.B"] {
        let loops = loops(&b, page);
        let laid: Vec<_> = loops
            .iter()
            .filter(|(c, _)| c == "cart~Shell.lines")
            .collect();
        assert_eq!(laid.len(), 2, "the layout's and its view's: {loops:?}");
        assert!(laid.iter().all(|(_, b)| b != "line"), "{page}: {loops:?}");
    }
}
