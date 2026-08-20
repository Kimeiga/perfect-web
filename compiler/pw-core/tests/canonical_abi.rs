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
//! # The finding this file exists to hold
//!
//! A host operation's ABI is derived **twice**, by two things that never meet:
//!
//! ```text
//! the contract      from the Pleris `fn` carrying the `host` binding
//! the deployment    from the WIT package it publishes
//! ```
//!
//! and *nothing compares them*. `wit_parser`'s resolve checks that the
//! interface exists and that every type a world names is declared. It does not
//! check that `carts#add` has the type the contract fixed, because the contract
//! is not an input to it.
//!
//! The two disagree today, in this repo, on every operation the store calls —
//! see `the_contracts_abi_and_the_deployments_wit_are_not_compared`. Until
//! 2026-08-20 there was nothing to compare: the contract gained `signature`
//! that day. So this is not a regression. It is a check that became possible
//! and does not exist, and the Canonical ABI is where it stops being harmless,
//! because the two flatten to different core signatures and the adapters must
//! lower to exactly one of them.

use wit_parser::abi::AbiVariant;

use pw_core::contract::{ComponentContract, ImportKind, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_core::wit;
use pw_syntax::parse_tree;

mod support;

/// The store demo's contracts and generated WIT.
fn generated() -> (String, Vec<ComponentContract>) {
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

    // The encoder's own answer for the same operation, for the record: three
    // parameters and one result, because `repr` gives each value one slot.
    // Neither number is wrong — they answer different questions — and step 4 is
    // making the module speak the second.
    assert_eq!(
        (3, 1),
        (3, 1),
        "documented rather than computed: see tests/wasm_encoding.rs"
    );
}

/// **The two derivations flatten to the SAME core signature, and that is what
/// makes the disagreement invisible.**
///
/// The comparison needs no hand transcription, because the compiler already
/// renders this exact signature — for an EXPORT. `store:data/carts#current` is
/// `(SessionId) -> Result<Cart, CartError>` in the contract, and the generated
/// package declares
///
/// ```wit
/// cart: func(arg0: domain-session-id) -> result<domain-cart, domain-cart-error>;
/// ```
///
/// the same signature through the same type mapping. So flattening
/// `resources-cart-api#cart` gives the core signature the contract's ABI
/// implies, and flattening `carts#current` gives the deployment's — both by
/// `wit_parser`, neither by anything written here.
///
/// ```text
/// contract    (SessionId) -> Result<Cart, CartError>   [Pointer, Length, Pointer] retptr
/// deployment  (string) -> string                       [Pointer, Length, Pointer] retptr
/// ```
///
/// Identical. `SessionId` is a `string` alias, and a `result<record, variant>`
/// and a `string` both exceed the flat limit and so both return through a
/// pointer. Every one of the six coincides this way.
///
/// I expected them to differ and asserted so; they do not, and the correction
/// is the finding. **No core-level check can catch this** — not the validator,
/// not the import types, not a signature comparison in the encoder — because at
/// the core level there is nothing to catch. The disagreement lives entirely in
/// the COMPONENT types, where the host lifts three `Pointer`s as a `string`
/// while the guest meant a `result<cart, cart-error>`: the same bytes read as
/// different shapes, with no trap and no diagnostic.
///
/// That is why the check has to compare component types, and why it cannot be
/// deferred to "the validator will notice".
#[test]
fn the_two_derivations_coincide_at_the_core_and_differ_above_it() {
    let resolve = resolved("pw-canonical-abi-distance");
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

    // **The core level: indistinguishable.**
    //
    // One pair, because `resources-cart-api#cart` is the only export whose
    // signature is the same as a host operation's — `(SessionId) ->
    // Result<Cart, CartError>`, which `Resources.cart` and `Carts.current`
    // share. `store-page-add-to-cart-api#add-to-cart` is NOT a second pair for
    // `carts#add`: the page takes `(MenuItemId, PositiveInt)` and reads the
    // session from context, so it flattens to four core parameters against the
    // operation's six. Comparing them would compare two different signatures
    // and prove nothing.
    let a = by("resources-cart-api#cart");
    let b = by("carts#current");
    assert_eq!(
        (&a.params, &a.results, a.retptr),
        (&b.params, &b.results, b.retptr),
        "these flatten differently, which would mean a core-level check could \
         catch the disagreement after all"
    );

    // The component level: different types entirely. Compared by NAME of the
    // result type, which is enough — one is a `result<…>` the package declares
    // and the other is the builtin `string`.
    let result_of = |key: &str| -> String {
        for (_, iface) in resolve.interfaces.iter() {
            let Some(name) = iface.name.as_deref() else {
                continue;
            };
            for (fname, func) in iface.functions.iter() {
                if format!("{name}#{fname}") == key {
                    return match func.result {
                        None => "()".to_string(),
                        Some(wit_parser::Type::String) => "string".to_string(),
                        Some(wit_parser::Type::Id(id)) => resolve.types[id]
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("{:?}", resolve.types[id].kind)),
                        Some(other) => format!("{other:?}"),
                    };
                }
            }
        }
        panic!("no operation {key}")
    };

    assert_eq!(result_of("carts#current"), "string");
    assert_ne!(
        result_of("resources-cart-api#cart"),
        "string",
        "the contract's result is a `result<cart, cart-error>`, and the \
         deployment publishes `string` for the same operation"
    );
}

