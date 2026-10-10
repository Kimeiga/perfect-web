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
