//! **`let _ = e` is an explicit discard, and an acquisition is held or
//! refused** (ADR-0250, ruling 0099-a).
//!
//! `let _ = e` did not parse ("expected a binding name"), and a program
//! discarded a `Result` by naming a binding it never read, `let _ignored =
//! e`. Now `_` binds nothing: the value is computed and dropped, a
//! `Result`'s failure with it, by the program's word, so PW0618 says
//! nothing. An affine resource bound to `_` is the exception, refused as
//! never consumed.
//!
//! Its probes found the larger hole: PW2005 followed only `let x =
//! acquire()` and `use x = ..`. An acquisition anywhere else, a statement,
//! an argument, a branch of an `if` statement, `let h = Maps.create(..)?`,
//! passed unread. Now each is held by a name, ended where it is made, given
//! to the caller, or held by a resource's `acquire` clause, or refused.
//! Each test states one case, with its control.

use pw_core::check::check_sources;

/// The `.pw` files of `dir`, under the repository, by name.
fn files(dir: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            (p.display().to_string(), src)
        })
        .collect()
}

/// One diagnostic of `app.pw`: its code, its message, the text it
/// underlines, its related spans' text and labels, and its repairs.
#[derive(Debug, PartialEq)]
struct Found {
    code: String,
    message: String,
    at: String,
    related: Vec<(String, String)>,
    repairs: Vec<String>,
}

/// What the checker reports of `app.pw`, checked with the platform and, for
/// `with_store`, the store's domain, library and accepted corpus.
fn found(src: &str, with_store: bool) -> Vec<Found> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut program = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        program.extend(files(d));
    }
    if with_store {
        program.push((
            "examples/domain.pw".to_string(),
            std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
        ));
        program.extend(files("examples/lib"));
        program.extend(files("examples/accepted"));
    }
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| Found {
            code: d.code.to_string(),
            message: d.message.clone(),
            at: src[d.primary_span.clone()].to_string(),
            related: d
                .related
                .iter()
                .filter(|r| r.span.end <= src.len())
                .map(|r| (src[r.span.clone()].to_string(), r.label.clone()))
                .collect(),
            repairs: d.repairs.into_iter().map(|r| r.description).collect(),
        })
        .collect()
}

/// Each diagnostic's code, message and underlined text.
fn said(f: &[Found]) -> Vec<(&str, &str, &str)> {
    f.iter()
        .map(|f| (f.code.as_str(), f.message.as_str(), f.at.as_str()))
        .collect()
}

const RESULT: &str = "module m\n\nfn r() -> Result<Int, String> !{} {\n    Ok(1)\n}\n\n\
                      fn f() -> Int !{} {\n    BODY\n    1\n}\n";

#[test]
fn a_result_discarded_by_let_is_handled() {
    assert_eq!(found(&RESULT.replace("BODY", "let _ = r()"), false), vec![]);
    // The control: dropped as a statement, it is PW0618, whose repair names
    // the discard.
    let dropped = found(&RESULT.replace("BODY", "r()"), false);
    assert_eq!(
        dropped
            .iter()
            .map(|f| (f.code.as_str(), f.at.as_str()))
            .collect::<Vec<_>>(),
        vec![("PW0618", "r()")],
        "{dropped:#?}"
    );
    assert!(
        dropped[0].repairs[0].ends_with("or discard it, `let _ = ..`"),
        "{dropped:#?}"
    );
}

#[test]
fn a_discard_still_holds_its_annotation() {
    let f = found(&RESULT.replace("BODY", "let _: String = 1"), false);
    assert_eq!(
        f.iter().map(|f| f.code.as_str()).collect::<Vec<_>>(),
        vec!["PW0607"],
        "{f:#?}"
    );
    assert_eq!(
        found(&RESULT.replace("BODY", "let _: Int = 1"), false),
        vec![]
    );
}

#[test]
fn a_discard_is_no_value_to_read() {
    let f = found(&RESULT.replace("BODY", "let _ = 1\n    let y = _"), false);
    assert_eq!(
        said(&f),
        vec![("PW0021", "`_` binds nothing, and is no value", "_")],
        "{f:#?}"
    );
    assert_eq!(
        f[0].repairs,
        vec!["bind the value to a name, `let x = ..`, and read `x`".to_string()]
    );
    // The control: a name nothing binds is unresolved, as before.
    let f = found(&RESULT.replace("BODY", "let y = z"), false);
    assert_eq!(
        said(&f),
        vec![("PW0021", "`z` does not resolve", "z")],
        "{f:#?}"
    );
}

