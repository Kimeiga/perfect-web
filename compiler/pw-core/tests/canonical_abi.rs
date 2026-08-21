//! **What the Canonical ABI asks of a host operation, and who gets to say.**
//!
//! E10-A step 4. Architect ruling, 2026-08-20:
//!
//! > Canonical ABI should be mechanical. Do **not** hand-implement the
//! > Component Model binary format — use upstream wasm-tools. Pleris owns
//! > lowering facts; upstream owns component-format encoding.
//!
//! So the flattening here is `wit_parser`'s — `Resolve::wasm_signature`, the
//! same function `wit-bindgen` and `wit-component` use. A flattening written in
//! this repo would be a second implementation of the Canonical ABI, and the
//! Canonical ABI is the entire product of this stage.
//!
//! # The second derivation is gone
//!
//! A host operation's ABI used to be derived twice — by the contract, from the
//! Pleris `fn` carrying the `host` binding, and by the deployment, in a WIT
//! package it authored — and nothing compared them. All six of the store's
//! operations disagreed while every gate stayed green.
//!
//! Architect ruling, 2026-08-20, resolving it by deletion rather than by
//! comparison:
//!
//! > Do not choose "emit vs check" based on `owner`. Choose it based on which
//! > artifact is the ABI source of truth. […] The deployment implements the
//! > emitted interface. It does not independently specify what that interface
//! > means.
//!
//! So `wit::package` emits `pw:host` and `store:data` itself, and a
//! hand-authored stand-in beside them is not a disagreement to detect — it is a
//! package defined twice, which the resolver refuses outright.
//!
//! What survives here is the fact that made the old defect invisible, because
//! it still governs what the audit must check: **core Wasm ABI equality is not
//! component ABI equality.**

use wit_parser::abi::AbiVariant;

use pw_core::contract::{ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_core::wit;
use pw_syntax::parse_tree;

mod support;

/// Every `.pw` source of the store demo, as the corpus check feeds it.
fn store_sources() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            sources.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    sources.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"));
    sources
}

/// The store demo's contracts and generated WIT.
fn generated() -> (String, Vec<ComponentContract>) {
    let sources = store_sources();

    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let (text, _) = wit::package(&refs, &ws, &cs).expect("the store demo generates");
    (text, cs)
}

/// Resolve the generated package against the deployment's stand-in, and hand
/// back the resolver so a caller can ask it about types.
fn resolved(name: &str) -> wit_parser::Resolve {
    let (text, _) = generated();
    let dir = support::wit_dir(name, &text);
    let mut resolve = wit_parser::Resolve::new();
    let result = resolve.push_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    result.unwrap_or_else(|e| panic!("the generated WIT does not resolve: {e:?}"));
    resolve
}

