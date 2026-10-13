//! **A member read of a query's value is computed by a host wherever the page
//! reads it outside a row** (ADR-0323, NEXT 3c).
//!
//! A text part at the top of a page that reads a member, `{cart.line_count}`,
//! was a part a host computes (ADR-0125). Inside a block it was refused, "which
//! a host would call per instance", though a block at the top of the page is
//! rendered once a change: `{#if cart.lines}<p>{cart.line_count}</p>{/if}` did
//! not build (found on the way to ADR-0320, 2026-10-12). So was one in an
//! attribute, what a block decides by, the title and the metadata. Now each is
//! computed once a change, into what the page is rendered with by its path, as
//! a computed hole inside a block is (ADR-0229); and a block the browser
//! renders again from a speculation computes it there, as it computes a
//! region's value (ADR-0235). A loop's list read through a member is still
//! refused: a host keeps a list it reads by field (ADR-0170). And one inside a
//! view that contains itself, which built and which nothing computed, is
//! refused as a computed value there is (ruling 0073-a). Each test states one
//! case, with its control.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_core::page_values::{PageValues, Step};
use pw_syntax::parse_tree;

/// The store, its `app.pw` changed by `app`.
fn store(app: &dyn Fn(&str) -> String) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            let src = std::fs::read_to_string(&p).expect("read");
            let src = if p.ends_with("examples/store/app.pw") {
                app(&src)
            } else {
                src
            };
            out.push((p.display().to_string(), src));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.into_iter()
        .map(|(path, src)| Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect()
}

/// The cart's page, `markup` at the top of its `<main>`, and `functions`
/// declared beside it: members its markup may read.
fn cart_page<'a>(markup: &'a str, functions: &'a str) -> impl Fn(&str) -> String + 'a {
    move |src: &str| {
        let main = "        <title>Your cart</title>\n        <main>\n";
        assert_eq!(src.matches(main).count(), 1, "the cart's page");
        let page = "user page CartPage() {\n";
        assert_eq!(src.matches(page).count(), 1, "the cart's page");
        src.replacen("import domain.{ ", "import domain.{ CartLine, ", 1)
            .replacen(main, &format!("{main}            {markup}\n"), 1)
            .replacen(page, &format!("{functions}\n{page}"), 1)
    }
}

/// The store built with `app`.
fn built(app: &dyn Fn(&str) -> String) -> pw_core::build::Build {
    pw_core::build::build(&store(app)).unwrap_or_else(|e| panic!("builds: {e}"))
}

/// `page`'s plan, or why it has none.
fn planned<'a>(build: &'a pw_core::build::Build, page: &str) -> &'a Result<PageValues, String> {
    &build
        .pages
        .iter()
        .find(|p| p.page == page)
        .unwrap_or_else(|| panic!("no page {page}"))
        .plan
}

fn plan<'a>(build: &'a pw_core::build::Build, page: &str) -> &'a PageValues {
    planned(build, page)
        .as_ref()
        .unwrap_or_else(|e| panic!("{page}: {e}"))
}

/// The ids of `page`'s blocks of `kind` decided by `value`.
fn blocks(build: &pw_core::build::Build, page: &str, kind: &str, value: &str) -> Vec<u32> {
    build
        .templates
        .iter()
        .find(|t| t.path == page)
        .unwrap_or_else(|| panic!("no template {page}"))
        .manifest()
        .into_iter()
        .filter(|e| e.kind == kind && e.value == value)
        .map(|e| e.id.0)
        .collect()
}

/// Every block of `page` decided by `value` is planned, and there is one.
fn all_planned(build: &pw_core::build::Build, page: &str, value: &str) {
    let ids = blocks(build, page, "conditional", value);
    let planned = &plan(build, page).blocks;
    assert!(!ids.is_empty(), "no block on `{value}`");
    assert!(
        ids.iter().all(|id| planned.contains(id)),
        "{ids:?} of {planned:?}"
    );
}

/// `plan`'s computed values at `path`: each a host computes, from `binding`,
/// through a member by its component.
fn computed_at<'a>(plan: &'a PageValues, path: &str) -> Vec<&'a pw_core::page_values::Part> {
    plan.derived.iter().filter(|d| d.path == path).collect()
}

fn through_a_member(part: &pw_core::page_values::Part, member: &str) -> bool {
    part.steps
        .iter()
        .any(|s| matches!(s, Step::Member(id) if id.ends_with(&format!(".{member}"))))
}

const CART: &str = "store.page.CartPage";
const HAS_LINES: &str =
    "fn has_lines(cart: UserCart) -> Bool !{} {\n    List.length(cart.lines) > 0\n}\n";
