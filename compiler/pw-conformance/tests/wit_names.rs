//! **A name WIT reserves is escaped where WIT text is written.**
//!
//! A Pleris name becomes a WIT identifier by `wit::ident`: a case `List` is
//! `list`, a field `own` is `own`, a query `Own` is the function `own`. Each
//! of those is a WIT keyword, and until 2026-09-26 each was written bare, so
//! the program's WIT package did not parse and every component in it was
//! refused, not only the one naming it. WIT reads `%list` as the identifier
//! `list`, so the escape is in the text alone: the component's values, and
//! the host that calls it, see the names unescaped.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module kw

type Kind =
    | List
    | Record(Int)

type Item = Item { func: Int, own: String }

public query Pick(n: Int) -> Kind {
    if n > 0 { Kind.Record(n) } else { Kind.List }
}

public query Made(n: Int) -> Item { Item { func: n, own: "x" } }

public query Own(n: Int) -> Int { n + 1 }
"#;

fn call(id: &str, args: &[Val]) -> Val {
    Runnable::new(compile(&units(&[("kw.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), args)
        .expect("runs")
        .remove(0)
}

#[test]
fn a_case_named_by_a_keyword_is_built_and_named_unescaped() {
    assert_eq!(
        call("kw.Pick", &[Val::S64(3)]),
        Val::Variant("record".into(), Some(Box::new(Val::S64(3))))
    );
    assert_eq!(
        call("kw.Pick", &[Val::S64(0)]),
        Val::Variant("list".into(), None)
    );
}

#[test]
fn a_field_named_by_a_keyword_is_named_unescaped() {
    assert_eq!(
        call("kw.Made", &[Val::S64(7)]),
        Val::Record(vec![
            ("func".into(), Val::S64(7)),
            ("own".into(), Val::String("x".into())),
        ])
    );
}

#[test]
fn a_query_named_by_a_keyword_is_exported_under_its_name() {
    assert_eq!(call("kw.Own", &[Val::S64(41)]), Val::S64(42));
}

#[test]
fn a_type_and_a_query_of_one_name_are_two_things() {
    // A type and a query both called `Either`, in their two namespaces.
    // Until 2026-09-26 the type took the query's place when the worlds were
    // generated, and the whole package failed: "missing component signature".
    let program = "module both\n\ntype Either =\n    | Left(Int)\n    | Right(String)\n\npublic query Either(n: Int) -> Int { n + 1 }\n";
    let out = Runnable::new(compile(&units(&[("both.pw", program)]), "both.Either"))
        .call(&BTreeMap::new(), &[Val::S64(1)])
        .expect("runs")
        .remove(0);
    assert_eq!(out, Val::S64(2));
}