/// Every operation the deployment publishes, by `interface#name`, with the flat
/// core signature the Canonical ABI gives it when a guest imports it.
fn flattened(resolve: &wit_parser::Resolve) -> Vec<(String, wit_parser::abi::WasmSignature)> {
    let mut out = Vec::new();
    for (_, iface) in resolve.interfaces.iter() {
        let Some(name) = iface.name.as_deref() else {
            continue;
        };
        for (fname, func) in iface.functions.iter() {
            out.push((
                format!("{name}#{fname}"),
                resolve.wasm_signature(AbiVariant::GuestImport, func),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// **The flat signature is not the written one, and the difference is the whole
/// stage.**
///
/// `wasm::repr` gives every non-scalar value one `i32` handle into the
/// invocation region. The Canonical ABI does not: a `string` is a pointer and a
/// length, and a result that does not fit the flat limit comes back through a
/// **return pointer** rather than as a core result. So the core module the
/// encoder emits today cannot be the core module a component wraps, and this
/// records by how much.
#[test]
fn a_string_parameter_is_two_core_parameters_and_a_result_is_a_pointer() {
    let resolve = resolved("pw-canonical-abi");
    let flat = flattened(&resolve);
    let by = |k: &str| {
        flat.iter()
            .find(|(n, _)| n == k)
            .map(|(_, s)| s)
            .unwrap_or_else(|| panic!("no operation {k} in {:?}", flat.iter().map(|f| &f.0)))
    };

    // `add: func(session: string, item: string, quantity: s64) -> string`
    //   two strings  -> four core parameters
    //   one s64      -> one
    //   the result   -> a return pointer, and NO core result
    let add = by("carts#add");
    assert_eq!(add.params.len(), 6, "{:?}", add.params);
    assert!(add.results.is_empty(), "{:?}", add.results);
    assert!(add.retptr, "the result arrives through a pointer");

    // And the discriminator: an operation with no parameters at all still takes
    // one, because the return pointer is a parameter. A layer that thought
    // "no arguments" meant "no core parameters" would be wrong here.
    let read = by("session#read");
    assert_eq!(read.params.len(), 1, "{:?}", read.params);
    assert!(read.retptr);

    // For contrast, the encoder's own answer for `carts#add` is three parameters
    // and one result, because `repr` gives each value one slot —
    // `tests/wasm_encoding.rs` asserts it there, where the Program is in hand.
    // Neither number is wrong; they answer different questions, and step 4 is
    // making the module speak the second.
}

/// **One ABI authority: the emitted interface IS the contract's signature.**
///
/// This used to assert that two derivations disagreed above the core level
/// while coinciding at it. The ruling deleted the second derivation, so the
/// assertion inverts: the operation the deployment sees and the operation the
/// contract fixed are now the same fact, and there is nothing to keep
/// synchronized.
///
/// The old finding survives as the reason the **component-level** audit is
/// mandatory. `resources-cart-api#cart` and `carts#current` are two different
/// signatures written two different ways —
/// `(SessionId) -> Result<Cart, CartError>` exported, and the same shape
/// imported — and they flatten to an identical core signature:
///
/// ```text
/// [Pointer, Length, Pointer]  retptr, no core result
/// ```
///
/// A `string` and a `result<record, variant>` alike exceed the flat limit and
/// return through a pointer. So a core-level check can never decide whether an
/// artifact imports the operation the contract meant: **core Wasm ABI equality
/// is not component ABI equality**, and the audit has to compare component
/// types.
#[test]
fn the_emitted_interface_is_the_contracts_signature_and_the_core_cannot_tell() {
    let (text, cs) = generated();

    // One authority: what the contract fixes is what the package publishes.
    let contract_sig = cs
        .iter()
        .flat_map(|c| &c.imports)
        .find(|i| i.key() == "store:data/carts#current")
        .and_then(|i| i.signature.as_ref())
        .expect("the contract carries it");
    assert_eq!(contract_sig.params, vec!["SessionId".to_string()]);
    assert_eq!(contract_sig.result, "Result<Cart, CartError>");
    assert!(
        text.contains(
            "current: func(arg0: capability-session-id) -> result<domain-cart, domain-cart-error>;"
        ),
        "the emitted interface is that signature, not a second opinion about it"
    );

    // And the core level still cannot tell one component type from another.
    let resolve = resolved("pw-canonical-abi-authority");
    let flat = flattened(&resolve);
    let by = |k: &str| {
        flat.iter()
            .find(|(n, _)| n == k)
            .map(|(_, s)| s)
            .unwrap_or_else(|| {
                panic!(
                    "no operation {k}; have {:?}",
                    flat.iter().map(|f| &f.0).collect::<Vec<_>>()
                )
            })
    };
    let exported = by("resources-cart-api#cart");
    let imported = by("carts#current");
    assert_eq!(
        (&exported.params, &exported.results, exported.retptr),
        (&imported.params, &imported.results, imported.retptr),
        "if these ever differ, a core-level check could decide component \
         identity after all and the component audit would be redundant"
    );
    assert!(
        exported.retptr && exported.results.is_empty(),
        "and the coincidence is the flat limit, not an accident of arity: \
         {exported:?}"
    );
}

/// **What the compiler would publish for each host operation.**
///
/// `wit::host_signatures` renders every operation's WIT from the Pleris `fn`
/// that carries its `host` binding, through exactly the machinery an export
/// goes through. It is what **both** answers to the blocked question need — to
/// emit the WIT, or to compare against a published one — so it presumes
/// neither.
///
/// Printed side by side with the deployment's, because the pair is the whole
/// argument and reading them together is the fastest way to see it.
#[test]
fn the_compiler_can_render_every_host_operations_wit_from_its_declaration() {
    let sources = store_sources();
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);

    let rendered = wit::host_signatures(&refs, &ws);

    println!("\n=== what the compiler would publish ===");
    for (k, v) in &rendered {
        match v {
            Ok(text) => println!("{k}\n    {text}"),
            Err(e) => println!("{k}\n    REFUSED: {e}"),
        }
    }

    // The application's own operations all render.
    for op in [
        "store:data/carts#add",
        "store:data/carts#clear",
        "store:data/carts#current",
        "store:data/stores#get",
        "store:data/menus#for-store",
    ] {
        assert!(
            matches!(rendered.get(op), Some(Ok(_))),
            "{op}: {:?}",
            rendered.get(op)
        );
    }

    // And it is the CONTRACT's ABI, not the deployment's: a `result<…>` where
    // the published stand-in says `string`. The disagreement as the two texts
    // rather than as two type names.
    let add = rendered["store:data/carts#add"].as_ref().expect("renders");
    assert!(
        add.contains("result<domain-cart, domain-cart-error>"),
        "the compiler's rendering of `carts#add` returns the declared result: {add}"
    );
    // And it is what the emitted package publishes, verbatim — one authority,
    // so the rendering and the artifact cannot drift.
    let (text, _) = generated();
    assert!(
        text.contains(add.as_str()),
        "the emitted WIT contains the rendered signature: {add}"
    );
}

/// **A privacy qualifier is transparent on the ABI, and the semantic contract
/// keeps what WIT never sees.**
///
/// Architect ruling, 2026-08-20, on the finding that `Session<SessionId>` had
/// no WIT form:
///
/// > Privacy qualification is semantic metadata; it need not have an
/// > independent runtime representation.
/// >
/// > ```text
/// > AbiRepresentation(Session<T>) = Transparent(AbiRepresentation(T))
/// > ```
///
/// The mistake was never that a label crosses — `current_session()` is exactly
/// the operation we need — it was that a *fixture* silently decided
/// `Session<SessionId> ≈ string` without the language saying so.
///
/// So the rule is authored once, in the one place that says what a privacy
/// qualifier is, and the restriction survives where it can actually be checked:
///
/// ```text
/// semantic signature    () -> Session<SessionId>     the contract keeps this
/// component signature   () -> domain-session-id      WIT sees only this
/// ```
///
/// WIT could not prove `Session<A> → Session<B>` anyway; that stays Pleris
/// contract semantics, exactly as capabilities stay outside core Wasm types.
#[test]
fn a_privacy_qualifier_is_transparent_to_the_type_it_qualifies() {
    let sources = store_sources();
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);

    let rendered = wit::host_signatures(&refs, &ws);
    let read = rendered["pw:host/session#read"]
        .as_ref()
        .unwrap_or_else(|e| panic!("`Session<SessionId>` must now render: {e}"));

    // Transparent: the qualifier is gone and the qualified type is what crosses.
    assert!(
        read.contains("session-id"),
        "the ABI is the representation of `SessionId`: {read}"
    );
    assert!(
        !read.contains("session<") && !read.to_lowercase().contains("-session:"),
        "and the qualifier itself has no WIT form: {read}"
    );

    // **The semantic contract still remembers the restriction.** This is the
    // half that makes the erasure honest rather than lossy: the label is not
    // discarded, it is kept where it can be checked.
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let semantic = cs
        .iter()
        .flat_map(|c| &c.imports)
        .find(|i| i.key() == "pw:host/session#read")
        .and_then(|i| i.signature.as_ref())
        .expect("the contract carries its semantic signature");
    assert_eq!(
        semantic.result, "Session<SessionId>",
        "the restriction survives in the semantic signature, which is what \
         `Session<A> -> Session<B>` is decided against"
    );
}

/// **The crucial restriction: opacity is not ABI transparency.**
///
/// > Do **not** establish `opaque type X = String → ABI(X) = string` for every
/// > opaque type. Opacity and ABI transparency are different facts.
///
/// So the rule above is about **privacy qualifiers** and nothing else. A
/// generic opaque type that is not one still has no WIT form, and refusing it
/// is the honest answer — the alternative is the plausible guess that
/// `docs/RISK_QUEUE.md` records as the shape this project keeps deleting.
///
/// Without this control, the arm in `wit_type` could be widened to "any generic
/// nominal is its argument's representation" and every test above would still
/// pass.
#[test]
fn a_generic_opaque_type_that_is_not_a_qualifier_still_has_no_wit_form() {
    let src = "\
module shop.opaque

opaque type Boxed<T> = String

fn fetch(id: Boxed<StoreId>) -> Boxed<StoreId>
    host \"store:data/boxes#get\"

type StoreId = StoreId { v: Int }
";
    let hir = lower_file(src, &parse_tree(src).green);
    let refs: Vec<&Hir> = vec![&hir];
    let ws = Workspace::build(&refs);

    let rendered = wit::host_signatures(&refs, &ws);
    let got = rendered
        .get("store:data/boxes#get")
        .expect("the operation is declared");
    assert!(
        matches!(got, Err(pw_core::wit::WitError::Unmappable { .. })),
        "`Boxed<StoreId>` is opaque, not privacy-qualified, so it has no ABI \
         until something says what it is: {got:?}"
    );
}

/// **PINS A FINDING: call sites are not type-checked at all.**
///
/// Found while executing the architect's step 2 — *make opaque/nominal
/// compatibility use resolved `DefId`, not representation* — which presupposes
/// a compatibility check to repair. There is none.
///
/// It is not about opaque types, and not about nominal identity. **No call
/// site is checked**: not the argument types, not the arity, not the result.
///
/// ```text
/// fn takes_str(s: String) -> Int      takes_str(42)            accepted
/// fn wrong_return() -> String { 42 }                           accepted
/// fn wrong_arity(a: Int, b: Int)      wrong_arity(1)           accepted
/// fn takes_store(s: Store)            takes_store(makes_cart())  accepted
/// ```
///
/// The last one is the decisive case: two `type` declarations in one module,
/// both resolvable, one returned from a call and passed where the other is
/// expected. Nothing resolves ambiguously and nothing is missing — the check
/// does not exist.
///
/// # What DOES get checked
///
/// The type machinery is real and is applied to specific relations, each with
/// its own controls: `{#each}` capture element types, handler signatures
/// (`PW0602`), an optimistic transition's target and value (`PW0331`), match
/// exhaustiveness, effect rows, and privacy labels. So this is not "types are
/// unimplemented" — it is that ordinary application of a function is not among
/// the relations checked.
///
/// # Why that matters here
///
/// `docs/MILESTONES.md` records **E9 — permanent value type checker** as
/// COMPLETE, and charter §14 M9A lists `unification-based inference` and
/// `opaque nominal types` among its contents. A value type checker that accepts
/// `takes_store(makes_cart())` is not one, so the milestone's headline claim
/// has no witness — the same claim-versus-witness gap the evidence-reachability
/// audit found in fixtures, one level up, at a gate.
///
/// It also blocks the ABI work in a specific way: the architect's invariant is
/// that the semantic signature is the most precise representation in the chain.
/// It cannot be, while the layer that would establish precision never runs.
#[test]
fn a_call_site_is_not_type_checked() {
    // Two declared records, both resolvable, in one module: the case with no
    // alternative explanation.
    let src = "\
module r

type Store = Store { id: Int }
type Cart = Cart { n: Int }

fn takes_store(s: Store) -> Int { 0 }

fn makes_cart() -> Cart { Cart(1) }

fn breaks() -> Int {
    takes_store(makes_cart())
}
";
    let codes: Vec<String> =
        pw_core::check::check_sources(&[("r.pw".to_string(), src.to_string())])
            .into_iter()
            .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
            .collect();
    assert!(
        codes.is_empty(),
        "PINNED: a call site is now checked. Delete this test and record the \
         rule — and check whether E9's gate needs re-evidencing. Got {codes:?}"
    );

    // And the premise: the two types ARE distinct declarations, so this is not
    // measuring a resolver that merged them.
    let hir = lower_file(src, &parse_tree(src).green);
    let names: Vec<String> = hir
        .all_decls()
        .filter(|(_, d)| d.kind == pw_core::hir::DeclKind::Type)
        .map(|(_, d)| d.name.clone())
        .collect();
    assert_eq!(names, ["Store", "Cart"], "{names:?}");
}
