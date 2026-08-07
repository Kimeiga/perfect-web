//! The platform library is compiler **input**, and gets checked like it.
//!
//! Architect ruling, 2026-08-06, after `Stores.find` was found returning
//! `Result<Store, StoreError>` — which is `get`'s type:
//!
//! > E2C declarations cannot be treated as unquestioned truth merely because
//! > they replaced hard-coded tables. The architecture is still right — one
//! > signature should feed types, effects, privacy, and placement — but the
//! > signatures themselves need validation.
//!
//! The failure that prompted it is worth stating precisely, because it is
//! subtle. R-008 annotated `let store: Option<Store> = Stores.find(id)`. The
//! annotation was right about the intent and wrong about the library, and the
//! rule read the annotation — so **the fixture's own text was establishing the
//! library's return type**. The fixture passed for a reason that had nothing
//! to do with the library being correct.
//!
//! What is checked here:
//!
//! - the platform parses, resolves and produces no diagnostics;
//! - every signature is reachable and complete;
//! - a declaration that promises a label or an effect actually carries it;
//! - the accepted corpus exercises the signatures the checkers depend on.
//!
//! What is NOT checked, and cannot be: whether a declaration matches an
//! implementation that does not exist. `browser.pw`'s `offsetWidth` has no
//! body — it describes something the platform does. Those are a **trusted
//! contract**, and the last test in this file hashes them so a change to one
//! is a visible event rather than a diff nobody reviewed.

use std::collections::BTreeSet;

use pw_core::check::check_sources;

fn platform() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push((
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).expect("read"),
                ));
            }
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.sort();
    out
}

fn with_lib() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = platform();
    for e in std::fs::read_dir(root.join("examples/lib")).expect("examples/lib") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.sort();
    out
}

#[test]
fn the_platform_library_parses_resolves_and_checks_clean() {
    let mut problems = Vec::new();
    for (name, diags) in check_sources(&with_lib()) {
        for d in &diags {
            problems.push(format!("{name}: [{}] {}", d.code, d.message));
        }
    }
    assert!(
        problems.is_empty(),
        "the platform library does not check clean, so every rule that reads it \
         is reading something the compiler itself rejects:\n  {}",
        problems.join("\n  ")
    );
}

/// A declaration that promises something must carry it.
///
/// Three internal consistency rules, each of which `Stores.find` would have
/// been caught by if it had promised anything — it did not, which is why this
/// test also has a fourth check that is about the corpus rather than the file.
#[test]
fn every_platform_signature_is_internally_consistent() {
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;
    use pw_syntax::parse_tree;

    let sources = with_lib();
    let hirs: Vec<_> = sources
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let sigs = Signatures::build(&Workspace::build(&refs), &refs);

    let mut problems = Vec::new();
    for (path, sig) in sigs.iter() {
        // A row naming a type argument must name a type: `secret<Payments>`
        // and `resource.acquire<MapHandle>` are checkable claims, and a
        // misspelt one silently stops matching anything.
        for e in &sig.effects {
            let Some((_, rest)) = e.split_once('<') else {
                continue;
            };
            let Some(arg) = rest.strip_suffix('>') else {
                problems.push(format!("{path}: effect `{e}` has an unclosed argument"));
                continue;
            };
            if arg.is_empty() {
                problems.push(format!("{path}: effect `{e}` has an empty argument"));
            }
        }
        // A return type head with no arguments where one is required.
        if matches!(sig.returns.as_deref(), Some("Option" | "Result" | "List"))
            && sig.returns_args.is_empty()
        {
            problems.push(format!(
                "{path}: returns `{}` with no type argument — a bare `Option` \
                 does not say what may be absent",
                sig.returns.as_deref().unwrap_or("?")
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n  "));
}

/// The signatures the checkers depend on are exercised by a program.
///
/// A declaration nothing uses is a declaration nothing has checked. These are
/// the ones a rule reads directly, so a change to one silently changes what a
/// rule does.
#[test]
fn the_signatures_the_checkers_depend_on_are_exercised() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut used = String::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("examples") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "pw") {
                used.push_str(&std::fs::read_to_string(&p).expect("read"));
            }
        }
    }

    // `(declaration, what a program that exercises it contains, which rule
    // reads it)`.
    //
    // The middle column is not always the declaration's name, and that is the
    // point of having it. `set_transform` is how `layout.rs` learns that
    // `transform` is compositable, but a program exercising it writes
    // `transform:` in an animation and never names the setter. Checking for
    // the declaration's own name reported it as unexercised — a false alarm
    // that would have been "fixed" by deleting the row.
    const LOAD_BEARING: &[(&str, &str, &str)] = &[
        (
            "set_padding",
            "set_padding",
            "layout.rs — layout-affecting write",
        ),
        (
            "set_transform",
            "transform:",
            "layout.rs — the compositable properties",
        ),
        (
            "set_opacity",
            "opacity:",
            "layout.rs — the compositable properties",
        ),
        ("raw_html", "raw_html", "check.rs — the unsafe-audit rule"),
        (
            "now_wall",
            "now_wall",
            "effects.rs — the reuse/determinism rule",
        ),
        (
            "current_location",
            "current_location",
            "check.rs — declared placement",
        ),
        (
            "set_text",
            "set_text",
            "effects.rs — a painter reaching the document",
        ),
        (
            "events.submit",
            "on:submit",
            "annotations.rs — the event a handler takes",
        ),
        (
            "begin",
            "Database.begin",
            "affine.rs — resource acquisition",
        ),
        ("rollback", "rollback", "affine.rs — resource release"),
        (
            "payments",
            "secrets.payments",
            "labels.rs — a secret's origin",
        ),
        ("public", "log.public", "check.rs — the public sink"),
        (
            "find",
            "Stores.find",
            "annotations.rs — an Option-returning initialiser",
        ),
        (
            "style",
            "self.style",
            "infer.rs — the ElementRef -> Style member chain",
        ),
    ];
    let mut unexercised = Vec::new();
    for (name, token, why) in LOAD_BEARING {
        if !used.contains(token) {
            unexercised.push(format!("{name} (looked for {token:?}) — {why}"));
        }
    }
    assert!(
        unexercised.is_empty(),
        "these platform declarations are read by a rule and used by no program, \
         so nothing would notice if they changed:\n  {}",
        unexercised.join("\n  ")
    );
}