/// A command that opens a transaction, as the corpus's commands do: `BODY`
/// before the cart is cleared.
const COMMAND: &str = "module m\n\nimport Carts\nimport Database\n\
     import context.{ current_session }\nimport Events.{ CartChanged }\n\
     import domain.{ CartError }\n\n\
     command clear_cart() -> Result<(), CartError>\n    requires SignedIn\n    \
     emits CartChanged(current_session())\n{\n    BODY\n    \
     let _cleared = Carts.clear(current_session())\n    Ok(())\n}\n";

#[test]
fn an_affine_resource_discarded_is_refused_as_never_consumed() {
    let f = found(&COMMAND.replace("BODY", "let _ = Database.begin()"), true);
    assert_eq!(
        said(&f),
        vec![(
            "PW2005",
            "affine resource `DatabaseTransaction` is bound to `_`, and nothing can release it",
            "Database.begin()"
        )],
        "{f:#?}"
    );
    assert_eq!(
        f[0].related[0],
        (
            "let _ = Database.begin()".to_string(),
            "`_` binds nothing".to_string()
        )
    );
    assert_eq!(
        f[0].repairs,
        vec!["bind it to a name, and end it with `commit` or `rollback`".to_string()]
    );
    // The control: bound to a name, and ended.
    let named = COMMAND.replace("BODY", "let tx = Database.begin()\n    let _ = tx.commit()");
    assert_eq!(found(&named, true), vec![]);
    // `let _ = tx` moves nothing, as in Rust: `tx` is still to end.
    let read = COMMAND.replace("BODY", "let tx = Database.begin()\n    let _ = tx");
    let f = found(&read, true);
    assert_eq!(
        f.iter().map(|f| f.message.as_str()).collect::<Vec<_>>(),
        vec!["affine resource `tx: DatabaseTransaction` is not consumed on every path"],
        "{f:#?}"
    );
}

#[test]
fn a_resource_a_statement_drops_is_refused() {
    let dropped = "affine resource `DatabaseTransaction` is acquired, and its statement drops it";
    let f = found(&COMMAND.replace("BODY", "Database.begin()"), true);
    assert_eq!(
        said(&f),
        vec![("PW2005", dropped, "Database.begin()")],
        "{f:#?}"
    );
    // Through an `if` statement's branches, and `Some(..)`, which holds what
    // it is given: each acquisition, once.
    let branches = COMMAND.replace(
        "BODY",
        "if true {\n        Database.begin()\n    } else {\n        Some(Database.begin())\n    }",
    );
    let f = found(&branches, true);
    assert_eq!(
        said(&f),
        vec![
            ("PW2005", dropped, "Database.begin()"),
            ("PW2005", dropped, "Database.begin()")
        ],
        "{f:#?}"
    );
    // The control: the same branches, a binding's value, ended.
    let bound = COMMAND.replace(
        "BODY",
        "let tx = if true {\n        Database.begin()\n    } else {\n        Database.begin()\n    }\n    \
         let _ = tx.commit()",
    );
    assert_eq!(found(&bound, true), vec![]);
}

/// Declarations over the store's library, `DECLS` among them.
const FNS: &str = "module m\n\nimport Database\nimport Maps\n\
     import browser.{ ElementRef, MapHandle, MapError }\n\
     import domain.{ LatLng, DatabaseTransaction, CartError }\n\nDECLS\n";

/// A module over the store's library declaring `decls`.
fn with(decls: &str) -> String {
    FNS.replace("DECLS", decls)
}

/// A module with `fn f`, answering `result` from `body`, whose row acquires
/// and releases transactions.
fn transacting(result: &str, body: &str) -> String {
    with(&format!(
        "fn f(n: Int) -> {result} !{{ database.transaction, \
         resource.acquire<DatabaseTransaction>, resource.release<DatabaseTransaction> }} \
         {{\n    {body}\n}}"
    ))
}

