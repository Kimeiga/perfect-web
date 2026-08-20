//! **A WIT world per contract, and the two things the gate asks of it.**
//!
//! Charter §14 M8 task 2:
//!
//! > Generate a WIT world per `ComponentContract`: the world's imports are
//! > exactly `contract.imports`, and `wit-bindgen` accepts it.
//!
//! Both halves are here.
//! `imports_are_exactly_the_contracts_imports_and_the_projection_inverts` is
//! the first, and `wit_parser_resolves_the_generated_package` is the second —
//! decided by `wit-parser`, the crate `wasm-tools` and `wit-bindgen` are both
//! built on. A WIT reader written in this repo would be a second implementation
//! of somebody else's format, and the format is the entire product here.
//!
//! # The host package is a fixture, deliberately
//!
//! A generated world imports `pw:host/carts`, which the HOST owns. The compiler
//! does not know its signatures and must not invent them — ADR-0018's boundary
//! — so a resolve test has to supply them. `HOST_WIT` below is that stand-in,
//! and its existence is the statement: **a deployment must publish a WIT
//! package for the operations it supplies, or these worlds do not resolve.**
//! Writing it here rather than generating it is what keeps that a requirement
//! on the host instead of a guess by the compiler.
//!
//! It said *the capabilities it grants* until 2026-08-20. A capability
//! authorizes an operation and does not identify one, and the Wasm encoder is
//! what proved a deployment must publish the second.

use std::collections::BTreeSet;

use pw_core::contract::{ComponentContract, ImportKind, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_core::wit;
use pw_syntax::parse_tree;

/// The store demo: the platform packages, the shared library, and the app.
fn program() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    out.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"));
    out
}

fn generated() -> (String, Vec<wit::World>, Vec<ComponentContract>) {
    let sources = program();
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let (text, worlds) = wit::package(&refs, &ws, &cs).expect("the store demo generates");
    (text, worlds, cs)
}

/// **The host's WIT, as a deployment would have to publish it.**
///
/// Hand-written, and that is the point: these are the host's signatures and the
/// compiler has no business deciding them.
const HOST_WIT: &str = "\
package pw:host;

/// **One interface per operation group, and one function per OPERATION.**
///
/// Rewritten 2026-08-20. It used to publish `database` with `read`/`write`,
/// because a component's imports were derived from its CAPABILITIES — and the
/// Wasm encoder proved that cannot work: `Carts.add(s, item, qty)` and
/// `Carts.clear(s)` both require `database.write<Carts>` and have different
/// ABIs, so one `write` could not have both signatures.
///
/// Architect ruling, 2026-08-20: a capability authorizes an operation and does
/// not identify one. `database.write<Carts>` is still the authority a
/// deployment grants; `pw:host/carts#add` is the callable it publishes.
///
/// The grouping into interfaces is an ABI/package-layout decision — `add` and
/// `clear` could equally live in one `carts` interface or two — and it does not
/// determine the capability semantics.
interface carts {
    current: func(session: string) -> string;
    add: func(session: string, item: string, quantity: s64) -> string;
    clear: func(session: string) -> string;
}

interface stores {
    get: func(id: string) -> string;
}

interface menus {
    %for-store: func(store: string) -> string;
}

/// The invocation context. Added on 2026-08-10, when `examples/store/app.pw`
/// gained the `import context.{ current_session }` it had been missing since
/// E4 — so the store's worlds began importing `pw:host/session` and stopped
/// resolving against a host that does not publish it.
///
/// That is this stand-in doing its job. A capability a component requires and
/// a deployment does not grant is a deployment that cannot run it, and the WIT
/// resolve is where that becomes visible rather than a runtime link failure.
interface session {
    read: func() -> string;
}
";

#[test]
fn imports_are_exactly_the_contracts_imports_and_the_projection_inverts() {
    // **The gate criterion.** A world imports an INTERFACE and a contract
    // imports a function within one, so the projection is many-to-one — which
    // is the only place a world could quietly gain or lose authority. Both
    // directions are checked: every contract import reaches a world import, and
    // every world import came from a contract import.
    let (_, worlds, cs) = generated();
    assert_eq!(worlds.len(), cs.len(), "one world per contract");

    for (w, c) in worlds.iter().zip(cs.iter()) {
        let expected: BTreeSet<String> = c
            .imports
            .iter()
            .map(|i| match i.kind {
                ImportKind::HostCapability => i.interface.clone(),
                // The exporter's generated interface name, which is what a
                // world can actually import.
                ImportKind::Component => format!(
                    "{}-api",
                    wit::ident(i.interface.strip_prefix("pw:app/").unwrap_or(&i.interface))
                ),
            })
            .collect();
        let actual: BTreeSet<String> = w.imports.iter().cloned().collect();
        assert_eq!(
            actual, expected,
            "{}: world imports {actual:?}, contract imports {expected:?}",
            c.component_id
        );
    }

    // And the control. Every assertion above holds for a program with no
    // imports at all, and this one does have them — a host capability and a
    // component dependency, which are the two kinds.
    let host: usize = cs
        .iter()
        .flat_map(|c| &c.imports)
        .filter(|i| i.kind == ImportKind::HostCapability)
        .count();
    let component: usize = cs
        .iter()
        .flat_map(|c| &c.imports)
        .filter(|i| i.kind == ImportKind::Component)
        .count();
    assert!(host >= 3, "only {host} host imports");
    assert!(component >= 3, "only {component} component imports");
}

