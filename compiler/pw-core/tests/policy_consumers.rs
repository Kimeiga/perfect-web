//! **Which analyses currently see a value written in POLICY position, frozen
//! before `PolicyExpr` is separated from `TermExpr`.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > Because today `policy values → body expression tree`, we do not yet know
//! > whether they have accidentally participated in: effect inference; privacy;
//! > placement; call graph construction; capability derivation; backend
//! > lowering. I would measure all six. […] If today's compiler gives some
//! > policy value runtime effects or call edges, freeze that result before
//! > changing it and classify the movement.
//!
//! # The instrument
//!
//! Same-spelling controls. Each pair of pages is identical except for **where**
//! the call is written:
//!
//! ```text
//! page Poll { key helper(a)  … }   the call is a POLICY VALUE
//! page Term { … {helper(a)} … }    the call is RENDERED
//! page Neither { … }               neither
//! ```
//!
//! A consumer that distinguishes the two positions gives `Poll` and `Term`
//! different answers. Every consumer measured below gives them the **same**
//! answer, which is the finding: today there is no policy position — there is
//! only a body, and a policy value is an expression in it.
//!
//! # The answer, as measured
//!
//! ```text
//! 1  effect inference        SEES IT   Poll gains `database.read<Carts>`
//! 2  privacy                 SEES IT   PW5001 fires on a policy value
//! 3  placement               SEES IT   Poll narrows to `origin`
//! 4  call graph              blind     no call edges from either position
//! 5  capability derivation   SEES IT   the CONTRACT requires it
//! 6  backend lowering        gated     this program does not check, and
//!                                      `Checked::of` is what `program` takes
//! ```
//!
//! Four of six, and one of the four is the artifact the host grants authority
//! from. `PW0401` — *a page may not perform this* — fires because of what a
//! **policy value** spells.
//!
//! # This file asserts today's values, so the split turns it red
//!
//! Deliberately, and for the same reason `unresolved_provenance.rs` does. Each
//! row that moves when `PolicyExpr` lands is a behaviour change to classify;
//! each row that does not is equally informative.

use std::collections::BTreeSet;

use pw_core::check::check_sources;
use pw_core::contract::{ComponentContract, contracts};
use pw_core::effects::Inference;
use pw_core::graph::Graph;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The three pairs.
///
/// `helper` is spelled deliberately. See
/// `a_call_is_a_call_or_a_keyword_depending_on_its_spelling` — an
/// identifier-led call whose name collides with a language keyword lowers to
/// `Expr::Keyword` instead of `Expr::Call`, and then contributes nothing. The
/// first draft of this probe called the helper `measure`, measured silence, and
/// would have frozen "policy values are invisible to inference" as the answer.
const PROBE: &str = "\
module probe

opaque type SessionId = String
type Carts = Carts { n: Int }

fn helper(n: Int) -> Int !{ database.read<Carts> } { todo }

page Poll(a: Int) {
    key helper(a)

    view {
        <p>hi</p>
    }
}

page Term(a: Int) {
    view {
        <p>{helper(a)}</p>
    }
}

page Neither(a: Int) {
    view {
        <p>{a}</p>
    }
}

session query Held(s: SessionId) -> Int
    freshness 0.seconds
    cache private
    key s
{
    1
}

page PrivPolicy(s: SessionId) {
    cache shared
    key Held(s)

    view {
        <p>hi</p>
    }
}

page PrivTerm(s: SessionId) {
    cache shared

    view {
        <p>{Held(s)}</p>
    }
}

page PrivNeither(s: SessionId) {
    cache shared

    view {
        <p>hi</p>
    }
}
";

/// The platform package, so `database.read<T>` and `session.read` are declared
/// effects with real placements rather than unknown families.
fn files() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("probe.pw".to_string(), PROBE.to_string()));
    out
}

struct Built {
    hirs: Vec<Hir>,
}

impl Built {
    fn new() -> Built {
        Built {
            hirs: files()
                .iter()
                .map(|(_, s)| lower_file(s, &parse_tree(s).green))
                .collect(),
        }
    }
    fn refs(&self) -> Vec<&Hir> {
        self.hirs.iter().collect()
    }
    /// `probe.pw` is pushed last.
    fn probe_unit(&self) -> usize {
        self.hirs.len() - 1
    }
}

fn caps(c: &ComponentContract) -> BTreeSet<String> {
    c.required_capabilities.iter().map(|k| k.name()).collect()
}

fn contract<'a>(cs: &'a [ComponentContract], id: &str) -> &'a ComponentContract {
    cs.iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("no contract `{id}`"))
}

// --- 1 ----------------------------------------------------------------------

