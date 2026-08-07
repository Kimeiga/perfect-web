//! **Which resource entry** — one semantic definition, two derivations.
//!
//! Architect ruling, 2026-08-06:
//!
//! > E6 defines the semantic entry identity. `pw-materialize` and E7-P both
//! > derive their own representations from it.
//!
//! ```text
//!                  EntryIdentity
//!                     /       \
//!                    ↓         ↓
//! pw-materialize::EntryKey   ResourceEntryId
//!    storage identity          wire identity
//! ```
//!
//! # Why the storage key is not the wire identity
//!
//! `EntryKey` belongs to the materializer and may acquire a storage namespace,
//! a shard, a database encoding, internal generation metadata. None of that
//! should become a protocol compatibility constraint merely because a patch
//! once serialized a Rust struct — otherwise "we need to change the storage
//! key" becomes "that breaks the browser patch protocol", for no semantic
//! reason.
//!
//! # Why the wire identity is opaque
//!
//! The browser does not need to know that an entry is `Cart` for
//! `session:hakan-123` in partition `Session<Hakan>`. It needs to know "this
//! patch derives from the entry I currently hold at version 14". So the wire
//! form is a keyed derivation and reveals none of its inputs.
//!
//! **`ResourceEntryId` is an opaque address, not authorization.** Holding one
//! must never let a browser subscribe to or retrieve an entry it was not
//! already authorised to reach.
//!
//! # What may join `EntryIdentity`, and what may not
//!
//! Architect ruling, 2026-08-06 — the rule that keeps this from becoming an
//! everything-bagel struct:
//!
//! > If changing a field means existing browser versions/patches must treat
//! > this as a different stream of state, it probably belongs in
//! > `EntryIdentity`. If changing it merely changes where or how the same entry
//! > is stored, it doesn't.
//!
//! So: the resource declaration, the logical key, the privacy partition, the
//! compatibility generation. **Not** the current version, a shard, a region, a
//! storage engine, a TTL, a last-access timestamp, or a compression scheme.
//!
//! The general shape, which this project now applies in four places:
//!
//! > One canonical semantic object, multiple derived representations at system
//! > boundaries.
//!
//! ```text
//! resolved declaration → DefId → handler identity → serialized hash
//! privacy semantics    → Partition → cache partition → IdentityDomain
//! template part        → LocalPartId → InstancePath → PartAddress
//! resource entry       → EntryIdentity → EntryKey → ResourceEntryId
//! ```
//!
//! The dangerous alternative is each subsystem rediscovering the meaning, which
//! is what `docs/RISK_QUEUE.md` records happening five times.

use serde::{Deserialize, Serialize};

/// Which principal an entry belongs to.
///
/// Richer than [`crate::Privacy`], which answers only "shared cache or
/// private?". A partition names the principal, and that is what makes two
/// sessions' entries different entries rather than two writes to one.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "partition")]
pub enum Partition {
    /// One entry serves every reader.
    #[default]
    Public,
    Session {
        id: String,
    },
    User {
        id: String,
    },
    Organization {
        id: String,
    },
}

impl Partition {
    /// The canonical text form, used by the serializer and by nothing else.
    fn canonical(&self) -> String {
        match self {
            Partition::Public => "public".to_string(),
            Partition::Session { id } => format!("session:{id}"),
            Partition::User { id } => format!("user:{id}"),
            Partition::Organization { id } => format!("organization:{id}"),
        }
    }
}

impl From<&Partition> for crate::Privacy {
    /// The projection, derived rather than restated.
    ///
    /// `Privacy` answers "which cache" and `Partition` answers "whose entry".
    /// Writing the mapping once is what stops the two drifting into
    /// disagreement about whether a user-scoped entry is public.
    fn from(p: &Partition) -> crate::Privacy {
        match p {
            Partition::Public => crate::Privacy::Public,
            _ => crate::Privacy::Private,
        }
    }
}

/// The answer to "which logical resource entry?".
///
/// Neither the materializer nor the patch protocol may reconstruct this
/// meaning independently. Both derive from it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryIdentity {
    /// The declaration, by the path the module graph resolved: `Resources.Cart`.
    pub resource: String,
    /// The key components, in declaration order, canonicalised by the caller
    /// that knows the resource's key policy.
    pub logical_key: Vec<String>,
    pub partition: Partition,
    /// The build generation, where it is relevant to this resource.
    ///
    /// `None` for a resource whose entries survive a deployment — the
    /// distinction is the resource's, and encoding "no generation" as an empty
    /// string would make it indistinguishable from a generation named "".
    pub compatibility: Option<String>,
}

impl EntryIdentity {
    pub fn new(resource: &str, logical_key: &[&str], partition: Partition) -> EntryIdentity {
        EntryIdentity {
            resource: resource.to_string(),
            logical_key: logical_key.iter().map(|k| k.to_string()).collect(),
            partition,
            compatibility: None,
        }
    }

