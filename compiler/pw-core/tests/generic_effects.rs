//! **A member read is a call wherever its receiver comes from** (ADR-XXXX,
//! the generic-effects track).
//!
//! A member read, `item.label` where `label` is a function of `Item`'s, is a
//! call (ADR-0048), and the effect inference charges what it performs by the
//! receiver's TYPE (ADR-0022). On 2026-10-16 every one of these checked clean
//! at master 22a0508e, each a body declared to perform nothing reading the
//! database through such a member:
//! - a receiver a GENERIC call computes: `let all = List.reverse(given)` then
//!   `for x in all { x.label }`, in a function and in a view's loop, through a
//!   user-written `fn same<T>(x: T) -> T`, a nested generic, a pipe, a lambda
//!   over a computed list and a match arm. `infer.rs` kept the call's declared
//!   result, `List<T>`, with the callee's `T` unsolved, and a member of `T` is
//!   none;
//! - and, not generic, a `for` loop's binder and an expression `match` arm's,
//!   which `infer.rs` typed not at all; a receiver that is not a bare name,
//!   `o.item.label`, which the member walk skipped; a generic record's field,
//!   `b.v.label`, typed as its declared `T`; a member with no declared row,
//!   charged its row (nothing) rather than what its body performs; and a
//!   member read on a value of a rigid type parameter, which the value
//!   relations left undecided.
//!
//! The same receivers carried a secret into markup unseen (PW5003), since a
//! member's label is read through the same types, and left a query's
//! contract without the capability its body needs.
//!
//! Each test states one case with its control: the same program reading a
//! record field, which performs nothing.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

/// Each diagnostic on `t.pw`, as its code and the text its primary span
/// covers: where it is reported is part of what is claimed.
fn reported(src: &str) -> Vec<(String, String)> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let at = src.get(d.primary_span.clone()).unwrap_or("?").to_string();
                (d.code.to_string(), at)
            })
        })
        .collect()
}

fn one(code: &str, at: &str) -> Vec<(String, String)> {
    vec![(code.to_string(), at.to_string())]
}

const NOTHING: Vec<(String, String)> = Vec::new();

/// A module with an `Item` whose member `label` reads the database, and
/// `rest` after it.
fn reading(rest: &str) -> String {
    format!(
        "module t\n\n\
         import List\n\n\
         type Item = Item {{ name: String }}\n\n\
         fn label(i: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n\
         {rest}\n"
    )
}

/// `src` refused once with `code` at `at`, and its control, the same
/// program reading the record's field `name` instead, accepted.
fn refused_with_control(src: &str, code: &str, at: &str) {
    assert_eq!(reported(src), one(code, at), "{src}");
    let control = src.replace(at, &at.replace(".label", ".name"));
    assert_ne!(control, src, "the control reads a field");
    assert_eq!(reported(&control), NOTHING, "{control}");
}

#[test]
fn a_member_read_through_a_generic_calls_result_in_a_function_is_seen() {
    let src = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    let all = List.reverse(given)\n    for x in all {\n        return x.label\n    }\n    \"none\"\n}",
    );
    refused_with_control(&src, "PW0400", "x.label");
}

#[test]
fn a_member_read_through_a_generic_calls_result_in_a_view_is_refused() {
    let src = reading(
        "view Menu(given: List<Item>) !{} {\n    let all = List.reverse(given)\n    <ul>\n        {#each all as item (item.name)}\n            <li>{item.label}</li>\n        {/each}\n    </ul>\n}",
    );
    refused_with_control(&src, "PW0401", "item.label");
}

#[test]
fn a_member_read_through_a_user_written_generic_function_is_seen() {
    let same = "fn same<T>(x: T) -> T !{} {\n    x\n}\n\n";
    let through_let = reading(&format!(
        "{same}fn first_label(given: Item) -> String !{{}} {{\n    let y = same(given)\n    y.label\n}}"
    ));
    refused_with_control(&through_let, "PW0400", "y.label");
    // Read on the call itself: charged until now by the member's SPELLING
    // (`value_effects` read `same(given).label` as a declaration `label`
    // named as a value), and now by the receiver's type.
    let direct = reading(&format!(
        "{same}fn first_label(given: Item) -> String !{{}} {{\n    same(given).label\n}}"
    ));
    refused_with_control(&direct, "PW0400", "same(given).label");
}

#[test]
fn a_member_read_through_a_nested_generic_is_seen() {
    let nested = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    let all = List.reverse(List.take(given, 2))\n    for x in all {\n        return x.label\n    }\n    \"none\"\n}",
    );
    refused_with_control(&nested, "PW0400", "x.label");
    // A lambda over a list a call computes: its parameter is typed by the
    // solved call, as one over a named list is.
    let lambda = reading(
        "fn labels(given: List<Item>) -> List<String> !{} {\n    List.map(List.reverse(given), i => i.label)\n}",
    );
    refused_with_control(&lambda, "PW0400", "i.label");
    // Piped, and read as a member.
    let piped = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    let all = given |> List.reverse()\n    for x in all {\n        return x.label\n    }\n    \"none\"\n}",
    );
    refused_with_control(&piped, "PW0400", "x.label");
    let member = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    let all = given.reverse\n    for x in all {\n        return x.label\n    }\n    \"none\"\n}",
    );
    refused_with_control(&member, "PW0400", "x.label");
}

#[test]
fn a_member_read_in_a_match_arm_over_a_generic_result_is_seen() {
    let expression = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    match List.get(given, 0) {\n        Some(x) => x.label,\n        None => \"none\",\n    }\n}",
    );
    refused_with_control(&expression, "PW0400", "x.label");
    let template = reading(
        "view Menu(given: List<Item>) !{} {\n    <div>\n        {#match List.get(given, 0)}\n            {:Some(x)}\n                <p>{x.label}</p>\n            {:None}\n                <p>none</p>\n        {/match}\n    </div>\n}",
    );
    refused_with_control(&template, "PW0401", "x.label");
}