/// **PINS A KNOWN DEFECT. The repair turns this red, and that is correct.**
///
/// The contract fixes an ABI for every host operation; the deployment publishes
/// one; they disagree on all six, and no check in this repo compares them.
///
/// ```text
/// operation                    the contract fixes            the deployment publishes
/// store:data/carts#add         (SessionId, MenuItemId,       (string, string, s64)
///                               PositiveInt)                  -> string
///                               -> Result<Cart, CartError>
/// pw:host/session#read         () -> Session<SessionId>      () -> string
/// ```
///
/// Every gate stays green: `wit_parser` resolves the worlds, the artifact audit
/// is satisfied because the NAMES match, and `contract::consistent` is
/// satisfied because the capabilities are untouched. It is the exact shape
/// `contract::abi` was written to refuse — *same `interface#operation`, wrong
/// ABI* — one layer further out, where the second signature is somebody else's
/// artifact rather than a mutation of ours.
///
/// It is also the shape `docs/RISK_QUEUE.md` records twice already: **one fact
/// derived twice, agreeing until an input mattered.** Here the input already
/// mattered.
///
/// Which derivation is authoritative is a model question, not a bug to patch:
/// for `store:data/*` the application declared the operation and arguably the
/// compiler should EMIT that WIT; for `pw:host/*` the platform publishes it and
/// the Pleris declaration is a claim to be CHECKED. That the answer differs by
/// `Import::owner` is why the ownership distinction had to come first.
#[test]
fn the_contracts_abi_and_the_deployments_wit_are_not_compared() {
    let (_, cs) = generated();
    let resolve = resolved("pw-canonical-abi-disagreement");

    // What the deployment publishes, by `interface#name` with the interface
    // unqualified — which is how `flattened` keys it.
    let mut published: Vec<String> = Vec::new();
    for (_, iface) in resolve.interfaces.iter() {
        let Some(name) = iface.name.as_deref() else {
            continue;
        };
        for fname in iface.functions.keys() {
            published.push(format!("{name}#{fname}"));
        }
    }

    let mut disagree = Vec::new();
    let mut compared = 0;
    for c in &cs {
        for i in &c.imports {
            if i.kind != ImportKind::HostCapability {
                continue;
            }
            let Some(sig) = &i.signature else { continue };
            let short = format!(
                "{}#{}",
                i.interface.rsplit('/').next().unwrap_or(&i.interface),
                i.name
            );
            if !published.contains(&short) {
                continue;
            }
            compared += 1;
            // The deployment's stand-in returns `string` from every operation.
            // The contract fixes a `Result` or a nominal type for every one, and
            // no written Pleris type maps to WIT `string` except `String`.
            if sig.result != "String" && sig.result != "Str" {
                disagree.push(format!("{short}: contract result is `{}`", sig.result));
            }
        }
    }

    assert!(compared >= 5, "only {compared} operations examined");
    disagree.sort();
    disagree.dedup();
    assert_eq!(
        disagree.len(),
        6,
        "PINNED: every host operation's contract ABI disagrees with the \
         deployment's published one, and nothing refuses it. If this count \
         changed, either the disagreement was repaired — delete this test and \
         write the check — or a new operation joined it. {disagree:?}"
    );
}
