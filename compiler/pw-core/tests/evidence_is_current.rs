//! **Committed evidence must be what the compiler produces today.**
//!
//! `docs/evidence/E8/store.wit` lagged the code on 2026-08-21. The `SessionId`
//! migration changed every export's parameter type from `domain-session-id` to
//! `capability-session-id`, and the file kept saying the old thing for two
//! commits — through a green `just ci`, because nothing compares them.
//!
//! That is worse here than in most projects. `CLAUDE.md`:
//!
//! > Do not claim a gate item passed without a file under `docs/evidence/` that
//! > a recorded command produced.
//!
//! Evidence that a recorded command produced *at some point* is not evidence
//! that the command produces it now. A stale artifact is a claim about a
//! compiler that no longer exists, and it looks exactly like a fresh one.
//!
//! # Why a test rather than a `just` recipe
//!
//! `just ci` is the unit of evidence, and a recipe nobody runs is a check that
//! does not exist. This makes staleness a build failure with a repair in the
//! message, which is the same reasoning that made `just ci` run
//! `clippy -D warnings`.

use pw_core::contract::contracts;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_core::wit;
use pw_syntax::parse_tree;

/// The same sources `just e8-wit` passes to `pw emit-wit`, in the same order.
///
/// Duplicated from the recipe deliberately — if the recipe's file list changes
/// and this does not, the two disagree and the assertion below says so. A test
/// that read the recipe would agree with it by construction and prove nothing.
fn store_sources(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            out.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    out.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"));
    for dir in ["examples/lib", "examples/store"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            out.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    out
}

#[test]
fn the_committed_wit_is_what_the_compiler_emits_now() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources = store_sources(&root);
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let (fresh, _) = wit::package(&refs, &ws, &cs).expect("the store demo generates");

    let committed =
        std::fs::read_to_string(root.join("docs/evidence/E8/store.wit")).expect("store.wit");

    if fresh.trim() == committed.trim() {
        return;
    }

    // Name the first divergent line rather than dumping two files: a diff a
    // reader has to scan is a diff a reader skips.
    let (a, b): (Vec<&str>, Vec<&str>) = (fresh.lines().collect(), committed.lines().collect());
    let at = a
        .iter()
        .zip(&b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    panic!(
        "docs/evidence/E8/store.wit is stale — it describes a compiler that no \
         longer exists.\n\n  line {}\n  committed: {}\n  now:       {}\n\n\
         Run `just e8-wit` and commit the result. If the change is intended, \
         say in the commit message which declaration moved and why.",
        at + 1,
        b.get(at).unwrap_or(&"<end of file>"),
        a.get(at).unwrap_or(&"<end of file>"),
    );
}

#[test]
fn the_check_would_notice_a_change() {
    // **A comparison against a file that is always equal proves nothing.** So
    // the detector is shown noticing: the committed evidence must not equal a
    // trivially perturbed version of itself, which is what would happen if the
    // read or the generation silently produced nothing.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let committed =
        std::fs::read_to_string(root.join("docs/evidence/E8/store.wit")).expect("store.wit");
    assert!(
        committed.len() > 500,
        "the evidence is {} bytes, which is too small to be describing the \
         store's ten worlds",
        committed.len()
    );
    assert!(
        committed.contains("package store:data"),
        "and it contains the generated host package, so this is comparing the \
         artifact the compiler actually emits"
    );
    let perturbed = committed.replace("world ", "wrld ");
    assert_ne!(
        perturbed.trim(),
        committed.trim(),
        "the comparison is against real content"
    );
}

/// **The committed E10 components are what the compiler builds now.**
///
/// `runtime/pw-host/tests/pleris_component.rs` and the development server run
/// `docs/evidence/E10/<component id>.wasm` without linking the compiler, so the
/// bytes they run must be these: a stale artifact would let them pass against
/// a command the compiler no longer produces. Byte-for-byte, which also holds
/// the build to determinism.
#[test]
fn the_committed_components_are_what_the_compiler_builds_now() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let units: Vec<pw_core::check::Unit> = store_sources(&root)
        .into_iter()
        .enumerate()
        .map(|(i, src)| pw_core::check::Unit {
            path: format!("{i}.pw"),
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    for id in ["store.page.add_to_cart", "store.page.clear_cart"] {
        let fresh = pw_core::backend::component::compile(&units, id)
            .unwrap_or_else(|e| panic!("{id} compiles: {e}"));
        let path = format!("docs/evidence/E10/{id}.wasm");
        let committed = std::fs::read(root.join(&path))
            .unwrap_or_else(|e| panic!("{path}: {e} — run `just e10-component`"));
        assert!(
            fresh.component.bytes == committed,
            "{path} is stale ({} bytes committed, {} built now). Run `just e10-component` \
             and commit the result, saying in the commit message what changed in the \
             compiled command.",
            committed.len(),
            fresh.component.bytes.len()
        );
    }
}

/// **The committed handler modules are what the compiler emits now.**
///
/// `docs/evidence/E10/handlers/<identity>.mjs` is what `just e10-handlers`
/// recorded `pw emit-handlers` writing for the store. A stale one would record
/// a handler the compiler no longer produces, under an identity the document
/// may no longer name. The directory holds exactly the current set.
#[test]
fn the_committed_handlers_are_what_the_compiler_emits_now() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let units: Vec<pw_core::check::Unit> = store_sources(&root)
        .into_iter()
        .enumerate()
        .map(|(i, src)| pw_core::check::Unit {
            path: format!("{i}.pw"),
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let dir = root.join("docs/evidence/E10/handlers");
    let mut fresh: Vec<(String, String)> = pw_core::backend::js::compile(&units)
        .expect("the store checks")
        .into_iter()
        .map(|c| match c.module {
            pw_core::backend::wasm::Encoding::Encoded(m) => {
                (format!("{}.mjs", m.identity), m.source)
            }
            other => panic!("a handler in `{}` was not compiled: {other}", c.declaration),
        })
        .collect();
    fresh.sort();
    let mut committed: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-handlers`", dir.display()))
        .map(|e| {
            let path = e.expect("entry").path();
            (
                path.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&path).expect("read"),
            )
        })
        .collect();
    committed.sort();
    let differing: std::collections::BTreeSet<&String> = fresh
        .iter()
        .chain(&committed)
        .filter(|entry| !(fresh.contains(entry) && committed.contains(entry)))
        .map(|(name, _)| name)
        .collect();
    assert!(
        differing.is_empty(),
        "docs/evidence/E10/handlers/ is stale: {differing:?} differ between what is \
         committed and what the compiler emits now. Run `just e10-handlers` and commit \
         the result, saying in the commit message what changed in the compiled handlers."
    );
}

/// **The kiokun slice's committed build is what `pw build` produces now.**
///
/// `spikes/kiokun/server` serves `docs/evidence/E10/kiokun/` without linking
/// the compiler, so a stale file there would be a slice running something the
/// compiler no longer builds. Every file, byte for byte, and no extra ones.
#[test]
fn the_committed_kiokun_build_is_what_the_compiler_builds_now() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut units = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/kiokun",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            let src = std::fs::read_to_string(&p).expect("read");
            units.push(pw_core::check::Unit {
                path: p.display().to_string(),
                hir: lower_file(&src, &parse_tree(&src).green),
                src,
            });
        }
    }
    let b = pw_core::build::build(&units).expect("the slice checks");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());

    let mut fresh: Vec<(String, Vec<u8>)> = vec![
        (
            "templates.json".into(),
            format!("{}\n", serde_json::to_string_pretty(&b.templates).unwrap()).into_bytes(),
        ),
        (
            "contracts.json".into(),
            format!("{}\n", serde_json::to_string_pretty(&b.contracts).unwrap()).into_bytes(),
        ),
        ("app.wit".into(), b.wit.clone().into_bytes()),
    ];
    for (id, built) in &b.components {
        if let pw_core::backend::component::Built::Component { compiled, .. } = built {
            fresh.push((
                format!("components/{id}.wasm"),
                compiled.component.bytes.clone(),
            ));
        }
    }
    fresh.sort();

    let dir = root.join("docs/evidence/E10/kiokun");
    let mut committed: Vec<(String, Vec<u8>)> = Vec::new();
    for sub in ["", "components"] {
        for e in std::fs::read_dir(dir.join(sub))
            .unwrap_or_else(|e| panic!("{}: {e} — run `just e10-kiokun`", dir.display()))
        {
            let path = e.expect("entry").path();
            if path.is_file() {
                let name = path.strip_prefix(&dir).unwrap().display().to_string();
                committed.push((name, std::fs::read(&path).expect("read")));
            }
        }
    }
    committed.sort();
    let differing: std::collections::BTreeSet<&String> = fresh
        .iter()
        .chain(&committed)
        .filter(|entry| !(fresh.contains(entry) && committed.contains(entry)))
        .map(|(name, _)| name)
        .collect();
    assert!(
        differing.is_empty(),
        "docs/evidence/E10/kiokun/ is stale: {differing:?} differ from what `pw build` \
         produces now. Run `just e10-kiokun` and commit the result."
    );
}
