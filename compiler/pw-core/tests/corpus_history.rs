//! Historical-corpus compatibility — the guard against editing a fixture into
//! compliance.
//!
//! Architect ruling, 2026-08-06:
//!
//! > Keep two distinct measurements. Current-corpus conformance tells you
//! > whether today's specification passes. Historical-corpus compatibility
//! > prevents a fixture from being edited into compliance and silently erasing
//! > a previously discovered failure.
//!
//! Ten fixtures changed on the way to 44/44. Nine gained an import or a
//! declaration they called and the program lacked — a legitimate repair, and
//! also exactly the shape of change that could remove the **defect** instead of
//! the **obstruction** without anyone noticing, because the fixture would go on
//! being reported either way.
//!
//! So the old text is kept, and asked the only question that distinguishes the
//! two: does it still fail?

use pw_core::check::check_sources;

/// Old texts that are expected to compile clean now, each **classified**.
///
/// Architect ruling, 2026-08-06:
///
/// > Do not force it to 10/10 merely for neatness. Classify the remaining
/// > case, and report that classification next to the result. The history
/// > suite is valuable precisely because it can disagree with the current
/// > corpus.
///
/// Four classifications are possible, and they mean very different things:
///
/// - `FixtureDidNotExpressIt` — the old text did not actually contain the
///   violation it declared. Benign; the corpus got more precise.
/// - `SpecificationChanged` — the language or the charter changed, so the old
///   text is no longer wrong. Needs a charter reference.
/// - `CompilerRegressed` — it used to be caught and is not. A bug, and this
///   test is how it surfaces.
/// - `KnownCheckerGap` — the current checker cannot see it. Belongs in
///   `examples/generality/` as a `slips-through.pw` too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    FixtureDidNotExpressIt,
    #[allow(dead_code)]
    SpecificationChanged,
    #[allow(dead_code)]
    CompilerRegressed,
    #[allow(dead_code)]
    KnownCheckerGap,
}

const EXPECTED_TO_PASS: &[(&str, Why, &str)] = &[(
    "R-023",
    Why::FixtureDidNotExpressIt,
    "its old text declared no route at all, so there was no route table for a \
     link to be dead relative to. `dead_internal_link` is a RELATION between a \
     link and a route table, and the old file contained only one half of it — \
     so the old text did not express the invariant it declared, and its \
     silence is correct rather than a gap.",
)];

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "examples/lib",
        "packages/pw-std",
        "packages/pw-platform-web",
    ] {
        for e in std::fs::read_dir(root.join(dir)).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_none_or(|x| x != "pw") {
                continue;
            }
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    // The accepted modules a fixture may import, as the current harness does.
    for e in std::fs::read_dir(root.join("examples/accepted")).expect("accepted") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
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
    out.sort();
    out
}

fn c0() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/history/C0");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&root)
        .expect("examples/history/C0")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_pre_change_text_of_every_repaired_fixture_still_fails() {
    let all = c0();
    assert_eq!(
        all.len(),
        10,
        "docs/CORPUS.md records ten changed fixtures; C0 holds {}",
        all.len()
    );

    let mut wrong = Vec::new();
    for (name, src) in &all {
        let id = &name[..5];
        let mut files = library();
        files.push((name.clone(), src.clone()));
        let diags = check_sources(&files)
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d)
            .unwrap_or_default();

        // Its DECLARED invariant, not merely "some error". Asking only whether
        // the old text is red would be satisfied by a syntax error, and an
        // earlier version of this test was: the first C0 baseline predated the
        // corpus being well-formed, so R-025's old text failed to PARSE and the
        // test called that historical compatibility.
        let declared = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/rejected")
                .join(name),
        )
        .expect("the current fixture")
        .lines()
        .find_map(|l| l.trim().strip_prefix("// @invariant:"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| panic!("{name} has no @invariant"));

        let caught = diags.iter().any(|d| d.symbol() == declared);
        let excused = EXPECTED_TO_PASS.iter().find(|(f, _, _)| *f == id);

        match (caught, excused) {
            (false, None) => wrong.push(format!(
                "{id}: its C0 text is no longer caught for `{declared}` (it reports \
                 {:?}). The repair recorded in docs/CORPUS.md removed the DEFECT, \
                 not the obstruction — a previously discovered failure has been \
                 erased.",
                diags.iter().map(|d| d.symbol()).collect::<Vec<_>>()
            )),
            (true, Some((_, class, why))) => wrong.push(format!(
                "{id}: listed as {class:?} but its C0 text is caught for \
                 `{declared}` again. Good news; remove the entry. The recorded \
                 reason was: {why}"
            )),
            // A regression or a known gap is NOT an excuse — it is a defect
            // being tracked in the wrong place. Only the two benign
            // classifications may sit in this list quietly.
            (false, Some((_, class, why)))
                if matches!(class, Why::CompilerRegressed | Why::KnownCheckerGap) =>
            {
                wrong.push(format!(
                    "{id}: classified {class:?}, which is a defect rather than an \
                     explanation. A regression must be fixed; a checker gap must \
                     also exist as a slips-through.pw witness. Reason on file: {why}"
                ))
            }
            _ => {}
        }
    }
    eprintln!(
        "  historical compatibility: {}/{} C0 texts still caught",
        all.len() - EXPECTED_TO_PASS.len(),
        all.len()
    );
    for (id, class, _) in EXPECTED_TO_PASS {
        eprintln!("    {id}: {class:?}");
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// The history is a record, not a second corpus.
///
/// If a C0 file drifts — reformatted, "fixed", quietly updated — it stops
/// answering the question it exists to answer, and the drift would be
/// invisible because nothing else reads these files.
#[test]
fn the_history_is_not_edited_to_match_the_present() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for (name, old) in c0() {
        let current = root.join("rejected").join(&name);
        let current = std::fs::read_to_string(&current).expect("the current fixture");
        assert_ne!(
            old.trim(),
            current.trim(),
            "{name}: the C0 text and the current text are identical, so the \
             history proves nothing. Either the fixture never changed and this \
             file should not exist, or the history has been overwritten."
        );
    }
}