/// **Effect inference sees policy values, and cannot tell them from terms.**
#[test]
fn effect_inference_sees_a_call_written_in_policy_position() {
    let built = Built::new();
    let refs = built.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let mut inf = Inference::new(&sigs, &ws);
    inf.run(&refs);

    let unit = built.probe_unit();
    let hir = refs[unit];
    let of = |name: &str| -> Vec<String> {
        let (id, _) = hir
            .all_decls()
            .find(|(_, d)| d.name == name)
            .unwrap_or_else(|| panic!("no decl `{name}`"));
        inf.effective_effects(unit, hir, id)
    };

    assert_eq!(
        of("Poll"),
        ["database.read<Carts>"],
        "TODAY: `key helper(a)` is a policy value, and inference charges the \
         page for the effect of running it"
    );
    assert_eq!(of("Term"), ["database.read<Carts>"], "the control");
    assert!(
        of("Neither").is_empty(),
        "and the discriminator: without the call, nothing"
    );
}

// --- 2 ----------------------------------------------------------------------

/// **Privacy sees them too, and reaches a real refusal.**
///
/// `PW5001` is charter §7.8's canonical case — a non-public value in a shared
/// cache. `PrivPolicy` never renders the session query; it merely names it in a
/// policy value. It is refused anyway, identically to the page that renders it.
///
/// Not an accident of one rule: `reads_label_with_source` walks `body.walk()`
/// and matches `Expr::Call` with no notion of position, so *every* privacy
/// conclusion drawn from what a declaration reads is drawn from policy values
/// as well.
#[test]
fn a_privacy_refusal_fires_because_of_a_policy_value() {
    let fs = files();
    let out = check_sources(&fs);
    let codes: Vec<(String, String)> = out
        .iter()
        .flat_map(|(_, ds)| ds.iter())
        .map(|d| (d.code.to_string(), d.message.clone()))
        .collect();

    let fired = |page: &str, code: &str| codes.iter().any(|(c, m)| c == code && m.contains(page));

    assert!(
        fired("PrivPolicy", "PW5001"),
        "TODAY: naming a session query in a POLICY VALUE makes the page \
         private. Got {codes:?}"
    );
    assert!(fired("PrivTerm", "PW5001"), "the control");
    assert!(
        !fired("PrivNeither", "PW5001"),
        "and the discriminator: the same page without the call is not refused"
    );

    // The effect side of the same walk, for completeness: a page is refused for
    // performing an effect its POLICY value performs.
    assert!(
        fired("Poll", "PW0401") && fired("Term", "PW0401"),
        "`a page may not do this` fires for a policy value: {codes:?}"
    );
}

// --- 3 and 5 ----------------------------------------------------------------

/// **Placement and capability derivation, which is the row that matters most.**
///
/// `required_capabilities` is what a host grants authority from (ADR-0018), and
/// `allowed_placements` is where the component may run. Both move because of a
/// value written in policy position.
#[test]
fn the_contract_requires_a_capability_because_of_a_policy_value() {
    let built = Built::new();
    let refs = built.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);

    let poll = contract(&cs, "probe.Poll");
    let term = contract(&cs, "probe.Term");
    let neither = contract(&cs, "probe.Neither");

    assert_eq!(
        caps(poll),
        BTreeSet::from(["database.read<Carts>".to_string()]),
        "TODAY: the DECLARED AUTHORITY of a component includes what its policy \
         values would need if they were executed"
    );
    assert_eq!(caps(term), caps(poll), "identical to the control");
    assert!(caps(neither).is_empty(), "the discriminator");

    assert_eq!(
        poll.allowed_placements,
        ["origin"],
        "and placement narrows with it — `database.read` is origin-only, so a \
         policy value decides where the page may run"
    );
    assert_eq!(term.allowed_placements, poll.allowed_placements);
    assert_eq!(
        neither.allowed_placements,
        ["build", "browser", "edge", "origin"],
        "the discriminator: unconstrained without the call"
    );
}

// --- 4 ----------------------------------------------------------------------

/// **The call graph is blind to both positions**, so nothing moves here.
///
/// Recorded because a row that does not move after the split is as much a
/// result as one that does. The graph's edges come from `Decl.policies` — the
/// declaration-level policy STRINGS (`invalidates`, `emits`, `invalidates_on`)
/// — and never from a body call in either position.
#[test]
fn the_call_graph_sees_neither_position() {
    let built = Built::new();
    let refs = built.refs();
    let ws = Workspace::build(&refs);
    let g = Graph::build(&refs, &ws);

    let from_probe: Vec<&pw_core::graph::Edge> = g
        .edges
        .iter()
        .filter(|e| e.from.starts_with("probe."))
        .collect();
    assert!(
        from_probe.is_empty(),
        "TODAY: no edges from either position. If the split gives policy \
         values their own resolution, an edge appearing here is a NEW \
         conclusion, not a preserved one: {from_probe:?}"
    );

    // The pages are nodes; it is the edges that are absent. Stated so a future
    // reading cannot mistake "the graph does not know these declarations" for
    // "the graph ignores policy values".
    let nodes: BTreeSet<&str> = g
        .nodes
        .iter()
        .filter(|n| n.path.starts_with("probe."))
        .map(|n| n.path.as_str())
        .collect();
    assert!(
        nodes.contains("probe.Poll") && nodes.contains("probe.Term"),
        "{nodes:?}"
    );
}

