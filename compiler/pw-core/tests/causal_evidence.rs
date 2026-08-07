//! R-037's claim, asserted as a chain rather than as a verdict.
//!
//! ADR-0022. The fixture says `layout.measure` propagates through `List.map`'s
//! callback. Before `3a0f319` that was false and the fixture was green: the
//! callback parameter was never bound, and the effect was found by matching the
//! spelling `getBoundingClientRect` against every declaration in the program.
//!
//! ```text
//! must contain                            must NOT contain
//!   a ReceiverType member resolution        a ProgramWideName route
//!   propagation through the callback
//!   the effect, caused by both
//! ```
//!
//! The pre-repair implementation emits the same effect set and the same
//! diagnostic. It cannot produce this chain.

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::provenance::{Evidence, FactKind, Route};
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The platform's shape, minimally: a list, an element, and a measuring member.
const PLATFORM: &str = "\
module browser

opaque type ElementRef = String

type Rect = Rect { width: Int }

fn getBoundingClientRect(el: ElementRef) -> Rect !{ layout.measure } { Rect { width: 0 } }
";

const LIST: &str = "\
module List

fn map(items: List<Unknown>, f: Decoder) -> List<Unknown> !{} { items }
";

/// R-037's shape: the effect is reached only through the callback's parameter.
const SMUGGLED: &str = "\
module app

import List
import browser

fn widths(items: List<ElementRef>) -> Int !{} {
    let ws = items |> List.map(fn(el) el.getBoundingClientRect().width)
    0
}
";

fn evidence_of(decl: &str) -> Evidence {
    let sources = [PLATFORM, LIST, SMUGGLED];
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let mut inference = pw_core::effects::Inference::new(&sigs, &ws);
    inference.run(&refs);

    let unit = 2;
    let hir = &hirs[unit];
    let (id, d) = hir
        .all_decls()
        .find(|(_, d)| d.name == decl)
        .unwrap_or_else(|| panic!("no `{decl}`"));
    let body = hir.body(d.body.expect("a body"));
    let types = pw_core::infer::Types::of_body(&sigs, d, body, hir.module_of(id));
    inference.infer_in_at(unit, body, types.bindings()).evidence
}

#[test]
fn the_effect_is_caused_by_a_receiver_type_resolution() {
    let e = evidence_of("widths");
    assert!(
        !e.is_empty(),
        "the analysis recorded nothing, so this test asserts nothing"
    );

    // REQUIRED EDGE 1: the member was found through the receiver's type.
    assert!(
        e.resolved_member_on("ElementRef", "getBoundingClientRect"),
        "no receiver-type resolution: {:?}",
        e.facts()
    );

    // REQUIRED EDGE 2: the effect exists, and its causes reach that
    // resolution — not merely coexist with it.
    let effect = e
        .effect_reached("layout.measure")
        .expect("the effect reached the body");
    let causes = e.causes(effect.id);
    assert!(
        causes.iter().any(|f| matches!(
            &f.kind,
            FactKind::ResolvedMember { receiver_type, member, via: Route::ReceiverType }
                if receiver_type == "ElementRef" && member == "getBoundingClientRect"
        )),
        "the effect does not depend on the resolution that produced it: {causes:?}"
    );
}

#[test]
fn the_effect_travelled_through_the_callback() {
    // The fixture's actual claim. An effect reaching the body directly would
    // satisfy the test above and would not be what R-037 is about.
    let e = evidence_of("widths");
    let effect = e
        .effect_reached("layout.measure")
        .expect("the effect reached the body");
    assert!(
        e.causes(effect.id).iter().any(|f| matches!(
            &f.kind,
            FactKind::ThroughCallback { passed_to } if passed_to.contains("map")
        )),
        "the chain does not go through the callback: {:?}",
        e.causes(effect.id)
    );
}

#[test]
fn no_conclusion_was_reached_by_a_program_wide_name() {
    // The FORBIDDEN SOURCE. `Route::ProgramWideName` is deleted from the
    // compiler and kept nameable so this assertion can exist — a route with no
    // name cannot be forbidden, and this is the one every instance of
    // coincidental correctness so far has travelled by.
    let e = evidence_of("widths");
    assert!(
        !e.routes().contains(&Route::ProgramWideName),
        "a conclusion was reached by matching a spelling against the whole \
         program: {:?}",
        e.facts()
    );
}

#[test]
fn a_receiver_with_no_such_member_produces_no_chain() {
    // The semantics-BREAKING control, in ADR-0022's terms. Change the one fact
    // the chain depends on — the receiver's type — and the conclusion must go
    // with it. Without this, the three tests above are satisfied by an
    // analysis that records a receiver-type edge for everything it sees.
    let plain = "\
module app

import List
import browser

fn widths(items: List<Rect>) -> Int !{} {
    let ws = items |> List.map(fn(el) el.getBoundingClientRect().width)
    0
}
";
    let sources = [PLATFORM, LIST, plain];
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let mut inference = pw_core::effects::Inference::new(&sigs, &ws);
    inference.run(&refs);

    let hir = &hirs[2];
    let (id, d) = hir
        .all_decls()
        .find(|(_, d)| d.name == "widths")
        .expect("widths");
    let body = hir.body(d.body.expect("a body"));
    let types = pw_core::infer::Types::of_body(&sigs, d, body, hir.module_of(id));
    let e = inference.infer_in_at(2, body, types.bindings()).evidence;

    assert!(
        !e.resolved_member_on("ElementRef", "getBoundingClientRect"),
        "a `Rect` receiver must not resolve an `ElementRef` member"
    );
    assert!(
        e.effect_reached("layout.measure").is_none(),
        "and the effect must not reach the body: {:?}",
        e.facts()
    );
}
