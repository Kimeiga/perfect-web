//! `pw-core` — E1A: the value semantics and boundary ABI the language owns.
//!
//! E0 measured that Koka cannot supply three things the charter requires
//! (ADR-0011):
//!
//! | requirement | what Koka does | what `pw-core` does |
//! |---|---|---|
//! | exhaustive matching (charter §7.1) | enforced only when the effect row excludes `exn` | [`exhaust`] — effect-independent, with named missing cases |
//! | nominal opaque types (§7.1) | erases single-field value structs | [`types::Opaque`] — a distinct type and a distinct ABI slot |
//! | `Option`/`List` distinction (§7.10) | both are `null`, runtime-ambiguous | [`abi`] — decoding is type-directed, never shape-directed |
//! | task-scope non-escape (§7.6) | not modelled at all | [`scope`] — E2A-S, the static half of structured concurrency |
//! | artifact capability compliance (§10.2) | n/a — a Wasm property | [`capability`] — declared vs actually-imported, checked on the built artifact |
//!
//! Every detector emits the single [`diagnostics::Diagnostic`] type, and the
//! public code identifies the violated **invariant** rather than the analysis
//! that found it. Rendering belongs to callers: no semantic rule lives in the
//! CLI.
//!
//! Nothing here depends on Koka, on an effect row, or on a backend
//! representation. That independence is the point.

pub mod abi;
pub mod capability;
pub mod check;
pub mod diag;
pub mod diagnostics;
pub mod exhaust;
pub mod hir;
pub mod koka;
pub mod lower;
pub mod marko;
pub mod rules;
pub mod scope;
pub mod types;

pub use abi::{Decoder, Mode, Value};
pub use capability::{CapabilityAudit, CapabilityManifest, RuntimeProfile, audit};
pub use diagnostics::{Detector, Diagnostic, Severity, canonical_code};
pub use exhaust::{Arm, MatchReport, Pattern, check_match, render_witness};
pub use scope::{HandleKind, Op, ScopeGraph, ScopeKind, ScopeViolation};
pub use types::{Adt, Ctor, Program, Type};
