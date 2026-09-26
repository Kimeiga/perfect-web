//! **A read names a member its value's type has** (PW0610, ADR-0048).
//!
//! `box.x` on a type with no `x`, or `{row.status}` on a type without it, was
//! unknown to every analysis and passed `pw check`. A-015 read two members a
//! `Rect` snapshot does not have, and the store's page rendered a cart field
//! the `Cart` type does not declare. Each test here is one reading of the
//! rule, with its control.

use pw_core::check::check_sources;
use pw_core::values::{Outcome, RelationKind};

/// The standard library and the web platform, as a real program has them.
fn platform() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    let mut out = Vec::new();
    for dir in ["pw-std", "pw-platform-web"] {
        for e in std::fs::read_dir(root.join(dir)).expect("package") {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push((
                    p.display().to_string(),
                    std::fs::read_to_string(&p).expect("read"),
                ));
            }
        }
    }
    out.sort();
    out
}

/// `(code, message)` for every diagnostic of the last file among `files`.
fn diagnostics(files: &[(&str, &str)]) -> Vec<(&'static str, String)> {
    let mut all = platform();
    all.extend(files.iter().map(|(n, s)| (n.to_string(), s.to_string())));
    let checked = check_sources(&all);
    checked
        .last()
        .expect("a file")
        .1
        .iter()
        .map(|d| (d.code, d.message.clone()))
        .collect()
}

fn codes(src: &str) -> Vec<&'static str> {
    diagnostics(&[("t.pw", src)])
        .into_iter()
        .map(|(c, _)| c)
        .collect()
}

const POINT: &str = "module m\n\ntype Point = Point { x: Int, y: Int }\n\n";

#[test]
fn a_field_the_record_does_not_have_is_refused() {
    let src = format!("{POINT}fn f(p: Point) -> Int !{{}} {{\n    p.z\n}}\n");
    let found = diagnostics(&[("t.pw", &src)]);
    assert!(
        found
            .iter()
            .any(|(c, m)| *c == "PW0610" && m.contains("m.Point") && m.contains("`z`")),
        "{found:?}"
    );

    // Control: a field it has.
    let fine = src.replace("p.z", "p.x");
    assert_eq!(codes(&fine), Vec::<&str>::new());
}

#[test]
fn a_declaration_taking_the_type_is_a_member() {
    // Read as a property, and called as a method.
    let src = format!(
        "{POINT}fn norm(p: Point) -> Int !{{}} {{ p.x + p.y }}\n\n\
         fn scale(p: Point, k: Int) -> Int !{{}} {{ p.x * k }}\n\n\
         fn f(p: Point) -> Int !{{}} {{\n    p.norm + p.scale(2)\n}}\n"
    );
    assert_eq!(codes(&src), Vec::<&str>::new());

    let missing = src.replace("p.scale(2)", "p.stretch(2)");
    assert!(codes(&missing).contains(&"PW0610"), "{:?}", codes(&missing));
}

#[test]
fn a_template_hole_reads_a_member_its_item_has() {
    let src = format!(
        "{POINT}view V(points: List<Point>) !{{}} {{\n    <ul>\n        {{#each points as p (p.x)}}\n            <li>{{p.status}}</li>\n        {{/each}}\n    </ul>\n}}\n"
    );
    assert!(codes(&src).contains(&"PW0610"), "{:?}", codes(&src));

    let fine = src.replace("{p.status}", "{p.y}");
    assert_eq!(codes(&fine), Vec::<&str>::new());
}

#[test]
fn a_value_of_unknown_type_is_not_judged() {
    // `x` has no written type: nothing is claimed about its members.
    let src = "module m\n\nfn f(x) -> Int !{} {\n    x.anything\n}\n";
    assert!(!codes(src).contains(&"PW0610"), "{:?}", codes(src));
}

#[test]
fn a_unit_on_a_number_is_a_literal_not_a_member() {
    let src = "module m\n\nfn f(w: Float) -> Bool !{} {\n    w > 900.px\n}\n";
    assert!(!codes(src).contains(&"PW0610"), "{:?}", codes(src));
}

