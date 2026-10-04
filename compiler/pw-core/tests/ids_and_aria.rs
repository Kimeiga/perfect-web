//! **Ids, and what names them, checked at build** (ADR-0185, charter §8.2's
//! "duplicate IDs" and "invalid ARIA relationships").
//!
//! Until 2026-10-04 these were the browser's to find, on a page it read:
//! ADR-0182's audit reads them at run time.
//!
//! - PW5031: an id names one element of its page.
//! - PW5032: an id reference names an element its page shows whenever the
//!   referrer is shown.
//! - PW5033: an ARIA attribute, its value and a role are ones WAI-ARIA
//!   defines.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A view whose markup is `markup`, given a flag, a list and a count.
fn view(markup: &str) -> String {
    format!(
        "module t\n\ntype Line = Line {{\n    id: String,\n    name: String,\n}}\n\n\
         view V(shown: Bool, lines: List<Line>, count: Int) !{{}} {{\n    <main>\n{markup}\n    </main>\n}}\n"
    )
}

fn clean(markup: &str) {
    assert_eq!(reported(&view(markup)), Vec::<String>::new(), "{markup}");
}

fn refused(markup: &str, said: &str) {
    let found = reported(&view(markup));
    assert_eq!(found, [said.to_string()], "{markup}");
}

#[test]
fn an_id_names_one_element_of_its_page() {
    refused(
        "<p id=\"total\">1</p>\n<p id=\"total\">2</p>",
        "PW5031 `total` names two elements of `V`",
    );
    // In a block's arm and outside it: both are shown.
    refused(
        "<p id=\"total\">1</p>\n{#if shown}<p id=\"total\">2</p>{/if}",
        "PW5031 `total` names two elements of `V`",
    );
    // In a loop: every row has it.
    refused(
        "<ul>{#each lines as line (line.id)}<li id=\"line\">{line.name}</li>{/each}</ul>",
        "PW5031 `line` names every row of a list in `V`",
    );
    // Controls: two ids; one id in each arm of a block, one of which is
    // shown; a row's id written from its key; a stream's arms.
    clean("<p id=\"a\">1</p>\n<p id=\"b\">2</p>");
    clean("{#if shown}<p id=\"state\">on</p>{:else}<p id=\"state\">off</p>{/if}");
    clean(
        "<ul>{#each lines as line (line.id)}<li id=\"line-{line.id}\">{line.name}</li>{/each}</ul>",
    );
}

#[test]
fn a_reference_names_an_element_shown_whenever_the_referrer_is() {
    refused(
        "<input id=\"note\" name=\"note\" aria-label=\"Note\" aria-describedby=\"help\" />",
        "PW5032 `aria-describedby` names `help`, which no element of `V` has",
    );
    refused(
        "<label for=\"nothing\">Note</label>\n<input id=\"note\" name=\"note\" aria-label=\"Note\" />",
        "PW5032 `for` names `nothing`, which no element of `V` has",
    );
    // Shown only in one arm of a block the field is outside of.
    refused(
        "<input id=\"note\" name=\"note\" aria-label=\"Note\" aria-describedby=\"help\" />\n\
         {#if shown}<p id=\"help\">Gate codes.</p>{/if}",
        "PW5032 `aria-describedby` names `help`, which `V` does not always show with its `<input>`",
    );
    // Controls: always shown; in every arm of a block; in the referrer's own
    // arm; and an id the page computes, which a reference may name.
    clean(
        "<section aria-labelledby=\"h\"><h2 id=\"h\">Cart</h2></section>\n\
         <input id=\"note\" name=\"note\" aria-label=\"Note\" aria-describedby=\"help other\" />\n\
         <p id=\"help\">Gate codes.</p><p id=\"other\">Or a note.</p>",
    );
    clean(
        "<input id=\"note\" name=\"note\" aria-label=\"Note\" aria-describedby=\"help\" />\n\
         {#if shown}<p id=\"help\">Gate codes.</p>{:else}<p id=\"help\">A note.</p>{/if}",
    );
    clean(
        "{#if shown}<input id=\"note\" name=\"note\" aria-label=\"Note\" aria-describedby=\"help\" />\
         <p id=\"help\">Gate codes.</p>{/if}",
    );
    clean(
        "<ul aria-describedby=\"line-espresso\">{#each lines as line (line.id)}\
         <li id=\"line-{line.id}\">{line.name}</li>{/each}</ul>",
    );
}

#[test]
fn a_reference_to_nothing_is_reported_once() {
    // The field's name is a reference to nothing: the reference is the
    // defect, and the unnamed field is not reported again (PW5014).
    refused(
        "<input id=\"q\" name=\"q\" type=\"search\" aria-labelledby=\"search-title\" />",
        "PW5032 `aria-labelledby` names `search-title`, which no element of `V` has",
    );
    // A reference to an id every row has is the row's defect, PW5031.
    refused(
        "<ul aria-describedby=\"line\">{#each lines as line (line.id)}<li id=\"line\">{line.name}</li>{/each}</ul>",
        "PW5031 `line` names every row of a list in `V`",
    );
}

#[test]
fn aria_is_what_wai_aria_defines() {
    refused(
        "<section aria-labeledby=\"h\"><h2 id=\"h\">Cart</h2></section>",
        "PW5033 `aria-labeledby` is no ARIA attribute",
    );
    refused(
        "<p aria-live=\"politely\">Items: {count}</p>",
        "PW5033 `aria-live=\"politely\"` is no value of `aria-live`",
    );
    refused(
        "<p aria-hidden>Hidden</p>",
        "PW5033 `aria-hidden=\"\"` is no value of `aria-hidden`",
    );
    refused(
        "<h2 role=\"heading\" aria-level=\"two\">Cart</h2>",
        "PW5033 `aria-level=\"two\"` is no value of `aria-level`",
    );
    refused(
        "<p role=\"stauts\">Saved</p>",
        "PW5033 `role=\"stauts\"` is no ARIA role",
    );
    refused(
        "<p role=\"status\" aria-relevant=\"additions changes\"></p>",
        "PW5033 `aria-relevant=\"additions changes\"` is no value of `aria-relevant`",
    );
    // The repair names the attribute meant.
    let src = view("<section aria-labeledby=\"h\"><h2 id=\"h\">Cart</h2></section>");
    let repairs: Vec<String> = check_sources(&[("t.pw".to_string(), src)])
        .into_iter()
        .flat_map(|(_, ds)| {
            ds.into_iter()
                .flat_map(|d| d.repairs.into_iter().map(|r| r.description))
        })
        .collect();
    assert!(
        repairs.iter().any(|r| r.contains("`aria-labelledby`")),
        "{repairs:?}"
    );
    // Controls: tokens, numbers, lists of roles and of tokens, a DPUB role,
    // free text and a value the program computes.
    clean(
        "<p role=\"status\" aria-live=\"polite\" aria-atomic=\"true\" aria-relevant=\"additions text\">Saved</p>\n\
         <div role=\"progressbar\" aria-label=\"Order\" aria-valuemin=\"0\" aria-valuemax=\"4\" aria-valuenow=\"2.5\"></div>\n\
         <span role=\"none presentation\" aria-hidden=\"true\">*</span>\n\
         <aside role=\"doc-tip\" aria-description=\"A tip\">Free over $20.</aside>\n\
         <p aria-label=\"Items {count}\">{count}</p>",
    );
}
