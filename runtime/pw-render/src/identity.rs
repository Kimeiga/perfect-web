//! Who shares an identity with whom.
//!
//! Architect ruling, 2026-08-06:
//!
//! > The identity domain should be whatever the materialization/privacy
//! > partition already separates. […] There should not be an independent E7
//! > concept of "which users should share this identity." E6 already answered
//! > that question.
//!
//! ```text
//! E6 privacy/cache partition
//!         ↓
//! E7 identity domain
//!         ↓
//! InstancePath → PartAddress
//! ```
//!
//! # Why this is a type
//!
//! It was a `String` the caller passed as `--document`, and nothing stopped a
//! caller passing a session identifier as the domain of a **public** fragment.
//! Two readers of one shared cache entry would then receive different bytes,
//! and either the cache is wrong or the fragment cannot be shared. The same
//! move as `Authorised` and the compatibility generation: make the invalid
//! composition unrepresentable rather than documented.
//!
//! # Shareability and addressability are independent
//!
//! A public materialization has ONE identity domain, so every reader of that
//! cached entry gets the same instance tokens. That is correct: the menu is
//! public, and correlating two readers of identical public bytes leaks nothing.
//!
//! Two session-scoped fragments have different physical identities and
//! therefore unrelated tokens, so identical application keys produce unrelated
//! markup.

use serde::{Deserialize, Serialize};

/// Which principal an entry belongs to.
///
/// Re-exported from `pw-resource`, not defined here. It WAS defined twice —
/// once for a resource entry and once for an identity domain — which is two
/// answers to "whose entry is this", and two answers to one question is what
/// `docs/RISK_QUEUE.md` records diverging five times.
///
/// E6 owns the concept; E7 reads it. `Partition::Public` is the default there,
/// and deliberately the *least* separating one: a caller that forgets gets a
/// domain shared by everyone, which is wrong for a private page in the
/// direction a test notices — two sessions rendering identical tokens — rather
/// than in the direction that silently breaks a cache.
pub use pw_resource::Partition;

/// The partition's canonical text, through the shared type so there is one
/// spelling. A `match` here would be a second definition wearing another name.
fn partition_text(p: &Partition) -> String {
    pw_resource::EntryIdentity::new("", &[], p.clone()).partition_text()
}

/// What a set of instance tokens is scoped to.
///
/// Two documents in the same domain derive the same token for the same key;
/// two documents in different domains derive unrelated ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityDomain {
    /// The materialization's physical key, or the document's logical key.
    pub key: String,
    pub partition: Partition,
    /// The build that produced the artifact — the same generation E6 injects.
    pub compatibility: String,
    /// The deployment's PRF key.
    ///
    /// Public by type and secret by deployment. With the default below the
    /// tokens are enumerable by anyone who knows the document and can guess the
    /// keys; with a deployment's own key they are not. The default exists so
    /// the renderer runs out of the box and it is named so that shipping it is
    /// a visible choice.
    #[serde(default = "default_identity_key")]
    pub identity_key: String,
}

fn default_identity_key() -> String {
    "pw-development-identity-key-not-for-deployment".to_string()
}

impl Default for IdentityDomain {
    fn default() -> Self {
        IdentityDomain {
            key: "document".to_string(),
            partition: Partition::Public,
            compatibility: "dev".to_string(),
            identity_key: default_identity_key(),
        }
    }
}

/// An opaque address for one instance of a repeatable scope.
///
/// **Never a capability.** Forging `<!--pw:s1@AAAA...-->` grants nothing: the
/// runtime resolves only tokens already present in the authorised document's
/// index, so an unknown token addresses no part rather than an unguarded one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceToken(String);

impl InstanceToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InstanceToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl IdentityDomain {
    /// The domain of a materialized fragment: its physical identity.
    pub fn materialization(key: &str, partition: Partition, compatibility: &str) -> IdentityDomain {
        IdentityDomain {
            key: key.to_string(),
            partition,
            compatibility: compatibility.to_string(),
            ..IdentityDomain::default()
        }
    }

    /// The domain of an ordinary page: its route identity and its partition.
    pub fn document(key: &str, partition: Partition, compatibility: &str) -> IdentityDomain {
        IdentityDomain::materialization(key, partition, compatibility)
    }

    /// Supply the deployment's PRF key.
    pub fn keyed(mut self, key: &str) -> IdentityDomain {
        self.identity_key = key.to_string();
        self
    }

    /// Derive the token for one instance.
    ///
    /// ```text
    /// token = PRF(identity_key,
    ///             domain ‖ parent InstancePath ‖ EachPartId ‖ key)[0..96 bits]
    /// ```
    ///
    /// BLAKE3's `keyed_hash` **is** the PRF — nothing here composes a
    /// construction by hand, which is the usual way a keyed hash becomes an
    /// unkeyed one.
    ///
    /// 96 bits, base64url, 16 characters. Not 256, because this is an address
    /// and not an authorization: what it must give is negligible accidental
    /// collision, no recovery of the raw key from markup, and no cheap
    /// dictionary test from the public document identifier alone. What it need
    /// not give is tamper resistance, because tampering addresses nothing.
    pub fn instance_token(
        &self,
        path: &[(u32, String)],
        each: crate::ir::PartId,
        key: &str,
    ) -> InstanceToken {
        // Length-prefixed, so no two different inputs can produce one byte
        // string. `("ab", "c")` and `("a", "bc")` concatenate identically and
        // must not derive one token.
        let mut input = Vec::new();
        let mut field = |bytes: &[u8]| {
            input.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            input.extend_from_slice(bytes);
        };
        field(self.key.as_bytes());
        field(partition_text(&self.partition).as_bytes());
        field(self.compatibility.as_bytes());
        field(&(path.len() as u64).to_le_bytes());
        for (part, token) in path {
            field(&part.to_le_bytes());
            field(token.as_bytes());
        }
        field(&each.0.to_le_bytes());
        field(key.as_bytes());

        let mut prf_key = [0u8; 32];
        let digest = blake3::hash(self.identity_key.as_bytes());
        prf_key.copy_from_slice(digest.as_bytes());
        let out = blake3::keyed_hash(&prf_key, &input);
        InstanceToken(base64url(&out.as_bytes()[..12]))
    }
}

