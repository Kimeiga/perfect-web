//! **A page says when its address is another address of the page**
//! (ADR-XXXX).
//!
//! `redirect_on StoreError.Moved permanent` names the declared error that
//! means a page's address is another of the page's, and how the move is
//! answered: 308 for good, 307 for now. kiokun.com answers 308 to the one
//! traditional form a simplified character equals in meaning.
//!
//! - PW0350: the clause is a page's, names a case of a declared type and how
//!   it moves, is not the case `not_found_on` names, and a query the page
//!   reads can answer it; the case carries one value, of the type of the one
//!   parameter the page's route carries.
//! - The page's plan carries the case, by its WIT name, and how it moves, on
//!   each binding whose query can answer it, and on no other.
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

/// A store page's declarations: a store, which is not found, moved to another
/// id, or unavailable, and its hours, whose error is another type.
fn declarations() -> String {
    format!(
        "opaque type StoreId = String\n\n\
         type Shop = Shop {{ name: String }}\n\n\
         type StoreError =\n    | NotFound\n    | Moved(StoreId)\n    | Renamed(String)\n    \
         | Split(StoreId, StoreId)\n    | Unavailable\n\n\
         type HoursError =\n    | Closed\n\n\
         public query Store(id: StoreId) -> Result<Shop, StoreError>\n{POLICIES}{{\n    todo\n}}\n\n\
         public query Hours(id: StoreId) -> Result<String, HoursError>\n{POLICIES}{{\n    todo\n}}\n\n"
    )
}

/// The store page at `route`, its `redirect_on` clause `clause` (none when
/// empty), reading the store and its hours.
fn page_at(route: &str, clause: &str) -> String {
    let clause = match clause {
        "" => String::new(),
        c => format!("    redirect_on {c}\n"),
    };
    format!(
        "page P(id: StoreId) {{\n    route \"{route}\"\n    not_found_on StoreError.NotFound\n\
         {clause}    cache private\n\n    \
         let store = query Store(id)\n    let hours = query Hours(id)\n\n    \
         view {{\n        <title>{{store.name}}</title>\n        \
         <main><h1>{{store.name}}</h1><p>{{hours}}</p></main>\n    }}\n}}\n"
    )
}

fn program(clause: &str) -> String {
    format!(
        "module t\n\n{}{}",
        declarations(),
        page_at("/stores/{id}", clause)
    )
}

#[test]
fn a_page_that_names_its_move_is_the_control() {
    for clause in [
        "",
        "StoreError.Moved permanent",
        "StoreError.Moved temporary",
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
             redirect_on StoreError.Moved permanent\n"
        ),
        page_at("/stores/{id}", "")
    );
    assert_eq!(
        reported(&src),
        ["PW0350 `redirect_on StoreError.Moved permanent` is a page's: `Store` is not one"]
    );
}

#[test]
fn the_clause_names_a_case_and_how_it_moves() {
    let how = "write `Type.Case permanent`, or `Type.Case temporary`";
    for (clause, why) in [
        ("StoreError.Moved", how.to_string()),
        ("StoreError.Moved permanent now", how.to_string()),
        (
            "StoreError.Moved forever",
            "`forever` is neither `permanent` nor `temporary`".to_string(),
        ),
        (
            "Moved permanent",
            "`Moved` is not a case: write `Type.Case`".to_string(),
        ),
        (
            "StoreError.Gone permanent",
            "`StoreError` has no case `Gone`".to_string(),
        ),
        (
            "Missing.Moved permanent",
            "`Missing` names no type visible here".to_string(),
        ),
    ] {
        assert_eq!(
            reported(&program(clause)),
            [format!("PW0350 `redirect_on {clause}`: {why}")],
            "{clause}"
        );
    }
}

#[test]
fn a_page_that_is_absent_is_not_elsewhere() {
    assert_eq!(
        reported(&program("StoreError.NotFound permanent")),
        [
            "PW0350 `redirect_on StoreError.NotFound permanent`: `NotFound` is the case \
             `not_found_on` names, and a page that is absent is not elsewhere"
        ]
    );
}

#[test]
fn a_query_the_page_reads_can_answer_it() {
    let src = program("OrderError.Moved permanent").replace(
        "type HoursError",
        "type OrderError =\n    | Moved(StoreId)\n\ntype HoursError",
    );
    assert_eq!(
        reported(&src),
        ["PW0350 `redirect_on OrderError.Moved permanent`: no query `P` reads can answer it"]
    );
}

#[test]
fn the_case_carries_the_one_value_the_route_carries() {
    for (clause, carried) in [
        // Nothing to fill the route with.
        ("StoreError.Unavailable permanent", "nothing"),
        // Another type than the route's parameter: a `String` is not a
        // `StoreId`, though one is the other underneath.
        ("StoreError.Renamed permanent", "a `String`"),
        // Two values for one parameter.
        ("StoreError.Split permanent", "2 values"),
    ] {
        assert_eq!(
            reported(&program(clause)),
            [format!(
                "PW0350 `redirect_on {clause}`: `{}` carries {carried}, and `P`'s route \
                 carries one `id`, a `StoreId`",
                clause
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .split('.')
                    .nth(1)
                    .unwrap()
            )],
            "{clause}"
        );
    }
    // A route that carries no parameter, or two, has no one place for it.
    let fixed = format!(
        "module t\n\n{}{}",
        declarations(),
        page_at("/the-store", "StoreError.Moved permanent")
            .replace("page P(id: StoreId)", "page P()")
            .replace("query Store(id)", "query Store(StoreId.the())")
            .replace("query Hours(id)", "query Hours(StoreId.the())")
    );
    let reported_fixed = reported(&fixed);
    assert!(
        reported_fixed.iter().any(|r| r.starts_with(
            "PW0350 `redirect_on StoreError.Moved permanent`: `P`'s route carries 0 parameters"
        )),
        "{reported_fixed:#?}"
    );
    let two = format!(
        "module t\n\n{}{}",
        declarations(),
        page_at("/stores/{id}/{shelf}", "StoreError.Moved permanent")
            .replace("page P(id: StoreId)", "page P(id: StoreId, shelf: String)")
            .replace("<p>{hours}</p>", "<p>{hours} {shelf}</p>")
    );
    assert_eq!(
        reported(&two),
        [
            "PW0350 `redirect_on StoreError.Moved permanent`: `P`'s route carries 2 parameters, \
          and a case carries one value to fill it"
        ]
    );
}

/// **The plan carries the case and how it moves** (ADR-XXXX), on each
/// binding whose query can answer it: a host answers 308 or 307 for it.
#[test]
fn the_plan_carries_the_move_on_the_bindings_that_can_answer_it() {
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;
    let redirects = |clause: &str| -> Vec<(String, Option<(String, bool)>)> {
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
            .map(|b| {
                (
                    b.binding.clone(),
                    b.redirect.as_ref().map(|r| (r.case.clone(), r.permanent)),
                )
            })
            .collect()
    };
    assert_eq!(
        redirects("StoreError.Moved permanent"),
        [
            ("store".to_string(), Some(("moved".to_string(), true))),
            ("hours".to_string(), None),
        ]
    );
    assert_eq!(
        redirects("StoreError.Moved temporary"),
        [
            ("store".to_string(), Some(("moved".to_string(), false))),
            ("hours".to_string(), None),
        ]
    );
    // Control: a page that names none carries none.
    assert_eq!(
        redirects(""),
        [("store".to_string(), None), ("hours".to_string(), None)]
    );
}
