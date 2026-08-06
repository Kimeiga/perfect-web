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
//!
//! Nothing here depends on Koka, on an effect row, or on a backend
//! representation. That independence is the point.

pub mod abi;
pub mod diag;
pub mod exhaust;
pub mod types;

pub use abi::{Decoder, Mode, Value};
pub use exhaust::{Arm, MatchReport, Pattern, check_match, render_witness};
pub use types::{Adt, Ctor, Program, Type};
