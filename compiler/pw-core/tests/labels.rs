//! **What names a form control** (ADR-0143).
//!
//! PW5014 refuses a control nothing names. Until 2026-10-02 it took any `id`
//! as a name, on the assumption that a `<label for>` pointed at it, and
//! nothing checked that one did: T06's unsafe store, a field with an `id`
//! and a placeholder and no label, passed `pw check`. It also refused a
//! control wrapped in its `<label>`, which the HTML standard defines as
//! labelling it.
//!
//! The rule now follows the HTML standard's labeled control and accname 1.2,
//! within the declaration that renders the control. Each test states one
//! part, with controls.

use pw_core::check::check_sources;

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

/// Every diagnostic, as `code message | each related note`.
fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let notes: Vec<String> = d.related.iter().map(|r| r.label.clone()).collect();
                format!("{} {} | {}", d.code, d.message, notes.join(" | "))
            })
        })
        .collect()
}

/// A view, given `caption` and `names`, whose form holds `markup`.
fn view(markup: &str) -> String {
    format!(
        "module t\n\nview V(caption: String, names: List<String>) !{{}} {{\n    <form>\n        \
         {markup}\n        <button type=\"submit\">Go</button>\n    </form>\n}}\n"
    )
}

/// The diagnostics `markup` gets, which must be PW5014's or none.
fn labels(markup: &str) -> Vec<String> {
    let got = reported(&view(markup));
    assert!(
        got.iter().all(|d| d.starts_with("PW5014 ")),
        "only PW5014 is in question here, for {markup}:\n{got:#?}"
    );
    got
}

fn clean(markup: &str) {
    assert_eq!(labels(markup), Vec::<String>::new(), "{markup} is named");
}

/// Refused, with a note that says `why`.
fn refused(markup: &str, why: &str) {
    let got = labels(markup);
    assert_eq!(got.len(), 1, "one unnamed control in {markup}: {got:#?}");
    assert!(
        got[0].contains(why),
        "{markup} is refused, and the note says {why:?}: {got:#?}"
    );
}

