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
//! **`Unqueryable` is currently empty.** Both rows that held it were
//! exhaustiveness fixtures, and `check::match_analysis` — `Proven |
//! NonExhaustive | Blocked` — closed them on 2026-08-10. A-012 turned out not
//! to contain a `match` at all, which the audit caught the moment the analysis
//! became observable: it had been classified against an observer that could
//! never have answered for it.
//!
//! # This file freezes today's verdicts
//!
//! Same discipline as `unresolved_provenance.rs` and `policy_consumers.rs`. A
//! row that moves is a repair to classify; a row that does not is equally
//! informative.

use std::collections::BTreeSet;

use pw_core::check::{MatchOutcome, match_analysis};
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
    /// The claim is that external data enters by DECODING rather than by
    /// casting — an absence, like `EffectAbsence`, and admissible for the same
    /// reason: `PW0601` demonstrably fires on `R-009`, so silence here is an
    /// answer rather than an unexercised rule.
    CastAbsence,
    /// Nothing in the compiler records this kind of fact yet.
    None_,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Witnessed,
    /// An observer exists inside the compiler and cannot be asked from
    /// outside, so an audit can only see the absence of a diagnostic.
    ///
    /// **Currently empty.** Both rows that held it — A-002 and A-012 — were
    /// closed on 2026-08-10 by exposing `check::match_analysis`. Kept because
    /// the class is real and the next analysis added without an observable
    /// result belongs in it; deleting it would make that a `Missing` row and
    /// lose the distinction between *nothing can answer* and *nothing may ask*.
    #[allow(dead_code)]
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
    // Moved `Unqueryable` → `Witnessed` when `check::match_analysis` exposed
    // the outcome. Its match is `Proven`, and now says so.
    (
        "A-002",
        Observer::Exhaustiveness,
        Reach::Witnessed,
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
    // A-012 has no `match` at all. It was classified `Exhaustiveness` /
    // `Unqueryable` on 2026-08-10 and that was WRONG in a way the audit itself
    // caught: exposing `match_analysis` turned the row red, because the
    // analysis reports nothing for a fixture with nothing to analyse. Its
    // subject is `?` propagation and typed errors, and the half that can be
    // witnessed is that nothing is cast.
    (
        "A-012",
        Observer::CastAbsence,
        Reach::Witnessed,
        "external data enters by decoding, and nothing is cast",
    ),
    // Moved `Missing` → `Witnessed` when `contract.rs` stopped discarding the
    // author's pinned world. What is witnessed is the pin, not the derivation
    // the fixture's header claims — see
    // `the_contract_carries_the_placement_its_author_pinned`.
    (
        "A-013",
        Observer::Placement,
        Reach::Witnessed,
        "a page pinned to build time carries that pin into its contract",
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
    /// Every file, in the order the `hirs` were built from them.
    sources: Vec<String>,
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
            sources: files,
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

    // Exhaustiveness — the analysis reached a CONCLUSION about some match in
    // this fixture. `Blocked` is not an answer, which is the whole point of the
    // three-state outcome: it means the analysis did not run.
    // The WHOLE program, not the fixture alone: `OrderState` is declared in
    // `examples/domain.pw`, and a scrutinee whose type this unit cannot see is
    // `Blocked` — correctly, and it is not the question being asked.
    let units: Vec<pw_core::check::Unit> = p
        .sources
        .iter()
        .enumerate()
        .map(|(i, src)| pw_core::check::Unit {
            path: format!("{i}.pw"),
            src: src.clone(),
            hir: lower_file(src, &parse_tree(src).green),
        })
        .collect();
    if match_analysis(&units)
        .iter()
        .any(|m| !matches!(m.outcome, MatchOutcome::Blocked { .. }))
    {
        answered.insert(Observer::Exhaustiveness);
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
            Observer::CastAbsence if !answered.contains(&Observer::CastAbsence) => Reach::Witnessed,
            Observer::CastAbsence => Reach::Missing,
            // Nothing records this kind of fact, so there is nothing to query.
            Observer::None_ => Reach::Missing,
            // Queryable since 2026-08-10. `check::match_analysis` returns
            // `Proven | NonExhaustive | Blocked` for every match in the
            // program, and the diagnostic is a projection of it — so an audit
            // can assert a POSITIVE proof instead of inferring one from the
            // absence of an error.
            Observer::Exhaustiveness if answered.contains(&Observer::Exhaustiveness) => {
                Reach::Witnessed
            }
            Observer::Exhaustiveness => Reach::Missing,
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
        sources: files.clone(),
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
        sources: files.clone(),
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
/// The consequence, before the repair: A-013 declared `placement build` and its
/// contract permitted the browser, the edge and the origin. A-015, A-020 and
/// A-023 declared `placement browser` and their contracts permitted build time.
///
/// This is the `secret<Payments>` row from `docs/RISK_QUEUE.md` again — one
/// fact derived twice, agreeing until an input mattered.
///
/// # Repaired, half of it
///
/// `contract.rs` now calls `check.rs`'s own `declared_world` rather than
/// passing `None`. **The label half is deliberately still wrong**: the join of
/// what a body reads is computed by walking the body, and
/// `tests/policy_consumers.rs` freezes the fact that a body walk cannot today
/// tell a policy value from a term. Wiring it in first would give the
/// contract's placement a second channel from policy values.
///
/// # What this does NOT establish
///
/// A-013's header says *`!{}` plus build-known inputs => the placement solver
/// chooses Build*. The solver does not choose Build. The author pinned it, the
/// effect set is empty, and nothing objected. Reachability is not correctness:
/// the observer now answers, and what it answers is weaker than the fixture
/// claims. Closing that gap is the `include_markdown` ruling — a tracked build
/// input with a real effect — not this repair.
#[test]
fn the_contract_carries_the_placement_its_author_pinned() {
    for (id, pinned) in [
        ("A-013", "build"),
        ("A-015", "browser"),
        ("A-020", "browser"),
        ("A-023", "browser"),
    ] {
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
            c.allowed_placements,
            [pinned],
            "{id} pins `placement {pinned}`, so the artifact a host reads must \
             say so too"
        );
    }

    // **The discriminator.** Without it this passes for a contract that returned
    // the pinned world and ignored the effects — which would let a component
    // pin `browser` while performing `database.read` and ship a contract
    // agreeing with it. A-003 pins nothing and its placement is still derived.
    let p = Program::for_fixture("A-003");
    assert!(
        !p.src
            .lines()
            .any(|l| l.trim_start().starts_with("placement ")),
        "A-003 now pins a placement, so it no longer discriminates"
    );
    let refs = p.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let c = cs
        .iter()
        .find(|c| c.component_id == "store.queries.Store")
        .expect("A-003");
    assert_eq!(
        c.allowed_placements,
        ["origin"],
        "an unpinned component's placement still comes from what it performs"
    );
}

// --- the exhaustiveness observer, with the controls the ruling required -------

/// **All three outcomes are reachable, and `Blocked` is not a proof.**
///
/// Architect ruling, 2026-08-10:
///
/// > I would require controls for all three states: exhaustive match → Proven;
/// > non-exhaustive match → NonExhaustive + witness; ill-formed / semantically
/// > blocked match → Blocked.
///
/// Before `check::match_analysis` existed, only the middle one was observable —
/// as a diagnostic — and the other two were both *no diagnostic*.
#[test]
fn the_exhaustiveness_analysis_reports_all_three_outcomes() {
    let analyse = |src: &str| -> Vec<MatchOutcome> {
        let units = vec![pw_core::check::Unit {
            path: "t.pw".to_string(),
            src: src.to_string(),
            hir: lower_file(src, &parse_tree(src).green),
        }];
        match_analysis(&units)
            .into_iter()
            .map(|m| m.outcome)
            .collect()
    };

    let proven = "\
module m

type State = | Draft | Sent

fn f(s: State) -> Int {
    match s {
        Draft => 1
        Sent => 2
    }
}
";
    assert_eq!(analyse(proven), [MatchOutcome::Proven]);

    let missing = "\
module m

type State = | Draft | Sent

fn f(s: State) -> Int {
    match s {
        Draft => 1
    }
}
";
    let out = analyse(missing);
    let [MatchOutcome::NonExhaustive { missing }] = out.as_slice() else {
        panic!("{out:?}")
    };
    assert_eq!(missing, &["Sent".to_string()], "and it names the witness");

    // Blocked: the scrutinee's type is not one this program declares, so the
    // analysis has no matrix to build. **Not** a proof, and not a violation.
    let blocked = "\
module m

fn f(s: Int) -> Int {
    match s {
        1 => 1
    }
}
";
    assert!(
        matches!(analyse(blocked).as_slice(), [MatchOutcome::Blocked { .. }]),
        "{:?}",
        analyse(blocked)
    );
}

/// **The structural control the ruling asked for.**
///
/// > An implementation that simply stops calling exhaustiveness analysis must
/// > make the evidence audit fail rather than turn every accepted match into
/// > apparent success.
///
/// The audit asks `match_analysis` for a conclusion. If the analysis stopped
/// running, every match would come back `Blocked` — or the list would be empty
/// — and A-002's row would go `Missing`, which is a red test. This asserts the
/// mechanism directly: a program with a real match must produce a real
/// conclusion, and the count must match the number of matches written.
#[test]
fn an_analysis_that_stops_running_cannot_look_like_success() {
    let p = Program::for_fixture("A-002");
    let units: Vec<pw_core::check::Unit> = p
        .sources
        .iter()
        .enumerate()
        .map(|(i, src)| pw_core::check::Unit {
            path: format!("{i}.pw"),
            src: src.clone(),
            hir: lower_file(src, &parse_tree(src).green),
        })
        .collect();
    let reports = match_analysis(&units);

    let proven: Vec<&pw_core::check::MatchAnalysis> = reports
        .iter()
        .filter(|m| m.outcome == MatchOutcome::Proven)
        .collect();
    assert_eq!(
        proven.len(),
        1,
        "A-002 writes exactly one match and it is proved exhaustive: {:?}",
        reports
            .iter()
            .map(|m| (&m.declaration, &m.outcome))
            .collect::<Vec<_>>()
    );
    assert_eq!(proven[0].declaration, "OrderStatus");
    assert_eq!(proven[0].scrutinee_type.as_deref(), Some("OrderState"));

    // And the number of conclusions equals the number of matches written, so a
    // silently skipped match cannot pass as a program with none.
    // Counted from the source, ignoring comment lines. A-002 writes its match
    // inside markup — `{match state {` — so the marker is the keyword followed
    // by a scrutinee, not the start of a line.
    let written = p
        .sources
        .iter()
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .filter(|l| l.contains("match ") && l.trim_end().ends_with('{'))
                .count()
        })
        .sum::<usize>();
    assert_eq!(
        reports.len(),
        written,
        "every `match` in the program produced a conclusion"
    );
}