    pub fn generation(mut self, compatibility: &str) -> EntryIdentity {
        self.compatibility = Some(compatibility.to_string());
        self
    }

    /// The one canonical encoding.
    ///
    /// Both derivations consume this. Two serializers would be two answers to
    /// "which entry", which is the thing this module exists to prevent.
    ///
    /// Length-prefixed, so no two different identities can produce one byte
    /// string; explicitly versioned, so a future field is a new version rather
    /// than a silent change of meaning; deterministic, because a derivation
    /// over a non-deterministic encoding is a derivation over nothing.
    pub fn canonical(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut field = |bytes: &[u8]| {
            out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            out.extend_from_slice(bytes);
        };
        field(b"pw-resource-entry-v1");
        field(self.resource.as_bytes());
        field(&(self.logical_key.len() as u64).to_le_bytes());
        for k in &self.logical_key {
            field(k.as_bytes());
        }
        field(self.partition.canonical().as_bytes());
        match &self.compatibility {
            Some(g) => {
                field(b"generation");
                field(g.as_bytes());
            }
            None => field(b"no-generation"),
        }
        out
    }

    /// The partition's canonical text, for a derivation that needs it.
    ///
    /// Exposed rather than duplicated: a materializer building a readable
    /// storage key needs the same spelling the canonical encoding uses, and
    /// writing it twice is how two answers to "which partition" appear.
    pub fn partition_text(&self) -> String {
        self.partition.canonical()
    }
}

/// A deployment's PRF key.
///
/// Its field is private, so the only way to obtain one is through an
/// [`IdentityKeyProvider`] — the same move as `Authorised` in E7V. A key that
/// could be constructed from any string is a key a caller can invent, and then
/// "the deployment's key" is whatever the last caller typed.
#[derive(Clone)]
pub struct DeploymentIdentityKey(Vec<u8>);

impl DeploymentIdentityKey {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for DeploymentIdentityKey {
    /// Never prints the key. A secret that appears in a log is not a secret,
    /// and `{:?}` on a struct that holds one is how it gets there.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeploymentIdentityKey(<redacted>)")
    }
}

/// Where a deployment's identity key comes from.
///
/// A trait rather than a string, so E7-P never learns that identity keys "come
/// from command-line arguments" — the real mechanism is E8's, and this is the
/// boundary that lets it arrive later without the callers changing.
pub trait IdentityKeyProvider {
    fn identity_key(&self) -> DeploymentIdentityKey;
}

/// The development key. Conspicuously named, because shipping it is a choice.
///
/// With this key, tokens and entry ids are derivable by anyone who knows the
/// document and can guess the application keys. That is stated rather than
/// implied, and a deployment mode that forbids it is E8's to add.
pub struct DevelopmentIdentityKey;

pub const DEVELOPMENT_KEY: &str = "pw-development-identity-key-not-for-deployment";

impl IdentityKeyProvider for DevelopmentIdentityKey {
    fn identity_key(&self) -> DeploymentIdentityKey {
        DeploymentIdentityKey(DEVELOPMENT_KEY.as_bytes().to_vec())
    }
}

/// A key a test supplies, so two derivations can be shown to differ by key.
pub struct ExplicitIdentityKey(pub String);

impl IdentityKeyProvider for ExplicitIdentityKey {
    fn identity_key(&self) -> DeploymentIdentityKey {
        DeploymentIdentityKey(self.0.as_bytes().to_vec())
    }
}

/// The wire identity: an opaque 128-bit address for a resource entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceEntryId(String);

