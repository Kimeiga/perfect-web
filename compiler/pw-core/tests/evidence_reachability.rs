//! **Evidence reachability: for every accepted fixture, can anything answer
//! the question it claims to settle?**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Don't write a gate like *every accepted fixture must produce a contract*,
//! > because a parser or type-system fixture may correctly produce no contract.
//! > Instead make the audit **claim → witness**. […] Then the failure condition
//! > is: fixture claims X but no observer capable of answering X exists →
//! > fixture is not evidence for X. I would call this an **evidence
//! > reachability audit**.
//!
//! The audit exists because two fixtures were found to be green without ever
//! having been in a position to answer their own question:
//!
//! ```text
//! A-014   reads as evidence that a page does not inherit its handler's
//!         authority — and declares a `view`, which produces no contract
//! A-013   claims build-time determinism — and its contract allows every
//!         world including the browser
//! ```
//!
//! Neither is a failing test. Both are *absent* tests wearing a passing one's
//! clothes.
//!
//! # What an observer is
//!
//! A designated artifact or conclusion that can distinguish the claim being
//! true from it being false. Per the ruling:
//!
//! ```text
//! syntax        → a canonical HIR node
//! effect        → the effective effect set
//! privacy       → a Label / flow conclusion
//! placement     → a placement conclusion
//! authority     → a ComponentContract
//! resume        → a manifest / capture decision
//! dependency    → a graph node with edges
//! exhaustiveness→ a MatchReport outcome
//! ```
//!
//! # Three verdicts, and the middle one is the interesting one
//!
//! ```text
//! Witnessed    an observer exists, was queried, and answered
//! Unqueryable  an observer exists INSIDE the compiler but cannot be asked
//!              from outside `check`, so an audit can only see the absence of
//!              a diagnostic — which is what an unexercised checker also
//!              produces
//! Missing      no observer capable of answering this claim exists at all
//! ```
//!
//! # This file freezes today's verdicts
//!
//! Same discipline as `unresolved_provenance.rs` and `policy_consumers.rs`. A
//! row that moves is a repair to classify; a row that does not is equally
//! informative.

use std::collections::BTreeSet;

use pw_core::contract::{ComponentContract, contracts};
use pw_core::effects::Inference;
use pw_core::graph::Graph;
use pw_core::hir::{DeclKind, Hir};
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

// --- the table ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Observer {
    Effect,
    /// The claim is that the fixture performs **nothing**. Silence is the
    /// answer — but only once a positive control proves the same code path
    /// speaks when there is something to say. `docs/RISK_QUEUE.md`'s
    /// admissibility rule, applied to an absence.
    ///
    /// Kept distinct from `Effect` because four §7.5A fixtures also produce an
    /// empty set and their claims are **positive**. Collapsing the two would
    /// read every one of them as proved.
    EffectAbsence,
    Privacy,
    Placement,
    Authority,
    Resume,
    Dependency,
    Exhaustiveness,
    /// Nothing in the compiler records this kind of fact yet.
    None_,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Witnessed,
    Unqueryable,
    Missing,
}