#[test]
fn an_id_names_nothing_by_itself() {
    // T06's unsafe store, reduced: before ADR-0143 this passed.
    refused(
        r#"<input id="q" type="text" name="q" placeholder="Query" />"#,
        "no `<label for=\"q\">` in `V` names this `id`",
    );
    refused(
        r#"<label for="other">Query</label><input id="q" type="text" name="q" />"#,
        "no `<label for=\"q\">` in `V` names this `id`",
    );
    // Controls: a label for it, before or after it.
    clean(r#"<label for="q">Query</label><input id="q" type="text" name="q" />"#);
    clean(r#"<input id="q" type="text" name="q" /><label for="q">Query</label>"#);
    clean(r#"<label for="s">Size</label><select id="s" name="s"><option>S</option></select>"#);
}

#[test]
fn a_label_names_its_control_only_when_it_has_text() {
    refused(
        r#"<label for="q"></label><input id="q" type="text" name="q" />"#,
        "this `<label for=\"q\">` has no text",
    );
    refused(
        r#"<label for="q">   </label><input id="q" type="text" name="q" />"#,
        "has no text",
    );
    // Hidden text is not read (accname 1.2, step 2A).
    refused(
        r#"<label for="q"><span aria-hidden="true">*</span></label><input id="q" type="text" name="q" />"#,
        "has no text",
    );
    // Controls: text in an element, an interpolation, an image's `alt`.
    clean(r#"<label for="q"><span>Query</span></label><input id="q" type="text" name="q" />"#);
    clean(r#"<label for="q">{caption}</label><input id="q" type="text" name="q" />"#);
    clean(
        r#"<label for="q"><img src="/q.png" alt="Query" /></label><input id="q" type="text" name="q" />"#,
    );
}

#[test]
fn a_label_that_wraps_its_control_names_it() {
    // Refused before ADR-0143: the rule read only the control's attributes.
    clean(r#"<label>Query <input type="text" name="q" /></label>"#);
    clean(r#"<label>Size <select name="s"><option>S</option></select></label>"#);
    clean(r#"<label>Note <textarea name="n"></textarea></label>"#);
    clean(r#"<label><span>Query</span> <input type="text" name="q" /></label>"#);
    // A hidden input is not labelable, so the one after it is the first.
    clean(
        r#"<label>Query <input type="hidden" name="t" value="1" /><input type="text" name="q" /></label>"#,
    );
    // Controls: no text, and its own options are not its label.
    refused(
        r#"<label><input type="text" name="q" /></label>"#,
        "this `<label>` has no text",
    );
    refused(
        r#"<label><select name="s"><option>S</option></select></label>"#,
        "this `<label>` has no text",
    );
}

#[test]
fn a_wrapping_label_names_only_its_first_control() {
    refused(
        r#"<label>Go <button type="button">x</button> <input type="text" name="q" /></label>"#,
        "names the first control inside it, which is another",
    );
    // With `for`, a label names that element, not what it wraps.
    refused(
        r#"<label for="other">Query <input type="text" name="q" /></label><input id="other" type="text" name="o" />"#,
        "has a `for`, so it names that element, not what it wraps",
    );
}

#[test]
fn which_control_a_label_names_must_not_depend_on_what_renders() {
    // In a block, the first control is whichever renders.
    refused(
        r#"<label>Query {#if caption == ""}<input type="text" name="a" />{/if}</label>"#,
        "depends on what renders inside it",
    );
    // A composed view's markup is not this declaration's to read.
    let src = "module t\n\nview Star() !{} {\n    <span>*</span>\n}\n\n\
               view V() !{} {\n    <form>\n        <label>Query <Star /> <input type=\"text\" \
               name=\"q\" /></label>\n        <button type=\"submit\">Go</button>\n    </form>\n}\n";
    let got = reported(src);
    assert_eq!(got.len(), 1, "{got:#?}");
    assert!(
        got[0].contains("depends on what renders inside it"),
        "{got:#?}"
    );
    // Control: a block that only holds text.
    clean(
        r#"<label>{#if caption == ""}Query{:else}{caption}{/if} <input type="text" name="q" /></label>"#,
    );
}

#[test]
fn aria_labelledby_must_reach_an_element_with_text() {
    refused(
        r#"<input type="text" name="q" aria-labelledby="nowhere" />"#,
        "no element in `V` with text has one of these ids",
    );
    refused(
        r#"<p id="h"></p><input type="text" name="q" aria-labelledby="h" />"#,
        "no element in `V` with text has one of these ids",
    );
    refused(
        r#"<input type="text" name="q" aria-labelledby={caption} />"#,
        "these ids are computed",
    );
    // Controls: one IDREF that reaches text is enough (accname 1.2, 2B).
    clean(r#"<h2 id="h">Query</h2><input type="text" name="q" aria-labelledby="h" />"#);
    clean(r#"<h2 id="h">Query</h2><input type="text" name="q" aria-labelledby="nowhere h" />"#);
    clean(r#"<p id="h" aria-label="Query"></p><input type="text" name="q" aria-labelledby="h" />"#);
}

#[test]
fn a_blank_aria_label_names_nothing() {
    // accname 1.2, step 2C: an empty or blank `aria-label` is skipped.
    refused(
        r#"<input type="text" name="q" aria-label="" />"#,
        "this `aria-label` is blank",
    );
    refused(
        r#"<input type="text" name="q" aria-label="  " />"#,
        "this `aria-label` is blank",
    );
    refused(
        r#"<input type="text" name="q" aria-label />"#,
        "this `aria-label` is blank",
    );
    // Controls: text, and a name the program computes.
    clean(r#"<input type="text" name="q" aria-label="Query" />"#);
    clean(r#"<input type="text" name="q" aria-label={caption} />"#);
}

#[test]
fn a_label_names_the_first_element_with_its_id() {
    // HTML: `for` names the first element in tree order with that `id`.
    refused(
        r#"<p id="q">Note</p><label for="q">Query</label><input id="q" type="text" name="q" />"#,
        "an earlier element has `id=\"q\"`, and a `<label for>` names that one",
    );
    // A computed `id` cannot be matched to a `for`.
    refused(
        r#"<label for="q">Query</label><input id={caption} type="text" name="q" />"#,
        "this `id` is computed",
    );
}

#[test]
fn an_id_inside_each_names_no_one_row() {
    // Every row has the `id`, so a `for` names only the first row's control.
    refused(
        r#"{#each names as n (n)}<label for="qty">{n}</label><input id="qty" type="text" name="qty" />{/each}"#,
        "every row of the `{#each}` has `id=\"qty\"`",
    );
    // Control: each row's label wraps its own control.
    clean(r#"{#each names as n (n)}<label>{n} <input type="text" name="qty" /></label>{/each}"#);
}

#[test]
fn hidden_submit_and_button_inputs_need_no_label() {
    clean(r#"<input type="hidden" name="t" value="1" />"#);
    clean(r#"<input type="HIDDEN" name="t" value="1" />"#);
    clean(r#"<input type="submit" value="Send" />"#);
    clean(r#"<input type="button" value="Send" />"#);
}
