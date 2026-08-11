//! **The executable code written in policy position, and what has looked at
//! it.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Being inside a policy never exempts an executable term from resolution,
//! > but not every call-shaped policy argument is an executable term.
//!
//! `src/policy.rs` says which policy heads hold terms. This file asks what the
//! corpus actually writes in those positions, and freezes the answer — it is
//! the work-list `PW0024` has to satisfy, and the before-state of the
//! `PolicyExpr` split.
//!
//! # The finding
//!
//! A declaration-level policy value is a `String` in the HIR: the characters
//! after the keyword, trimmed. Not an expression, not a tree, not spans. So for
//! every one of the term positions below, **nothing has ever parsed it**.
//!
//! ```text
//! optimistic cart.add(item, quantity)
//! rollback   cart.remove(item)
//! ```
//!
//! That is real client-side code — it runs before the round trip, and the
//! rollback ran if the command failed. `cart` was not a module, not an import,
//! and not a binding in either declaration that wrote it. Two calls into the
//! void, checking clean since E4 for a reason distinct from the four in
//! `unresolved_provenance.rs`: those resolve to nothing, and these were never
//! anything the compiler could resolve.
//!
//! Both are repaired (ADR-0025). `optimistic` names the resource ENTRY it
//! updates and binds its current value; `rollback` is written by nobody,
//! because the platform restores the value it held and a written inverse is
//! generally false.
//!
//! # Why the list is frozen rather than merely printed
//!
//! Because the repair is to parse these, and once they are parsed some of them
//! will be errors. The diff between this list and the diagnostics that follow
//! is the classification.

use std::collections::BTreeSet;

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::policy::{Domain, carries_terms, domain_of};
use pw_syntax::parse_tree;