const LINES_AGAIN: &str =
    "fn lines_again(cart: UserCart) -> List<CartLine> !{} {\n    cart.lines\n}\n";

/// **The finding**: `{cart.line_count}` inside `{#if cart.lines}` builds,
/// computed by a host through `line_count`'s component, and the block is
/// planned, so a host tells it when the cart changes (ADR-0320).
#[test]
fn a_member_read_inside_a_block_is_computed_by_a_host() {
    let markup = "{#if cart.lines}<p id=\"t\">{cart.line_count}</p>{/if}";
    let b = built(&cart_page(markup, ""));
    let p = plan(&b, CART);
    let computed = computed_at(p, "cart.line_count");
    assert_eq!(computed.len(), 1, "{:?}", p.derived);
    assert_eq!(computed[0].binding, "cart");
    assert!(
        through_a_member(computed[0], "line_count"),
        "{:?}",
        computed[0].steps
    );
    all_planned(&b, CART, "cart.lines");
    // Control: the page as it is computes its count at the top, a part of
    // its own, and nothing inside a block.
    let b = built(&|s| s.to_string());
    let p = plan(&b, CART);
    assert!(
        computed_at(p, "cart.line_count").is_empty(),
        "{:?}",
        p.derived
    );
    assert!(p.parts.iter().any(|x| x.path == "cart.line_count"));
}

/// Inside a loop's row the page's own binding is not read at all, by field
/// or through a member: a row is rendered again for its own item alone
/// (ADR-0146), so this ruling leaves it refused. A member of the row's item
/// is the row's (ADR-0169), as before.
#[test]
fn a_member_of_the_page_inside_a_row_is_refused_as_any_read_of_it_is() {
    for markup in [
        "<ul id=\"t\">{#each cart.lines as l (l.item_id)}<li>{cart.line_count}</li>{/each}</ul>",
        "<ul id=\"t\">{#each cart.lines as l (l.item_id)}<li hidden={cart.lines}>x</li>{/each}</ul>",
    ] {
        let b = built(&cart_page(markup, ""));
        let refused = planned(&b, CART).as_ref().expect_err(markup);
        assert!(
            refused.contains(
                "reads `cart` inside a loop's row, and a row is rendered again for \
                              its own item alone (ADR-0146)"
            ),
            "{markup}: {refused}"
        );
    }
    // Control: the row's own member.
    let markup =
        "<ul id=\"t\">{#each cart.lines as l (l.item_id)}<li>{l.total.display}</li>{/each}</ul>";
    let b = built(&cart_page(markup, ""));
    let p = plan(&b, CART);
    assert!(
        p.rows
            .iter()
            .any(|r| r.binding == "l" && r.path == "l.total.display"),
        "{markup}: {:?}",
        p.rows
    );
}

/// In an attribute, at the top of the page or inside a block; in what a block
/// decides by; in the page's title and in its metadata. At the top, the
/// attribute is one a host sets again, as a computed one is (ADR-0226).
#[test]
fn a_member_read_outside_text_is_computed_by_a_host() {
    // An attribute at the top of the page.
    let markup = "<p id=\"t\" data-count={cart.line_count}>x</p>";
    let b = built(&cart_page(markup, ""));
    let p = plan(&b, CART);
    let computed = computed_at(p, "cart.line_count");
    assert_eq!(computed.len(), 1, "{markup}: {:?}", p.derived);
    assert!(
        p.attributes.contains(&computed[0].part),
        "{markup}: {:?}",
        p.attributes
    );
    // Inside a block, and what a block decides by.
    for (markup, functions, path, subject) in [
        (
            "{#if cart.lines}<p id=\"t\" data-count={cart.line_count}>x</p>{/if}",
            "",
            "cart.line_count",
            "cart.lines",
        ),
        (
            "{#if cart.has_lines}<p id=\"t\">Ready.</p>{/if}",
            HAS_LINES,
            "cart.has_lines",
            "cart.has_lines",
        ),
    ] {
        let b = built(&cart_page(markup, functions));
        let p = plan(&b, CART);
        assert_eq!(computed_at(p, path).len(), 1, "{markup}: {:?}", p.derived);
        all_planned(&b, CART, subject);
    }
    // The title, and the metadata.
    for (from, to) in [
        (
            "        <title>Your cart</title>\n",
            "        <title>Your cart, {cart.line_count} items</title>\n",
        ),
        (
            "        <title>Your cart</title>\n",
            "        <title>Your cart</title>\n        <meta name=\"description\" content=\"{cart.line_count} items\" />\n",
        ),
    ] {
        let app = |s: &str| {
            assert_eq!(s.matches(from).count(), 1, "the cart's title");
            s.replacen(from, to, 1)
        };
        let b = built(&app);
        let p = plan(&b, CART);
        assert_eq!(
            computed_at(p, "cart.line_count").len(),
            1,
            "{to}: {:?}",
            p.derived
        );
    }
}