#[test]
fn a_resource_given_to_a_call_that_does_not_release_it_is_refused() {
    let src = transacting("()", "helper(Database.begin())\n    ()").replace(
        "fn f(",
        "fn helper(tx: DatabaseTransaction) -> () !{} {\n    ()\n}\n\nfn f(",
    );
    let f = found(&src, true);
    assert_eq!(
        said(&f),
        vec![(
            "PW2005",
            "affine resource `DatabaseTransaction` is given to `helper`, which does not release it",
            "Database.begin()"
        )],
        "{f:#?}"
    );
    // The control: ended where it is made, as an argument or a receiver.
    for ended in [
        "Database.commit(Database.begin())",
        "Database.begin().commit()",
    ] {
        assert_eq!(
            found(&transacting("Result<(), CartError>", ended), true),
            vec![],
            "{ended}"
        );
    }
}

#[test]
fn a_resource_kept_in_a_value_or_returned_by_a_function_value_is_refused() {
    let f = found(
        &transacting("()", "let txs = [Database.begin()]\n    ()"),
        true,
    );
    assert_eq!(
        said(&f),
        vec![(
            "PW2005",
            "affine resource `DatabaseTransaction` is kept in a value that nothing follows",
            "Database.begin()"
        )],
        "{f:#?}"
    );
    // A function value's result, written as its body or after a `return`:
    // the `return` leaves the function value, not `f`.
    for lambda in [
        "() => Database.begin()",
        "() => {\n        return Database.begin()\n    }",
    ] {
        let src = transacting(
            "DatabaseTransaction",
            &format!("let open = {lambda}\n    Database.begin()"),
        );
        let f = found(&src, true);
        assert_eq!(
            said(&f),
            vec![(
                "PW2005",
                "affine resource `DatabaseTransaction` is returned by a function value, which \
                 nothing follows",
                "Database.begin()"
            )],
            "{lambda}: {f:#?}"
        );
        // The second acquisition, `f`'s value, is the caller's.
        assert!(f[0].related[0].0.starts_with("() =>"), "{f:#?}");
    }
}

#[test]
fn a_declaration_gives_its_caller_what_it_declares() {
    // Given to the caller, as the body's value or a `return`'s, bound or
    // not, and held by `Ok(..)`: the caller's to end.
    for body in [
        "Database.begin()",
        "let tx = Database.begin()\n    tx",
        "let tx = Database.begin()\n    return tx",
        "return Database.begin()",
        // A `return` that is not the body's last statement.
        "let tx = Database.begin()\n    if n > 0 {\n        return tx\n    }\n    tx",
    ] {
        assert_eq!(
            found(&transacting("DatabaseTransaction", body), true),
            vec![],
            "{body}"
        );
    }
    let wrapped = with(
        "fn open(c: ElementRef, at: LatLng) -> Result<MapHandle, MapError> \
         !{ dom.mutate, layout.measure, resource.acquire<MapHandle> } {\n    \
         let h = Maps.create(c, at)?\n    Ok(h)\n}",
    );
    assert_eq!(found(&wrapped, true), vec![]);
    // The control: a body declaring `()` ends in a statement, and gives its
    // caller nothing; until ADR-0250 its last value was the caller's.
    let f = found(&transacting("()", "Database.begin()"), true);
    assert_eq!(
        said(&f),
        vec![(
            "PW2005",
            "affine resource `DatabaseTransaction` is acquired, and its statement drops it",
            "Database.begin()"
        )],
        "{f:#?}"
    );
    let f = found(
        &transacting("()", "let tx = Database.begin()\n    tx"),
        true,
    );
    assert_eq!(
        f.iter().map(|f| f.message.as_str()).collect::<Vec<_>>(),
        vec!["affine resource `tx: DatabaseTransaction` is not consumed on every path"],
        "{f:#?}"
    );
}

/// A module with `fn f` over a map, with `body`.
fn mapping(body: &str) -> String {
    with(&format!(
        "fn f(c: ElementRef, at: LatLng) -> Result<(), MapError> \
         !{{ dom.mutate, layout.measure, resource.acquire<MapHandle>, \
         resource.release<MapHandle> }} {{\n    {body}\n}}"
    ))
}