// --- 6 ----------------------------------------------------------------------

/// **Backend lowering: the question does not reach it, and now cannot.**
///
/// `lower.rs` walks bodies with no notion of position either, so the answer for
/// this row would be the same as the other five. Two things stop it being
/// asked, and they are different in kind:
///
/// ```text
/// E10-A's supported set        a `page` is not in it — a scope decision
/// Checked::of                  this program does not check — a gate
/// ```
///
/// The second is the architect's step 10, landed 2026-08-10. The probe below
/// deliberately contains `PW0401` and `PW5001` — it exists to measure them —
/// so the backend cannot be handed it at all. Widening the backend to pages
/// before the `PolicyExpr` split lands would still be wrong; what changed is
/// that it would be wrong about a program that checks, rather than about this
/// one.
#[test]
fn the_backend_is_never_handed_this_program() {
    use pw_core::backend::lower::{Checked, Context};
    let built = Built::new();
    let refs = built.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    let units: Vec<pw_core::check::Unit> = files()
        .into_iter()
        .zip(built.hirs.iter())
        .map(|((path, src), hir)| pw_core::check::Unit {
            path,
            src,
            hir: hir.clone(),
        })
        .collect();
    let refused = Checked::of(&units, cx)
        .err()
        .expect("the probe renders a page that performs a database read");
    assert!(
        refused.iter().any(|d| d.code == "PW0401"),
        "and for the reason the rows above are about — a page performing what a \
         POLICY VALUE spells: {:?}",
        refused.iter().map(|d| d.code).collect::<Vec<_>>()
    );
}

// --- the finding that nearly cost the measurement ----------------------------

/// **An identifier-led call is a `Call` or a `Keyword` depending on how it is
/// spelled** — and only one of the two contributes anything.
///
/// `helper(a)` lowers to `Expr::Call`. `measure(a)`, written in exactly the
/// same position, lowers to `Expr::Keyword { keyword: "measure" }`, because
/// `measure` is in the parser's statement-keyword set. Inference contributes
/// nothing for a `Keyword`, so the page is silently effect-free.
///
/// This is the fifth spelling-based resolution defect in the project and the
/// first one in the parser. It is recorded here because it is what the
/// architect's *exactly-one-semantic-owner* gate has to answer for:
/// `Expr::Keyword` is the bucket a call falls into when nothing claims it, and
/// membership is decided by a word list.
#[test]
fn a_call_is_a_call_or_a_keyword_depending_on_its_spelling() {
    use pw_core::hir::Expr;

    let shape = |name: &str| -> String {
        let src = format!(
            "module m\n\nfn {name}(n: Int) -> Int !{{ database.read<Carts> }} {{ todo }}\n\n\
             page P(a: Int) {{\n    key {name}(a)\n\n    view {{\n        <p>hi</p>\n    }}\n}}\n"
        );
        let hir = lower_file(&src, &parse_tree(&src).green);
        let (_, d) = hir.all_decls().find(|(_, d)| d.name == "P").expect("P");
        let body = hir.body(d.body.expect("body"));
        body.walk()
            .iter()
            .find_map(|id| match body.expr(*id) {
                Expr::Call { .. } => Some("Call".to_string()),
                Expr::Keyword { keyword, .. } if keyword == name => Some("Keyword".to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "neither".to_string())
    };

    assert_eq!(shape("helper"), "Call");
    assert_eq!(
        shape("measure"),
        "Keyword",
        "TODAY: the same syntax in the same position lowers to a different \
         node because of the callee's spelling"
    );

    // And the consequence, so this is a defect report rather than a curiosity.
    let effects_of = |name: &str| -> Vec<String> {
        let src = format!(
            "module m\n\ntype Carts = Carts {{ n: Int }}\n\n\
             fn {name}(n: Int) -> Int !{{ database.read<Carts> }} {{ todo }}\n\n\
             page P(a: Int) {{\n    key {name}(a)\n\n    view {{\n        <p>hi</p>\n    }}\n}}\n"
        );
        let hir = lower_file(&src, &parse_tree(&src).green);
        let refs = vec![&hir];
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        let mut inf = Inference::new(&sigs, &ws);
        inf.run(&refs);
        let (id, _) = hir.all_decls().find(|(_, d)| d.name == "P").expect("P");
        inf.effective_effects(0, &hir, id)
    };

    assert_eq!(effects_of("helper"), ["database.read<Carts>"]);
    assert!(
        effects_of("measure").is_empty(),
        "TODAY: renaming the helper to a word the parser knows makes the page's \
         authority disappear. Neither answer is derived from what the callee \
         is; both are derived from how it is spelled."
    );
}
