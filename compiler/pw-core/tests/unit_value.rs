//! **`()` is the unit value, and nothing the compiler cannot read checks**
//! (ADR-0200).
//!
//! `()` written as a value lowered to an error node, which the checker types
//! as anything, since a parser's error says what is wrong; the parser had
//! accepted it, so nothing did. `fn f() -> Int !{} { () }` checked. And
//! `check_sources`, which every test calls, reported no syntax error at all:
//! a test program with a typo checked clean.

use pw_core::check::{Unit, check_sources, check_units};
use pw_core::hir::{Expr, Pattern};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn reported(files: &[(&str, &str)]) -> Vec<String> {
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(p, s)| (p.to_string(), s.to_string()))
        .collect();
    check_sources(&files)
        .into_iter()
        .flat_map(|(path, ds)| {
            ds.into_iter()
                .map(move |d| format!("{path}: {} {}", d.code, d.message))
        })
        .collect()
}

#[test]
fn unit_is_the_one_value_of_its_type() {
    let unit = "module u\n\nfn f() -> () !{} {\n    ()\n}\n";
    assert_eq!(reported(&[("u.pw", unit)]), Vec::<String>::new());
    // And of no other: an `Int` declared, `()` given.
    let int = unit.replace("-> ()", "-> Int");
    assert_eq!(
        reported(&[("u.pw", &int)]),
        ["u.pw: PW0606 `u.f` declares its result `Int` and this produces `Unit`"]
    );
    // Through a binding, as a `String`.
    let bound = "module u\n\nfn f() -> String !{} {\n    let x = ()\n    x\n}\n";
    assert_eq!(
        reported(&[("u.pw", bound)]),
        ["u.pw: PW0606 `u.f` declares its result `String` and this produces `Unit`"]
    );
}

#[test]
fn unit_is_built() {
    // A command that answers `()`, and the page that sends it.
    let src = "module u\n\nopaque type InteractionId = String\n\n\
               command clear() -> ()\n    requires      SignedIn\n    \
               idempotent_by InteractionId\n{\n    ()\n}\n\n\
               page P() {\n    cache private\n\n    \
               view {\n        <main><button type=\"button\" on:press|refusable={() => clear()}>Go</button></main>\n    }\n}\n";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut units = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let s = std::fs::read_to_string(&p).expect("read");
            units.push(Unit {
                path: p.display().to_string(),
                hir: lower_file(&s, &parse_tree(&s).green),
                src: s,
            });
        }
    }
    units.push(Unit {
        path: "u.pw".to_string(),
        hir: lower_file(src, &parse_tree(src).green),
        src: src.to_string(),
    });
    let b = pw_core::build::build(&units).expect("builds");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
}

#[test]
fn a_file_that_does_not_parse_reports_its_syntax_errors() {
    // As `pw check` does: until ADR-0200 `check_sources` reported none.
    let broken = "module u\n\nfn f() -> Int !{} {\n    1 +\n}\n";
    let found = reported(&[("u.pw", broken)]);
    assert!(
        found.iter().any(|d| d.starts_with("u.pw: PW0009")),
        "{found:?}"
    );
}

#[test]
fn a_file_that_does_not_parse_is_kept_out_of_the_program() {
    // `b` imports `a`, which does not parse. As in `pw check a.pw b.pw`,
    // what recovery made of `a` is no module of the program.
    let a = "module a\n\nfn helper() -> Int !{} {\n    1 +\n}\n";
    let b = "module b\n\nimport a.{ helper }\n\nfn g() -> Int !{} {\n    helper()\n}\n";
    let found = reported(&[("a.pw", a), ("b.pw", b)]);
    assert!(
        found.iter().any(|d| d.starts_with("a.pw: PW0009")),
        "{found:?}"
    );
    assert!(found.iter().any(|d| d.starts_with("b.pw: ")), "{found:?}");
    // Control: parsed, `a` is a module, and `b` checks.
    let fixed = a.replace("1 +", "1");
    assert_eq!(
        reported(&[("a.pw", &fixed), ("b.pw", b)]),
        Vec::<String>::new()
    );
}

#[test]
fn an_expression_the_compiler_cannot_read_is_refused() {
    // Every file the checker is given parses, so an error node is text the
    // lowering has no meaning for. Made here by hand, since no file that
    // parses lowers to one (`every_expression_is_read.rs`).
    let src = "module u\n\nfn f() -> Int !{} {\n    1\n}\n";
    let unit = |hir| Unit {
        path: "u.pw".to_string(),
        src: src.to_string(),
        hir,
    };
    let read = lower_file(src, &parse_tree(src).green);
    let clean: Vec<String> = check_units(&[unit(read.clone())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
        .collect();
    assert_eq!(clean, Vec::<String>::new());
    let mut unread = read;
    let body = unread.bodies.get_mut(0).expect("a body");
    let literal = (0..body.exprs.len())
        .find(|i| matches!(body.exprs.get(*i), Some(Expr::Literal(_))))
        .expect("the literal");
    *body.exprs.get_mut(literal).expect("it") = Expr::Error;
    let found: Vec<String> = check_units(&[unit(unread)])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0015") && d.contains("`1`")),
        "{found:?}"
    );
}

#[test]
fn a_pattern_the_compiler_cannot_read_is_refused() {
    let src = "module u\n\nfn f(n: Int) -> Int !{} {\n    match n {\n        _ => 0,\n    }\n}\n";
    let mut hir = lower_file(src, &parse_tree(src).green);
    let body = hir.bodies.get_mut(0).expect("a body");
    let wild = (0..body.pats.len())
        .find(|i| matches!(body.pats.get(*i), Some(Pattern::Wild)))
        .expect("the wildcard");
    *body.pats.get_mut(wild).expect("it") = Pattern::Error;
    let found: Vec<String> = check_units(&[Unit {
        path: "u.pw".to_string(),
        src: src.to_string(),
        hir,
    }])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
    .collect();
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0015") && d.contains("`_` is a pattern")),
        "{found:?}"
    );
}
