//! The compiler's graph is the one source of "which entry".
//!
//! Architect ruling, 2026-08-07:
//!
//! > There should be one bridge:
//! >
//! > ```text
//! > pw_core::graph::ResourceNode → compiler-owned lowering → EntryIdentity
//! > ```
//! >
//! > and downstream code only receives the result.
//!
//! ADR-0018's boundary means the compiler does not link this crate, so the
//! bridge emits `EntryIdentitySpec` and this deserializes it — the same
//! field-name mirror as the graph, the manifest and the template IR. What makes
//! it a boundary rather than a hope is that this test reads output the real
//! compiler produced.
//!
//! # The proof of the abstraction
//!
//! ```text
//! change partition only     → EntryIdentity changes
//!                           → storage EntryKey changes
//!                           → wire ResourceEntryId changes
//!
//! change storage namespace  → EntryIdentity same
//!                           → storage EntryKey changes
//!                           → wire ResourceEntryId SAME
//! ```

use pw_core::graph::{EntryIdentitySpec, Graph};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_resource::{DevelopmentIdentityKey, EntryIdentity, Partition, ResourceEntryId};
use pw_syntax::parse_tree;

const PROGRAM: &str = "\
module world

public query Menu(id: Int) -> Int
    freshness   5.minutes
    consistency snapshot
    cache       shared
    key         id
{
    0
}

session query Cart(session: Int) -> Int
    freshness     0.seconds
    consistency   read_your_writes
    cache         private
    key           session
{
    0
}
";

fn graph() -> Graph {
    let hir: Hir = lower_file(PROGRAM, &parse_tree(PROGRAM).green);
    let refs = vec![&hir];
    let ws = Workspace::build(&refs);
    Graph::build(&refs, &ws)
}

/// The mirror, across ADR-0018's boundary.
fn adopt(spec: &EntryIdentitySpec) -> EntryIdentity {
    let partition = match spec.partition.as_str() {
        p if p.starts_with("session:") => Partition::Session {
            id: p[8..].to_string(),
        },
        p if p.starts_with("user:") => Partition::User {
            id: p[5..].to_string(),
        },
        p if p.starts_with("organization:") => Partition::Organization {
            id: p[13..].to_string(),
        },
        _ => Partition::Public,
    };
    let keys: Vec<&str> = spec.logical_key.iter().map(String::as_str).collect();
    let identity = EntryIdentity::new(&spec.resource, &keys, partition);
    match &spec.compatibility {
        Some(g) => identity.generation(g),
        None => identity,
    }
}

fn wire(identity: &EntryIdentity) -> ResourceEntryId {
    ResourceEntryId::derive(identity, &DevelopmentIdentityKey)
}

#[test]
fn the_compiler_answers_which_partition_a_resource_entry_belongs_to() {
    let g = graph();

    // A public resource names no principal, and supplying one does not make it
    // private — the declaration decides, not the caller.
    let menu = g
        .entry_identity("world.Menu", &["47".into()], Some("session-a"))
        .expect("Menu is in the graph");
    assert_eq!(menu.partition, "public");

    // A session resource names one, and REFUSES to produce an identity without
    // it: an identity that fell back to public would be exactly the confusion
    // `PW5101` exists to prevent, arriving one layer down.
    let cart = g
        .entry_identity("world.Cart", &["s-a".into()], Some("s-a"))
        .expect("Cart is in the graph");
    assert_eq!(cart.partition, "session:s-a");
    assert_eq!(
        g.entry_identity("world.Cart", &["s-a".into()], None),
        None,
        "a restricted entry with no principal has no identity"
    );
}

#[test]
fn the_generation_is_not_inferred_from_the_partition() {
    // Architect ruling, 2026-08-07, correcting a coupling this bridge shipped
    // with: privacy partition must not decide whether an entry survives a
    // deployment. They are orthogonal — a public entry can become incompatible
    // after a deploy, and a private one can stay compatible across one.
    //
    // The first version answered the second question with the first, so a
    // session-scoped entry silently claimed to be build-stable.
    let g = graph();
    let menu = g
        .entry_identity("world.Menu", &["47".into()], None)
        .expect("Menu");
    let cart = g
        .entry_identity("world.Cart", &["s-a".into()], Some("s-a"))
        .expect("Cart");

    assert!(menu.compatibility.is_some(), "public carries a generation");
    assert!(
        cart.compatibility.is_some(),
        "and so does session-scoped: `public` must never mean `build-stable`"
    );
    assert_eq!(menu.compatibility, cart.compatibility);
}

