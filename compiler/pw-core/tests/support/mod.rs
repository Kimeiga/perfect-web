//! **The effect vocabulary a hand-written test program has to declare.**
//!
//! Since `World::worlds_for` was deleted on 2026-08-07, an effect a program
//! does not declare has no placement and no capability — it is `Blocked`, and
//! the compiler says so rather than answering from a table it once carried.
//! Architect ruling:
//!
//! > if unresolved: PW5201 / Blocked, placement does not continue […] A pure
//! > standalone file can still check without a web platform package. But if it
//! > writes `!{ database.read<Stores> }` without a selected/imported platform
//! > environment that declares `database.read`, the compiler should say,
//! > effectively: `database.read` has no meaning in this program.
//!
//! Tests that build a program from string literals are exactly that case. This
//! is the vocabulary they select, in one copy rather than one per file.
//!
//! # Why not the real platform packages
//!
//! `tests/platform_contracts.rs` and `tests/checking_source.rs` do use them,
//! because what those tests are ABOUT is the platform and the corpus. These
//! fixtures are about a mechanism — how a contract is shaped, how an effect
//! crosses a module — and reading four hundred lines of `pw-platform-web` to
//! ask whether a capability keeps its type argument would make the fixture
//! depend on every future edit to the platform's vocabulary.
//!
//! Kept deliberately in the platform's own words: same clauses, same arities.
//! `tests/effect_vocabulary.rs` is what holds the platform to its declarations.

/// A module declaring the effects the hand-written fixtures perform.
///
/// `prelude Effect` so a fixture writes `!{ database.read<Stores> }` without
/// importing anything — which is how a real program reaches a platform's
/// vocabulary, and therefore what these fixtures should be exercising.
pub const VOCABULARY: &str = "\
module test.effects

prelude Effect

effect database.read<T> {
    placement  origin
    capability database.read<T>
    host       \"pw:host/database#read\"
}

effect database.write<T> {
    placement  origin
    capability database.write<T>
    host       \"pw:host/database#write\"
}

effect secret<C> {
    placement  origin
    capability secret<C>
    host       \"pw:host/secrets#get\"
}

effect secret.read {
    placement  origin
    capability secret.read
    host       \"pw:host/secrets#read\"
}

effect device.location {
    placement  browser
    capability device.location
    host       \"pw:host/device#location\"
}

effect dom.mutate {
    impact     dom_write
    impact     layout_write
    placement  browser
    capability none
}

effect trace {
    capability none
}
";
