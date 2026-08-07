//! A package declares which namespace it exports ambiently.
//!
//! Architect ruling, 2026-08-07, settling the blocker that stopped effect
//! resolution reaching the checker:
//!
//! > Pleris should have a package-declared prelude per namespace. For now,
//! > introduce an Effect prelude specifically. Effect names resolve through
//! > that prelude; their arguments resolve through ordinary lexical/module
//! > visibility.
//!
//! and the rule that makes it safe:
//!
//! > Effect declarations are ambient only because the selected platform package
//! > explicitly exports them into the Effect prelude. **Their arguments are not
//! > ambient.**
//!
//! # Why this is not the ambient union in a new costume
//!
//! Assumption A-009 removed a whole-program search for every name. This adds a
//! whole-program search of exactly ONE namespace, and only because a package
//! said so in its own source. The distinction is load-bearing and every test
//! below that starts `an_effect_prelude_does_not` is about it: `prelude Effect`
//! must leave Term, Type, Ui and Event exactly as strict as they were.
//!
//! An effect name appears only in a row and never in an expression, which is
//! what makes the Effect namespace the one where this costs nothing.
//!
//! # Where the declaration lives
//!
//! The ruling sketched `prelude Effect from web.effects` in a package manifest.
//! Pleris has no manifest format — a package is a directory of `.pw` files — so
//! the declaration lives in the module that owns the declarations and names no
//! path. Same semantics for the one case that exists, one fewer invention on
//! the way, and it generalises to `prelude Type` unchanged. What it cannot
//! express is a package electing a module it does not own; a manifest is where
//! that belongs when packages get one.

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::{Namespace, Resolution, Workspace};
use pw_syntax::parse_tree;

/// A platform package: an effect, a function and a type, one namespace
/// exported.
const PLATFORM: &str = "module web.effects\n\n\
     prelude Effect\n\n\
     effect database.read<T> {\n    capability database.read<T>\n}\n\n\
     fn helper(x: Int) -> Int !{} { x }\n\n\
     type Marker = Marker {}\n";

/// A package that exports nothing. The control for every prelude assertion.
const SILENT: &str = "module quiet.effects\n\n\
     effect cache.read {\n    capability none\n}\n";

/// A user file importing neither.
const USER: &str = "module app\n\nfn f() -> Int !{} { 0 }\n";

fn workspace(sources: &[&str]) -> (Vec<Hir>, Workspace) {
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    (hirs, ws)
}

fn resolves(ws: &Workspace, unit: usize, ns: Namespace, name: &str) -> bool {
    matches!(
        ws.resolve_in(unit, ns, name),
        Resolution::Local(_) | Resolution::Imported { .. }
    )
}

#[test]
fn an_exported_effect_resolves_without_an_import() {
    let (_h, ws) = workspace(&[PLATFORM, USER]);
    assert!(
        resolves(&ws, 1, Namespace::Effect, "database.read"),
        "`app` imports nothing and must still see the platform's effects"
    );
}

#[test]
fn an_effect_from_a_package_that_exports_nothing_stays_invisible() {
    // The control that makes the test above mean something. Both packages
    // declare an effect; only one says `prelude Effect`.
    let (_h, ws) = workspace(&[PLATFORM, SILENT, USER]);
    assert!(resolves(&ws, 2, Namespace::Effect, "database.read"));
    assert!(
        !resolves(&ws, 2, Namespace::Effect, "cache.read"),
        "`quiet.effects` exports nothing, so its effects are not ambient"
    );
}

#[test]
fn an_effect_prelude_does_not_make_terms_ambient() {
    // **The ruling's boundary.** `web.effects` declares `fn helper`, and
    // exporting the Effect namespace must not bring it into scope. If this
    // ever passes, `prelude Effect` has become the whole-program union A-009
    // removed, and it will have arrived silently.
    let (_h, ws) = workspace(&[PLATFORM, USER]);
    assert!(
        !resolves(&ws, 1, Namespace::Term, "helper"),
        "a Term is not exported by `prelude Effect`"
    );
}

