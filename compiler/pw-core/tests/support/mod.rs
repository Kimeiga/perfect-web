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

// --- the deployment's WIT ----------------------------------------------------
//
// Moved here from `tests/wit_worlds.rs` on 2026-08-20, when a second test
// binary needed it. One copy: a stand-in for the deployment's own published
// signatures is exactly the artifact that must not exist twice, since two
// copies can disagree and each would look authoritative in its own file.

/// **The PLATFORM's WIT** — `pw:host`, which every Pleris deployment publishes.
///
/// Hand-written, and that is the point: these are the host's signatures and the
/// compiler has no business deciding them.
///
/// Rewritten 2026-08-20. It used to publish `database` with `read`/`write`,
/// because a component's imports were derived from its CAPABILITIES — and the
/// Wasm encoder proved that cannot work: `Carts.add(s, item, qty)` and
/// `Carts.clear(s)` both require `database.write<Carts>` and have different
/// ABIs, so one `write` could not have both signatures. A capability authorizes
/// an operation and does not identify one: `database.write<Carts>` is still the
/// authority a deployment grants; the operation is what it publishes.
pub const PLATFORM_WIT: &str = "\
package pw:host;

/// The invocation context. Added on 2026-08-10, when `examples/store/app.pw`
/// gained the `import context.{ current_session }` it had been missing since
/// E4 — so the store's worlds began importing `pw:host/session` and stopped
/// resolving against a host that does not publish it.
///
/// That is this stand-in doing its job. A capability a component requires and
/// a deployment does not grant is a deployment that cannot run it, and the WIT
/// resolve is where that becomes visible rather than a runtime link failure.
interface session {
    read: func() -> string;
}
";

/// **The APPLICATION's WIT** — `store:data`, which only the store's deployment
/// publishes.
///
/// A second package, and the split is the substance. Architect ruling,
/// 2026-08-20:
///
/// > `pw:host/carts#add` wrongly implies Pleris defines a universal carts API.
/// > […] Do **not** let `pw:host/carts` become the permanent standard-library
/// > design merely because it was the first thing that made the demo
/// > executable.
///
/// These operations were in `pw:host` until then, which said that a cart is a
/// Pleris platform facility. It is not: it is this application's data access,
/// externally implemented today. `Import::owner` records the same distinction
/// inside the contract, and `docs/NEXT.md` carries the follow-up — these may
/// become compiled Pleris over a narrower platform primitive.
///
/// The grouping into interfaces is an ABI/package-layout decision — `add` and
/// `clear` could equally live in one `carts` interface or two — and it does not
/// determine the capability semantics.
pub const APPLICATION_WIT: &str = "\
package store:data;

interface carts {
    current: func(session: string) -> string;
    add: func(session: string, item: string, quantity: s64) -> string;
    clear: func(session: string) -> string;
}

interface stores {
    get: func(id: string) -> string;
}

interface menus {
    %for-store: func(store: string) -> string;
}
";

/// Lay out a WIT directory the way `push_dir` expects: the package under test
/// at the root, and every package it depends on under `deps/`.
///
/// Both dependency packages, always. A deployment that publishes the platform's
/// operations and not the application's is one the store cannot run, and that
/// has to be visible here rather than at instantiation.
pub fn wit_dir(name: &str, app: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("deps/host")).expect("temp dir");
    std::fs::create_dir_all(dir.join("deps/store-data")).expect("temp dir");
    std::fs::write(dir.join("app.wit"), app).expect("write");
    std::fs::write(dir.join("deps/host/host.wit"), PLATFORM_WIT).expect("write");
    std::fs::write(dir.join("deps/store-data/store-data.wit"), APPLICATION_WIT).expect("write");
    dir
}
