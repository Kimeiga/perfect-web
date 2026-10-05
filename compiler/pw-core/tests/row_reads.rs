//! **What a template reads through a member function, a host computes or
//! the build refuses** (ADR-0169).
//!
//! The renderer reads a value's fields. Through a member function, as
//! `{item.price.display}` reads `display`, a host computes the value: from a
//! query's value in text at the top of a page (ADR-0125), and since
//! 2026-10-03 from the item of a loop over a query's list, for each row.
//! Until then a member read anywhere else was neither planned nor refused: in
//! a row, in an attribute, in what a block decides by, or in a loop's list.
//! The page built, and failed when it was rendered.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::page_values::{PageValues, RowRead, Step};
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

/// A menu page: items with prices, sections of items, and a total, each a
/// query's, with `signals` declared in its body. `markup` is its `<main>`.
fn page_with(signals: &str, markup: &str) -> String {
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
         type Item = Item {{ id: String, name: String, price: Money }}\n\n\
         type Section = Section {{ title: String, items: List<Item> }}\n\n\
         fn display(price: Money) -> String !{{}} {{\n    \"${{price.minor_units / 100}}\"\n}}\n\n\
         fn is_free(price: Money) -> Bool !{{}} {{\n    price.minor_units == 0\n}}\n\n\
         fn discount(price: Money) -> Option<Money> !{{}} {{\n    None\n}}\n\n\
         fn on_sale(item: Item) -> Bool !{{}} {{\n    item.price.minor_units < 300\n}}\n\n\
         fn featured(section: Section) -> List<Item> !{{}} {{\n    section.items\n}}\n\n\
         {}{}{}\
         page P(id: String) {{\n    cache private\n\n    \
         let menu = query Menu(id)\n    let sections = query Sections(id)\n    \
         let total = query Total(id)\n{signals}\n    view {{\n        <main>\n            \
         {markup}\n        </main>\n    }}\n}}\n",
        query("Menu", "List<Item>"),
        query("Sections", "List<Section>"),
        query("Total", "Money"),
    )
}

fn sources_with(signals: &str, markup: &str) -> Vec<(String, String)> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), page_with(signals, markup)));
    sources
}

