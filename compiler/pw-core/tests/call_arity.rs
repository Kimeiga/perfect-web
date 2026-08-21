//! **PW0604 — the first check of an ordinary call site.**
//!
//! Until 2026-08-20 no call site was checked at all: not the arity, not the
//! argument types, not the result. `takes_str(42)`, `wrong_arity(1)` and
//! `takes_store(makes_cart())` all passed `pw check`, while
//! `docs/MILESTONES.md` recorded E9 — *permanent value type checker* — as
//! complete and charter §14 M9A listed `unification-based inference` among its
//! contents. `canonical_abi::a_call_site_is_not_type_checked` pinned it.
//!
//! Architect ruling, 2026-08-20, whose step 2 was *make opaque/nominal
//! compatibility use resolved `DefId`, not representation* — a step that
//! presupposed a check to repair. This is the first piece of building one.
//!
//! # Why arity first
//!
//! It needs no inference. The callee's declaration says how many parameters it
//! has and the call says how many arguments it passes, so it can be correct
//! today against the whole corpus without waiting for a type comparison to
//! exist. Argument TYPES are the next piece and are deliberately not here —
//! shipping a half-typed comparison would be worse than shipping none, because
//! a rule that fires on some mismatches reads as a rule that catches them.
//!
//! # The corpus is silent, so these controls carry the weight
//!
//! The 24 accepted fixtures, the platform library and the store all check clean
//! under this rule. That is the right outcome and it is also why every
//! assertion below has a discriminator: a rule that never fires and a rule that
//! cannot fire are indistinguishable from a green corpus.

use pw_core::check::check_sources;

fn codes(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
        .collect()
}

#[test]
fn too_few_arguments_is_refused_and_the_right_number_is_not() {
    let bad = "\
module z

fn two(a: Int, b: Int) -> Int { 0 }

fn calls() -> Int {
    two(1)
}
";
    assert!(
        codes(bad).contains(&"PW0604".to_string()),
        "{:?}",
        codes(bad)
    );

    // The discriminator, in the same shape: one more argument and it is silent.
    let good = "\
module z

fn two(a: Int, b: Int) -> Int { 0 }

fn calls() -> Int {
    two(1, 2)
}
";
    assert!(
        !codes(good).contains(&"PW0604".to_string()),
        "{:?}",
        codes(good)
    );
}

#[test]
fn too_many_arguments_is_refused() {
    // The other direction, which a rule written as `args < params` would miss.
    let src = "\
module z

fn one(a: Int) -> Int { 0 }

fn calls() -> Int {
    one(1, 2, 3)
}
";
    assert!(
        codes(src).contains(&"PW0604".to_string()),
        "{:?}",
        codes(src)
    );
}

#[test]
fn a_call_across_modules_is_checked_by_resolved_identity() {
    // **Not by spelling.** Two modules each declare `two`, with different
    // arities, and the call resolves to the imported one. A lookup keyed on the
    // written path would answer for whichever declaration happened to carry
    // that spelling — the failure `Signatures::member_of` refuses to make, at a
    // different site.
    let a = "module alpha\n\nfn two(a: Int, b: Int) -> Int { 0 }\n";
    let b = "module beta\n\nfn two(a: Int) -> Int { 0 }\n";
    let user = "\
module gamma

import beta.{ two }

fn calls() -> Int {
    two(1)
}
";
    let got: Vec<String> = check_sources(&[
        ("a.pw".to_string(), a.to_string()),
        ("b.pw".to_string(), b.to_string()),
        ("c.pw".to_string(), user.to_string()),
    ])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
    .collect();
    assert!(
        !got.contains(&"PW0604".to_string()),
        "`two` resolves to beta's one-parameter declaration, so one argument is \
         correct: {got:?}"
    );

    // And the discriminator: importing the OTHER one makes the same call wrong.
    let user2 = user.replace("import beta.{ two }", "import alpha.{ two }");
    let got2: Vec<String> = check_sources(&[
        ("a.pw".to_string(), a.to_string()),
        ("b.pw".to_string(), b.to_string()),
        ("c.pw".to_string(), user2),
    ])
    .into_iter()
    .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
    .collect();
    assert!(
        got2.contains(&"PW0604".to_string()),
        "same call, same spelling, different resolved declaration: {got2:?}"
    );
}

#[test]
fn a_callee_that_does_not_resolve_is_not_an_arity_error() {
    // Silence is the three-valued discipline, not an oversight. *Wrong*,
    // *right*, and *this analysis has nothing to say* are different answers,
    // and reporting an unresolved name as an arity mistake would be a
    // right-shaped verdict from the wrong mechanism.
    let src = "\
module z

fn calls() -> Int {
    nobody_declares_this(1, 2, 3)
}
";
    assert!(
        !codes(src).contains(&"PW0604".to_string()),
        "{:?}",
        codes(src)
    );
}

#[test]
fn a_constructor_is_left_to_pw0603() {
    // `Cart(1)` reaches a TYPE declaration, whose "parameters" are a variant's
    // fields — a different relation, already owned by `PW0603`. Two codes for
    // one defect is how a reader learns to distrust both.
    let src = "\
module z

type Cart = Cart { n: Int }

fn makes() -> Cart {
    Cart(1)
}
";
    assert!(
        !codes(src).contains(&"PW0604".to_string()),
        "{:?}",
        codes(src)
    );
}

/// **`|>` supplies an argument the call does not carry.**
///
/// `items |> List.map(f)` is `List.map(items, f)`. Counting only the call's own
/// arguments reports every pipelined call as one short — which is what the
/// first version did, and the corpus caught it on the rule's first run: five
/// sites across A-001 and A-016, all correct code.
///
/// Kept as its own control because the corpus test below would go quiet the
/// day those two fixtures change, and this relation would then be unguarded.
#[test]
fn a_piped_receiver_counts_as_an_argument() {
    let good = "\
module z

fn twice(a: Int, b: Int) -> Int { 0 }

fn calls() -> Int {
    1 |> twice(2)
}
";
    assert!(
        !codes(good).contains(&"PW0604".to_string()),
        "the piped value is the first argument: {:?}",
        codes(good)
    );

    // The discriminator: the same pipeline one argument short still fails, so
    // this is counting the piped value rather than disabling the rule for any
    // call that happens to sit on the right of a `|>`.
    let bad = "\
module z

fn thrice(a: Int, b: Int, c: Int) -> Int { 0 }

fn calls() -> Int {
    1 |> thrice(2)
}
";
    assert!(
        codes(bad).contains(&"PW0604".to_string()),
        "{:?}",
        codes(bad)
    );
}

#[test]
fn the_real_corpus_is_clean_under_this_rule() {
    // The claim the controls above are guarding: this rule was added to a
    // compiler whose accepted corpus, platform library and demo all pass it, so
    // it reports no false positives on real code. Stated as a test rather than
    // trusted, because "I ran it once" is not evidence.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<(String, String)> = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/accepted",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            files.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    files.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));

    let hits: Vec<String> = check_sources(&files)
        .into_iter()
        .flat_map(|(name, ds)| {
            ds.into_iter()
                .filter(|d| d.code == "PW0604")
                .map(move |d| format!("{name}: {}", d.message))
        })
        .collect();
    assert!(hits.is_empty(), "{hits:#?}");
    assert!(files.len() >= 50, "only {} files examined", files.len());
}
