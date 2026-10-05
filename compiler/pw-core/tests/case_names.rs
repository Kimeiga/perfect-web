//! **A pattern tells a case from a binding by its capital** (ADR-0197,
//! ADR-0195's ruling 2), as Haskell, OCaml and Elm do.
//!
//! Until ADR-0197 a bare name in a pattern was a case where some type in the
//! program had a case of that name, and a binding otherwise; and six analyses
//! each decided it again. A misspelt case, `Circel`, bound a name and matched
//! every value: rustc's E0170. The parser now decides once.

use pw_core::check::check_sources;

/// The standard packages, then the program.
fn reported(program: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            sources.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    sources.push(("m.pw".to_string(), program.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(path, _)| path == "m.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

const SHAPE: &str = "module m\n\ntype Shape =\n    | Circle(Int)\n    | Empty\n\n";

#[test]
fn a_misspelt_case_is_a_case_its_type_lacks_not_a_binding() {
    let misspelt = format!(
        "{SHAPE}fn area(s: Shape) -> Int {{\n    match s {{\n        Circle(r) => r,\n        Emty => 0,\n    }}\n}}\n"
    );
    let found = reported(&misspelt);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0608") && d.contains("Emty")),
        "{found:?}"
    );
    // Control: spelt as declared, the match is exhaustive and clean.
    let spelt = misspelt.replace("Emty", "Empty");
    assert_eq!(reported(&spelt), Vec::<String>::new());
    // And a lowercase name binds, and takes every value it is given.
    let bound = misspelt.replace("Emty", "other");
    assert_eq!(reported(&bound), Vec::<String>::new());
}

#[test]
fn a_case_written_twice_in_one_pattern_binds_nothing() {
    // `Draft` is a case, twice: no name is bound twice (PW0028 was reported).
    let program = "module m\n\ntype Doc =\n    | Draft\n    | Done\n\ntype Pair = | Pair(Doc, Doc)\n\nfn both(p: Pair) -> Int {\n    match p {\n        Pair(Draft, Draft) => 2,\n        Pair(_, _) => 0,\n    }\n}\n";
    assert_eq!(reported(program), Vec::<String>::new());
}

#[test]
fn a_case_named_in_lowercase_is_refused_where_it_is_declared() {
    let program = "module m\n\ntype Status =\n    | active\n    | Done\n";
    let found = reported(program);
    assert!(
        found.iter().any(|d| d.starts_with("PW0625")
            && d.contains("`Status`'s case `active` is not named with a capital letter")),
        "{found:?}"
    );
    // Control: named with a capital, it is clean.
    assert_eq!(
        reported(&program.replace("active", "Active")),
        Vec::<String>::new()
    );
}

#[test]
fn true_and_false_are_bools_cases_in_a_pattern() {
    let program = "module m\n\nfn flip(b: Bool) -> Int {\n    match b {\n        true => 1,\n        false => 0,\n    }\n}\n";
    assert_eq!(reported(program), Vec::<String>::new());
    // One missing is a missing case.
    let missing = program.replace("        false => 0,\n", "");
    assert!(
        reported(&missing).iter().any(|d| d.starts_with("PW0305")),
        "{:?}",
        reported(&missing)
    );
}

#[test]
fn another_types_case_is_refused_against_this_one() {
    // `None` is `Option`'s case, against a `Shape`: refused, as before.
    let program = format!(
        "{SHAPE}fn area(s: Shape) -> Int {{\n    match s {{\n        Circle(r) => r,\n        None => 0,\n    }}\n}}\n"
    );
    let found = reported(&program);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0608") && d.contains("None")),
        "{found:?}"
    );
}

#[test]
fn a_template_arms_field_that_is_a_case_is_no_arm() {
    // `{:Some(Draft)}` bound a name `Draft` until ADR-0197, and matched every
    // `Some`. An arm takes one case apart, so a field that is a case is a
    // nested pattern no arm has: refused by name.
    let program = "module m\n\ntype Doc =\n    | Draft\n    | Done\n\nview V(d: Option<Doc>) !{} {\n    <p>{#match d}{:Some(Draft)}draft{:Some(x)}other{:None}none{/match}</p>\n}\n";
    let found = reported(program);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5019") && d.contains("`{:Some(Draft)}` is not an arm")),
        "{found:?}"
    );
    // Control: a binding there is an arm.
    let bound = program.replace("{:Some(Draft)}draft", "");
    assert_eq!(reported(&bound), Vec::<String>::new());
}