#[test]
fn a_module_path_is_a_name_not_a_member() {
    // ADR-0047 resolves it. The member relation records nothing about it,
    // not even an undecided one.
    let src = "module m\n\nimport List\n\nfn f() -> Int !{} {\n    let g = List.map\n    1\n}\n";
    let mut files = platform();
    files.push(("t.pw".to_string(), src.to_string()));
    let units: Vec<pw_core::check::Unit> = files
        .iter()
        .map(|(path, src)| pw_core::check::Unit {
            path: path.clone(),
            src: src.clone(),
            hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
        })
        .collect();
    let analysis = pw_core::values::analysis(&units);
    let (_, relations) = analysis.last().expect("t.pw");
    assert!(
        relations.iter().all(|r| r.kind != RelationKind::Member),
        "{:?}",
        relations
            .iter()
            .filter(|r| r.kind == RelationKind::Member)
            .collect::<Vec<_>>()
    );

    // Control: a member read in the same file is recorded, and agrees.
    let with_member = format!(
        "{POINT}import List\n\nfn f(p: Point) -> Int !{{}} {{\n    let g = List.map\n    p.x\n}}\n"
    );
    let units: Vec<pw_core::check::Unit> = files
        .iter()
        .take(files.len() - 1)
        .cloned()
        .chain([("t.pw".to_string(), with_member)])
        .map(|(path, src)| pw_core::check::Unit {
            hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let analysis = pw_core::values::analysis(&units);
    let (_, relations) = analysis.last().expect("t.pw");
    let members: Vec<_> = relations
        .iter()
        .filter(|r| r.kind == RelationKind::Member)
        .collect();
    assert_eq!(members.len(), 1, "{members:?}");
    assert_eq!(members[0].outcome, Outcome::Agree);
}

const COUNT: &str = "module counts\n\nopaque type Count = Int\n\n";

#[test]
fn an_opaque_types_value_is_its_representation_in_its_own_module() {
    let src = format!("{COUNT}fn raw(c: Count) -> Int !{{}} {{\n    c.value\n}}\n");
    assert_eq!(codes(&src), Vec::<&str>::new());

    // It is the representation's type: an `Int` is not a `String`.
    let wrong = src.replace("-> Int", "-> String");
    assert!(codes(&wrong).contains(&"PW0606"), "{:?}", codes(&wrong));
}

#[test]
fn an_opaque_types_value_is_refused_outside_its_module() {
    let user =
        "module user\n\nimport counts.{ Count }\n\nfn raw(c: Count) -> Int !{} {\n    c.value\n}\n";
    let found = diagnostics(&[("counts.pw", COUNT), ("user.pw", user)]);
    assert!(
        found
            .iter()
            .any(|(c, m)| *c == "PW0610" && m.contains("opaque here")),
        "{found:?}"
    );

    // Control: an accessor its module declares is what other modules read.
    let with_accessor = format!("{COUNT}fn count(c: Count) -> Int !{{}} {{ c.value }}\n");
    let reads = user.replace("c.value", "c.count");
    let found = diagnostics(&[("counts.pw", &with_accessor), ("user.pw", &reads)]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_generic_representation_is_read_as_value() {
    // Until 2026-09-25 the representation was kept as a spelling, and one
    // with arguments never resolved: `.value` was refused as a member the
    // type did not have (ADR-0054).
    let src = "module names\n\nimport List\n\nopaque type Names = List<String>\n\n\
               fn count(n: Names) -> Int !{} {\n    List.length(n.value)\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());

    // It is `List<String>`: not an `Int`.
    let wrong = src.replace("List.length(n.value)", "n.value + 1");
    assert!(codes(&wrong).contains(&"PW0609"), "{:?}", codes(&wrong));

    // And it is still private to its module.
    let user = "module user\n\nimport names.{ Names }\n\nfn raw(n: Names) -> Int !{} {\n    0 + n.value\n}\n";
    let found = diagnostics(&[("names.pw", src), ("user.pw", user)]);
    assert!(
        found
            .iter()
            .any(|(c, m)| *c == "PW0610" && m.contains("opaque here")),
        "{found:?}"
    );
}

#[test]
fn a_declared_value_member_is_the_member() {
    // `LayoutSnapshot<T>` is opaque, and its module declares `value`, which
    // gives the `T`: that is what `snapshot.value` reads, anywhere.
    let src = "module m\n\nimport browser.{ LayoutSnapshot, Rect }\n\nfn left(s: LayoutSnapshot<Rect>) -> Float !{} {\n    s.value.left\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());
}

#[test]
fn a_member_of_an_optionals_contents_is_pw0600() {
    // Through a parameter, which the `let`-bound rule in `annotations` does
    // not see, and through a `Result`.
    let option = format!("{POINT}fn f(p: Option<Point>) -> Int !{{}} {{\n    p.x\n}}\n");
    assert_eq!(codes(&option), ["PW0600"]);
    let result = format!("{POINT}fn f(p: Result<Point, String>) -> Int !{{}} {{\n    p.x\n}}\n");
    assert_eq!(codes(&result), ["PW0600"]);

    // A member the contents do not have either is the ordinary refusal.
    let neither = option.replace("p.x", "p.z");
    assert_eq!(codes(&neither), ["PW0610"]);
}

#[test]
fn one_defect_reached_by_two_rules_is_reported_once() {
    // A `let`-bound `Option`, which `annotations` reports as PW0600 too.
    let src = format!(
        "{POINT}fn find(n: Int) -> Option<Point> !{{}} {{ None }}\n\n\
         fn f() -> Int !{{}} {{\n    let p = find(1)\n    p.x\n}}\n"
    );
    assert_eq!(codes(&src), ["PW0600"]);
}
