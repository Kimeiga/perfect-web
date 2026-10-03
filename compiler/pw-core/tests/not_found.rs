//! **A page says when it is absent** (ADR-0163).
//!
//! `not_found_on StoreError.NotFound` names the declared error that means a
//! page's address names nothing, and a host answers it 404 rather than 503.
//! Until 2026-10-03 a page whose queries failed was answered 503 whatever
//! they answered (ADR-0147), so `/stores/999` said "try again later" of a
//! store that will never be there.
//!
//! - PW0342: the clause is a page's, names a case of a declared type, and a
//!   query the page reads can answer it.
//! - The page's plan carries the case, by its WIT name, on each binding whose
//!   query can answer it, and on no other.
//!
//! Each test states one part, with controls that check clean.

use pw_core::check::check_sources;

fn reported_in(sources: &[(&str, &str)]) -> Vec<String> {
    let sources: Vec<(String, String)> = sources
        .iter()
        .map(|(n, s)| (n.to_string(), s.to_string()))
        .collect();
    check_sources(&sources)
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn reported(src: &str) -> Vec<String> {
    reported_in(&[("t.pw", src)])
}

/// The queries' policies, as the store declares them.
const POLICIES: &str = "    freshness     30.seconds\n    consistency   snapshot\n    \
                        cache         shared\n    key           id\n    \
                        concurrency   one_per_key\n    on_key_change cancel\n    \
                        timeout       2.seconds\n";

/// The declarations a store page reads: a store, which is not found or
/// unavailable, and its hours, whose error is another type.
fn declarations() -> String {
    format!(
        "opaque type StoreId = String\n\n\
         type Shop = Shop {{ name: String }}\n\n\
         type StoreError =\n    | NotFound\n    | Unavailable\n    | DecodeFailed(String)\n\n\
         type HoursError =\n    | Closed\n\n\
         public query Store(id: StoreId) -> Result<Shop, StoreError>\n{POLICIES}{{\n    todo\n}}\n\n\
         public query Hours(id: StoreId) -> Result<String, HoursError>\n{POLICIES}{{\n    todo\n}}\n\n"
    )
}

/// The store page, its `not_found_on` clause `clause` (none when empty),
/// reading the store and its hours.
fn page(clause: &str) -> String {
    let clause = match clause {
        "" => String::new(),
        c => format!("    not_found_on {c}\n"),
    };
    format!(
        "page P(id: StoreId) {{\n    route \"/stores/{{id}}\"\n{clause}    cache private\n\n    \
         let store = query Store(id)\n    let hours = query Hours(id)\n\n    \
         view {{\n        <main><h1>{{store.name}}</h1><p>{{hours}}</p></main>\n    }}\n}}\n"
    )
}

fn program(clause: &str) -> String {
    format!("module t\n\n{}{}", declarations(), page(clause))
}

#[test]
fn a_page_that_names_its_absence_is_the_control() {
    for clause in [
        "",
        "StoreError.NotFound",
        // Any case of the type: what it means is the page's to say.
        "StoreError.DecodeFailed",
        "HoursError.Closed",
    ] {
        let found = reported(&program(clause));
        assert!(found.is_empty(), "{clause:?}: {found:#?}");
    }
}

#[test]
fn the_clause_is_a_pages() {
    let src = format!(
        "module t\n\n{}{}",
        declarations().replace(
            "public query Store(id: StoreId) -> Result<Shop, StoreError>\n",
            "public query Store(id: StoreId) -> Result<Shop, StoreError>\n    \
             not_found_on StoreError.NotFound\n"
        ),
        page("")
    );
    assert_eq!(
        reported(&src),
        ["PW0342 `not_found_on StoreError.NotFound` is a page's: `Store` is not one"]
    );
}

#[test]
fn the_clause_names_a_case_of_a_declared_type() {
    for (clause, why) in [
        ("NotFound", "`NotFound` is not a case: write `Type.Case`"),
        ("Missing.NotFound", "`Missing` names no type visible here"),
        ("StoreError.Gone", "`StoreError` has no case `Gone`"),
        // A record has no cases.
        ("Shop.Shop", "`Shop` has no case `Shop`"),
        // A query is no type.
        ("Store.NotFound", "`Store` names no type visible here"),
    ] {
        assert_eq!(
            reported(&program(clause)),
            [format!("PW0342 `not_found_on {clause}`: {why}")],
            "{clause}"
        );
    }
}

#[test]
fn a_query_the_page_reads_can_answer_it() {
    // A declared error none of the page's queries answers: the clause would
    // never fire, and the absence it names would be answered 503.
    let src = program("OrderError.Missing").replace(
        "type HoursError",
        "type OrderError =\n    | Missing\n\ntype HoursError",
    );
    assert_eq!(
        reported(&src),
        ["PW0342 `not_found_on OrderError.Missing`: no query `P` reads can answer it"]
    );
    // A query whose value holds the error answers with it, and does not fail
    // with it.
    let src = program("StoreError.NotFound")
        .replace("-> Result<Shop, StoreError>", "-> Map<String, StoreError>")
        .replace("<h1>{store.name}</h1>", "<h1>Store</h1>");
    assert_eq!(
        reported(&src),
        ["PW0342 `not_found_on StoreError.NotFound`: no query `P` reads can answer it"]
    );
}

#[test]
fn an_error_imported_from_another_module_is_named_as_it_is_imported() {
    // The store's own shape: the error is the domain's, imported by name.
    let domain = "module d\n\n\
                  type StoreError =\n    | NotFound\n    | Unavailable\n";
    let app = program("StoreError.NotFound")
        .replace("module t\n\n", "module t\n\nimport d.{ StoreError }\n\n")
        .replace(
            "type StoreError =\n    | NotFound\n    | Unavailable\n    | DecodeFailed(String)\n\n",
            "",
        );
    let found = reported_in(&[("d.pw", domain), ("t.pw", &app)]);
    assert!(found.is_empty(), "{found:#?}");
    // Or by its module's name.
    let qualified = app.replace(
        "not_found_on StoreError.NotFound",
        "not_found_on d.StoreError.NotFound",
    );
    let found = reported_in(&[("d.pw", domain), ("t.pw", &qualified)]);
    assert!(found.is_empty(), "{found:#?}");
    // And a case the imported type does not have is refused as one.
    let gone = app.replace(
        "not_found_on StoreError.NotFound",
        "not_found_on StoreError.Gone",
    );
    assert_eq!(
        reported_in(&[("d.pw", domain), ("t.pw", &gone)]),
        ["PW0342 `not_found_on StoreError.Gone`: `StoreError` has no case `Gone`"]
    );
}

/// **The plan carries the case** (ADR-0163), by its WIT name, on each
/// binding whose query can answer it: a host answers 404 for it.
#[test]
fn the_plan_carries_the_case_on_the_bindings_that_can_answer_it() {
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;
    let not_found = |clause: &str| -> Vec<(String, Vec<String>)> {
        let src = program(clause);
        let hir = lower_file(&src, &pw_syntax::parse_tree(&src).green);
        let hirs = vec![&hir];
        let ws = Workspace::build(&hirs);
        let sigs = Signatures::build(&ws, &hirs);
        let planned = pw_core::page_values::pages(&hirs, &ws, &sigs);
        let plan = planned
            .iter()
            .find(|p| p.page == "t.P")
            .expect("a plan for P")
            .plan
            .as_ref()
            .unwrap_or_else(|e| panic!("P does not plan: {e}"));
        plan.bindings
            .iter()
            .map(|b| (b.binding.clone(), b.not_found.clone()))
            .collect()
    };
    let named = |pairs: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        pairs
            .iter()
            .map(|(b, cases)| (b.to_string(), cases.iter().map(|c| c.to_string()).collect()))
            .collect()
    };
    assert_eq!(
        not_found("StoreError.NotFound"),
        named(&[("store", &["not-found"]), ("hours", &[])])
    );
    assert_eq!(
        not_found("HoursError.Closed"),
        named(&[("store", &[]), ("hours", &["closed"])])
    );
    // A page that names none is answered 503 for any failure.
    assert_eq!(not_found(""), named(&[("store", &[]), ("hours", &[])]));
}
