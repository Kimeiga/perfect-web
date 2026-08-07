//! **Pleris may reject authored semantics; it must never silently erase them.**
//!
//! Architect ruling, 2026-08-07, after `impact` was written in the platform
//! library for a whole commit while `POLICY_KEYWORDS` did not contain it:
//!
//! > `[]` means: the author specified no policy. `Unknown("partiton")` means:
//! > the author attempted to specify policy semantics that the compiler does
//! > not understand. Those must never be observationally equivalent.
//!
//! An unrecognised policy head used to produce no node at all. The declaration
//! still checked clean, and every rule reading that clause got the same answer
//! it gets for a declaration that never had one. The phase rules stopped firing
//! entirely and nothing said anything.
//!
//! This is the third instance of one shape — a central table that silently
//! discards what it does not recognise. The diagnostic registry solved it for
//! codes by making an unregistered code a test failure; the effect ontology
//! solved it for effects by making an undeclared family `PW5201`. This is the
//! parser's answer, and it replaces the text scanner that was the emergency
//! guard.

use pw_syntax::kind::SyntaxKind as K;
use pw_syntax::parse_tree;

/// Does this source contain a preserved unknown-policy node?
fn has_unknown(src: &str) -> bool {
    parse_tree(src)
        .green
        .descendants()
        .any(|n| n.kind() == K::UnknownPolicy)
}

/// Does it report one?
fn reports_unknown(src: &str) -> bool {
    !parse_tree(src).ok()
}

/// The heads the parser DID recognise.
fn known_heads(src: &str) -> Vec<String> {
    parse_tree(src)
        .green
        .descendants()
        .filter(|n| n.kind() == K::Policy)
        .filter_map(|n| {
            n.text()
                .to_string()
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .collect()
}

// --- preserved, per construct ------------------------------------------------

#[test]
fn an_unknown_policy_in_an_effect_is_preserved_and_reported() {
    let src = "module p\n\neffect a.b {\n    capability none\n    impakt layout_write\n}\n";
    assert!(has_unknown(src), "the clause must survive into the tree");
    assert!(reports_unknown(src), "and be reported");
    assert_eq!(
        known_heads(src),
        ["capability"],
        "the KNOWN clause beside it still parses"
    );
}

#[test]
fn an_unknown_policy_in_a_materialize_is_preserved_and_reported() {
    let src = "module p\n\nmaterialize Menu {\n    partiton public\n}\n";
    assert!(has_unknown(src), "a misspelled `partition` must not vanish");
    assert!(reports_unknown(src));
}

#[test]
fn an_unknown_policy_in_a_query_is_preserved_and_reported() {
    let src =
        "module p\n\nquery Cart(s: Int) -> Int\n    cache shared\n    freshnes 30\n{\n    0\n}\n";
    assert!(has_unknown(src));
    assert!(reports_unknown(src));
    assert_eq!(known_heads(src), ["cache"]);
}

#[test]
fn an_unknown_policy_in_a_command_is_preserved_and_reported() {
    let src = "module p\n\ncommand Add(s: Int) -> Int\n    emmits CartChanged\n{\n    0\n}\n";
    assert!(has_unknown(src));
    assert!(reports_unknown(src));
}

// --- what must NOT become an unknown policy ---------------------------------

#[test]
fn a_known_policy_still_reaches_the_policy_node() {
    // The positive control for every test above. If nothing were recognised,
    // "preserved as unknown" would be trivially true everywhere.
    let src =
        "module p\n\nquery Cart(s: Int) -> Int\n    cache shared\n    freshness 30\n{\n    0\n}\n";
    assert!(!has_unknown(src), "both heads are known");
    assert!(parse_tree(src).ok());
    assert_eq!(known_heads(src), ["cache", "freshness"]);
}

#[test]
fn an_expression_in_a_materialize_body_is_not_mistaken_for_a_policy() {
    // **The containment.** A materialize body holds real expressions after its
    // policies, and both are identifier-led: `Stores.get(1)` and
    // `partiton public` differ only in what follows the identifier. A policy
    // head is never followed by `.`, `(`, `=`, `<`, `,` or `{`.
    let src = "module p\n\nimport Stores\n\nmaterialize Menu {\n    partition public\n    Stores.get(1)\n}\n";
    assert!(
        !has_unknown(src),
        "a call is an expression, not an unknown clause"
    );
    assert!(parse_tree(src).ok(), "and the file still parses");
}

#[test]
fn a_bare_identifier_alone_on_a_line_is_not_a_policy() {
    // A policy head takes a value on its own line. A lone identifier is an
    // expression — `todo` is the corpus's most common one.
    let src = "module p\n\nmaterialize Menu {\n    partition public\n    todo\n}\n";
    assert!(!has_unknown(src), "`todo` is an expression");
    assert!(parse_tree(src).ok());
}

#[test]
fn the_whole_corpus_contains_no_unknown_policy() {
    // The strongest statement available: every `.pw` this repository ships
    // parses with every policy head recognised. This is what the effect-only
    // text scanner used to assert, now covering every construct and every
    // directory rather than `packages/` and `effect { .. }`.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut stack = vec![root.join("packages"), root.join("examples")];
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "pw") {
                scanned += 1;
                let src = std::fs::read_to_string(&p).expect("read");
                if has_unknown(&src) {
                    offenders.push(p.to_string_lossy().to_string());
                }
            }
        }
    }
    assert!(scanned >= 100, "only {scanned} files scanned");
    assert!(
        offenders.is_empty(),
        "these files contain a policy clause the parser does not know:\n  {}",
        offenders.join("\n  ")
    );
}

// --- the formatter must not canonicalise what the parser rejected -----------

#[test]
fn a_file_with_an_unknown_policy_is_not_formattable_in_place() {
    // Architect ruling, 2026-08-07:
    //
    // > Formatting to stdout for inspection is one thing; overwriting source
    // > whose meaning the parser has explicitly said it does not understand is
    // > another.
    //
    // Asserted here as the PROPERTY the CLI depends on — the file does not
    // parse — because a test that shelled out to `pw fmt` would be testing the
    // process rather than the rule. `fmt_command` refuses on exactly this.
    let src = "module p\n\nmaterialize Menu {\n    partiton public\n}\n";
    assert!(
        !parse_tree(src).ok(),
        "an unknown policy makes the file unparseable, which is what `pw fmt` \
         refuses on"
    );
    // And formatting is still LOSSLESS if someone asks to see it: no token may
    // be dropped from a file the compiler could not read.
    let formatted = pw_syntax::format_source(src);
    assert!(
        formatted.contains("partiton"),
        "the clause survives formatting for inspection: {formatted}"
    );
}