#[test]
fn a_resource_a_result_carries_is_held_with_a_question_mark() {
    // `let h = Maps.create(..)?` was followed by nothing until ADR-0250:
    // never destroyed, it passed.
    let f = found(&mapping("let h = Maps.create(c, at)?\n    Ok(())"), true);
    assert_eq!(
        f.iter().map(|f| f.message.as_str()).collect::<Vec<_>>(),
        vec!["affine resource `h: MapHandle` is not consumed on every path"],
        "{f:#?}"
    );
    // The control: destroyed.
    let src = mapping("let h = Maps.create(c, at)?\n    Maps.destroy(h)\n    Ok(())");
    assert_eq!(found(&src, true), vec![]);
    // `?` gives what it takes out its type, so a member names its
    // declaration: `tx.commit()` is `Database.commit`, which ends it.
    let open = "fn open() -> Result<DatabaseTransaction, CartError> !{ database.transaction, \
                resource.acquire<DatabaseTransaction> } {\n    Ok(Database.begin())\n}\n\n";
    let src = transacting("Result<(), CartError>", "let tx = open()?\n    tx.commit()")
        .replace("fn f(", &format!("{open}fn f("));
    assert_eq!(found(&src, true), vec![]);
    // Matched where it is made, what the arms bind is not followed. The
    // repair names `?`, and only `destroy`: `f`'s row releases a handle, and
    // takes none.
    let matched = mapping(
        "match Maps.create(c, at) {\n        Ok(h) => Maps.destroy(h),\n        \
         Err(_) => (),\n    }\n    Ok(())",
    );
    let f = found(&matched, true);
    assert_eq!(
        said(&f),
        vec![(
            "PW2005",
            "affine resource `MapHandle` is matched where it is acquired, and what the arms \
             bind is not followed",
            "Maps.create(c, at)"
        )],
        "{f:#?}"
    );
    assert_eq!(
        f[0].repairs,
        vec!["bind it to a name with `?`, `let x = ..?`, and end it with `destroy`".to_string()]
    );
    let f = found(&mapping("let _ = Maps.create(c, at)?\n    Ok(())"), true);
    assert_eq!(
        f.iter().map(|f| f.message.as_str()).collect::<Vec<_>>(),
        vec!["affine resource `MapHandle` is bound to `_`, and nothing can release it"],
        "{f:#?}"
    );
}

#[test]
fn a_binding_from_branches_is_typed_by_them() {
    // `tx.commit()` names a member only where `tx` has a type: here the
    // branches' and the arms', which agree.
    for init in [
        "if n > 0 {\n        Database.begin()\n    } else {\n        Database.begin()\n    }",
        "match n {\n        0 => Database.begin(),\n        _ => Database.begin(),\n    }",
    ] {
        let src = transacting(
            "Result<(), CartError>",
            &format!("let tx = {init}\n    tx.commit()"),
        );
        assert_eq!(found(&src, true), vec![], "{init}");
    }
}

/// A component holding a map while it is visible, as A-019 does: its
/// `resource` statement's `release` clause is `RELEASE`.
const COMPONENT: &str = "module m\n\nimport Maps\nimport domain.{ LatLng }\n\n\
     component LazyMap(center: LatLng) {\n    placement browser\n\n    \
     let visible = observe intersection(self, threshold = 0.1) -> Bool\n\n    \
     resource map when visible {\n        scope component\n        \
     acquire {\n            Maps.create(self, center)\n        }\n        \
     release(handle) {\n            RELEASE\n        }\n    }\n\n    \
     view { <section aria-label=\"Store location\" /> }\n}\n";

#[test]
fn a_resources_acquire_clause_holds_its_value() {
    assert_eq!(
        found(&COMPONENT.replace("RELEASE", "Maps.destroy(handle)"), true),
        vec![]
    );
    // The control: in the `release` clause, a statement drops what it
    // acquires, and the `Result` that carries it.
    let src = COMPONENT.replace(
        "RELEASE",
        "Maps.create(self, center)\n            Maps.destroy(handle)",
    );
    let f = found(&src, true);
    assert_eq!(
        said(&f),
        vec![
            (
                "PW0618",
                "this `Result<MapHandle, MapError>` is dropped, and its failure with it",
                "Maps.create(self, center)"
            ),
            (
                "PW2005",
                "affine resource `MapHandle` is acquired, and its statement drops it",
                "Maps.create(self, center)"
            )
        ],
        "{f:#?}"
    );
}