/// What `pw check` reports of the page: nothing, for every case here. Each
/// read is a well-typed read; whether a host computes it is the build's.
fn reported(signals: &str, markup: &str) -> Vec<String> {
    check_sources(&sources_with(signals, markup))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn plan(markup: &str) -> Result<PageValues, String> {
    plan_with("", markup)
}

fn plan_with(signals: &str, markup: &str) -> Result<PageValues, String> {
    let checked = reported(signals, markup);
    assert!(checked.is_empty(), "{markup}: {checked:#?}");
    let units: Vec<Unit> = sources_with(signals, markup)
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

fn row(collection: &str, path: &str, steps: &[Step]) -> RowRead {
    RowRead {
        collection: collection.to_string(),
        binding: path.split('.').next().unwrap_or_default().to_string(),
        path: path.to_string(),
        steps: steps.to_vec(),
    }
}

fn field(name: &str) -> Step {
    Step::Field(name.to_string())
}

fn member(id: &str) -> Step {
    Step::Member(id.to_string())
}

#[test]
fn a_row_reads_a_member_of_its_item_once_in_text_and_attributes_alike() {
    let plan = plan(
        "<ul>{#each menu as item (item.id)}<li title={item.price.display} \
         aria-label=\"Add {item.name}, {item.price.display}\">{item.name} \
         {item.price.display}<small>{item.price.display}</small></li>{/each}</ul>",
    )
    .expect("planned");
    assert_eq!(
        plan.rows,
        [row(
            "menu",
            "item.price.display",
            &[field("price"), member("t.display")]
        )]
    );
}

#[test]
fn what_a_row_decides_by_and_a_list_it_iterates_are_computed_for_it() {
    let plan = plan(
        "<ul>{#each menu as item (item.id)}<li>{#if item.on_sale}<b>sale</b>{/if}\
         {item.name}</li>{/each}</ul>\
         {#each sections as section (section.title)}<h2>{section.title}</h2><ul>\
         {#each section.featured as item (item.id)}<li>{item.name}</li>{/each}</ul>{/each}",
    )
    .expect("planned");
    assert_eq!(
        plan.rows,
        [
            row("menu", "item.on_sale", &[member("t.on_sale")]),
            row("sections", "section.featured", &[member("t.featured")]),
        ]
    );
}

#[test]
fn a_row_of_a_loop_inside_a_loop_reads_members_of_its_item() {
    // ADR-0181: the inner loop's `section.items` is `sections.*.items`, the
    // list in each item of the query's `sections`, and its rows read members
    // as a query's list's do, in text and attributes alike. Until 2026-10-04
    // it was refused as a list that is not a query's.
    let plan = plan(
        "{#each sections as section (section.title)}<ul>{#each section.items as item \
         (item.id)}<li title={item.price.display}>{item.price.display}</li>{/each}</ul>{/each}",
    )
    .expect("planned");
    assert_eq!(
        plan.rows,
        [row(
            "sections.*.items",
            "item.price.display",
            &[field("price"), member("t.display")]
        )]
    );
}

#[test]
fn a_member_read_no_host_computes_is_refused_where_it_is() {
    for (markup, refused) in [
        // At the top of the page: an attribute, and what a block decides by.
        (
            "<p title={total.display}>total</p>",
            "part 0 reads `total.display` through a member function in an attribute",
        ),
        (
            "<p aria-label=\"Total {total.display}\">total</p>",
            "part 0 reads `total.display` through a member function in an attribute",
        ),
        (
            "{#if total.is_free}<p>free</p>{/if}",
            "part 0 reads `total.is_free` through a member function in what a block decides by",
        ),
        (
            "{#if menu}<p>a menu</p>{:else if total.is_free}<p>free</p>{/if}",
            "part 1 reads `total.is_free` through a member function in what a block decides by",
        ),
        (
            "{#match total.discount}{:Some(d)}<p>{d.minor_units}</p>{:None}<p>none</p>{/match}",
            "part 0 reads `total.discount` through a member function in what a block decides by",
        ),
    ] {
        let why = plan(markup).expect_err(markup);
        assert!(why.starts_with(refused), "{markup}: {why}");
        assert!(
            why.ends_with(
                "which no host computes there: one computes it from a query's value in text at \
                 the top of the page (ADR-0125), or from the item of a loop over a query's list, \
                 or over a list in such an item (ADR-0169, ADR-0181)"
            ),
            "{why}"
        );
    }
}

#[test]
fn fields_are_the_renderer_s_and_a_query_s_member_in_text_at_the_top_is_a_part() {
    // Controls: fields read anywhere, and ADR-0125's part.
    let plan = plan(
        "<p>{total.display}</p><p title={total.minor_units}>x</p>\
         <ul>{#each menu as item (item.id)}<li title={item.name}>{item.name} \
         {item.price.minor_units}</li>{/each}</ul>\
         {#each sections as section (section.title)}<ul>{#each section.items as item \
         (item.id)}<li>{item.name}</li>{/each}</ul>{/each}",
    )
    .expect("planned");
    assert!(plan.rows.is_empty(), "{:#?}", plan.rows);
    assert_eq!(
        plan.parts
            .iter()
            .map(|p| (p.path.as_str(), p.steps.clone()))
            .collect::<Vec<_>>(),
        [("total.display", vec![member("t.display")])]
    );
}

#[test]
fn a_signal_s_member_is_refused_since_the_browser_reads_a_signal_by_field() {
    let signals = "    signal shown: Money = Money { minor_units: 0 }\n";
    let why = plan_with(signals, "<p>{shown.display}</p>").expect_err("refused");
    assert!(
        why.starts_with("part 0 reads `shown.display` through a member function in a text part"),
        "{why}"
    );
    // Control: its field.
    plan_with(signals, "<p>{shown.minor_units}</p>").expect("planned");
}

#[test]
fn a_member_read_in_a_row_of_a_list_no_query_gives_is_refused() {
    // A row of a signal's list, in a block a signal decides: the browser
    // renders it again and reads its item by field (ADR-0137), so no host
    // computes a member there. Until ADR-0181 this case was a loop over a
    // list inside another row's item, which a host computes since, and the
    // refusal was left with no test.
    let signals = "    signal picks: List<Item> = []\n    signal open: Bool = true\n";
    let rows = |li: &str| {
        format!("{{#if open}}<ul>{{#each picks as item (item.id)}}{li}{{/each}}</ul>{{/if}}")
    };
    let why = plan_with(signals, &rows("<li>{item.price.display}</li>")).expect_err("refused");
    assert!(
        why.starts_with(
            "part 2 reads `item.price.display` through a member function in a text part"
        ),
        "{why}"
    );
    // Control: its field.
    plan_with(signals, &rows("<li>{item.name}</li>")).expect("planned");
}
