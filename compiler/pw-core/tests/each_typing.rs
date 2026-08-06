//! `{#each xs as x}` gives `x` the element type of `xs`.
//!
//! An E9 slice pulled forward, because E7 cannot be honest without it. A
//! resumable handler's captures are hashed into a schema the running code
//! compares against its own build's. A capture whose type the build cannot
//! name yields a hash derived from a guess, and a guessed hash matches
//! nothing — so the build refuses (`PW5016`) rather than emitting a handler
//! that would fail at resume time for a reason nobody could diagnose.
//!
//! That refusal is only correct if the type rule is real. Before this rule the
//! only way to put a resumable handler inside a loop was for the adapter to
//! assume `item` "probably means `MenuItem`", which would put serialization,
//! nominal identity, privacy and handler compatibility on a guess.
//!
//! Architect ruling, 2026-08-06:
//!
//! > `type known -> generate resumable handler`, `type unresolved -> compile
//! > diagnostic`, not `type unresolved -> quietly fall back to an ordinary
//! > handler`. The latter would make performance and compatibility depend on
//! > compiler blind spots.
//!
//! So both columns are tested. A test that only showed the accepting side
//! would pass just as well if the rule typed everything, including things it
//! cannot know.

use pw_core::check::check_sources;

const DOMAIN: &str = r#"
module domain

public type ItemId = { id: String }
public type Bag    = { size: Int }
"#;

/// Run one page against a fixed domain module.
fn diags(page: &str) -> Vec<(String, String)> {
    let files = vec![
        ("domain.pw".to_string(), DOMAIN.to_string()),
        ("page.pw".to_string(), page.to_string()),
    ];
    check_sources(&files)
        .into_iter()
        .filter(|(p, _)| p == "page.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| (d.code.to_string(), d.message.clone()))
        .collect()
}

fn page(query_return: &str, collection: &str) -> String {
    format!(
        r#"
module page

import domain.{{ ItemId, Bag }}

public query Items() -> {query_return}
    freshness   5.minutes
    consistency snapshot
    cache       shared
    key         ()

command pick(item: ItemId) -> Result<Bag, String> {{
    Ok(Bag {{ size: 1 }})
}}

page P() {{
    placement origin
    cache     private

    let items = query Items()

    view {{
        <ul>
            {{#each {collection} as item (item.id)}}
                <li>
                    <button
                        type="button"
                        on:press={{resumable(captures = {{ item }}) => pick(item)}}
                    >Pick</button>
                </li>
            {{/each}}
        </ul>
    }}
}}
"#
    )
}

fn unnameable(ds: &[(String, String)]) -> Vec<&(String, String)> {
    ds.iter().filter(|(c, _)| c == "PW5016").collect()
}

#[test]
fn a_loop_binding_over_a_list_gets_the_element_type() {
    let ds = diags(&page("List<ItemId>", "items"));
    assert!(
        unnameable(&ds).is_empty(),
        "`items : List<ItemId>` must make `item` an `ItemId`, got {ds:?}"
    );
}

#[test]
fn the_carriers_a_query_returns_are_seen_through() {
    // A query returns `Result<List<T>, E>` and the loop iterates what is
    // inside BOTH wrappers. Seeing through `Result` here is not the same as
    // ignoring it: the error side still has to be handled, and that is a
    // different rule. This one answers only "what is one element".
    for ret in [
        "Result<List<ItemId>, String>",
        "Option<List<ItemId>>",
        "List<ItemId>",
    ] {
        let ds = diags(&page(ret, "items"));
        assert!(
            unnameable(&ds).is_empty(),
            "`{ret}` must yield an element type, got {ds:?}"
        );
    }
}

#[test]
fn a_loop_over_something_with_no_element_type_refuses_the_handler() {
    // The negative control the accepting tests are worthless without. A rule
    // that typed every loop binding would pass all of them.
    let ds = diags(&page("Result<Bag, String>", "items"));
    assert_eq!(
        unnameable(&ds).len(),
        1,
        "`Bag` is not a collection, so `item` has no type and the resumable \
         handler must be refused, not quietly downgraded. Got {ds:?}"
    );
}

#[test]
fn a_loop_over_an_unknown_name_refuses_the_handler() {
    // `absent` is bound by nothing. The rule must answer "this program does
    // not say", not reach for the only `List` in scope.
    let ds = diags(&page("List<ItemId>", "absent"));
    assert_eq!(
        unnameable(&ds).len(),
        1,
        "an unbound collection cannot type its loop binding, got {ds:?}"
    );
}

#[test]
fn the_element_type_is_the_argument_and_not_the_carrier() {
    // The defect this rule replaced, stated directly: `Param` and `Decl` kept
    // a type's HEAD and dropped its arguments, so `List<ItemId>` reduced to
    // `List` and the element was gone before any rule could ask for it. A
    // build that typed `item` as `List` would satisfy `PW5016` — a capture
    // would have *a* type — while hashing the wrong schema, which is worse
    // than refusing, because it fails at resume time instead of at build time.
    use pw_core::lower::lower_file;
    use pw_syntax::parse_tree;

    let src = page("Result<List<ItemId>, String>", "items");
    let p = parse_tree(&src);
    let hir = lower_file(&src, &p.green);
    let items = hir
        .all_decls()
        .find(|(_, d)| d.name == "Items")
        .map(|(_, d)| d.clone())
        .expect("Items is declared");

    assert_eq!(items.ret.as_deref(), Some("Result"));
    assert_eq!(
        items.ret_args,
        vec!["List<ItemId>".to_string(), "String".to_string()],
        "the return type's arguments must keep their own arguments"
    );
}

#[test]
fn a_resumable_capture_inside_a_loop_reaches_the_manifest() {
    // The end this rule exists for. `PW5016` staying silent is necessary but
    // not sufficient: the manifest and the handler artifact are generated by
    // two different walks and compared (`PW5017`), so a schema built from a
    // half-resolved type shows up here rather than in production.
    let ds = diags(&page("Result<List<ItemId>, String>", "items"));
    assert!(
        ds.iter().all(|(c, _)| c != "PW5017"),
        "manifest and artifact must agree about a loop-bound capture, got {ds:?}"
    );
}

/// The demo this rule was pulled forward for.
///
/// `examples/store/app.pw` is the E4/E5 fixture, and its Add button sits
/// inside `{#each menu as item}`. Until this rule existed the button could
/// only be an ORDINARY handler, and the file said so in a comment. Asserting
/// against the real file rather than a fixture is the point: a synthetic
/// program can be written to suit the rule.
#[test]
fn the_store_demo_generates_a_resume_manifest_for_its_loop_handler() {
    use pw_core::lower::lower_file;
    use pw_core::resolve::Workspace;
    use pw_core::signatures::Signatures;
    use pw_syntax::parse_tree;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |p: &str| std::fs::read_to_string(root.join(p)).expect(p);
    let (domain, app) = (read("examples/domain.pw"), read("examples/store/app.pw"));

    let hirs: Vec<_> = [&domain, &app]
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<_> = hirs.iter().collect();
    let sigs = Signatures::build(&Workspace::build(&refs), &refs);

    let pairs = pw_core::resume_artifacts::generate(&app, &hirs[1], &sigs, "test-build");
    assert_eq!(
        pairs.len(),
        1,
        "the store page declares exactly one resumable handler"
    );
    let (m, a) = &pairs[0];
    assert_eq!(
        pw_core::resume_artifacts::disagreement(m, a),
        None,
        "the manifest and the handler artifact must agree"
    );
    assert!(
        !m.capture_schema.is_empty(),
        "a loop-bound capture must reach the schema"
    );
}
