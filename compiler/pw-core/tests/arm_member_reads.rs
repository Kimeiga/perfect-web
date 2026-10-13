//! **An arm's binding read through a member function, a host computes**
//! (the integrator's, 2026-10-13; W9's finding in NEXT 6b).
//!
//! `{#match order_total}{:Some(total)}{total.display}{/match}` was refused
//! by the build ("which no host computes there"), and the order page worded
//! its total in the query instead. A host now computes it from the query's
//! value the match decides by, through the case's payload, into what the
//! block is rendered with, as a member read inside a block is (ADR-0323);
//! a value of another case sets nothing, its arm not shown.
//!
//! Each test states one case, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::page_values::{PageValues, Part, Step};
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
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
    out
}

/// A page binding queries of each kind of case, `markup` its `<main>`.
fn page(markup: &str) -> String {
    let query = |name: &str, result: &str| {
        format!(
            "public query {name}(id: String) -> {result}\n    freshness     30.seconds\n    \
             consistency   snapshot\n    cache         shared\n    key           id\n    \
             timeout       2.seconds\n{{\n    todo\n}}\n\n"
        )
    };
    format!(
        "module t\n\n\
         type Money = Money {{ minor_units: Int }}\n\n\
         type Item = Item {{ id: String, sale: Option<Money> }}\n\n\
         type Order = Order {{ id: String, discount: Option<Money> }}\n\n\
         type Settlement = Settlement {{ outcome: Result<Money, Refund> }}\n\n\
         type Ceiling = Ceiling {{ at: Money }}\n\n\
         type Refund = Refund {{ amount: Money }}\n\n\
         type Price =\n    | Fixed(Money)\n    | Between(Money, Ceiling)\n    | Free\n\n\
         fn display(price: Money) -> String !{{}} {{\n    \"${{price.minor_units / 100}}\"\n}}\n\n\
         fn is_free(price: Money) -> Bool !{{}} {{\n    price.minor_units == 0\n}}\n\n\
         fn discount(price: Money) -> Option<Money> !{{}} {{\n    None\n}}\n\n\
         {}{}{}{}{}{}\
         page P(id: String) {{\n    cache private\n\n    \
         let total = query Total(id)\n    let charged = query Charged(id)\n    \
         let outcome = query Outcome(id)\n    let price = query PriceOf(id)\n    \
         let order = query OrderOf(id)\n    let menu = query Menu(id)\n\n    \
         view {{\n        <main>\n            {markup}\n        </main>\n    }}\n}}\n",
        query("Total", "Money"),
        query("Charged", "Option<Money>"),
        query("Outcome", "Settlement"),
        query("PriceOf", "Price"),
        query("OrderOf", "Order"),
        query("Menu", "List<Item>"),
    )
}

fn sources(markup: &str) -> Vec<(String, String)> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), page(markup)));
    sources
}

/// The page's plan, or why the build refused it. The page checks clean in
/// every case here: whether a host computes a read is the build's.
fn plan(markup: &str) -> Result<PageValues, String> {
    let checked: Vec<String> = check_sources(&sources(markup))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    assert!(checked.is_empty(), "{markup}: {checked:#?}");
    let units: Vec<Unit> = sources(markup)
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    pw_core::build::build(&units)
        .expect("builds")
        .pages
        .into_iter()
        .find(|p| p.page == "t.P")
        .expect("a page")
        .plan
}

fn payload(case: &str, field: Option<usize>) -> Step {
    Step::Payload {
        case: case.to_string(),
        field,
    }
}

fn member(id: &str) -> Step {
    Step::Member(id.to_string())
}

fn field(name: &str) -> Step {
    Step::Field(name.to_string())
}

/// The part planned for `path`.
fn computed<'a>(plan: &'a PageValues, path: &str) -> Vec<&'a Part> {
    plan.derived.iter().filter(|d| d.path == path).collect()
}

