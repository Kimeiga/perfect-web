#![allow(dead_code)]
//! **Shared fixtures for the integration tests.**
//!
//! Dead code is allowed here for a structural reason, not a convenient one: an
//! integration test module is compiled separately into *every* binary that
//! writes `mod support;`, and each of them uses a subset. `VOCABULARY` is used
//! by a dozen; the deployment's WIT by two. Without this, adding a fixture that
//! two files need makes the other ten fail to build — which would push every
//! shared fixture back into per-file copies, and a stand-in for somebody else's
//! published signatures is exactly the artifact that must exist once.
//!
//! # The effect vocabulary a hand-written test program has to declare
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
}

effect database.write<T> {
    placement  origin
    capability database.write<T>
}

effect secret<C> {
    placement  origin
    capability secret<C>
}

effect secret.read {
    placement  origin
    capability secret.read
}

effect device.location {
    placement  browser
    capability device.location
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

// The affine pair. The type argument is the resource being acquired, so an
// acquire of one kind and a release of another do not cancel — and so that a
// type IS a resource because a declaration says `resource.acquire<T>` of it,
// which is what makes an unserializable capture a declared fact rather than a
// list in the compiler.
effect resource.acquire<R> {
    capability resource.acquire<R>
}

effect resource.release<R> {
    capability resource.release<R>
}
";

// --- the WIT directory ------------------------------------------------------

/// Lay out a WIT directory the way `push_dir` expects.
///
/// **One file, and no `deps/`.** It wrote hand-authored `pw:host` and
/// `store:data` packages until 2026-08-20, when the architect ruled that the
/// Pleris declaration is the ABI authority for every operation this program
/// declares:
///
/// > The deployment implements the emitted interface. It does not
/// > independently specify what that interface means.
///
/// So the generator emits those packages itself, nested in the same file, and a
/// stand-in beside them would be the second authority the ruling deleted — the
/// resolver says so directly: *package `pw:host` is defined in two different
/// locations*.
///
/// A deployment still writes an implementation by hand. What it no longer
/// writes is the signature.
pub fn wit_dir(name: &str, app: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("app.wit"), app).expect("write");
    dir
}
