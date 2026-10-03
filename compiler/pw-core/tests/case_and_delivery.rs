//! **Two forms gate item 5 found open** (ADR-0153, ADR-0154).
//!
//! - A template tested a value's case with `{#if status == Placed}`, which
//!   nothing held to cover the cases. PW0337 refuses it: `{#match}` names
//!   each case, and PW0305 holds it to every one.
//! - A command a page's handler calls could declare no `idempotent_by`, and
//!   ran again on a second delivery of its request. PW0338 refuses it.
//!
//! Each test states one part, with controls.

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

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn with(code: &str, reported: &[String]) -> Vec<String> {
    reported
        .iter()
        .filter(|d| d.starts_with(code))
        .cloned()
        .collect()
}

// --- ADR-0153 ------------------------------------------------------------------

/// A page showing an order's status: `markup` in its view.
fn order_page(markup: &str) -> String {
    format!(
        "module t\n\n\
         type Status =\n    | Placed\n    | Preparing\n    | Delivered\n\n\
         page P(status: Status, maybe: Option<String>, other: Status, ready: Bool) {{\n    cache private\n\n    \
         view {{\n        <main>\n            {markup}\n        </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_template_tests_a_case_with_match() {
    for markup in [
        "{#if status == Placed}<p>placed</p>{/if}",
        "{#if Placed == status}<p>placed</p>{/if}",
        "{#if status != Delivered}<p>coming</p>{/if}",
        "{#if status == Status.Placed}<p>placed</p>{/if}",
        "{#if maybe == None}<p>none</p>{/if}",
        // Only the `{:else if}` tests a case.
        "{#if ready}<p>a</p>{:else if status == Placed}<p>b</p>{/if}",
    ] {
        let reported = reported(&order_page(markup));
        let refused = with("PW0337", &reported);
        assert!(!refused.is_empty(), "{markup}: {reported:?}");
        assert!(refused[0].contains("`{#match "), "{markup}: {refused:?}");
    }
}

#[test]
fn every_case_named_by_a_match_is_the_control() {
    let markup = "{#match status}{:Placed}<p>placed</p>{:Preparing}{:Delivered}{/match}";
    assert_eq!(reported(&order_page(markup)), Vec::<String>::new());
}

#[test]
fn a_case_of_another_type_spelled_alike_is_not_this_ones() {
    // `Other.Placed` is `Other`'s case, whatever it is spelled: the
    // qualifier is resolved, not read (RISK_QUEUE 34).
    let mut sources = library();
    sources.push((
        "u.pw".to_string(),
        "module u\n\ntype Other =\n    | Placed\n    | Gone\n".to_string(),
    ));
    sources.push((
        "t.pw".to_string(),
        order_page("{#if status == Other.Placed}<p>placed</p>{/if}")
            .replace("module t\n", "module t\n\nimport u.{ Other }\n"),
    ));
    let reported: Vec<String> = check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    assert!(with("PW0337", &reported).is_empty(), "{reported:?}");
}

#[test]
fn a_value_compared_with_a_computed_one_is_not_a_case_test() {
    // `other` is a value of the same type, not one of its cases.
    let markup = "{#if status == other}<p>same</p>{/if}";
    assert!(with("PW0337", &reported(&order_page(markup))).is_empty());
}

// --- ADR-0154 ------------------------------------------------------------------

/// A page whose button calls `add`, declared with `policies`, and a command
/// no handler calls.
fn command_page(policies: &str) -> String {
    format!(
        "module t\n\n\
         opaque type InteractionId = String\n\n\
         command add(n: Int) -> Result<Int, String>\n    requires      SignedIn\n{policies}\
         {{\n    Ok(n)\n}}\n\n\
         command internal(n: Int) -> Result<Int, String>\n    requires      SignedIn\n\
         {{\n    Ok(n)\n}}\n\n\
         page P() {{\n    cache private\n\n    view {{\n        <main>\n            \
         <button type=\"button\" on:press={{() => add(1)}}>Add</button>\n        \
         </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_command_a_page_handler_calls_declares_idempotent_by() {
    let reported = reported(&command_page(""));
    assert_eq!(
        with("PW0338", &reported),
        ["PW0338 `add` is called by a page's handler, and declares no `idempotent_by`"],
        "{reported:?}"
    );
}

#[test]
fn an_idempotent_command_and_one_no_handler_calls_are_the_controls() {
    let reported = reported(&command_page("    idempotent_by InteractionId\n"));
    assert!(with("PW0338", &reported).is_empty(), "{reported:?}");
    // `internal` is called by nothing a browser sends.
    assert!(
        !reported.iter().any(|d| d.contains("`internal`")),
        "{reported:?}"
    );
}