/// Every accepted fixture: what it claims, who could witness it, and what the
/// audit found today.
///
/// The claim column is the fixture's own `@category` and `@charter` header,
/// read as a question. Where a fixture claims several things, the row names
/// the one its `@category` leads with — the audit is about whether the fixture
/// can be cited as evidence for its stated subject.
const CLAIMS: &[(&str, Observer, Reach, &str)] = &[
    (
        "A-001",
        Observer::EffectAbsence,
        Reach::Witnessed,
        "a pure calculation performs nothing",
    ),
    (
        "A-002",
        Observer::Exhaustiveness,
        Reach::Unqueryable,
        "every order state is rendered",
    ),
    (
        "A-003",
        Observer::Authority,
        Reach::Witnessed,
        "a public query needs a database read and runs at the origin",
    ),
    (
        "A-004",
        Observer::Privacy,
        Reach::Witnessed,
        "a session-scoped query is not public",
    ),
    (
        "A-005",
        Observer::Authority,
        Reach::Witnessed,
        "an idempotent command's authority is the write it performs",
    ),
    (
        "A-006",
        Observer::Effect,
        Reach::Witnessed,
        "a subscription performs a network subscribe",
    ),
    (
        "A-007",
        Observer::Effect,
        Reach::Witnessed,
        "a resource acquires and releases",
    ),
    (
        "A-008",
        Observer::Authority,
        Reach::Witnessed,
        "a streamed public query fetches and may run anywhere but build",
    ),
    (
        "A-009",
        Observer::Dependency,
        Reach::Witnessed,
        "an edge fragment's dependencies and invalidations are explicit",
    ),
    (
        "A-010",
        Observer::Placement,
        Reach::Witnessed,
        "a secret operation is confined to the origin",
    ),
    (
        "A-011",
        Observer::None_,
        Reach::Missing,
        "an offline draft's conflict policy is explicit rather than \
         last-write-wins",
    ),
    (
        "A-012",
        Observer::Exhaustiveness,
        Reach::Unqueryable,
        "a decoder handles malformed input",
    ),
    (
        "A-013",
        Observer::Placement,
        Reach::Missing,
        "a page whose inputs are build-known is produced at build time",
    ),
    (
        "A-014",
        Observer::Resume,
        Reach::Witnessed,
        "a resumable handler's identity is the hash of its code and captures",
    ),
    (
        "A-015",
        Observer::Effect,
        Reach::Missing,
        "a measure is scheduled before a mutate",
    ),
    (
        "A-016",
        Observer::Effect,
        Reach::Witnessed,
        "several measures share one layout snapshot",
    ),
    (
        "A-017",
        Observer::Effect,
        Reach::Witnessed,
        "layout-affecting mutations are batched",
    ),
    (
        "A-018",
        Observer::Effect,
        Reach::Missing,
        "resize observation updates derived state",
    ),
    (
        "A-019",
        Observer::Effect,
        Reach::Witnessed,
        "intersection observation bounds a resource's lifetime",
    ),
    (
        "A-020",
        Observer::Effect,
        Reach::Missing,
        "a compositor animation touches only transform and opacity",
    ),
    (
        "A-021",
        Observer::Effect,
        Reach::Missing,
        "a painter is a pure function of its declared inputs",
    ),
    (
        "A-022",
        Observer::Effect,
        Reach::Witnessed,
        "post-paint work performs no layout",
    ),
    (
        "A-023",
        Observer::Effect,
        Reach::Missing,
        "a contained subtree is semantically independent",
    ),
    (
        "A-024",
        Observer::Effect,
        Reach::Witnessed,
        "an audited imperative escape declares what it does",
    ),
];

// --- the program -------------------------------------------------------------

struct Program {
    hirs: Vec<Hir>,
    src: String,
}

impl Program {
    /// One accepted fixture, checked against the shared library, as `ci` does.
    fn for_fixture(id: &str) -> Program {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        for dir in [
            "packages/pw-std",
            "packages/pw-platform-web",
            "examples/lib",
        ] {
            let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
                .unwrap_or_else(|e| panic!("{dir}: {e}"))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect();
            ps.sort();
            for p in ps {
                files.push(std::fs::read_to_string(&p).expect("read"));
            }
        }
        files.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"));

        let fixture = std::fs::read_dir(root.join("examples/accepted"))
            .expect("accepted")
            .map(|e| e.expect("entry").path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(id))
            .unwrap_or_else(|| panic!("no fixture `{id}`"));
        let src = std::fs::read_to_string(&fixture).expect("read fixture");
        files.push(src.clone());

        Program {
            hirs: files
                .iter()
                .map(|s| lower_file(s, &parse_tree(s).green))
                .collect(),
            src,
        }
    }

    fn refs(&self) -> Vec<&Hir> {
        self.hirs.iter().collect()
    }
    /// The fixture is pushed last.
    fn unit(&self) -> usize {
        self.hirs.len() - 1
    }
    fn hir(&self) -> &Hir {
        &self.hirs[self.unit()]
    }
    fn module(&self) -> String {
        self.hir()
            .modules
            .iter()
            .map(|(_, m, _)| m.name.clone())
            .next()
            .unwrap_or_default()
    }
}