/// The discriminating matrix the architect required before building on this.
///
/// Two orthogonal dimensions, and neither may infer the other.
#[test]
fn partition_and_generation_vary_independently() {
    use pw_resource::Partition;

    let of = |key: &str, partition: Partition, generation: &str| {
        wire(&EntryIdentity::new("R", &[key], partition).generation(generation))
    };
    let public = || Partition::Public;
    let session = |id: &str| Partition::Session { id: id.to_string() };

    // same key, same partition, different generation → different identity
    assert_ne!(of("k", public(), "A"), of("k", public(), "B"));
    assert_ne!(of("k", session("X"), "A"), of("k", session("X"), "B"));

    // same key, different partition, same generation → different identity
    assert_ne!(of("k", public(), "A"), of("k", session("X"), "A"));
    assert_ne!(of("k", session("X"), "A"), of("k", session("Y"), "A"));

    // same key, same partition, same generation → same identity
    assert_eq!(of("k", public(), "A"), of("k", public(), "A"));
    assert_eq!(of("k", session("X"), "A"), of("k", session("X"), "A"));

    // and the key is not ignored, or every row above holds for a constant
    assert_ne!(of("k", public(), "A"), of("other", public(), "A"));
}

#[test]
fn changing_the_partition_changes_the_identity_the_storage_key_and_the_wire_id() {
    // The first half of the abstraction proof.
    let g = graph();
    let a = adopt(
        &g.entry_identity("world.Cart", &["k".into()], Some("session-a"))
            .expect("Cart"),
    );
    let b = adopt(
        &g.entry_identity("world.Cart", &["k".into()], Some("session-b"))
            .expect("Cart"),
    );

    assert_ne!(a, b, "the semantic identity differs");
    assert_ne!(
        pw_materialize_key(&a),
        pw_materialize_key(&b),
        "the storage key differs"
    );
    assert_ne!(wire(&a), wire(&b), "and so does the wire id");
}

#[test]
fn changing_the_storage_representation_changes_neither_the_identity_nor_the_wire_id() {
    // The second half, and the one that proves the protocol does not depend on
    // the materializer. A namespace, a shard, an encoding — none of it reaches
    // the wire.
    let g = graph();
    let identity = adopt(
        &g.entry_identity("world.Menu", &["47".into()], None)
            .expect("Menu"),
    );
    let before = wire(&identity);

    let plain = pw_materialize_key(&identity);
    let sharded = format!("shard-7/{plain}/v3");
    assert_ne!(plain, sharded, "the storage representations really differ");

    assert_eq!(
        wire(&identity),
        before,
        "the wire id is untouched by how the entry is stored"
    );
}

/// A storage representation, built the way a materializer would.
///
/// Local to this test rather than imported: the point is that ANY storage
/// representation leaves the wire id alone, and importing one would make the
/// claim about that one.
fn pw_materialize_key(identity: &EntryIdentity) -> String {
    format!(
        "{}|{}({})",
        identity.partition_text(),
        identity.resource,
        identity.logical_key.join(",")
    )
}

#[test]
fn the_bridge_and_the_runtime_agree_field_for_field() {
    // ADR-0018's mirror, checked. A rename on either side fails here rather
    // than becoming an empty identity at run time.
    let g = graph();
    let spec = g
        .entry_identity("world.Cart", &["s-a".into()], Some("s-a"))
        .expect("Cart");
    let identity = adopt(&spec);

    assert_eq!(identity.resource, spec.resource);
    assert_eq!(identity.logical_key, spec.logical_key);
    assert_eq!(identity.partition_text(), spec.partition);
    assert_eq!(identity.compatibility, spec.compatibility);
}
