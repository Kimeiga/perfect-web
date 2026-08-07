//! When does an effect happen, and therefore whose authority is it?
//!
//! Architect ruling, 2026-08-07:
//!
//! > Make sure `infer_excluding` excludes only the **deferred handler body**,
//! > not computations that actually occur while constructing its captures.
//! > […] This should get a small accepted/rejected matrix now, because
//! > execution-context partitioning has produced several subtle bugs already.
//!
//! The partition:
//!
//! ```text
//! render time        page/component authority
//!   the surrounding body
//!   the lambda's DESCRIPTOR — `resumable(captures = { .. })`, whose values
//!   are read, typed and serialized while the page is being built
//!
//! interaction time   handler authority
//!   the lambda's BODY, which runs when someone presses the control
//! ```
//!
//! The matrix below is written as "where did the effect land", because that is
//! the question a host answers with a capability grant. Every row has a
//! neighbour that differs only in WHERE the effect is written, so a rule that
//! ignored position would fail one of each pair.

use pw_core::contract::{Capability, ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

const PRELUDE: &str = "\
module shop.ui

fn peek() -> Int !{ database.read<Stores> } { 0 }
fn poke(x: Int) -> Int !{ database.write<Carts> } { x }

command Buy(id: Int) -> Int
    requires SignedIn
{
    poke(id)
}
";

fn caps_of(source: &str, id: &str) -> Vec<String> {
    let text = format!("{PRELUDE}\n{source}\n");
    let hir: Hir = lower_file(&text, &parse_tree(&text).green);
    let refs = vec![&hir];
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let all: Vec<ComponentContract> = contracts(&refs, &sigs, &ws);
    all.into_iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("no contract for {id}"))
        .required_capabilities
        .iter()
        .map(Capability::name)
        .collect()
}

// --- interaction time: the handler's authority, not the page's --------------

#[test]
fn an_effect_inside_the_handler_body_is_the_handlers() {
    // The base case, and the one E8-0 was built for. The page renders a button;
    // pressing it writes. Rendering does not write.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    view {
        <button on:press={resumable() => Buy(id)}>Buy</button>
    }
}",
        "shop.ui.Shop",
    );
    assert!(caps.is_empty(), "rendering requires nothing, got {caps:?}");
}

#[test]
fn a_deeper_call_inside_the_handler_body_is_still_the_handlers() {
    // Depth must not matter. A rule that only looked one level into the lambda
    // would attribute this correctly and the next nesting incorrectly.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    view {
        <button on:press={resumable() => Buy(poke(id))}>Buy</button>
    }
}",
        "shop.ui.Shop",
    );
    assert!(
        caps.is_empty(),
        "still nothing at render time, got {caps:?}"
    );
}

// --- render time: the page's authority --------------------------------------

#[test]
fn an_effect_in_the_surrounding_body_is_the_pages() {
    // The neighbour of the first case, differing only in position. The value is
    // read while the page is built, so the page performed the read — and it
    // would still have performed it if nobody ever pressed anything.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    let seen = peek()

    view {
        <button on:press={resumable() => Buy(id)}>{seen}</button>
    }
}",
        "shop.ui.Shop",
    );
    assert_eq!(
        caps,
        ["database.read<Stores>"],
        "a read outside the lambda is render-time authority"
    );
}

#[test]
fn an_effect_in_the_capture_descriptor_is_the_pages() {
    // The case the ruling singled out. `resumable(captures = { .. })` is
    // evaluated while the page renders: the captured values are read, typed and
    // serialized then. Excluding the whole lambda — as the first version did —
    // would let a page perform an effect at render time and attribute it to a
    // button nobody has pressed.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    view {
        <button on:press={resumable(captures = { peek() }) => Buy(id)}>Buy</button>
    }
}",
        "shop.ui.Shop",
    );
    assert_eq!(
        caps,
        ["database.read<Stores>"],
        "building a capture happens at render time"
    );
}

#[test]
fn a_value_read_before_the_handler_and_captured_by_it_is_the_pages() {
    // The architect's example:
    //
    //     let secret = secret.read()
    //     on:press={() => do_thing(secret)}
    //
    // The read already happened. Whether the handler is ever invoked changes
    // nothing about it.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    let seen = peek()

    view {
        <button on:press={resumable(captures = { seen }) => Buy(seen)}>Buy</button>
    }
}",
        "shop.ui.Shop",
    );
    assert_eq!(caps, ["database.read<Stores>"]);
}

// --- both at once ------------------------------------------------------------

#[test]
fn a_page_that_does_both_gets_only_the_render_time_half() {
    // The discriminating row. One effect at render time and a different one
    // inside the handler: a rule that took everything would report both, and a
    // rule that took nothing would report neither.
    let caps = caps_of(
        "\
page Shop(id: Int) {
    let seen = peek()

    view {
        <button on:press={resumable(captures = { seen }) => Buy(seen)}>{seen}</button>
    }
}",
        "shop.ui.Shop",
    );
    assert_eq!(caps, ["database.read<Stores>"]);
    assert!(
        !caps.contains(&"database.write<Carts>".to_string()),
        "the write belongs to the command, which has its own contract"
    );

    // And it is not lost — recorded once, against the thing that performs it.
    assert_eq!(
        caps_of(
            "\
page Shop(id: Int) {
    view {
        <button on:press={resumable() => Buy(id)}>Buy</button>
    }
}",
            "shop.ui.Buy"
        ),
        ["database.write<Carts>"]
    );
}