/// What each observer answers for one fixture. `None` means the observer had
/// nothing to say — which is the audit's subject, not its failure.
fn interrogate(p: &Program) -> BTreeSet<Observer> {
    let refs = p.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs: Vec<ComponentContract> = contracts(&refs, &sigs, &ws);
    let mut inf = Inference::new(&sigs, &ws);
    inf.run(&refs);
    let module = p.module();
    let hir = p.hir();

    let mut answered = BTreeSet::new();

    // Effect — any declaration of the fixture performs something.
    if hir.all_decls().any(|(id, d)| {
        !matches!(d.kind, DeclKind::Import) && !inf.effective_effects(p.unit(), hir, id).is_empty()
    }) {
        answered.insert(Observer::Effect);
    }

    let mine: Vec<&ComponentContract> = cs
        .iter()
        .filter(|c| c.component_id.starts_with(&format!("{module}.")))
        .collect();

    // Authority — a contract exists AND requires something. A contract with an
    // empty capability set is an answer only if the fixture's claim is about
    // absence, which no `@category` in the accepted corpus makes its subject.
    if mine.iter().any(|c| !c.required_capabilities.is_empty()) {
        answered.insert(Observer::Authority);
    }

    // Placement — the contract narrows to fewer than every world. All four is
    // what a component with nothing to say gets, so it is not an answer.
    if mine.iter().any(|c| c.allowed_placements.len() < 4) {
        answered.insert(Observer::Placement);
    }

    // Privacy — some declaration carries a non-public label.
    if hir.all_decls().any(|(_, d)| {
        d.visibility.as_deref() == Some("session") || d.visibility.as_deref() == Some("private")
    }) {
        answered.insert(Observer::Privacy);
    }

    // Dependency — a graph node with at least one edge.
    let g = Graph::build(&refs, &ws);
    if g.edges.iter().any(|e| e.from.starts_with(&module)) {
        answered.insert(Observer::Dependency);
    }

    // Resume — a manifest was generated.
    if !pw_core::resume_artifacts::generate(&p.src, hir, &sigs, "dev").is_empty() {
        answered.insert(Observer::Resume);
    }

    answered
}

// --- the audit ---------------------------------------------------------------