/// The trusted contract, content-hashed.
///
/// `offsetWidth`, `now_wall`, `raw_html` and the rest have no implementation
/// here — they describe what the platform does, and nothing in this repository
/// can verify that description. They are therefore **trusted**, and the only
/// honest handling of a trusted input is to make a change to it visible.
///
/// When this test fails, the platform contract changed. That is allowed. What
/// is not allowed is it changing without anyone deciding to.
#[test]
fn the_trusted_platform_contract_is_hashed() {
    // FNV-1a. Not cryptographic — this detects an unreviewed edit, not an
    // adversary — and it is stable across machines and Rust versions, which a
    // DefaultHasher is not.
    fn fnv(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut hash: u64 = 0;
    for (name, src) in platform() {
        // Comments and blank lines are documentation, not contract.
        let contract: String = src
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        hash ^= fnv(format!("{name}\n{contract}").as_bytes());
        names.insert(name);
    }

    // Changed 2026-08-06: `examples/domain.pw` gained `MenuItem { id, name }`.
    // `Menu` returned `List<MenuItemId>` while the store page rendered
    // `item.name` — describing a list of ids and reading a list of items. That
    // was invisible while nothing depended on the element's type and stopped
    // being invisible when a resumable handler inside the loop began hashing
    // it into a capture schema.
    //
    // Changed 2026-08-07: `packages/pw-platform-web/style.pw` gained
    // `type LayoutAffect`. `style.mutate<LayoutAffect>` appears in five corpus
    // files — R-034, R-035, A-020 and two generality witnesses — and
    // `LayoutAffect` was declared NOWHERE. The distinction the module's own
    // comment calls "the point" rested on a spelling no checker could resolve.
    // Found by `PW5200` on its first run against the real corpus.
    //
    // Changed 2026-08-07: `packages/pw-std/effects.pw` and
    // `packages/pw-platform-web/effects.pw` are new — the effect ontology's
    // slice 3, declaring all 25 effects the corpus writes. This is the largest
    // single change to the trusted contract so far and it is deliberately a
    // change to the contract rather than to any checker: `interface_for` still
    // formats `pw:host/{family}`, and the `host` clauses here are what will
    // replace it. Nothing reads them yet, so this commit changes what the
    // platform SAYS and not yet what the compiler does.
    //
    // Two declared arities disagree with the corpus and are recorded rather
    // than reconciled — `tests/effect_vocabulary.rs`'s `ARITY_UNSETTLED`.
    //
    // Changed again 2026-08-07: every effect that has one gained a `placement`
    // clause. Architect ruling — an effect with `capability none` still needs
    // somewhere it is meaningful, and for browser-semantic effects the
    // declaration is the only thing that can say so. This is the fact
    // `World::worlds_for` holds as a hard-coded table, and moving it here is
    // what eventually lets that table be deleted rather than relocated.
    // Changed again 2026-08-07: `style.pw` gained `type PaintOnly` and its two
    // bare `style.mutate` rows became `style.mutate<PaintOnly>`. Architect
    // ruling — `effect style.mutate<T>` binds a parameter and a use must
    // supply it, so the bare form said "does not invalidate layout" by
    // omission. There was no way to say it positively; now there is.
    const EXPECTED: u64 = 0x563f_1bb0_e013_54d9;
    eprintln!(
        "  platform contract: {} files, hash {hash:#018x}",
        names.len()
    );
    {
        assert_eq!(
            hash, EXPECTED,
            "the trusted platform contract changed. If that was intended, update \
             EXPECTED and say in the commit message which declaration changed and \
             why — every checker that reads it is affected."
        );
    }
}
