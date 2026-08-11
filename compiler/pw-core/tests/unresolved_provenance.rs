//! **What each unresolved call is currently worth, frozen before it is
//! repaired.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Freeze one matrix before repairing them. […] The StorePage discovery tells
//! > us why this matters: resolving a previously nonexistent operation can
//! > change much more than name checking. So when you regenerate E8 evidence,
//! > regenerate it against ALL semantic movements caused by this resolution
//! > repair, not just `current_session`.
//!
//! Four **accepted** programs call names that resolve to nothing:
//!
//! ```text
//! A-005   current_consumer()          in a command body
//! A-014   add_to_cart(..)             in a resumable handler
//! A-013   include_markdown("..")      in a build-time page
//! store   current_session()           in a page and two commands
//! ```
//!
//! Each has been checking clean since it was written. Not because a rule was
//! wrong, but because **no rule asked**: every analysis produces an answer for a
//! call it cannot resolve, and each of those answers is indistinguishable from
//! *this call is harmless*.
//!
//! ```text
//! inference   contributes no effects
//! privacy     contributes no label
//! placement   contributes no constraint
//! contract    contributes no capability
//! ```
//!
//! This file records what those four programs claim TODAY. After the repair,
//! each row moves or it does not, and every movement gets classified — which is
//! the only way "we fixed an import" can be told apart from "the declared
//! authority of an accepted program changed".
//!
//! # It asserts today's values, so it fails on the repair
//!
//! Deliberately. `docs/RISK_QUEUE.md`'s admissibility rule wants the instrument
//! to predate the change; a matrix that merely *printed* would let a repair land
//! with nobody comparing. This goes red, and the diff is the classification.

use std::collections::BTreeSet;

use pw_core::contract::{ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The shared library every accepted fixture is checked against.
fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "examples/lib",
        "packages/pw-std",
        "packages/pw-platform-web",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

fn contracts_for(extra: &[&str]) -> Vec<ComponentContract> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = library();
    for rel in extra {
        files.push((
            rel.rsplit('/').next().unwrap().to_string(),
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")),
        ));
    }
    let hirs: Vec<Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
}

fn of<'a>(cs: &'a [ComponentContract], id: &str) -> &'a ComponentContract {
    cs.iter().find(|c| c.component_id == id).unwrap_or_else(|| {
        panic!(
            "no contract for `{id}`; have {:?}",
            cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
        )
    })
}

fn caps(c: &ComponentContract) -> BTreeSet<String> {
    c.required_capabilities.iter().map(|k| k.name()).collect()
}

// --- the four rows -----------------------------------------------------------

/// **A-005 — `current_consumer()` in a command body. REPAIRED.**
///
/// ```text
/// add_to_cart   {database.write<Carts>}   ->   {+ session.read}
/// placements    origin                          unchanged
/// ```
///
/// # The repair is the name, and this is a deviation to record
///
/// The architect expected a new platform operation:
///
/// > `current_consumer()` — likely another invocation-context operation. Give
/// > it a real semantic declaration rather than merely adding the spelling to a
/// > prelude exemption.
///
/// Two independent pieces of evidence say this call site is the **session**:
/// `Carts.add(s: SessionId, ..)` is what it feeds, and the command's own
/// `invalidates Cart(current_session())` says which cache entry it drops. A
/// consumer identity in one and a session in the other would be two different
/// carts.
///
/// So no `current_consumer` was declared. Inventing a platform operation to
/// justify a name that the types say is wrong would be ADR-0022's coincidental
/// correctness in reverse: a real declaration built to make a mistaken call
/// site typecheck. `ConsumerId` does exist in the domain and A-011 uses it — if
/// a consumer-scoped context operation is wanted, it should be added where
/// something needs it.
#[test]
fn a005_idempotent_command_after_repair() {
    let cs = contracts_for(&["examples/accepted/A-005-idempotent-command.pw"]);
    let c = of(&cs, "cart.commands.add_to_cart");
    assert_eq!(
        caps(c),
        BTreeSet::from([
            "database.write<Carts>".to_string(),
            "session.read".to_string()
        ]),
        "the write it always had, plus the session read it was doing without \
         saying"
    );
    assert_eq!(c.allowed_placements, ["origin"]);
}

