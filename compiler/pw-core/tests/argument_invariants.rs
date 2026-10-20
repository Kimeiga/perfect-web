//! **What a host holds a value from outside to, as the build states it**
//! (ADR-0346, argument invariants; ADR-0179, ADR-0225).
//!
//! A value of an opaque type with a `where` invariant exists only if it
//! passed the invariant. The build states, from each type's declaration and
//! never per program, where a value a browser gives must hold one: each
//! command's arguments in its contract, at the top and inside (a list's
//! item, a record's field, an `Option`'s inside, a case's payload, a map's
//! key and value, a set's item), and each page's parameters in its plan.

use pw_core::check::Unit;
use serde_json::json;

fn units(extra: &[(&str, &str)]) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut here: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        here.sort();
        paths.extend(here);
    }
    let mut sources: Vec<(String, String)> = paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            (p.display().to_string(), src)
        })
        .collect();
    for (path, src) in extra {
        sources.push((path.to_string(), src.to_string()));
    }
    sources
        .into_iter()
        .map(|(path, src)| Unit {
            hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
            path,
            src,
        })
        .collect()
}

/// `examples/demo/invariants.pw`, with the domain it imports, built.
fn fixture() -> pw_core::build::Build {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).expect("read");
    let (domain, demo) = (
        read("examples/domain.pw"),
        read("examples/demo/invariants.pw"),
    );
    let build = pw_core::build::build(&units(&[
        ("examples/domain.pw", &domain),
        ("examples/demo/invariants.pw", &demo),
    ]))
    .expect("the fixture builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    build
}

/// What the contract of `id` states its export's arguments must hold.
fn arguments_hold(build: &pw_core::build::Build, id: &str) -> serde_json::Value {
    let contract = build
        .contracts
        .iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("no contract `{id}`"));
    let export = contract.exports[0]
        .component
        .as_ref()
        .expect("the contract locates its export");
    serde_json::to_value(&export.bounded).expect("serializes")
}

fn stars(argument: usize, path: &[&str]) -> serde_json::Value {
    let mut b = json!({
        "argument": argument,
        "type": "demo.invariants.Stars",
        "holds": "value >= 1 & value <= 5",
        "at_least": 1,
        "at_most": 5,
    });
    if !path.is_empty() {
        b["path"] = json!(path);
    }
    b
}

fn note(argument: usize, path: &[&str]) -> serde_json::Value {
    let mut b = json!({
        "argument": argument,
        "type": "demo.invariants.Note",
        "holds": "String.length(value) <= 20",
        "measure": "length",
        "at_most": 20,
    });
    if !path.is_empty() {
        b["path"] = json!(path);
    }
    b
}

fn code(argument: usize) -> serde_json::Value {
    json!({
        "argument": argument,
        "type": "demo.invariants.Code",
        "holds": "String.length(value) >= 2 & String.length(value) <= 8",
        "measure": "length",
        "at_least": 2,
        "at_most": 8,
    })
}

/// **A page's plan states what each of its parameters must hold**: its
/// path's `Code` where it is, and its query's `Note` inside the `Option` the
/// address gives. A page whose parameters' types state nothing states
/// nothing.
#[test]
fn a_pages_plan_states_what_its_parameters_must_hold() {
    let build = fixture();
    let plan = |page: &str| {
        build
            .pages
            .iter()
            .find(|p| p.page == page)
            .unwrap_or_else(|| panic!("no page `{page}`"))
            .plan
            .clone()
            .expect("planned")
    };
    let held = plan("demo.invariants.InvariantsPage~ui");
    assert_eq!(held.params, ["code", "note"]);
    assert_eq!(
        serde_json::to_value(&held.bounded).expect("serializes"),
        json!([code(0), note(1, &["some"])])
    );
    // Written only where something is stated: the plan's JSON of a page
    // with nothing to hold has no `bounded` at all.
    let mut nothing = held.clone();
    nothing.bounded.clear();
    let written = serde_json::to_value(&nothing).expect("serializes");
    assert!(written.get("bounded").is_none(), "{written}");
}

/// **A command's contract states each place inside its arguments**: at the
/// top, in a list's items' fields, in an `Option` inside them, in each case's
/// payload, and in an `Option` alone.
#[test]
fn a_commands_contract_states_each_place_inside_its_arguments() {
    let build = fixture();
    assert_eq!(
        arguments_hold(&build, "demo.invariants.rate"),
        json!([code(0), stars(1, &[]), note(2, &[])])
    );
    assert_eq!(
        arguments_hold(&build, "demo.invariants.rate_all"),
        json!([stars(0, &["*", "stars"]), note(0, &["*", "note", "some"])])
    );
    assert_eq!(
        arguments_hold(&build, "demo.invariants.mark"),
        json!([stars(0, &["starred"]), note(0, &["noted"])])
    );
    assert_eq!(
        arguments_hold(&build, "demo.invariants.annotate"),
        json!([note(0, &["some"])])
    );
}

/// **A map crosses as its entries and a set as its items** (`list<tuple<k,
/// v>>` and `list<t>`, ADR-0057): a key at `0` of each entry, a value at `1`,
/// and a set's item as a list's is.
#[test]
fn a_maps_keys_and_values_and_a_sets_items_are_stated() {
    let program = "module m\n\nimport Map\nimport Set\n\n\
                   opaque type Likes = Int where value >= 0\n\n\
                   public query Valued(m: Map<String, Likes>) -> Int { Map.size(m) }\n\n\
                   public query Keyed(m: Map<Likes, String>) -> Int { Map.size(m) }\n\n\
                   public query Counted(s: Set<Likes>) -> Int { Set.size(s) }\n";
    let build = pw_core::build::build(&units(&[("m.pw", program)])).expect("builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let likes = |path: &[&str]| {
        json!({
            "argument": 0,
            "path": path,
            "type": "m.Likes",
            "holds": "value >= 0",
            "at_least": 0,
        })
    };
    assert_eq!(
        arguments_hold(&build, "m.Valued"),
        json!([likes(&["*", "1"])])
    );
    assert_eq!(
        arguments_hold(&build, "m.Keyed"),
        json!([likes(&["*", "0"])])
    );
    assert_eq!(arguments_hold(&build, "m.Counted"), json!([likes(&["*"])]));
}
