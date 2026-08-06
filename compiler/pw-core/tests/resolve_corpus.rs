//! Does the corpus resolve? (E2B gate)
//!
//! The corpus is **not** one program, and the resolver proved it: 5 module
//! names are reused inside `rejected/`, because each rejected file is an
//! independent counterexample and several are variants of the same module.
//!
//! The real shape, which is also how the corpus is used:
//!
//! ```text
//! domain.pw + accepted/*      one program        (0 duplicate module names)
//! domain.pw + one rejected/*  one program each   (a counterexample)
//! ```
//!
//! Asserting a single ambient workspace over everything would have been the
//! same mistake assumption A-009 made, one level up.

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::*;
use pw_syntax::parse_tree;

/// The library every corpus program is checked against: the shared `domain`
/// module plus E2C's platform interfaces. Charter §12's package tree, in the
/// shape the architect ruled: corpus-owned types in the corpus, platform
/// contracts in `packages/`.
fn library() -> Vec<(String, Hir)> {
    let mut out = read("domain.pw");
    out.extend(read_at("../packages/pw-std"));
    out.extend(read_at("../packages/pw-platform-web"));
    out
}

fn read(rel: &str) -> Vec<(String, Hir)> {
    read_at(rel)
}

fn read_at(rel: &str) -> Vec<(String, Hir)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples/");
    let path = root.join(rel);
    let mut out = Vec::new();
    if path.is_file() {
        let src = std::fs::read_to_string(&path).expect("read");
        out.push((rel.to_string(), lower_file(&src, &parse_tree(&src).green)));
        return out;
    }
    for e in std::fs::read_dir(&path).expect("dir") {
        let p = e.expect("entry").path();
        if p.extension().is_none_or(|x| x != "pw") {
            continue;
        }
        let src = std::fs::read_to_string(&p).expect("read");
        out.push((
            p.file_name().unwrap().to_string_lossy().to_string(),
            lower_file(&src, &parse_tree(&src).green),
        ));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn build(files: &[(String, Hir)]) -> (Workspace, Vec<String>) {
    let hirs: Vec<&Hir> = files.iter().map(|(_, h)| h).collect();
    let ws = Workspace::build(&hirs);
    let problems = ws
        .errors
        .iter()
        .map(|e| format!("{}: {}", files[e.unit].0, e.kind.message()))
        .collect();
    (ws, problems)
}

#[test]
fn the_accepted_corpus_plus_domain_is_one_resolvable_program() {
    let mut files = library();
    files.extend(read("accepted"));
    let (ws, problems) = build(&files);
    assert!(
        problems.is_empty(),
        "{} resolution failure(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
    assert!(ws.modules.len() >= 24, "saw {} modules", ws.modules.len());
}

#[test]
fn every_rejected_fixture_resolves_against_domain() {
    // Each on its own, because five of them share module names with each other.
    let domain = library();
    let mut problems = Vec::new();
    let mut checked = 0;

    for one in read("rejected") {
        let mut files = domain.clone();
        let name = one.0.clone();
        files.push(one);
        checked += 1;
        let (_, mut p) = build(&files);
        for msg in p.drain(..) {
            problems.push(format!("{name}: {msg}"));
        }
    }

    assert_eq!(checked, 44, "expected the whole rejected corpus");
    assert!(
        problems.is_empty(),
        "{} resolution failure(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_domain_import_resolves_through_the_module_graph() {
    // `examples/domain.pw` exists so this can be true. Before it, twelve files
    // imported a module that did not exist, and R-007 reached its type only
    // through assumption A-009's ambient union.
    let mut files = library();
    files.extend(read("accepted"));
    let (ws, _) = build(&files);

    let domain = ws
        .modules
        .iter()
        .find(|m| m.name == "domain")
        .expect("the domain module");
    assert!(domain.defines.len() >= 25, "{} types", domain.defines.len());

    let mut checked = 0;
    for m in &ws.modules {
        for imp in m.imports.iter().filter(|i| i.module == "domain") {
            for name in &imp.names {
                checked += 1;
                assert!(
                    matches!(ws.resolve(m.unit, name), Resolution::Imported { .. }),
                    "{}: `{name}` did not resolve through the graph",
                    files[m.unit].0
                );
            }
        }
    }
    assert!(checked >= 25, "expected a real sample, checked {checked}");
}

#[test]
fn r007_reaches_its_defect_through_an_import_not_ambient_visibility() {
    // Architect ruling: "name-resolution failure must not be what makes an
    // exhaustiveness fixture red." R-007 now imports `OrderState` from domain.
    let mut files = library();
    files.extend(read("rejected/R-007-unhandled-ADT-variant.pw"));
    let (ws, problems) = build(&files);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(
        matches!(
            ws.resolve(library().len(), "OrderState"),
            Resolution::Imported { ref from, .. } if from == "domain"
        ),
        "{:?}",
        ws.resolve(library().len(), "OrderState")
    );
}

#[test]
fn the_whole_corpus_is_deliberately_not_one_program() {
    // Recorded as a property, not discovered again later. Five rejected files
    // reuse module names because each is an independent counterexample.
    let mut files = library();
    files.extend(read("accepted"));
    files.extend(read("rejected"));
    let (_, problems) = build(&files);
    assert!(
        problems.len() >= 5,
        "the rejected fixtures are supposed to collide as one workspace; if this \
         is now empty the corpus changed and the per-fixture model can be \
         simplified"
    );
}
