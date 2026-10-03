//! **What a command answers its caller** (ADR-0157).
//!
//! A page's handler is answered whether its command committed, and the error
//! the command declares if it did not: `Result<(), E>`, never the value,
//! which reaches the page from the query the command invalidates.
//!
//! - PW0339: a command is called only by a page's handler. Called from a
//!   declaration, its body ran without its policies, and the checker and the
//!   backend typed the call differently.
//! - PW0620: a handler that binds what a command answered for its value
//!   binds nothing, and is refused where it is written.
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

/// A program with two commands a page's handler can send, one declaring a
/// `Result` and one not, a function that returns a `Result`, `extra`
/// declarations, and a button whose handler is `handler`.
fn program(extra: &str, handler: &str) -> String {
    format!(
        "module t\n\n\
         opaque type InteractionId = String\n\n\
         type Cart = Cart {{ n: Int }}\n\n\
         type CartError =\n    | Unavailable\n    | Expired\n\n\
         command add(n: Int) -> Result<Cart, CartError>\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{{\n    Ok(Cart {{ n: n }})\n}}\n\n\
         command count(n: Int) -> Int\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{{\n    n\n}}\n\n\
         fn parse(n: Int) -> Result<Int, String> {{\n    Ok(n)\n}}\n\n\
         {extra}\n\n\
         page P() {{\n    cache private\n\n    signal note: String = \"\"\n\n    \
         view {{\n        <main>\n            \
         <button type=\"button\" on:press={{() => {handler}}}>Add</button>\n        \
         </main>\n    }}\n}}\n"
    )
}

// --- PW0339 --------------------------------------------------------------------

#[test]
fn a_command_called_from_another_command_is_refused() {
    let reported = reported(&program(
        "command both(n: Int) -> Int\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{\n    count(n)\n}",
        "add(1)",
    ));
    assert_eq!(
        with("PW0339", &reported),
        ["PW0339 `count` is a command, and `both` calls it: only a page's handler sends a command"],
        "{reported:?}"
    );
}

#[test]
fn a_command_called_from_a_function_is_refused() {
    let reported = reported(&program(
        "fn twice(n: Int) -> Int {\n    count(n)\n}",
        "add(1)",
    ));
    assert_eq!(
        with("PW0339", &reported),
        [
            "PW0339 `count` is a command, and `twice` calls it: only a page's handler sends a command"
        ],
        "{reported:?}"
    );
}

#[test]
fn a_command_a_handler_sends_and_a_function_a_command_calls_are_the_controls() {
    // The handler sends `add`, and the command calls a function, as the
    // store's `add_to_cart` calls `Carts.add`.
    let reported = reported(&program(
        "fn plus(n: Int) -> Int {\n    n + 1\n}\n\n\
         command bump(n: Int) -> Int\n    requires      SignedIn\n    \
         idempotent_by InteractionId\n{\n    plus(n)\n}",
        "add(1)",
    ));
    // Clean, so the control is not passing on a program that never checked.
    assert!(reported.is_empty(), "{reported:?}");
}

// --- PW0620 --------------------------------------------------------------------

#[test]
fn a_handler_that_binds_ok_s_value_is_refused() {
    let reported = reported(&program(
        "",
        "match add(1) {\n                \
         Ok(cart) => note = \"{cart.n}\",\n                \
         Err(_) => note = \"no\",\n            }",
    ));
    assert_eq!(
        with("PW0620", &reported),
        ["PW0620 `add` answers whether it committed, not its value: `cart` is bound to nothing"],
        "{reported:?}"
    );
}

#[test]
fn a_value_bound_through_a_name_is_refused_too() {
    let reported = reported(&program(
        "",
        "{\n                let answer = add(1)\n                match answer {\n                    \
         Ok(cart) => note = \"{cart.n}\",\n                    \
         Err(_) => note = \"no\",\n                }\n            }",
    ));
    assert_eq!(
        with("PW0620", &reported),
        ["PW0620 `add` answers whether it committed, not its value: `cart` is bound to nothing"],
        "{reported:?}"
    );
}

#[test]
fn a_command_declaring_no_result_answers_nothing_to_bind() {
    let reported = reported(&program(
        "",
        "{\n                let n = count(1)\n                note = \"sent\"\n            }",
    ));
    assert_eq!(
        with("PW0620", &reported),
        [
            "PW0620 `n` is bound to what `count` answered, which is nothing: `count` declares no `Result`"
        ],
        "{reported:?}"
    );
}

#[test]
fn ok_matched_whole_and_err_bound_are_the_controls() {
    // `Ok(_)` reads no value, and the declared error is answered whole.
    for handler in [
        "match add(1) {\n                Ok(_) => note = \"\",\n                \
         Err(e) => note = \"no\",\n            }",
        "{\n                let answer = add(1)\n                match answer {\n                    \
         Ok(_) => note = \"\",\n                    Err(_) => note = \"no\",\n                }\n            }",
    ] {
        let reported = reported(&program("", handler));
        assert!(reported.is_empty(), "{handler}: {reported:?}");
    }
}

#[test]
fn a_result_that_is_not_a_commands_answer_is_the_control() {
    // A function's `Result` carries its value: `v` is bound to something.
    let reported = reported(&program(
        "",
        "match parse(1) {\n                Ok(v) => note = \"{v}\",\n                \
         Err(e) => note = e,\n            }",
    ));
    assert!(reported.is_empty(), "{reported:?}");
}

// --- one type for a command's answer, in every typer -----------------------------

/// The value relations type a command's call as the handler is answered it
/// (ADR-0157). Given where the value is declared, the answer is refused at
/// `pw check`, where until then only the backend refused it, reading a field
/// of nothing.
#[test]
fn an_answer_given_where_the_value_is_declared_is_refused() {
    let reported = reported(&program(
        "fn describe(r: Result<Cart, CartError>) -> String {\n    match r {\n        \
         Ok(c) => \"{c.n}\",\n        Err(_) => \"no\",\n    }\n}",
        "note = describe(add(1))",
    ));
    assert_eq!(
        with("PW0605", &reported),
        [
            "PW0605 argument 1 of `t.describe` is declared `Result<t.Cart, t.CartError>` and \
             this is `Result<Unit, t.CartError>`"
        ],
        "{reported:?}"
    );
}

/// The checker's typer types it so too: an answer dropped as a statement is
/// named as the handler is given it, `Result<Unit, CartError>`, not as the
/// command declares its result.
#[test]
fn a_dropped_answer_is_named_as_the_handler_is_given_it() {
    let reported = reported(&program(
        "",
        "{\n                add(1)\n                note = \"sent\"\n            }",
    ));
    assert_eq!(
        with("PW0618", &reported),
        ["PW0618 this `Result<Unit, CartError>` is dropped, and its failure with it"],
        "{reported:?}"
    );
}