/// 12 bytes as 16 base64url characters, no padding.
///
/// Written out rather than taken as a dependency: it is sixteen lines, and the
/// alphabet is the point — `+` and `/` would need escaping in the markup this
/// lands in, and `=` padding would be noise in every token.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..chunk.len() + 1 {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::PartId;

    fn domain() -> IdentityDomain {
        IdentityDomain::materialization("MenuFragment(47)", Partition::Public, "B1")
    }

    #[test]
    fn a_token_is_sixteen_url_safe_characters() {
        let t = domain().instance_token(&[], PartId(1), "espresso");
        assert_eq!(t.as_str().len(), 16, "96 bits, base64url: {t}");
        assert!(
            t.as_str()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "no character needing escape in markup: {t}"
        );
    }

    #[test]
    fn the_same_key_in_the_same_domain_derives_the_same_token() {
        // Determinism at the identity layer: two renders of one document must
        // agree, or an address means nothing across a reload.
        let d = domain();
        assert_eq!(
            d.instance_token(&[], PartId(1), "espresso"),
            d.instance_token(&[], PartId(1), "espresso")
        );
    }

    #[test]
    fn a_different_key_parent_or_loop_derives_a_different_token() {
        let d = domain();
        let base = d.instance_token(&[], PartId(1), "espresso");
        assert_ne!(base, d.instance_token(&[], PartId(1), "cortado"));
        assert_ne!(base, d.instance_token(&[], PartId(2), "espresso"));
        assert_ne!(
            base,
            d.instance_token(&[(9, "outer".into())], PartId(1), "espresso")
        );
    }

    #[test]
    fn two_privacy_partitions_derive_unrelated_tokens() {
        // The composition with E6: identical application keys in different
        // partitions must not correlate.
        let a = IdentityDomain::materialization(
            "CartFragment",
            Partition::Session { id: "A".into() },
            "B1",
        );
        let b = IdentityDomain::materialization(
            "CartFragment",
            Partition::Session { id: "B".into() },
            "B1",
        );
        assert_ne!(
            a.instance_token(&[], PartId(1), "line-1"),
            b.instance_token(&[], PartId(1), "line-1")
        );
    }

    #[test]
    fn one_public_entry_derives_one_set_of_tokens() {
        // The other half, and the one that makes a shared fragment shareable:
        // every reader of a public materialization gets the same bytes.
        let a = IdentityDomain::materialization("MenuFragment(47)", Partition::Public, "B1");
        let b = IdentityDomain::materialization("MenuFragment(47)", Partition::Public, "B1");
        assert_eq!(
            a.instance_token(&[], PartId(1), "espresso"),
            b.instance_token(&[], PartId(1), "espresso")
        );
    }

    #[test]
    fn a_different_compatibility_generation_derives_different_tokens() {
        let a = IdentityDomain::materialization("MenuFragment(47)", Partition::Public, "B1");
        let b = IdentityDomain::materialization("MenuFragment(47)", Partition::Public, "B2");
        assert_ne!(
            a.instance_token(&[], PartId(1), "espresso"),
            b.instance_token(&[], PartId(1), "espresso")
        );
    }

    #[test]
    fn the_deployment_key_changes_every_token() {
        // Without this, "keyed" is a word. With the default key the tokens are
        // enumerable by anyone who can guess the application keys; a deployment
        // supplying its own is what removes that, and it has to have an effect.
        let a = domain();
        let b = domain().keyed("a real deployment key");
        assert_ne!(
            a.instance_token(&[], PartId(1), "espresso"),
            b.instance_token(&[], PartId(1), "espresso")
        );
    }

    #[test]
    fn fields_are_length_prefixed_so_no_two_inputs_collide_by_concatenation() {
        // `("ab","c")` and `("a","bc")` concatenate identically. A derivation
        // that fed them raw would give one token to two different instances,
        // and the collision check would then be reporting a real defect in the
        // derivation rather than an astronomical coincidence.
        let d = domain();
        assert_ne!(
            d.instance_token(&[(1, "ab".into())], PartId(3), "c"),
            d.instance_token(&[(1, "a".into())], PartId(3), "bc")
        );
    }

    #[test]
    fn a_token_does_not_contain_its_key() {
        for key in ["espresso", "customer-123", "s-abc"] {
            let t = domain().instance_token(&[], PartId(1), key);
            assert!(!t.as_str().contains(key), "{t} reveals {key}");
        }
    }
}