/// **Each kind of case's payload**: `Some`, `Ok` and `Err`, a declared case
/// of one field and one of several, read through a member in text.
#[test]
fn an_arms_binding_is_read_through_a_member_from_the_value_its_match_decides_by() {
    for (markup, path, binding, steps) in [
        (
            "{#match charged}{:Some(amount)}<p>{amount.display}</p>{:None}<p>none</p>{/match}",
            "amount.display",
            "charged",
            vec![payload("some", None), member("t.display")],
        ),
        (
            "{#match outcome.outcome}{:Ok(paid)}<p>{paid.display}</p>{:Err(owed)}<p>{owed.amount.minor_units}</p>{/match}",
            "paid.display",
            "outcome",
            vec![field("outcome"), payload("ok", None), member("t.display")],
        ),
        (
            "{#match outcome.outcome}{:Ok(paid)}<p>{paid.minor_units}</p>{:Err(owed)}<p>{owed.amount.display}</p>{/match}",
            "owed.amount.display",
            "outcome",
            vec![
                field("outcome"),
                payload("err", None),
                field("amount"),
                member("t.display"),
            ],
        ),
        (
            "{#match price}{:Fixed(at)}<p>{at.display}</p>{:Between(low, high)}<p>range</p>{:Free}<p>free</p>{/match}",
            "at.display",
            "price",
            vec![payload("fixed", None), member("t.display")],
        ),
        (
            "{#match price}{:Fixed(at)}<p>fixed</p>{:Between(low, high)}<p>{low.minor_units} to {high.at.display}</p>{:Free}<p>free</p>{/match}",
            "high.at.display",
            "price",
            vec![
                payload("between", Some(1)),
                field("at"),
                member("t.display"),
            ],
        ),
        // The subject read by field, and through a member (ADR-0229).
        (
            "{#match order.discount}{:Some(off)}<p>{off.display}</p>{:None}<p>none</p>{/match}",
            "off.display",
            "order",
            vec![
                field("discount"),
                payload("some", None),
                member("t.display"),
            ],
        ),
        (
            "{#match total.discount}{:Some(off)}<p>{off.display}</p>{:None}<p>none</p>{/match}",
            "off.display",
            "total",
            vec![
                member("t.discount"),
                payload("some", None),
                member("t.display"),
            ],
        ),
    ] {
        let plan = plan(markup).unwrap_or_else(|e| panic!("{markup}: {e}"));
        let found = computed(&plan, path);
        assert_eq!(found.len(), 1, "{markup}: {:#?}", plan.derived);
        assert_eq!(found[0].binding, binding, "{markup}");
        assert_eq!(found[0].steps, steps, "{markup}");
        assert!(plan.rows.is_empty(), "{markup}: {:#?}", plan.rows);
    }
}

/// **An arm's name that hides a binding of the page is renamed** where the
/// template lowers it (`total` beside the page's `total`), and its read is
/// planned by the name the renderer binds.
#[test]
fn an_arms_renamed_binding_is_read_by_its_new_name() {
    let plan =
        plan("{#match charged}{:Some(total)}<p>{total.display}</p>{:None}<p>none</p>{/match}")
            .expect("planned");
    let found: Vec<&Part> = plan
        .derived
        .iter()
        .filter(|d| d.binding == "charged")
        .collect();
    assert_eq!(found.len(), 1, "{:#?}", plan.derived);
    assert!(found[0].path.ends_with(".display") && found[0].path != "total.display");
    assert_eq!(
        found[0].steps,
        vec![payload("some", None), member("t.display")]
    );
}

/// **In an attribute, and in what a block in the arm decides by**, as in
/// its text; the same read twice is planned once.
#[test]
fn an_arms_member_read_outside_text_is_computed_too_and_planned_once() {
    let plan = plan(
        "{#match charged}{:Some(amount)}<p title={amount.display}>{amount.display}</p>\
         {#if amount.is_free}<p>free</p>{/if}{:None}<p>none</p>{/match}",
    )
    .expect("planned");
    assert_eq!(
        computed(&plan, "amount.display").len(),
        1,
        "{:#?}",
        plan.derived
    );
    assert_eq!(
        computed(&plan, "amount.is_free")[0].steps,
        vec![payload("some", None), member("t.is_free")]
    );
}

