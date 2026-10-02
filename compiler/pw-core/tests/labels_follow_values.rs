//! **A value's label follows it through calls, bodies, branches and
//! assignments, and a public log takes only a public value** (ADR-0129).
//!
//! ADR-0128 made the cache rules read a declaration's whole label. The sinks
//! read a value's label, and at `34aa7bd` each of these checked clean:
//! - a session id given to a parameter that states its label came out of the
//!   call public (ADR-0085's contract), and so did a secret a body was given
//!   and returned in a `String`;
//! - a helper returning a secret it read itself, in a `String`;
//! - `if secret { "a" } else { "b" }`, and a public log inside such a branch;
//! - a mutable binding assigned a secret;
//! - a session id logged at `log<Public>`, which refused only secrets;
//! - a value that is both a session's and a secret, rendered into markup:
//!   PW5003 read only the first restriction of its label.
//!
//! Each test states the defects with controls.

use pw_core::check::check_sources;

fn sources(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(src: &str) -> Vec<String> {
    check_sources(&sources(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn codes(src: &str) -> Vec<String> {
    reported(src)
        .into_iter()
        .map(|d| d.split(' ').next().unwrap_or_default().to_string())
        .collect()
}

const IMPORTS: &str = "module t\n\nimport log\nimport secrets\nimport Carts\nimport Receipts\n\
     import context.{ current_session }\n\
     import capability.{ Payments, Public, Secret, Session, SessionId }\n\
     import domain.{ OrderId, Cart, CartError }\n\n";

/// A function `audit` whose row admits a public log, a secret and the
/// session, with `body`.
fn audit(body: &str) -> String {
    format!(
        "fn audit(order: OrderId) -> () !{{ log<Public>, secret<Payments>, session.read, \
         database.read<Carts> }} {{\n    {body}\n}}\n"
    )
}

/// A public log refused, by PW5006, mentioning `what`.
fn refused(src: &str, what: &str) {
    let found = reported(src);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5006") && d.contains(what)),
        "expected PW5006 mentioning `{what}`, got {found:?}\n{src}"
    );
}

fn clean(src: &str) {
    assert_eq!(
        codes(src),
        Vec::<String>::new(),
        "{:?}\n{src}",
        reported(src)
    );
}

#[test]
fn an_argument_comes_out_of_a_call_unless_it_is_a_key_the_call_uses() {
    // A session id, through a parameter that states its label.
    let mine = "fn mine(s: Session<SessionId>) -> String !{} {\n    \"{s}\"\n}\n\n";
    refused(
        &format!(
            "{IMPORTS}{mine}{}",
            audit("log.public(mine(current_session()))")
        ),
        "Session<SessionId>",
    );
    // A host binding's: the cart a session's id fetched is the session's.
    refused(
        &format!(
            "{IMPORTS}{}",
            audit("log.public(Carts.current(current_session()))")
        ),
        "Session<SessionId>",
    );
    // A secret a body is given and returns.
    let shown = "fn shown(key: Secret<Payments>) -> String !{} {\n    \"{key}\"\n}\n\n";
    refused(
        &format!(
            "{IMPORTS}{shown}{}",
            audit("log.public(shown(secrets.payments()))")
        ),
        "Secret<Payments>",
    );

    // Control: a key the call uses is not in what it makes.
    clean(&format!(
        "{IMPORTS}{}",
        audit("log.public(Receipts.render(secrets.payments(), order))")
    ));
    // Control: an argument that carries nothing.
    clean(&format!("{IMPORTS}{mine}{}", audit("log.public(order)")));
}

#[test]
fn a_call_carries_what_its_callees_body_makes() {
    let leak = "fn shown() -> String !{ secret<Payments> } {\n    \
                let t = secrets.payments()\n    \"{t}\"\n}\n\n";
    refused(
        &format!("{IMPORTS}{leak}{}", audit("log.public(shown())")),
        "Secret<Payments>",
    );
    // Through two helpers.
    let relay = "fn relayed() -> String !{ secret<Payments> } {\n    shown()\n}\n\n";
    refused(
        &format!("{IMPORTS}{leak}{relay}{}", audit("log.public(relayed())")),
        "Secret<Payments>",
    );

    // Control: a helper that reads the secret and returns something else.
    let used = "fn used() -> String !{ secret<Payments> } {\n    \
                let t = secrets.payments()\n    \"done\"\n}\n\n";
    clean(&format!("{IMPORTS}{used}{}", audit("log.public(used())")));
}

#[test]
fn a_branch_carries_its_condition() {
    // In the value.
    refused(
        &format!(
            "{IMPORTS}{}",
            audit(
                "let token = secrets.payments()\n    \
                 let bit = if token == token { \"yes\" } else { \"no\" }\n    \
                 log.public(bit)"
            )
        ),
        "Secret<Payments>",
    );
    // At the sink it decides.
    let src = format!(
        "{IMPORTS}{}",
        audit(
            "let token = secrets.payments()\n    \
             if token == token {\n        log.public(\"yes\")\n    }"
        )
    );
    refused(&src, "decides it");

    // A `match` on one, in the value.
    refused(
        &format!(
            "{IMPORTS}{}",
            audit(
                "let token = secrets.payments()\n    \
                 let bit = match token == token {\n        true => \"yes\",\n        \
                 false => \"no\",\n    }\n    log.public(bit)"
            )
        ),
        "Secret<Payments>",
    );
    // A `?` that may fail: what follows it runs only if it did not.
    let loaded = "fn loaded() -> Result<(), CartError> !{ log<Public>, session.read, \
                  database.read<Carts> } {\n    \
                  let cart = Carts.current(current_session())?\n    \
                  log.public(\"loaded\")\n    Ok(())\n}\n";
    refused(&format!("{IMPORTS}{loaded}"), "decides it");
    // A lambda run once per element of a secret list.
    refused(
        &format!(
            "{IMPORTS}import List\n\n{}",
            audit(
                "let tokens = [secrets.payments()]\n    \
                 let ticks = List.map(tokens, fn(t) log.public(\"tick\"))"
            )
        ),
        "decides it",
    );

    // Control: a branch on a public value.
    clean(&format!(
        "{IMPORTS}{}",
        audit("if order == order {\n        log.public(\"yes\")\n    }")
    ));
}

#[test]
fn an_assigned_binding_carries_what_it_is_assigned() {
    refused(
        &format!(
            "{IMPORTS}{}",
            audit(
                "let token = secrets.payments()\n    \
                 let mut shown = \"none\"\n    shown = \"{token}\"\n    log.public(shown)"
            )
        ),
        "Secret<Payments>",
    );
    // Control: assigned a public value.
    clean(&format!(
        "{IMPORTS}{}",
        audit("let mut shown = \"none\"\n    shown = \"{order}\"\n    log.public(shown)")
    ));
}

#[test]
fn a_public_log_takes_no_readers_value() {
    refused(
        &format!(
            "{IMPORTS}{}",
            audit("let s = current_session()\n    log.public(\"{s}\")")
        ),
        "Session<SessionId>",
    );
    // Control: a private log admits it.
    clean(&format!(
        "{}fn audit(order: OrderId) -> () !{{ log<Private>, session.read }} {{\n    \
         let s = current_session()\n    log.private(\"{{s}}\")\n}}\n",
        IMPORTS.replace("{ Payments, Public,", "{ Payments, Private, Public,")
    ));
}

#[test]
fn a_secret_is_seen_in_markup_beside_a_readers_value() {
    let src = format!(
        "{IMPORTS}component Badge() {{\n    placement origin\n    \
         let key = secrets.payments()\n    let s = current_session()\n    \
         view {{\n        <div><p>{{\"{{s}}{{key}}\"}}</p></div>\n    }}\n}}\n"
    );
    let found = reported(&src);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5003") && d.contains("Secret<Payments>")),
        "{found:?}"
    );

    // Control: the reader's value alone is not a secret.
    let src = format!(
        "{IMPORTS}component Badge() {{\n    placement origin\n    \
         let s = current_session()\n    \
         view {{\n        <div><p>{{\"{{s}}\"}}</p></div>\n    }}\n}}\n"
    );
    assert!(
        !codes(&src).contains(&"PW5003".to_string()),
        "{:?}",
        reported(&src)
    );
}
