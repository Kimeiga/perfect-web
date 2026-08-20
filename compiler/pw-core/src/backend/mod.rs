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
pub fn host_binding(decl: &crate::hir::Decl) -> Option<ir::ImportId> {
    let p = decl.policy("host")?;
    // `host "pw:host/carts#add"` — the same spelling an `effect` declaration
    // uses for the interface serving it, and parsed the same way.
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