#[test]
fn an_effect_prelude_does_not_make_types_ambient() {
    // The same boundary, and the one that matters most for A-017: an effect's
    // TYPE ARGUMENT resolves in the Type namespace, so if this leaked, the
    // argument-visibility rule would be unenforceable by construction.
    let (_h, ws) = workspace(&[PLATFORM, USER]);
    assert!(
        !resolves(&ws, 1, Namespace::Type, "Marker"),
        "a Type is not exported by `prelude Effect`; this is what makes \
         `database.read<Marker>` from an unrelated file an error"
    );
}

#[test]
fn importing_the_package_still_brings_its_terms_and_types() {
    // The prelude narrows nothing. An explicit import works exactly as before,
    // so a file that wants `helper` says so and gets it.
    let importing = "module app\n\nimport web.effects\n\nfn f() -> Int !{} { 0 }\n";
    let (_h, ws) = workspace(&[PLATFORM, importing]);
    assert!(resolves(&ws, 1, Namespace::Term, "helper"));
    assert!(resolves(&ws, 1, Namespace::Type, "Marker"));
    assert!(resolves(&ws, 1, Namespace::Effect, "database.read"));
}

#[test]
fn a_local_declaration_wins_over_the_prelude() {
    // Shadowing in the one direction that cannot surprise a reader of the
    // file: what is written here beats what the platform provides. The reverse
    // would mean a package upgrade silently changed what a local name meant.
    let shadowing = "module app\n\n\
                     effect database.read<T> {\n    capability none\n}\n";
    let (_h, ws) = workspace(&[PLATFORM, shadowing]);
    assert!(
        matches!(
            ws.resolve_in(1, Namespace::Effect, "database.read"),
            Resolution::Local(_)
        ),
        "a local declaration is not shadowed by a prelude"
    );
}

#[test]
fn a_prelude_declaration_defines_no_name_of_its_own() {
    // `prelude Effect` is a statement about the module, not a declaration
    // called `Effect`. If it took a slot in some namespace, `type Effect`
    // elsewhere would collide with it.
    let (_h, ws) = workspace(&[PLATFORM, USER]);
    for ns in Namespace::ALL {
        assert!(
            !resolves(&ws, 0, ns, "Effect"),
            "`prelude Effect` must not define a name in {ns:?}"
        );
    }
    assert!(
        ws.errors.is_empty(),
        "and it is not a duplicate of anything: {:?}",
        ws.errors
    );
}

#[test]
fn the_real_platform_exports_the_effect_namespace_and_only_that() {
    // Against the actual packages, not a fixture — the property the corpus
    // depends on. A test using its own miniature platform would pass while
    // `packages/` had forgotten the declaration.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources: Vec<String> = ["packages/pw-std", "packages/pw-platform-web"]
        .iter()
        .flat_map(|dir| {
            let mut out: Vec<String> = std::fs::read_dir(root.join(dir))
                .expect("package")
                .filter_map(|e| {
                    let p = e.expect("entry").path();
                    (p.extension()? == "pw").then(|| std::fs::read_to_string(&p).expect("read"))
                })
                .collect();
            out.sort();
            out
        })
        .chain(std::iter::once(USER.to_string()))
        .collect();
    let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    let (_h, ws) = workspace(&refs);
    let user = refs.len() - 1;

    // Every effect the corpus writes, from a file that imports nothing.
    for effect in [
        "database.read",
        "layout.measure",
        "style.mutate",
        "log",
        "trace",
        "secret",
        "network.fetch",
    ] {
        assert!(
            resolves(&ws, user, Namespace::Effect, effect),
            "`{effect}` must resolve through the prelude"
        );
    }

    // And nothing else did. `Element` is a type the platform declares and
    // `measure` is a function it declares; a file that imports nothing sees
    // neither.
    assert!(!resolves(&ws, user, Namespace::Type, "Element"));
    assert!(!resolves(&ws, user, Namespace::Term, "measure"));
    assert!(!resolves(&ws, user, Namespace::Type, "LayoutAffect"));
}
