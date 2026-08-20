//! **E10-A — the first backend slice.**
//!
//! Architect ruling, 2026-08-09:
//!
//! > **Narrow application surface, general backend spine.** Compile exactly one
//! > real Pleris command (`add_to_cart`) end-to-end first, but do not build any
//! > stage specifically around `add_to_cart`.
//!
//! ```text
//! checked Pleris HIR
//!       ↓
//! Backend IR                       <- ir.rs, general
//!       ↓
//! Wasm core module
//!       ↓
//! existing ComponentContract + generated WIT
//!       ↓
//! Wasm Component
//!       ↓
//! E8 host
//!       ↓
//! add_to_cart actually runs        <- E10-I
//! ```
//!
//! # What this must not do
//!
//! > The backend consumes semantics already established upstream. It must not
//! > rediscover effects, capabilities, placement, privacy, or ABI types.
//!
//! Every one of those has an owner: `effects.rs`, `ontology.rs`, `contract.rs`,
//! `placement.rs`, `boundary.rs`. A backend that re-derived any of them would
//! be a second answer to a question the project has already spent a milestone
//! making singular.
//!
//! # And no Rust in the middle
//!
//! > I would avoid `Pleris → Rust → Wasm` even temporarily. That would satisfy
//! > E10-I but leave the hardest backend questions outsourced to Rust,
//! > especially memory layout and ownership.
//!
//! `wasm-encoder` ENCODES the artifact; the lowering is ours. The distinction
//! is the same one ADR-0002 draws about Marko and ADR-0001 about Koka, and both
//! needed an explicit deletion condition to stay temporary. A Rust stage would
//! have needed one too, and would have been harder to tell apart from the real
//! backend afterwards.

pub mod ir;
pub mod lower;
pub mod wasm;

/// **Is this declaration's implementation supplied from outside, and by what?**
///
/// Architect ruling, 2026-08-20:
///
/// > Host implementation is explicit declaration metadata, never inferred from
/// > a missing/`todo` body or from the effect row.
///
/// So it is the `host` policy and nothing else. `todo` means *there is not an
/// implementation here*; it does not secretly mean *the platform supplies an
/// FFI implementation*, and the two are materially different semantics.
///
/// One reader, used by lowering and by the contract, because "which callables
/// does this component depend on" must have one answer all the way through:
/// contract → Backend IR → core Wasm imports → WIT → the E8 artifact audit.
///
/// # An `effect` declaration is not a callable
///
/// `effect session.read { host "pw:host/session#read" }` uses the same spelling,
/// and it means something else: it is the residue of deriving an operation from
/// a CAPABILITY, which the same ruling removed —
///
/// > A capability authorizes an operation. It does not identify the operation.
///
/// So an effect declaration is refused here. It matters because the effect and
/// the `fn` that performs it name the *same* operation today —
/// `pw:host/session#read` is claimed by both `effect session.read` and
/// `fn current_session()` — and a reader that accepted either would answer with
/// whichever it met last. It did: `wit::host_signatures` rendered the effect's
/// empty signature over the function's real one, and the wrong answer looked
/// like a right one.
///
/// The ontology reads that clause separately, and nothing consumes what it
/// reads. See `docs/RISK_QUEUE.md`.
pub fn host_binding(decl: &crate::hir::Decl) -> Option<ir::ImportId> {
    if decl.kind == crate::hir::DeclKind::Effect {
        return None;
    }
    let p = decl.policy("host")?;
    let raw = p.value.trim().trim_matches('"');
    let (interface, name) = raw.split_once('#')?;
    if interface.is_empty() || name.is_empty() {
        return None;
    }
    Some(ir::ImportId {
        interface: interface.to_string(),
        name: name.to_string(),
    })
}

/// **The WIT package the Pleris platform itself owns.**
///
/// Architect ruling, 2026-08-20:
///
/// > `pw:host/carts#add` looks like Pleris itself defines a universal carts
/// > API. It doesn't. […] Do not let `pw:host/carts` become the permanent
/// > standard-library design merely because it was the first thing that made
/// > the demo executable.
///
/// So ownership is read from the package the author WROTE, exactly as a WIT
/// package name is an ownership assertion everywhere else. `pw:host/session#read`
/// is a platform facility with platform ABI-stability expectations;
/// `store:data/carts#add` is the store's own data layer, externally implemented
/// today and eventually replaceable by compiled Pleris.
///
/// Not a spelling-based resolution: the package IS the identity, and this reads
/// it rather than guessing from a function's name.
pub const PLATFORM_PACKAGE: &str = "pw:host";

/// **One canonical callable definition, consumed by both the contract and the
/// backend.**
///
/// Architect ruling, 2026-08-20:
///
/// ```text
/// operation declaration
///        ↓
/// CallableImport
///       ↙   ↘
/// contract   backend
/// ```
///
/// > not: operation declaration → contract guess / backend guess. You've spent
/// > multiple milestones eliminating the second pattern.
///
/// It was the second pattern for one commit: `contract::host_calls` and
/// `backend::lower::host_imports` each built the facts from the declaration,
/// and they would have agreed until one of them learned something.
/// `ty` resolves a written type; `capability` says what authority an effect
/// requires, and returns `None` for one that requires none.
///
/// Both are passed in rather than decided here. Whether `dom.mutate` needs a
/// host capability is the ONTOLOGY's answer — `capability none` on the effect's
/// own declaration — and a second derivation in the backend is what put a
/// capability on a browser operation that needs none.
pub fn callable_of(
    decl: &crate::hir::Decl,
    callee: crate::resolve::DefId,
    ty: impl Fn(&str) -> Option<ir::Type>,
    capability: impl Fn(&str) -> Option<crate::contract::Capability>,
) -> Option<ir::CallableImport> {
    let id = host_binding(decl)?;
    let binding = match id.interface == PLATFORM_PACKAGE
        || id.interface.starts_with(&format!("{PLATFORM_PACKAGE}/"))
    {
        true => ir::ImportBinding::PlatformHost,
        false => ir::ImportBinding::External,
    };
    let mut params = Vec::new();
    for p in &decl.params {
        params.push(ty(&p.ty.as_ref()?.written())?);
    }
    let result = match &decl.ret {
        Some(head) => {
            ty(&crate::hir::DeclaredType::new(head.clone(), decl.ret_args.clone()).written())?
        }
        None => ir::Type::Unit,
    };
    Some(ir::CallableImport {
        id,
        callee,
        binding,
        signature: ir::BackendSignature { params, result },
        required_capabilities: decl
            .declared_effects
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|e| capability(&e.written).map(ir::CapabilityId))
            .collect(),
    })
}
