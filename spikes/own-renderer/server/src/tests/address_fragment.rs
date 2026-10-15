//! **A page's fragment binding reaches its document** (track `todomvc`,
//! ADR-XXXX): `fragment tab "/" => "overview", ..` binds a signal to the
//! address's fragment, and the document of a page that states it says which
//! signal and each form the fragment takes, for the browser to keep the two
//! in step. A host renders the signal's first value: a request carries no
//! fragment. Each test states one case, with its control.

use super::*;

/// A page whose one block a signal decides.
fn template() -> Template {
    serde_json::from_value(serde_json::json!({
        "path": "demo.fragment.FragmentPage",
        "name": "FragmentPage",
        "params": [],
        "schema": "s",
        "chunks": [
            { "chunk": "static", "value": "<main>" },
            { "chunk": "dynamic", "value": {
                "part": "conditional", "id": 0, "value": "open",
                "then": [{ "chunk": "static", "value": "<p id=\"shown\">open</p>" }],
                "otherwise": []
            } },
            { "chunk": "static", "value": "</main>" }
        ]
    }))
    .expect("a template")
}

/// A plan with one signal, and the binding where `fragment` is given.
fn plan(fragment: Option<serde_json::Value>) -> serde_json::Value {
    let mut plan = serde_json::json!({
        "signals": [{ "name": "open", "initial": false }],
        "live": [{ "part": 0, "signal": "open", "path": "open", "kind": "conditional",
                   "reads": ["open"] }]
    });
    if let Some(binding) = fragment {
        plan["fragment"] = binding;
    }
    plan
}

fn binding() -> serde_json::Value {
    serde_json::json!({
        "signal": "open",
        "forms": [
            { "form": "/", "value": false },
            { "form": "/open", "value": true }
        ]
    })
}

/// The manifest of the document of a page that reads queries.
fn queried(plan: &serde_json::Value) -> serde_json::Value {
    let templates = vec![template()];
    manifest_of(&document(
        "<main></main>",
        "Fragment",
        "",
        "",
        &templates[0],
        &templates,
        plan,
        3,
        None,
        &BTreeMap::new(),
        COMMITTED_ARTIFACTS,
    ))
}

/// The manifest of the document of a page of signals alone.
fn alone(plan: &serde_json::Value) -> serde_json::Value {
    let templates = vec![template()];
    manifest_of(&signal_document(
        "<main></main>",
        "Fragment",
        "",
        &templates[0],
        plan,
        &templates,
        COMMITTED_ARTIFACTS,
    ))
}

#[test]
fn the_document_of_a_page_that_reads_queries_names_its_binding() {
    let manifest = queried(&plan(Some(binding())));
    assert_eq!(manifest["fragment"], binding(), "{manifest}");
    // Its signals and what they decide are carried as they were.
    assert_eq!(manifest["signals"], serde_json::json!({ "open": false }));
    // The control: a page that binds none says none.
    assert!(queried(&plan(None)).get("fragment").is_none());
}

#[test]
fn the_document_of_a_page_of_signals_alone_names_its_binding() {
    let manifest = alone(&plan(Some(binding())));
    assert_eq!(manifest["fragment"], binding(), "{manifest}");
    assert_eq!(manifest["signals"], serde_json::json!({ "open": false }));
    assert!(alone(&plan(None)).get("fragment").is_none());
}

/// **A host renders the signal's first value**, whatever the address's
/// fragment was: a request carries none, so the document is the same bytes.
#[test]
fn the_host_renders_the_first_value_whatever_the_fragment() {
    let template = template();
    let plan = plan(Some(binding()));
    let shut =
        pw_render::render(&template, &with_signals(Env::new(), &plan), &[]).expect("renders");
    assert!(!shut.contains("id=\"shown\""), "{shut}");
    // The control: the same page where the signal starts at the form's other
    // value renders its block, so the render does read the first value.
    let mut open = plan.clone();
    open["signals"] = serde_json::json!([{ "name": "open", "initial": true }]);
    let shown =
        pw_render::render(&template, &with_signals(Env::new(), &open), &[]).expect("renders");
    assert!(shown.contains("id=\"shown\""), "{shown}");
}