/// **A-014 — `add_to_cart(..)` inside a resumable handler. REPAIRED.**
///
/// The row that had no capability column at all, because a `view` produced no
/// contract — so its before-state was not *requires nothing* but *there is
/// nothing to ask*, and the E8-0 property it read as evidence for was not being
/// exercised by it in either direction.
///
/// Two repairs, and both were needed:
///
/// ```text
/// component_kind gains `view`      there is now something to ask
/// A-014 imports add_to_cart        there is now an answer
/// ```
///
/// The result is the architecture the ruling asked for:
///
/// ```text
/// store.add_button.AddToCartButton   caps {}
///                                    imports pw:app/cart.commands.add_to_cart
/// cart.commands.add_to_cart          caps {database.write<Carts>, session.read}
/// ```
///
/// The view does **not** inherit the command's authority. It declares a
/// component dependency on it, identified by resolved identity — which is
/// exactly what the architect specified, and the opposite of `view requires
/// database.write<Carts>`.
#[test]
fn a014_content_addressed_handler_after_repair() {
    // A-005 too: it declares the command A-014 imports. The dependency is
    // between two fixtures, which is what makes it a component dependency
    // rather than a local call.
    let cs = contracts_for(&[
        "examples/accepted/A-005-idempotent-command.pw",
        "examples/accepted/A-014-content-addressed-resumable-handler.pw",
    ]);
    let view = of(&cs, "store.add_button.AddToCartButton");

    assert!(
        caps(view).is_empty(),
        "the view renders and holds no authority: {:?}",
        caps(view)
    );
    assert_eq!(
        view.imports
            .iter()
            .map(|i| i.interface.as_str())
            .collect::<Vec<_>>(),
        ["pw:app/cart.commands.add_to_cart"],
        "and its handler's reference is a component DEPENDENCY"
    );

    // The other half of the separation: the authority is recorded once, against
    // the thing that performs it. Without this the assertion above would also
    // pass for a program where nobody requires the write.
    assert_eq!(
        caps(of(&cs, "cart.commands.add_to_cart")),
        BTreeSet::from([
            "database.write<Carts>".to_string(),
            "session.read".to_string()
        ]),
        "the command carries what the view does not"
    );
}

/// **The page-level witness the ruling asked to be kept separate.**
///
/// > Because the documented claim specifically says *a page* does not inherit
/// > its handler's authority, I'd also retain or create an actual **page-level
/// > witness** for that claim rather than quietly substituting a view.
///
/// `StorePage` renders `on:press={resumable(..) => add_to_cart(..)}` where
/// `add_to_cart` is a command in its own module requiring
/// `database.write<Carts>`. The page requires `session.read` — which it does
/// need, to build its query key — and not the write.
///
/// Stated as its own test rather than left implicit in the store's contract,
/// because an incidental fact nobody asserts is exactly what A-014 turned out
/// to be.
#[test]
fn a_page_does_not_inherit_its_handlers_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut extra: Vec<String> = Vec::new();
    for e in std::fs::read_dir(root.join("examples/store")).expect("store") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            extra.push(format!(
                "examples/store/{}",
                p.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    extra.sort();
    let refs: Vec<&str> = extra.iter().map(String::as_str).collect();
    let cs = contracts_for(&refs);

    let page = of(&cs, "store.page.StorePage");
    let handler = of(&cs, "store.page.add_to_cart");

    assert!(
        caps(handler).contains("database.write<Carts>"),
        "the handler writes: {:?}",
        caps(handler)
    );
    assert!(
        !caps(page).contains("database.write<Carts>"),
        "and the page that renders the button does NOT: {:?}",
        caps(page)
    );

    // The discriminator. A page requiring nothing at all would satisfy the
    // assertion above without proving any separation — this one requires
    // exactly what it performs while rendering, and nothing the deferred
    // handler performs.
    assert_eq!(
        caps(page),
        BTreeSet::from(["session.read".to_string()]),
        "the page requires what IT does, which is read the session to build a          query key"
    );
}

/// **A-013 — `include_markdown("..")` in a build-time page. REPAIRED.**
///
/// ```text
/// Terms   caps {}                              -> {build.input.read<WorkspaceFile>}
///         placements everywhere -> ["build"]      (by the CONTRACT repair)
///                               -> ["build"]      (now also by the EFFECT)
/// ```
///
/// Two repairs land on this row and they are independent. The placement moved
/// first because `contract.rs` stopped discarding the author's pin. The
/// capability moved second, and it is the one that matters: the page is now
/// confined to build time **by what it does**, not by what it declares.
///
/// Architect ruling, 2026-08-10:
///
/// > `include_markdown` should not be an exempt compiler spelling, and I would
/// > not model it as an unrestricted ordinary filesystem read. It is a
/// > **tracked build input operation**. […] tracked source input read ≠ ambient
/// > filesystem I/O.
///
/// So `effect build.input.read<T>` is `placement build`, and the page's
/// determinism claim is carried by an effect instead of by an absence. Remove
/// the pin and the answer would still be `build`, which is what says the claim
/// is now measured rather than asserted.
#[test]
fn a013_build_time_page_after_repair() {
    let cs = contracts_for(&["examples/accepted/A-013-build-time-deterministic-page.pw"]);
    let c = cs
        .iter()
        .find(|c| c.exports.iter().any(|e| e.kind == "page"))
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        caps(c),
        BTreeSet::from(["build.input.read<WorkspaceFile>".to_string()]),
        "the page's authority is the build input it reads"
    );
    assert_eq!(
        c.allowed_placements,
        ["build"],
        "and it can only be produced at build time, which is now derived from \
         the effect rather than accepted from the pin"
    );
}