/// **Every accepted fixture, against its designated observer.**
#[test]
fn every_accepted_fixture_has_the_reachability_the_table_records() {
    // The positive control for every `EffectAbsence` row below. A-003's query
    // reads the database, so the effect observer demonstrably speaks on this
    // exact code path — without which "A-001 performs nothing" and "the
    // analysis never ran" are the same observation.
    assert!(
        interrogate(&Program::for_fixture("A-003")).contains(&Observer::Effect),
        "the effect observer answered nothing for a fixture that reads the \
         database, so it cannot witness an absence either"
    );

    let mut wrong = Vec::new();
    for (id, observer, expected, claim) in CLAIMS {
        let p = Program::for_fixture(id);
        let answered = interrogate(&p);
        let got = match observer {
            // Silence, with the control above standing behind it.
            Observer::EffectAbsence if !answered.contains(&Observer::Effect) => Reach::Witnessed,
            Observer::EffectAbsence => Reach::Missing,
            // Nothing records this kind of fact, so there is nothing to query.
            Observer::None_ => Reach::Missing,
            // The exhaustiveness algorithm runs inside `check` and returns
            // early on `Proven`. `exhaust::check_match` is public but the HIR→
            // pattern lowering it needs is not, so an audit cannot ask whether
            // a match was proved — only whether a diagnostic appeared, which is
            // also what a checker that never ran produces.
            Observer::Exhaustiveness => Reach::Unqueryable,
            o if answered.contains(o) => Reach::Witnessed,
            _ => Reach::Missing,
        };
        if got != *expected {
            wrong.push(format!(
                "{id}: table says {expected:?}, audit found {got:?} \
                 (observer {observer:?}, answered {answered:?}) — claim: {claim}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the evidence-reachability table is stale. Each line is a fixture whose \
         ability to witness its own claim changed; classify the movement before \
         updating the table:\n  {}",
        wrong.join("\n  ")
    );
}

/// **The negative control the ruling asked for.**
///
/// > A particularly useful control is exactly the A-014 shape: a fixture whose
/// > syntax is accepted but whose intended contract subject cannot be found.
/// > The audit must go red.
///
/// Without this, `every_accepted_fixture_has_the_reachability_the_table_records`
/// would pass for an audit whose `interrogate` returned every observer for
/// everything — the table is a list of expectations, and expectations that are
/// always met measure nothing.
#[test]
fn the_audit_detects_a_fixture_that_cannot_witness_its_own_claim() {
    // Accepted syntax. Claims authority. Declares a `view`, so `contracts`
    // produces nothing for it — exactly A-014's shape, in miniature.
    let src = "\
module probe.silent

view Button(label: String) !{} {
    <button>{label}</button>
}
";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain")];
    files.push(src.to_string());
    let hirs: Vec<Hir> = files
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let p = Program {
        hirs,
        src: src.to_string(),
    };

    // It parses and lowers — the syntax is genuinely accepted.
    assert!(
        p.hir().all_decls().any(|(_, d)| d.name == "Button"),
        "the control must be a program the compiler accepts, or it is testing \
         the parser instead of the audit"
    );

    // And no observer answers an authority claim about it.
    let answered = interrogate(&p);
    assert!(
        !answered.contains(&Observer::Authority),
        "the audit failed to detect an unwitnessable authority claim: {answered:?}"
    );

    // The positive half of the control, so `interrogate` is not simply always
    // silent: the same declaration written as a `component` IS witnessed.
    let with_contract = "\
module probe.loud

fn read(id: String) -> String !{ database.read<Stores> } { todo }

component Button(label: String) {
    view {
        <button>{read(label)}</button>
    }
}
";
    let mut files = vec![std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain")];
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for path in ps {
            files.push(std::fs::read_to_string(&path).expect("read"));
        }
    }
    files.push(with_contract.to_string());
    let hirs: Vec<Hir> = files
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let p = Program {
        hirs,
        src: with_contract.to_string(),
    };
    let answered = interrogate(&p);
    assert!(
        answered.contains(&Observer::Authority),
        "the control's positive half did not answer, so its negative half \
         proves nothing: {answered:?}"
    );
}

// --- the defect the audit found ---------------------------------------------

/// **The placement observer answers a different question than the one the
/// fixtures claim.**
///
/// `contract.rs` builds its `Demand` with `label: Label::public()` and
/// `declared: None`, four lines below a comment saying:
///
/// > Not re-derived from the capability list: the solver also weighs privacy
/// > labels, and a second derivation here would agree until a label mattered.
///
/// `check.rs` builds the same `Demand` with the real label and the author's
/// pinned world. So `pw check` refuses a bad placement correctly, while the
/// **artifact a host grants placement from** ignores both inputs.
///
/// The consequence, frozen below: A-013 declares `placement build` and its
/// contract permits the browser, the edge and the origin. A-015, A-020 and
/// A-023 declare `placement browser` and their contracts permit build time.
///
/// This is the `secret<Payments>` row from `docs/RISK_QUEUE.md` again — one
/// fact derived twice, agreeing until an input mattered — and it is why the
/// four `Missing` §7.5A rows above are `Missing`: their subject is *where and
/// when* work runs, and the artifact that would say so is not being told.
#[test]
fn the_contract_ignores_the_placement_its_author_pinned() {
    let cases = [
        ("A-013", "build", vec!["build", "browser", "edge", "origin"]),
        (
            "A-015",
            "browser",
            vec!["build", "browser", "edge", "origin"],
        ),
        (
            "A-020",
            "browser",
            vec!["build", "browser", "edge", "origin"],
        ),
        (
            "A-023",
            "browser",
            vec!["build", "browser", "edge", "origin"],
        ),
    ];
    for (id, pinned, expected) in cases {
        let p = Program::for_fixture(id);
        assert!(
            p.src.contains(&format!("placement {pinned}")),
            "{id} no longer pins `placement {pinned}`"
        );
        let refs = p.refs();
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        let cs = contracts(&refs, &sigs, &ws);
        let module = p.module();
        let c = cs
            .iter()
            .find(|c| c.component_id.starts_with(&format!("{module}.")))
            .unwrap_or_else(|| panic!("no contract for {id}"));
        assert_eq!(
            c.allowed_placements, expected,
            "TODAY, and it is WRONG: {id} pins `placement {pinned}` and its \
             contract permits {:?}. When the demand is repaired this row moves \
             to `[{pinned}]` and the movement is the finding.",
            c.allowed_placements
        );
    }
}
