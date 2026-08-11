//! **Which analyses see a value written in POLICY position — measured before
//! the split, and re-measured after it.**
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
//! different answers.
//!
//! # Before the split, 2026-08-10
//!
//! Every consumer gave them the **same** answer. There was no policy position:
//! there was only a body, and a policy value was an expression in it.
//!
//! ```text
//! 1  effect inference        SAW IT    Poll gained `database.read<Carts>`
//! 2  privacy                 SAW IT    PW5001 fired on a policy value
//! 3  placement               SAW IT    Poll narrowed to `origin`
//! 4  call graph              blind
//! 5  capability derivation   SAW IT    the CONTRACT required it
//! 6  backend lowering        gated
//! ```
//!
//! Four of six, and one of the four is the artifact a host grants authority
//! from. `PW0401` — *a page may not perform this* — fired because of what a
//! **policy value** spelled.
//!
//! # After the split, 2026-08-11
//!
//! ```text
//! 1  effect inference        blind     Poll performs nothing
//! 2  privacy                 blind     PW5001 fires on the RENDERED page only
//! 3  placement               blind     Poll is unconstrained
//! 4  call graph              blind     unchanged
//! 5  capability derivation   blind     the contract requires nothing
//! 6  backend lowering        gated     unchanged
//! ```
//!
//! Architect ruling: policy values leave the executable body tree. A UI
//! declaration now parses its policies inside its braces, the way
//! `materialize` has since E6, so `key helper(a)` is a `Policy` on the
//! declaration rather than two bare names in the body.
//!
//! **Every row that moved, moved to `blind`, and the control rows did not
//! move**: `Term` — the same call, rendered — still contributes everything it
//! did. That is what says the split separated the positions rather than
//! silencing the analyses.

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

/// **Effect inference no longer sees policy values, and still sees terms.**
#[test]
fn effect_inference_does_not_see_a_call_written_in_policy_position() {
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

    assert!(
        of("Poll").is_empty(),
        "`key helper(a)` is a policy value; the page does not perform it. It \
         read `database.read<Carts>` until 2026-08-11. Got {:?}",
        of("Poll")
    );
    assert_eq!(
        of("Term"),
        ["database.read<Carts>"],
        "**the control, and it did not move**: the same call, rendered, still \
         contributes. Without this the row above would pass for an inference \
         that stopped working"
    );
    assert!(
        of("Neither").is_empty(),
        "and the discriminator: without the call, nothing"
    );
}

// --- 2 ----------------------------------------------------------------------

/// **Privacy no longer sees them, and still reaches a real refusal.**
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
fn a_privacy_refusal_no_longer_fires_because_of_a_policy_value() {
    let fs = files();
    let out = check_sources(&fs);
    let codes: Vec<(String, String)> = out
        .iter()
        .flat_map(|(_, ds)| ds.iter())
        .map(|d| (d.code.to_string(), d.message.clone()))
        .collect();

    let fired = |page: &str, code: &str| codes.iter().any(|(c, m)| c == code && m.contains(page));

    assert!(
        !fired("PrivPolicy", "PW5001"),
        "naming a session query in a POLICY VALUE no longer makes the page \
         private. It did until 2026-08-11. Got {codes:?}"
    );
    assert!(
        fired("PrivTerm", "PW5001"),
        "**the control, and it did not move**: the page that RENDERS the \
         session query is still refused. Without this the row above would pass \
         for a privacy rule that stopped running: {codes:?}"
    );
    assert!(
        !fired("PrivNeither", "PW5001"),
        "and the discriminator: the same page without the call is not refused"
    );

    // The effect side of the same walk. `a page may not do this` fired for a
    // policy value too, and now fires only for the rendered one.
    assert!(
        !fired("Poll", "PW0401"),
        "a page is not refused for what a policy value spells: {codes:?}"
    );
    assert!(fired("Term", "PW0401"), "and is, for what it renders");
}

// --- 3 and 5 ----------------------------------------------------------------

/// **Placement and capability derivation, which was the row that mattered
/// most.**
///
/// `required_capabilities` is what a host grants authority from (ADR-0018), and
/// `allowed_placements` is where the component may run. Both move because of a
/// value written in policy position.
#[test]
fn the_contract_no_longer_requires_a_capability_for_a_policy_value() {
    let built = Built::new();
    let refs = built.refs();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);

    let poll = contract(&cs, "probe.Poll");
    let term = contract(&cs, "probe.Term");
    let neither = contract(&cs, "probe.Neither");

    assert!(
        caps(poll).is_empty(),
        "the DECLARED AUTHORITY of a component no longer includes what its \
         policy values would need if they were executed. It did until \
         2026-08-11 — and this is the artifact a host grants authority from. \
         Got {:?}",
        caps(poll)
    );
    assert_eq!(
        caps(term),
        BTreeSet::from(["database.read<Carts>".to_string()]),
        "**the control, and it did not move**"
    );
    assert!(caps(neither).is_empty(), "the discriminator");

    assert_eq!(
        poll.allowed_placements,
        ["build", "browser", "edge", "origin"],
        "and placement is unconstrained with it — a policy value no longer \
         decides where the page may run"
    );
    assert_eq!(
        term.allowed_placements,
        ["origin"],
        "while the rendered call still narrows to where `database.read` is \
         granted"
    );
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
/// first one in the parser. **It is still unrepaired**, and it is measured here
/// in a TERM position: `key measure(a)` is a policy value now, so the original
/// probe no longer reaches an expression at all — which is the split working,
/// and would have quietly retired this measurement with it.
#[test]
fn a_call_is_a_call_or_a_keyword_depending_on_its_spelling() {
    use pw_core::hir::Expr;

    let shape = |name: &str| -> String {
        let src = format!(
            "module m\n\ntype Carts = Carts {{ n: Int }}\n\n\
             fn {name}(n: Int) -> Int !{{ database.read<Carts> }} {{ todo }}\n\n\
             page P(a: Int) {{\n    view {{\n        <p>{{{name}(a)}}</p>\n    }}\n}}\n"
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
             page P(a: Int) {{\n    view {{\n        <p>{{{name}(a)}}</p>\n    }}\n}}\n"
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