/// Controls: **a field of an arm's binding is the renderer's**, as before,
/// and nothing is planned for it.
#[test]
fn a_field_of_an_arms_binding_is_the_renderers() {
    let plan = plan(
        "{#match charged}{:Some(amount)}<p>{amount.minor_units}</p>{:None}<p>none</p>{/match}",
    )
    .expect("planned");
    assert!(
        !plan.derived.iter().any(|d| d.path.starts_with("amount")),
        "{:#?}",
        plan.derived
    );
}

/// **Two arms that bind one name and read one path through it from
/// different values are refused**, naming both parts: a host sets a block's
/// value by its path.
#[test]
fn two_arms_reading_one_path_from_different_values_are_refused() {
    let refused = plan(
        "{#match charged}{:Some(x)}<p>{x.display}</p>{:None}<p>none</p>{/match}\
         {#match order.discount}{:Some(x)}<p>{x.display}</p>{:None}<p>none</p>{/match}",
    )
    .expect_err("refused");
    assert!(refused.contains("named apart"), "{refused}");
    // Control: named apart, both are computed.
    let plan = plan(
        "{#match charged}{:Some(x)}<p>{x.display}</p>{:None}<p>none</p>{/match}\
         {#match order.discount}{:Some(y)}<p>{y.display}</p>{:None}<p>none</p>{/match}",
    )
    .expect("planned");
    assert_eq!(computed(&plan, "x.display")[0].binding, "charged");
    assert_eq!(computed(&plan, "y.display")[0].binding, "order");
}

/// **An arm whose match decides by no query's value is refused**, by name:
/// a loop's item's.
#[test]
fn an_arm_of_a_match_on_a_loops_item_is_refused_by_name() {
    let refused = plan(
        "<ul>{#each menu as item (item.id)}<li>{#match item.sale}{:Some(off)}{off.display}\
         {:None}none{/match}</li>{/each}</ul>",
    )
    .expect_err("refused");
    assert!(refused.contains("decides by no query's value"), "{refused}");
}

// --- in a region the browser renders again from a speculation ---------------

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
/// declared beside it.
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

const FIRST_LINE: &str = "fn first_line(cart: UserCart) -> Option<CartLine> !{} {\n    None\n}\n";

/// The store built with the cart page's `markup`.
fn built(markup: &str) -> pw_core::build::Build {
    pw_core::build::build(&store(&cart_page(markup, FIRST_LINE)))
        .unwrap_or_else(|e| panic!("builds: {e}"))
}

/// The cart page's speculation, as `build` encodes it.
fn cart_speculation(build: &pw_core::build::Build) -> &pw_core::backend::speculation::Compiled {
    build
        .speculations
        .iter()
        .find(|s| s.page == "store.page.CartPage")
        .expect("the cart's page speculates")
}

/// **An arm's member read inside a block the browser renders again from a
/// speculation is refused there**, by name: the browser, rendering it from
/// the cart it speculates on, would have no such value. Control: a field of
/// the arm's binding there, which the renderer reads.
#[test]
fn an_arms_member_read_in_a_speculated_region_is_refused_by_name() {
    let refused = built(
        "{#if cart.lines}{#match cart.first_line}{:Some(first)}<p id=\"t\">{first.total.display}</p>\
         {:None}<p>none</p>{/match}{/if}",
    );
    let pw_core::backend::wasm::Encoding::Unsupported { construct, .. } =
        &cart_speculation(&refused).module
    else {
        panic!("{:?}", cart_speculation(&refused).module);
    };
    assert_eq!(
        *construct,
        "a member read of an arm's binding in a speculated region"
    );
    let control = built(
        "{#if cart.lines}{#match cart.first_line}{:Some(first)}<p id=\"t\">{first.name}</p>\
         {:None}<p>none</p>{/match}{/if}",
    );
    assert!(
        matches!(
            cart_speculation(&control).module,
            pw_core::backend::wasm::Encoding::Encoded(_)
        ),
        "{:?}",
        cart_speculation(&control).module
    );
}