/// Every `.pw` file the corpus checks, excluding the frozen history.
fn corpus() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs = vec![
        root.join("examples"),
        root.join("examples/accepted"),
        root.join("examples/rejected"),
        root.join("examples/lib"),
        root.join("examples/store"),
        root.join("packages/pw-std"),
        root.join("packages/pw-platform-web"),
    ];
    if let Ok(rd) = std::fs::read_dir(root.join("examples/rules")) {
        dirs.extend(rd.map(|e| e.expect("entry").path()));
    }
    if let Ok(rd) = std::fs::read_dir(root.join("examples/generality")) {
        dirs.extend(rd.map(|e| e.expect("entry").path()));
    }
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut ps: Vec<std::path::PathBuf> = rd
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            out.push((
                p.strip_prefix(&root).unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

/// `(head, value)` for every declaration-level policy in a term position.
fn term_positions() -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for (_, src) in corpus() {
        let hir: Hir = lower_file(&src, &parse_tree(&src).green);
        for (_, decl) in hir.all_decls() {
            for p in &decl.policies {
                if carries_terms(&p.name) {
                    out.insert((p.name.clone(), p.value.trim().to_string()));
                }
            }
        }
    }
    out
}

/// **The work-list.**
///
/// Grouped by what the repair for each is, because the architect's ruling is
/// that they must not all receive the same one:
///
/// ```text
/// ResourceRef   depends_on, invalidates   names a DECLARATION, keys are terms
/// EventRef      emits, invalidates_on     names a DECLARATION, keys are terms
/// PredicateRef  requires                  names a predicate, args are terms
/// Term          optimistic, rollback      IS a term, all the way down
/// ```
#[test]
fn the_corpus_writes_exactly_these_terms_in_policy_position() {
    let found = term_positions();
    let expected: BTreeSet<(String, String)> = [
        ("depends_on", "Cart(session)"),
        ("depends_on", "Draft(session)"),
        ("depends_on", "Menu(id)"),
        ("depends_on", "Menu(id), Cart(id)"),
        ("depends_on", "Orders(consumer)"),
        ("depends_on", "Personalised(session)"),
        ("depends_on", "Store(id), Menu(id)"),
        ("depends_on", "Summary(session)"),
        ("emits", "CartChanged(current_session())"),
        // New 2026-08-11: `privacy Public` on `A-013`'s page. It was two bare
        // names in the executable body until UI declarations began parsing
        // their policies inside their braces, so this list could not see it —
        // which is the list doing its job, not a new defect. `privacy` is a
        // `LabelCtor` and `Public` is a nullary one.
        ("privacy", "Public"),
        ("invalidates", "Cart(current_session())"),
        ("invalidates", "Order(order)"),
        ("invalidates_on", "CartChanged(session)"),
        ("invalidates_on", "MenuChanged(id)"),
        (
            "invalidates_on",
            "MenuChanged(id), InventoryChanged(id, _item: MenuItemId)",
        ),
        (
            "invalidates_on",
            "MenuChanged(id), InventoryChanged(id, item), StoreChanged(id)",
        ),
        (
            "invalidates_on",
            "MenuChanged(id), InventoryChanged(id, item)",
        ),
        ("invalidates_on", "MenuChanged(id), PriceChanged(id)"),
        ("invalidates_on", "MenuChanged(id), StoreChanged(id)"),
        ("invalidates_on", "StoreChanged(consumer)"),
        ("invalidates_on", "StoreChanged(id)"),
        (
            "optimistic",
            "Cart(current_session()) as cart => Carts.add(current_session(), item, quantity)",
        ),
        (
            "optimistic",
            "Cart(current_session()) as cart => Carts.current(current_session())",
        ),
        (
            "optimistic",
            "Cart(current_session()) as cart => Carts.with_line(cart, item, quantity)",
        ),
        ("requires", "SignedIn"),
        ("requires", "SignedIn, OwnsOrder(order)"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();

    let added: Vec<_> = found.difference(&expected).collect();
    let gone: Vec<_> = expected.difference(&found).collect();
    assert!(
        added.is_empty() && gone.is_empty(),
        "the executable code written in policy position changed.\n  \
         new: {added:?}\n  gone: {gone:?}\n\n\
         Every entry here is code nothing parses. Adding one adds an \
         unchecked call; removing one is a repair that needs classifying \
         against `docs/RISK_QUEUE.md`."
    );
}

/// **An `optimistic` clause names an entry, binds its value, and transforms
/// it — and there is no written inverse.**
///
/// The sharpest row, isolated so it cannot be lost in the list. It read
/// `optimistic cart.add(item, quantity)` with `cart` bound by nothing, and
/// nothing had ever parsed it.
///
/// Architect ruling, 2026-08-11 (ADR-0025):
///
/// > An optimistic clause identifies a resource entry and binds its current
/// > value; its body is an ordinary Pleris transition expression.
///
/// A bare lambda said what transformation to perform and not which entry it
/// applies to, and `Cart(session A)` and `Cart(session B)` are different
/// objects of the same type.
#[test]
fn an_optimistic_clause_has_a_target_a_binder_and_a_transition() {
    assert_eq!(domain_of("optimistic"), Some(Domain::Transition));
    assert_eq!(
        domain_of("rollback"),
        Some(Domain::Derived),
        "`rollback` is written by nobody: the platform restores the value it \
         held, and a hand-written inverse describes a different operation"
    );

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(root.join("examples/store/app.pw")).expect("store");
    let hir = lower_file(&src, &parse_tree(&src).green);
    let (_, add) = hir
        .all_decls()
        .find(|(_, d)| d.name == "add_to_cart")
        .expect("add_to_cart");
    let body = hir.body(add.body.expect("body"));

    let (_, t) = add.transitions().next().expect("an optimistic transition");
    assert_eq!(t.binder, "cart");

    // Three pieces of meaning, each with a span in the FILE. A sub-parse starts
    // at zero, so without the shift a diagnostic would underline the first few
    // columns of the module declaration.
    assert_eq!(&src[body.expr_span(t.target)], "Cart(current_session())");
    assert_eq!(&src[t.binder_span.clone()], "cart");
    assert_eq!(
        &src[body.expr_span(t.body)],
        "Carts.with_line(cart, item, quantity)"
    );

    // And the compiler binds nothing on its own: drop the binder and `cart` is
    // an unresolved name rather than something supplied by the keyword.
    let without = src.replace(
        "Cart(current_session()) as cart =>",
        "Cart(current_session()) as _c =>",
    );
    let out = pw_core::check::check_sources(&[("app.pw".to_string(), without)]);
    assert!(
        out.iter()
            .flat_map(|(_, ds)| ds)
            .any(|d| d.code == "PW0021" && d.message.contains("cart")),
        "renaming the binder must expose `cart`, not fall back to a magic \
         binding named after the policy"
    );
}

/// **Every term-carrying head is a head the parser knows.**
///
/// Otherwise `carries_terms` would be describing a policy nobody can write,
/// and the work-list above would be a list of hypotheticals.
#[test]
fn every_term_position_is_a_real_policy() {
    let heads: BTreeSet<String> = term_positions().into_iter().map(|(h, _)| h).collect();
    for h in &heads {
        assert!(
            pw_syntax::POLICY_KEYWORDS.contains(&h.as_str()),
            "`{h}` is not a policy the parser recognises"
        );
        assert!(carries_terms(h));
    }
    // The discriminator: the corpus also writes plenty of policies that hold
    // no code, and none of them are in the list.
    for h in ["cache", "freshness", "key", "placement", "retry"] {
        assert!(!heads.contains(h), "`{h}` should not carry terms");
    }
}