#[test]
fn a_world_never_imports_a_capability_its_contract_does_not_require() {
    // The direction that matters, stated on its own. A world with one extra
    // import is a component authorised for something the compiler never
    // approved, and the audit downstream compares against the CONTRACT — so an
    // over-generous world would link cleanly and pass the audit.
    let (_, worlds, cs) = generated();
    for (w, c) in worlds.iter().zip(cs.iter()) {
        for i in &w.imports {
            if !i.starts_with("pw:host/") {
                continue;
            }
            assert!(
                c.imports
                    .iter()
                    .any(|ci| ci.kind == ImportKind::HostCapability && &ci.interface == i),
                "{} imports `{i}`, which its contract does not",
                c.component_id
            );
        }
    }
}

#[test]
fn wit_parser_resolves_the_generated_package() {
    // **The second half of the gate**, decided by the parser `wit-bindgen` is
    // built on rather than by anything here. Resolving is stricter than
    // parsing: every `use` must name a type that exists, every imported
    // interface must be found, and every world must be well-formed.
    let (text, _, _) = generated();

    let dir = std::env::temp_dir().join("pw-wit-worlds");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("deps/host")).expect("temp dir");
    std::fs::write(dir.join("app.wit"), &text).expect("write");
    std::fs::write(dir.join("deps/host/host.wit"), HOST_WIT).expect("write");

    let mut resolve = wit_parser::Resolve::new();
    let result = resolve.push_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);

    let (pkg, _) = result.unwrap_or_else(|e| panic!("the generated WIT does not resolve: {e:?}"));

    // Resolved, and it resolved to something: a package with the worlds in it,
    // not an empty file the parser was happy to accept.
    let worlds: Vec<&str> = resolve.packages[pkg]
        .worlds
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert!(
        worlds.iter().any(|w| w.contains("store-page")),
        "the store's worlds are there: {worlds:?}"
    );
    assert!(
        worlds.len() >= 8,
        "only {} worlds: {worlds:?}",
        worlds.len()
    );
}

#[test]
fn the_resolver_would_reject_a_world_naming_an_interface_nobody_publishes() {
    // The negative control for the test above. `push_dir` accepting the real
    // package means nothing unless it would refuse a broken one — and the
    // specific breakage that matters is an import the host does not publish,
    // because that is what a compiler inventing a host interface would produce.
    let dir = std::env::temp_dir().join("pw-wit-worlds-control");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("deps/host")).expect("temp dir");
    std::fs::write(
        dir.join("app.wit"),
        "package pw:app@0.1.0;\n\nworld w {\n    import pw:host/nobody-publishes-this;\n}\n",
    )
    .expect("write");
    std::fs::write(dir.join("deps/host/host.wit"), HOST_WIT).expect("write");

    let mut resolve = wit_parser::Resolve::new();
    let result = resolve.push_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(result.is_err(), "an unpublished interface must not resolve");
}

#[test]
fn every_type_an_exported_signature_names_is_declared_in_the_package() {
    // What `use types.{..}` is for, checked as text rather than through the
    // resolver — so a failure says WHICH name is missing instead of pointing at
    // a line in someone else's error type.
    let (text, _, _) = generated();
    let declared: BTreeSet<&str> = text
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            for kw in ["record ", "variant ", "type ", "enum "] {
                if let Some(rest) = l.strip_prefix(kw) {
                    return rest.split([' ', '{', '=']).next().filter(|s| !s.is_empty());
                }
            }
            None
        })
        .collect();
    let mut missing: Vec<String> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("use types.{") else {
            continue;
        };
        for name in rest.trim_end_matches("};").split(',') {
            let name = name.trim();
            if !name.is_empty() && !declared.contains(name) {
                missing.push(name.to_string());
            }
        }
    }
    assert!(missing.is_empty(), "used but not declared: {missing:?}");
    assert!(
        declared.len() >= 10,
        "only {} types declared, which is too few for this to be checking \
         anything",
        declared.len()
    );

    // And the other direction: the package holds only what an export can
    // reach. `Decoder`, `Style` and `ElementRef` are declared by the platform
    // and cross no boundary, so putting them on the ABI would describe shapes
    // nothing sends.
    for never_crosses in ["decoder", "style", "element-ref"] {
        assert!(
            !declared.contains(never_crosses),
            "`{never_crosses}` is on the ABI and nothing exports it"
        );
    }
}