/// **The store — `current_session()` in a page and two commands. REPAIRED.**
///
/// The row the architect ruled on first, and the classification of its
/// movement:
///
/// ```text
/// StorePage     {}                        -> {session.read}
/// add_to_cart   {database.write<Carts>}   -> {+ session.read}
/// clear_cart    {database.write<Carts>}   -> {+ session.read}
/// placements    origin                       unchanged
/// ```
///
/// The repair is one line — `import context.{ current_session }` — and
/// `packages/pw-platform-web/context.pw` had declared the function since E2C.
/// Nothing was invented; a name that already existed was brought into scope.
///
/// **What moved is a claim, not a spelling.** `StorePage` renders by building
/// a query key from the session, so it reads the session to render. The E8
/// evidence saying a page renders without authority described a page whose
/// call named nothing. That is the whole reason the matrix was frozen first.
#[test]
fn store_page_after_repair() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut extra: Vec<String> = Vec::new();
    for e in std::fs::read_dir(root.join("examples/store")).expect("store") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            extra.push(format!(
                "examples/store/{}",
                p.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    extra.sort();
    let refs: Vec<&str> = extra.iter().map(String::as_str).collect();
    let cs = contracts_for(&refs);

    let page = of(&cs, "store.page.StorePage");
    assert_eq!(
        caps(page),
        BTreeSet::from(["session.read".to_string()]),
        "the page reads the session to render, and now says so"
    );
    assert_eq!(page.allowed_placements, ["origin"]);

    let add = of(&cs, "store.page.add_to_cart");
    assert_eq!(
        caps(add),
        BTreeSet::from([
            "database.write<Carts>".to_string(),
            "session.read".to_string()
        ]),
        "the write AND the session read — which is what the backend saw as two \
         host calls before the import was applied"
    );

    // The discriminator. `Menu` is on the same page, in the same module, and
    // calls no context operation — so the movement above is attributable to
    // the repair rather than to something that moved every contract.
    assert_eq!(
        caps(of(&cs, "store.page.Menu")),
        BTreeSet::from(["database.read<Menus>".to_string()]),
        "an unrelated query in the same module did not move"
    );
}

// --- the property that makes the matrix worth freezing ------------------------

/// **The work-list is empty, and that is the claim this file now makes.**
///
/// Every row above was a value derived, in part, from a call that named
/// nothing — and each analysis that contributed to it did so by contributing
/// NOTHING. Four programs, four repairs, no two alike:
///
/// ```text
/// current_session    an import; `context.pw` had declared it since E2C
/// current_consumer   the NAME was wrong; the types said which one belonged
/// include_markdown   a new tracked build-input operation, `placement build`
/// add_to_cart        an import, plus `view` becoming a component kind so
///                    there was a contract for the dependency to live in
/// ```
///
/// This test is what stops the list from silently regrowing.
#[test]
fn no_accepted_program_calls_a_name_it_does_not_import() {
    use pw_core::resolve::{Resolution, local_bindings};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = library();
    let mut names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    let mut accepted: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("examples/accepted"))
        .expect("accepted")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    accepted.sort();
    for p in accepted {
        names.push(p.file_name().unwrap().to_string_lossy().to_string());
        files.push((
            p.file_name().unwrap().to_string_lossy().to_string(),
            std::fs::read_to_string(&p).expect("read"),
        ));
    }
    let hirs: Vec<Hir> = files
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);

    // The four names, by their spelling, since that is what a regression would
    // reintroduce. `tests/semantic_ownership.rs` is the general form of this
    // question; here it is pinned to the specific history.
    const REPAIRED: &[&str] = &[
        "current_session",
        "current_consumer",
        "include_markdown",
        "add_to_cart",
    ];
    let mut broken = Vec::new();
    for (unit, hir) in refs.iter().enumerate() {
        for (_, d) in hir.all_decls() {
            let Some(bid) = d.body else { continue };
            let body = hir.body(bid);
            let locals = local_bindings(body);
            for id in body.walk() {
                let pw_core::hir::Expr::Call { callee, .. } = body.expr(id) else {
                    continue;
                };
                let path = pw_core::infer::path_of(body, *callee);
                if !REPAIRED.contains(&path.as_str()) || locals.contains(&path) {
                    continue;
                }
                if matches!(ws.resolve_path(unit, &path), Resolution::Unresolved) {
                    broken.push(format!("{}: {path}", names[unit]));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "a repaired name is being called without being imported again: {broken:?}"
    );
}