/// **A loop's list read through a member** is refused, by name: a host keeps
/// a list it reads by field, and patches its rows where they are (ADR-0170).
/// Control: the same list read by field builds.
#[test]
fn a_loops_list_read_through_a_member_is_refused() {
    let markup =
        "<ul id=\"t\">{#each cart.lines_again as l (l.item_id)}<li>{l.name}</li>{/each}</ul>";
    let b = built(&cart_page(markup, LINES_AGAIN));
    let refused = planned(&b, CART).as_ref().expect_err(markup);
    assert!(
        refused.contains("reads `cart.lines_again` through a member function in a loop's list"),
        "{refused}"
    );
    let markup = "<ul id=\"t\">{#each cart.lines as l (l.item_id)}<li>{l.name}</li>{/each}</ul>";
    plan(&built(&cart_page(markup, LINES_AGAIN)), CART);
}

/// **A block the browser renders again from a speculation** computes the
/// member there, from the value it speculates on, as it computes a region's
/// value (ADR-0235): the cart's page speculates on the cart, and its block
/// shows the count it speculates. Control: the page as it is computes none
/// inside a block.
#[test]
fn a_member_read_inside_a_speculated_block_is_computed_in_the_browser() {
    let markup = "{#if cart.lines}<p id=\"t\">{cart.line_count}</p>{/if}";
    let source = |app: &dyn Fn(&str) -> String| {
        let b = built(app);
        let cart = b
            .speculations
            .iter()
            .find(|s| s.page == CART)
            .expect("the cart's page speculates");
        let pw_core::backend::wasm::Encoding::Encoded(module) = &cart.module else {
            panic!("{:?}", cart.module);
        };
        module.source.clone()
    };
    let changed = source(&cart_page(markup, ""));
    assert!(changed.contains("\"cart.line_count\": f"), "{changed}");
    // And in an attribute inside one.
    let markup = "{#if cart.lines}<p id=\"t\" data-count={cart.line_count}>x</p>{/if}";
    let changed = source(&cart_page(markup, ""));
    assert!(
        changed.contains("\"cart.line_count\": f"),
        "{markup}: {changed}"
    );
    let as_it_is = source(&|s: &str| s.to_string());
    assert!(!as_it_is.contains("\"cart.line_count\": f"), "{as_it_is}");
}

/// **A member read inside a view that contains itself is refused**, as a
/// computed value there is (ruling 0073-a): each use is an instance made at
/// run time, and a host computes a member from a query's value or a row's
/// item, never from an instance's. Until 2026-10-12 one built, and nothing
/// computed it (found writing this ruling's tests). Control: a field there,
/// which the renderer reads from the instance's value.
#[test]
fn a_member_read_inside_a_view_that_contains_itself_is_refused() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    let text = "        <p>{post.text}</p>\n        <ul>\n            {#each post.replies as reply (reply.id)}";
    assert_eq!(src.matches(text).count(), 1, "the view's text");
    let view = "view Replies(post: Post) {\n";
    assert_eq!(src.matches(view).count(), 1, "the view");
    let shout = "fn shout(post: Post) -> String !{} {\n    post.text\n}\n\n";
    let feed = |read: &str| {
        let src = src
            .replacen(text, &text.replacen("{post.text}", read, 1), 1)
            .replacen(view, &format!("{shout}{view}"), 1);
        let mut files = Vec::new();
        for d in ["packages/pw-std", "packages/pw-platform-web"] {
            let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
                .unwrap_or_else(|e| panic!("{d}: {e}"))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect();
            paths.sort();
            for p in paths {
                let s = std::fs::read_to_string(&p).expect("read");
                files.push((p.display().to_string(), s));
            }
        }
        files.push(("app.pw".to_string(), src));
        let units: Vec<Unit> = files
            .into_iter()
            .map(|(path, src)| Unit {
                hir: lower_file(&src, &parse_tree(&src).green),
                path,
                src,
            })
            .collect();
        match pw_core::build::build(&units) {
            Ok(b) => b.refusals().join("\n"),
            Err(e) => e,
        }
    };
    let refused = feed("{post.shout}");
    assert!(
        refused.contains(
            "`<Replies>` contains itself and reads `post.shout` through a member function, and \
             a view that contains itself computes none yet (ruling 0073-a)"
        ),
        "{refused}"
    );
    assert_eq!(feed("{post.text}"), "");
}