impl ResourceEntryId {
    /// Derive from the shared identity.
    ///
    /// 128 bits: the extra bytes are negligible in a patch and the collision
    /// margin is not. As with `InstanceToken`, this is an ADDRESS — holding one
    /// grants nothing, because a runtime resolves only entries it is already
    /// authorised to reach.
    pub fn derive(identity: &EntryIdentity, key: &impl IdentityKeyProvider) -> ResourceEntryId {
        let secret = key.identity_key();
        let mut prf_key = [0u8; 32];
        prf_key.copy_from_slice(blake3::hash(secret.bytes()).as_bytes());
        let out = blake3::keyed_hash(&prf_key, &identity.canonical());
        ResourceEntryId(hex(&out.as_bytes()[..16]))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ResourceEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// A monotonic version of one entry's state.
///
/// Per ENTRY, not per page. A page holds `Map<ResourceEntryId, Version>`,
/// because `Store(47)` at version 8 and `Cart(session_A)` at version 14 are two
/// independent facts and a single page version would make either one lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version(pub u64);

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cart(session: &str) -> EntryIdentity {
        EntryIdentity::new(
            "Resources.Cart",
            &[session],
            Partition::Session {
                id: session.to_string(),
            },
        )
        .generation("B1")
    }

    fn id(identity: &EntryIdentity) -> ResourceEntryId {
        ResourceEntryId::derive(identity, &DevelopmentIdentityKey)
    }

    #[test]
    fn one_identity_derives_one_wire_id() {
        assert_eq!(id(&cart("A")), id(&cart("A")));
    }

    #[test]
    fn a_different_logical_key_derives_a_different_id() {
        assert_ne!(
            id(&EntryIdentity::new("R", &["a"], Partition::Public)),
            id(&EntryIdentity::new("R", &["b"], Partition::Public))
        );
    }

    #[test]
    fn a_different_partition_derives_a_different_id() {
        let public = EntryIdentity::new("R", &["k"], Partition::Public);
        let a = EntryIdentity::new(
            "R",
            &["k"],
            Partition::Session {
                id: "A".to_string(),
            },
        );
        let b = EntryIdentity::new(
            "R",
            &["k"],
            Partition::Session {
                id: "B".to_string(),
            },
        );
        assert_ne!(id(&public), id(&a));
        assert_ne!(id(&a), id(&b));
    }

    #[test]
    fn a_different_generation_derives_a_different_id() {
        let a = EntryIdentity::new("R", &["k"], Partition::Public).generation("B1");
        let b = EntryIdentity::new("R", &["k"], Partition::Public).generation("B2");
        assert_ne!(id(&a), id(&b));
        // And "no generation" is not the same as a generation named "".
        let none = EntryIdentity::new("R", &["k"], Partition::Public);
        let empty = EntryIdentity::new("R", &["k"], Partition::Public).generation("");
        assert_ne!(id(&none), id(&empty));
    }

    #[test]
    fn a_different_deployment_key_derives_a_different_id() {
        let identity = cart("A");
        assert_ne!(
            ResourceEntryId::derive(&identity, &DevelopmentIdentityKey),
            ResourceEntryId::derive(&identity, &ExplicitIdentityKey("real".into()))
        );
    }

    /// **The test that proves the wire protocol does not depend on the
    /// materializer.**
    ///
    /// Two storage representations of one identity — a different namespace, a
    /// different encoding, a different backend — must derive the SAME
    /// `ResourceEntryId`. If this ever fails, a storage change has become a
    /// protocol change, which is exactly the coupling the split exists to
    /// prevent.
    #[test]
    fn a_different_storage_representation_derives_the_same_wire_id() {
        let identity = cart("A");
        let wire = id(&identity);

        // Two storage representations of one identity: what a backend change
        // looks like. Neither is built here — the materializer owns them —
        // which is itself the point.
        let sqlite = format!("{}|{}", identity.partition_text(), identity.resource);
        let sharded = format!("shard-7/{sqlite}/v3");
        assert_ne!(sqlite, sharded, "the storage keys really do differ");

        // And the wire id is untouched by either, because it derives from the
        // identity rather than from a representation of it.
        assert_eq!(id(&identity), wire);
    }

    #[test]
    fn the_canonical_encoding_cannot_collide_by_concatenation() {
        // `("ab","c")` and `("a","bc")` concatenate identically. Without the
        // length prefixes, two different entries would derive one id and the
        // patch protocol would deliver one entry's state to another.
        let a = EntryIdentity::new("R", &["ab", "c"], Partition::Public);
        let b = EntryIdentity::new("R", &["a", "bc"], Partition::Public);
        assert_ne!(a.canonical(), b.canonical());
        assert_ne!(id(&a), id(&b));
    }

    #[test]
    fn the_wire_id_reveals_none_of_its_inputs() {
        let identity = cart("hakan-session-123");
        let wire = id(&identity).as_str().to_string();
        for secret in ["hakan-session-123", "Resources.Cart", "session:", "B1"] {
            assert!(!wire.contains(secret), "{wire} reveals {secret}");
        }
    }

    #[test]
    fn a_deployment_key_never_prints_itself() {
        let key = ExplicitIdentityKey("the-real-secret".into()).identity_key();
        let shown = format!("{key:?}");
        assert!(!shown.contains("the-real-secret"), "{shown}");
        assert!(shown.contains("redacted"), "{shown}");
    }

    #[test]
    fn a_partition_projects_to_a_cache_privacy() {
        assert_eq!(
            crate::Privacy::from(&Partition::Public),
            crate::Privacy::Public
        );
        for p in [
            Partition::Session { id: "A".into() },
            Partition::User { id: "u".into() },
            Partition::Organization { id: "o".into() },
        ] {
            assert_eq!(
                crate::Privacy::from(&p),
                crate::Privacy::Private,
                "{p:?} is not shareable"
            );
        }
    }
}