#[test]
fn a_member_read_on_a_loops_or_an_arms_binder_is_seen() {
    // Not generic: `infer.rs` typed no `for` binder and no expression match
    // arm's.
    let for_loop = reading(
        "fn first_label(given: List<Item>) -> String !{} {\n    for x in given {\n        return x.label\n    }\n    \"none\"\n}",
    );
    refused_with_control(&for_loop, "PW0400", "x.label");
    let arm = reading(
        "fn first_label(given: Option<Item>) -> String !{} {\n    match given {\n        Some(x) => x.label,\n        None => \"none\",\n    }\n}",
    );
    refused_with_control(&arm, "PW0400", "x.label");
    // A loop over a name a `let` aliases, typed before the `let` was.
    let alias = reading(
        "view Menu(given: List<Item>) !{} {\n    let all = given\n    <ul>\n        {#each all as item (item.name)}\n            <li>{item.label}</li>\n        {/each}\n    </ul>\n}",
    );
    refused_with_control(&alias, "PW0401", "item.label");
}

#[test]
fn a_member_read_on_any_receiver_is_seen() {
    // A field's value, not a bare name: the member walk read only a name.
    let chain = reading(
        "type Order = Order { item: Item }\n\nfn first_label(o: Order) -> String !{} {\n    o.item.label\n}",
    );
    refused_with_control(&chain, "PW0400", "o.item.label");
    // A generic record's field is its declared `T` under the receiver's
    // arguments, not `T`.
    let boxed = reading(
        "type Box<T> = Box { v: T }\n\nfn first_label(b: Box<Item>) -> String !{} {\n    b.v.label\n}",
    );
    refused_with_control(&boxed, "PW0400", "b.v.label");
}

/// A module whose member `label` declares no row, and performs a read.
fn open_row(rest: &str) -> String {
    format!(
        "module t\n\n\
         type Item = Item {{ name: String }}\n\n\
         fn read_label(i: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n\
         fn label(i: Item) -> String {{\n    read_label(i)\n}}\n\n\
         {rest}\n"
    )
}

#[test]
fn a_member_with_no_row_is_charged_what_its_body_performs() {
    let function = open_row("fn first_label(given: Item) -> String !{} {\n    given.label\n}");
    refused_with_control(&function, "PW0400", "given.label");
    let view = open_row("view Show(given: Item) !{} {\n    <p>{given.label}</p>\n}");
    refused_with_control(&view, "PW0401", "given.label");
}

#[test]
fn a_member_read_on_a_rigid_type_parameter_is_refused() {
    // `T` has no members: whatever it is instantiated with, the body does not
    // say which `label` it reads, nor what that performs.
    let src = reading("fn first_label<T>(x: T) -> String !{} {\n    x.label\n}");
    assert_eq!(reported(&src), one("PW0610", "x.label"));
    // The control: a value of `T` handed on whole.
    let control = reading("fn same<T>(x: T) -> T !{} {\n    x\n}");
    assert_eq!(reported(&control), NOTHING);
}

/// A module whose `Item` has a member `token` that is a secret, and a view
/// `Menu` over `given` with `{markup}` as its body.
fn secret(markup: &str) -> String {
    format!(
        "module t\n\n\
         import List\n\
         import capability.{{ Secret, Payments }}\n\n\
         type Item = Item {{ name: String }}\n\n\
         fn token(i: Item) -> Secret<Payments> !{{}}\n    host \"shop:data/items#token\"\n\n\
         view Menu(given: List<Item>) !{{}} {{\n{markup}\n}}\n"
    )
}

#[test]
fn a_secret_read_through_a_generic_result_is_refused_in_markup() {
    for list in ["List.reverse(given)", "given"] {
        let src = secret(&format!(
            "    let all = {list}\n    <ul>\n        {{#each all as item (item.name)}}\n            <li>{{item.token}}</li>\n        {{/each}}\n    </ul>"
        ));
        assert_eq!(reported(&src), one("PW5003", "item.token"), "{src}");
        let control = src.replace("{item.token}", "{item.name}");
        assert_eq!(reported(&control), NOTHING, "{control}");
    }
    let arm = secret(
        "    <div>\n        {#match List.get(given, 0)}\n            {:Some(x)}\n                <p>{x.token}</p>\n            {:None}\n                <p>none</p>\n        {/match}\n    </div>",
    );
    assert_eq!(reported(&arm), one("PW5003", "x.token"));
    assert_eq!(reported(&arm.replace("{x.token}", "{x.name}")), NOTHING);
}

/// The capabilities `component` requires, its contract's.
fn required(src: &str, component: &str) -> Vec<String> {
    use pw_core::contract::{Capability, contracts};
    let files = program(src);
    let hirs: Vec<pw_core::hir::Hir> = files
        .iter()
        .map(|(_, s)| pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
        .into_iter()
        .find(|c| c.component_id == component)
        .unwrap_or_else(|| panic!("no contract for {component}"))
        .required_capabilities
        .iter()
        .map(Capability::name)
        .collect()
}

#[test]
fn a_querys_contract_requires_what_a_member_of_a_generic_result_reads() {
    let src = reading(
        "query Labels(given: List<Item>) -> List<String> {\n    List.map(List.reverse(given), i => i.label)\n}",
    );
    assert_eq!(required(&src, "t.Labels"), ["database.read<Item>"]);
    assert!(required(&src.replace("i.label", "i.name"), "t.Labels").is_empty());
}
