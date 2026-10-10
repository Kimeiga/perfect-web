//! **A predicate a program declares says what a refusal by it is told**
//! (ADR-0302).
//!
//! `predicate OwnsPost(post: PostId)  says "…"`: the deployment says what a
//! predicate means (ADR-0115), and words of its own; a program may declare
//! the words a reader is told, and what the predicate takes. PW0351 holds a
//! declared predicate to its words, and a `requires` that names one to its
//! parameters by count and type. One the program does not declare is the
//! deployment's alone.
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

/// What PW0351 says of `src`.
fn refused(src: &str) -> Vec<String> {
    reported(src)
        .into_iter()
        .filter(|d| d.starts_with("PW0351 "))
        .collect()
}

const SIGNED_IN: &str = "predicate SignedIn\n    says \"Sign in to delete a post.\"\n\n";
const OWNS_POST: &str =
    "predicate OwnsPost(post: PostId)\n    says \"Only its author can delete a post.\"\n\n";

/// A command deleting a post, its `requires` clause `requires`, beside the
/// declarations `declared`.
fn program(declared: &str, requires: &str) -> String {
    format!(
        "module t\n\nopaque type PostId = String\n\n{declared}\
         command Delete(post: PostId, note: String) -> PostId !{{}}\n    requires {requires}\n{{\n    post\n}}\n"
    )
}

#[test]
fn a_command_whose_predicates_are_declared_is_the_control() {
    let src = program(
        &format!("{SIGNED_IN}{OWNS_POST}"),
        "SignedIn, OwnsPost(post)",
    );
    let found = reported(&src);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_predicate_the_program_does_not_declare_is_the_deployments() {
    // Told in the deployment's words; nothing of the program's to hold.
    let found = reported(&program(SIGNED_IN, "SignedIn, OwnsPost(post)"));
    assert!(found.is_empty(), "{found:#?}");
    let found = reported(&program("", "SignedIn, OwnsPost(post)"));
    assert!(found.is_empty(), "{found:#?}");
    // A function of the name is not its declaration: a predicate is named
    // only in `requires`, and one declared takes what it declares.
    let shadowed = program(
        &format!("{SIGNED_IN}fn OwnsPost(post: PostId, other: Int) -> Bool {{\n    true\n}}\n\n"),
        "SignedIn, OwnsPost(post)",
    );
    assert!(refused(&shadowed).is_empty(), "{:#?}", refused(&shadowed));
}

#[test]
fn a_predicate_imported_from_another_module_is_declared() {
    let domain = format!("module d\n\nopaque type PostId = String\n\n{SIGNED_IN}{OWNS_POST}");
    let app = "module t\n\nimport d.{ PostId, SignedIn, OwnsPost }\n\n\
               command Delete(post: PostId) -> PostId !{}\n    requires SignedIn, OwnsPost(post)\n{\n    post\n}\n";
    let found = reported_in(&[("d.pw", &domain), ("t.pw", app)]);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_predicate_is_given_its_parameters_by_type() {
    // One argument too few.
    assert_eq!(
        refused(&program(
            &format!("{SIGNED_IN}{OWNS_POST}"),
            "SignedIn, OwnsPost"
        )),
        ["PW0351 `OwnsPost` takes 1 argument, and `requires` gives it 0"]
    );
    // One too many.
    assert_eq!(
        refused(&program(
            &format!("{SIGNED_IN}{OWNS_POST}"),
            "SignedIn(post), OwnsPost(post)"
        )),
        ["PW0351 `SignedIn` takes 0 arguments, and `requires` gives it 1"]
    );
    // A `String` is not a `PostId`, though one is the other underneath.
    assert_eq!(
        refused(&program(
            &format!("{SIGNED_IN}{OWNS_POST}"),
            "SignedIn, OwnsPost(note)"
        )),
        ["PW0351 `OwnsPost` takes a `PostId`, and `note` is a `String`"]
    );
}

#[test]
fn a_predicate_says_what_a_reader_is_told() {
    for (says, why) in [
        (
            None,
            "`predicate SignedIn` says nothing, and a refusal by it would be told nothing",
        ),
        (
            Some("Sign in"),
            "`predicate SignedIn`: its words are one string: `says \"Sign in to post.\"`",
        ),
        (
            Some("\"Sign in\" \"to post\""),
            "`predicate SignedIn`: its words are one string: `says \"Sign in to post.\"`",
        ),
        (Some("\"\""), "`predicate SignedIn`: its words are empty"),
        (Some("\"   \""), "`predicate SignedIn`: its words are empty"),
        (
            Some("\"Sign in, {name}.\""),
            "`predicate SignedIn`: its words name nothing the refusal read: a `{` would be a hole",
        ),
    ] {
        let declared = match says {
            None => "predicate SignedIn\n\n".to_string(),
            Some(s) => format!("predicate SignedIn\n    says {s}\n\n"),
        };
        assert_eq!(
            refused(&program(&declared, "SignedIn")),
            [format!("PW0351 {why}")],
            "{says:?}"
        );
    }
}

#[test]
fn says_is_a_predicates_alone() {
    let src = program(SIGNED_IN, "SignedIn").replace(
        "    requires SignedIn\n",
        "    requires SignedIn\n    says \"Sign in.\"\n",
    );
    let found = reported(&src);
    assert!(
        found
            .iter()
            .any(|d| d.contains("says") && d.contains("a predicate")),
        "{found:#?}"
    );
}

#[test]
fn a_predicate_is_declared_once_in_a_program() {
    // A host tells a refusal by it in one set of words: a second module's
    // declaration of the name is refused, at the second.
    let one = format!("module d\n\n{SIGNED_IN}");
    let two = format!("module e\n\n{SIGNED_IN}");
    let found: Vec<String> = reported_in(&[("d.pw", &one), ("e.pw", &two)])
        .into_iter()
        .filter(|d| d.starts_with("PW0351 "))
        .collect();
    assert_eq!(
        found,
        [
            "PW0351 `predicate SignedIn` is declared in `d` already: a refusal by it is told \
          in one set of words"
        ]
    );
}
