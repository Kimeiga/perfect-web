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
    out.extend(read_at("lib"));
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
fn every_rejected_fixture_resolves_against_the_library_it_imports() {
    // Each on its own, because five of them share module names with each other
    // — plus whatever accepted modules the fixture explicitly imports. A
    // counterexample may depend on a correct module: `R-004` is a page that
    // MISUSES a correctly-declared session query, and the label that makes it a
    // violation comes from that query's own declaration.
    let lib = library();
    let accepted = read("accepted");
    let mut problems = Vec::new();
    let mut checked = 0;

    for one in read("rejected") {
        let name = one.0.clone();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/rejected")
            .join(&name);
        let src = std::fs::read_to_string(&path).expect("read");
        let heads: Vec<String> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("import "))
            .map(|m| m.split(['.', ' ']).next().unwrap_or("").to_string())
            .collect();

        let mut files = lib.clone();
        for (n, h) in &accepted {
            let module = h
                .modules
                .iter()
                .next()
                .map(|(_, m, _)| m.name.clone())
                .unwrap_or_default();
            let head = module.split('.').next().unwrap_or(&module).to_string();
            if heads.contains(&head) {
                files.push((n.clone(), h.clone()));
            }
        }
        files.push(one);
        checked += 1;
        let (_, mut p) = build(&files);
        for msg in p.drain(..) {
            problems.push(format!("{name}: {msg}"));
        }
    }

    // Counted from the directory, not written as a constant: a corpus that
    // grows past a hardcoded floor leaves its new fixtures unchecked while the
    // assertion still passes.
    assert_eq!(
        checked,
        read("rejected").len(),
        "expected the whole rejected corpus"
    );
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

#[test]
fn a_fixture_sees_the_same_diagnostics_alone_as_in_the_harness() {
    // The corpus invariant the architect made permanent:
    //
    //   Running one fixture alone and running the complete corpus harness must
    //   produce the same diagnostics for that fixture.
    //
    // It failed before E2B — the harness compiled all 68 files as one
    // workspace, so a fixture's result depended on which siblings happened to
    // be in the set.
    use pw_core::check::check_sources;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let lib: Vec<(String, String)> = {
        let mut v = Vec::new();
        for dir in ["lib", "../packages/pw-std", "../packages/pw-platform-web"] {
            for e in std::fs::read_dir(root.join(dir)).expect("dir") {
                let p = e.expect("entry").path();
                if p.extension().is_some_and(|x| x == "pw") {
                    v.push((
                        p.file_name().unwrap().to_string_lossy().to_string(),
                        std::fs::read_to_string(&p).expect("read"),
                    ));
                }
            }
        }
        v.push((
            "domain.pw".into(),
            std::fs::read_to_string(root.join("domain.pw")).expect("domain"),
        ));
        v
    };

    let mut checked = 0;
    for e in std::fs::read_dir(root.join("rejected")).expect("rejected") {
        let p = e.expect("entry").path();
        if p.extension().is_none_or(|x| x != "pw") {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).expect("read");

        let mut alone = lib.clone();
        alone.push((name.clone(), src.clone()));
        let a: Vec<&str> = check_sources(&alone)
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, d)| d.iter().map(|d| d.code).collect())
            .unwrap_or_default();

        // The same fixture, with a sibling that shares its module name added.
        let mut with_sibling = alone.clone();
        with_sibling.push((
            "sibling.pw".into(),
            src.replace("// @id:", "// @sibling @id:"),
        ));
        let b: Vec<&str> = check_sources(&with_sibling)
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, d)| d.iter().map(|d| d.code).collect())
            .unwrap_or_default();

        checked += 1;
        assert_eq!(
            a, b,
            "{name}: a sibling with the same module name changed its diagnostics \
             — fixtures are not isolated"
        );
    }
    assert_eq!(
        checked,
        read("rejected").len(),
        "checked {checked} fixtures"
    );
}

#[test]
fn two_fixtures_sharing_a_module_name_do_not_contaminate_each_other() {
    // The architect's explicit request: deliberately give two sibling fixtures
    // the same module name; both must pass independently rather than collide.
    //
    // Synthetic rather than corpus-derived, so the control is provable: each
    // fixture declares a DIFFERENT type under the SAME module name, and the
    // other fixture must not be able to see it.
    use pw_core::check::check_sources;
    use pw_core::resolve::{Resolution, Workspace};

    let a = "module store.page\n\ntype OnlyInA = OnlyInA { x: Int }\n";
    let b = "module store.page\n\ntype OnlyInB = OnlyInB { y: Int }\n";

    // Each alone: clean, and sees only its own type.
    for (name, src, mine, theirs) in [
        ("a.pw", a, "OnlyInA", "OnlyInB"),
        ("b.pw", b, "OnlyInB", "OnlyInA"),
    ] {
        let files = vec![(name.to_string(), src.to_string())];
        let diags = &check_sources(&files)[0].1;
        assert!(diags.is_empty(), "{name} alone must be clean: {diags:?}");

        let hir = lower_file(src, &parse_tree(src).green);
        let ws = Workspace::build(&[&hir]);
        assert!(matches!(ws.resolve(0, mine), Resolution::Local(_)));
        assert_eq!(
            ws.resolve(0, theirs),
            Resolution::Unresolved,
            "{name} must not see the sibling's declaration"
        );
    }

    // Together they are a genuine collision — two modules of one name have no
    // single graph — and the compiler says so rather than picking one.
    let both = vec![
        ("a.pw".to_string(), a.to_string()),
        ("b.pw".to_string(), b.to_string()),
    ];
    let reported: usize = check_sources(&both).iter().map(|(_, d)| d.len()).sum();
    assert!(
        reported > 0,
        "compiling both as one program must report the collision, not resolve it \
         silently — that silence is what let fixture results depend on which \
         siblings happened to be in the set"
    );
}
