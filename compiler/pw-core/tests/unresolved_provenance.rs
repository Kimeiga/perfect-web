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

/// **A-014 — `add_to_cart(..)` inside a resumable handler.**
///
/// The row with no capability column, and the reason is worth recording: A-014
/// declares a `view`, and a view produces **no contract at all** —
/// `component_kind` covers query, command, page and component. So the
/// before-state here is not "requires nothing"; it is "there is nothing to ask".
///
/// That makes the fixture's own subject unmeasurable from the contract side.
/// It is a content-addressed resumable handler, and the handler calls a name
/// that resolves to nothing — so whatever authority `add_to_cart` would carry
/// is attributed to no one, and the E8-0 property it looks like evidence for
/// (a page does not inherit its handler's authority) is not being exercised.
#[test]
fn a014_content_addressed_handler_before_repair() {
    let cs = contracts_for(&["examples/accepted/A-014-content-addressed-resumable-handler.pw"]);
    assert!(
        !cs.iter()
            .any(|c| c.component_id.starts_with("store.add_button")),
        "TODAY: a `view` produces no contract, so this fixture contributes no          capability claim in either direction. If `view` ever becomes a          component kind, this row gains a capability column and the handler's          unresolved `add_to_cart` becomes visible as an absence. Got {:?}",
        cs.iter().map(|c| &c.component_id).collect::<Vec<_>>()
    );

    // And the call itself. Asserted on the source, because there is no derived
    // artifact to read it from — which is the finding.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(
        root.join("examples/accepted/A-014-content-addressed-resumable-handler.pw"),
    )
    .expect("A-014");
    assert!(src.contains("add_to_cart("), "the handler calls it");
    assert!(
        !src.contains("import cart") && !src.contains("{ add_to_cart }"),
        "and imports it from nowhere"
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

/// **Nothing above is a statement that these programs are correct.**
///
/// Every row is a value derived, in part, from a call that names nothing — and
/// each analysis that contributed to it did so by contributing NOTHING. The
/// matrix exists so that after the repair, a row that did not move is as
/// interesting as one that did.
#[test]
fn every_frozen_row_is_derived_from_a_call_that_resolves_to_nothing() {
    // The four programs, and the name each one calls into the void. Listed here
    // rather than re-derived, so this file states its own subject: if a repair
    // lands and this list is stale, the test that reads it fails.
    // Three of the four are REPAIRED, each differently, and each classified in
    // the test above it:
    //
    //     current_session    an import; the declaration already existed
    //     current_consumer   the NAME was wrong; the types said so
    //     include_markdown   a new tracked build-input operation
    //
    // They are named here rather than deleted so this reads as a work-list with
    // items struck through, not as a list that was always one long.
    const UNRESOLVED: &[(&str, &str)] = &[(
        "examples/accepted/A-014-content-addressed-resumable-handler.pw",
        "add_to_cart",
    )];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (file, name) in UNRESOLVED {
        let src =
            std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            src.contains(&format!("{name}(")),
            "{file} no longer calls `{name}` — the matrix above is describing a \
             program that has changed, and its rows are no longer the \
             before-state of anything"
        );
        // And it is still unimported. A repair adds an import or a declaration;
        // either way this stops holding, and the row above has to be
        // reclassified rather than silently kept.
        assert!(
            !src.contains(&format!("{{ {name} }}")) && !src.contains(&format!("{name} }}")),
            "{file} now imports `{name}`. Reclassify the frozen row: what did \
             its effects, capabilities, label and placement become?"
        );
    }
}
