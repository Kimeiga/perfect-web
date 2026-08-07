//! The versioned browser↔server application protocol.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Neither endpoint gets to own the contract.
//!
//! ```text
//! server/runtime                     browser runtime
//!       \                                /
//!        \                              /
//!                  pw-protocol
//!                 /          \
//!        pw-resource      pw-document
//! ```
//!
//! Named `pw-protocol` rather than `pw-patch` because only one of the three
//! frame kinds is a patch, and a name that is wrong for two thirds of its
//! contents is a name that gets worked around.
//!
//! # What this crate owns
//!
//! The shared vocabulary, the canonical encoding, the protocol version, and
//! decoder validation. Not either side's behaviour.
//!
//! # What it does not own
//!
//! ```text
//! how a ResourceEntryId is semantically constructed   pw-resource
//! how an EntryKey is stored                            pw-materialize
//! how a PartAddress is assigned                        pw-render's indexing
//! how HTML is rendered                                 pw-render
//! how a patch mutates the DOM                          the browser runtime
//! whether SSE, streaming fetch or long-poll carries it a transport adapter
//! ```
//!
//! The last is the one that earns its place soonest: a frame says
//! `StreamFrame::Patch(..)` without caring what delivered it, so E7-P can
//! change transport without changing patch semantics — and the long-poll that
//! E7-R used stays a legitimate adapter rather than becoming the mechanism.

use serde::{Deserialize, Serialize};

pub use pw_document::{InstanceToken, LocalPartId, PartAddress, TemplateSchemaId};
pub use pw_resource::{ResourceEntryId, Version};

/// Which version of this protocol a frame speaks.
///
/// Carried on every frame rather than negotiated once, because a connection
/// that resumed across a deployment would otherwise carry frames of one version
/// under a handshake from another — the same mixed-build failure `patch_applies`
/// refuses in E7V, arriving through the stream instead of through a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u32);

/// What this build speaks.
pub const CURRENT: ProtocolVersion = ProtocolVersion(1);

/// One resource entry at one version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBasis {
    pub entry: ResourceEntryId,
    pub version: Version,
}

/// The resource state a patch was derived from.
///
/// A **collection from the first version**, even while every patch carries one
/// entry. Architect ruling, 2026-08-06:
///
/// > A computed total might be derived from several independently versioned
/// > entries. […] define the protocol field as a collection so multi-resource
/// > patches don't require a breaking redesign later.
///
/// Not a vector clock. These are not peer replicas — it is a version vector
/// over the entries that contributed, and the only comparison it supports is
/// "would applying this move any dependency backward?".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalBasis {
    pub resources: Vec<ResourceBasis>,
}

impl CausalBasis {
    pub fn of(entry: ResourceEntryId, version: Version) -> CausalBasis {
        CausalBasis {
            resources: vec![ResourceBasis { entry, version }],
        }
    }

    pub fn and(mut self, entry: ResourceEntryId, version: Version) -> CausalBasis {
        self.resources.push(ResourceBasis { entry, version });
        self
    }

    /// Would applying this move any dependency backward?
    ///
    /// The question a receiver asks, and it is asked over EVERY entry in the
    /// basis. A patch derived from a fresh cart and a stale promotion must be
    /// refused: applying it would show a total computed from state the page has
    /// already moved past, and the page would be consistent with nothing.
    pub fn is_newer_than(&self, held: &impl Held) -> bool {
        if self.resources.is_empty() {
            // A patch derived from nothing has no causal claim. Refused rather
            // than applied: "no basis" is not "any basis".
            return false;
        }
        let mut advances = false;
        for r in &self.resources {
            match held.version_of(&r.entry) {
                Some(current) if r.version < current => return false,
                Some(current) if r.version > current => advances = true,
                Some(_) => {}
                // An entry the receiver does not hold is new state, not stale.
                None => advances = true,
            }
        }
        advances
    }
}

/// What a receiver currently holds, by entry.
///
/// A trait, so the protocol does not decide how a browser stores its versions —
/// and so a test can supply a map without a runtime.
pub trait Held {
    fn version_of(&self, entry: &ResourceEntryId) -> Option<Version>;
}

impl Held for std::collections::BTreeMap<ResourceEntryId, Version> {
    fn version_of(&self, entry: &ResourceEntryId) -> Option<Version> {
        self.get(entry).copied()
    }
}

/// What a patch does to the part it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum PatchOp {
    /// Replace a range's text content.
    ReplaceText {
        text: String,
    },
    /// Replace a range's nodes with markup.
    ///
    /// The markup is produced by the SERVER's renderer. This crate carries it
    /// and does not produce it, which is what keeps the renderer out of a
    /// browser runtime's dependency closure.
    ReplaceRange {
        html: String,
    },
    SetAttribute {
        name: String,
        value: String,
    },
    RemoveAttribute {
        name: String,
    },

    // --- keyed list operations, E7-P ---------------------------------------
    //
    // Declared now and not yet emitted. The address space they need exists —
    // an `InstancePath` frame names which instance — and reserving the shape
    // means a list operation is a new variant rather than a new protocol.
    /// Insert a rendered instance before the instance the address names.
    /// Insert before an instance, or at the HEAD of the collection when
    /// `instance` is `None`.
    ///
    /// The anchor is optional because a keyed collection can be empty, and an
    /// empty collection has no instance to anchor to. Without this the first
    /// item could only ever arrive by re-rendering the document, and a
    /// collection that emptied would be permanently unfillable — a hole that
    /// only appears in the one state a demo never reaches.
    InsertBefore {
        instance: Option<InstanceToken>,
        html: String,
    },
    /// Insert after an instance, or at the TAIL when `instance` is `None`.
    InsertAfter {
        instance: Option<InstanceToken>,
        html: String,
    },
    RemoveInstance {
        instance: InstanceToken,
    },
    /// Move an instance to sit after another, or to the front when `after` is
    /// `None`. A move rather than a remove-and-insert, because the node keeps
    /// its identity — which is the whole reason a list is keyed.
    MoveInstance {
        instance: InstanceToken,
        after: Option<InstanceToken>,
    },
}

/// One change, to one place, derived from known resource state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    pub protocol: ProtocolVersion,
    pub basis: CausalBasis,
    pub target: PartAddress,
    pub operation: PatchOp,
}

/// Why a document cannot continue, and what a receiver may do instead.
///
/// The E7V vocabulary, on the stream. A recovery is never "try anyway".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "recovery")]
pub enum Recovery {
    RefetchRegion { target: PartAddress },
    RerenderPrivateSlot { target: PartAddress },
    Reload,
    RetryInteraction,
    RequireUserConfirmation,
    RejectIrrecoverable { why: String },
}

/// A frame on the server→browser stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "frame")]
pub enum StreamFrame {
    /// An entry advanced. The receiver may hold this without any patch —
    /// subscription and patching are logically separate, and a server MAY
    /// derive a patch from a change rather than MUST.
    ResourceChanged {
        protocol: ProtocolVersion,
        entry: ResourceEntryId,
        version: Version,
    },
    Patch(Patch),
    Recovery {
        protocol: ProtocolVersion,
        recovery: Recovery,
    },
}

/// Why a frame was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// A version this build does not speak.
    ///
    /// **Incomparable, not different** — the E7V lesson. A frame from another
    /// protocol version is refused before anything it contains is interpreted,
    /// because interpreting it would be reading unknown bytes under known rules.
    UnsupportedProtocol { saw: ProtocolVersion },
    /// The bytes are not a frame.
    Undecodable { why: String },
    /// The frame is larger than a receiver will read.
    TooLarge { bytes: usize, limit: usize },
    /// A patch whose basis would move a dependency backward, or has none.
    NotNewer,
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Malformed::UnsupportedProtocol { saw } => write!(
                f,
                "protocol version {} is incomparable with {}",
                saw.0, CURRENT.0
            ),
            Malformed::Undecodable { why } => write!(f, "not a frame: {why}"),
            Malformed::TooLarge { bytes, limit } => {
                write!(f, "{bytes} bytes exceeds the {limit}-byte frame limit")
            }
            Malformed::NotNewer => f.write_str("the basis does not advance any held entry"),
        }
    }
}

/// The largest frame a receiver will decode.
///
/// A bound rather than a trust: a stream is attacker-reachable input, and a
/// decoder without a limit is a decoder that allocates whatever it is told to.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

impl StreamFrame {
    pub fn protocol(&self) -> ProtocolVersion {
        match self {
            StreamFrame::ResourceChanged { protocol, .. }
            | StreamFrame::Recovery { protocol, .. } => *protocol,
            StreamFrame::Patch(p) => p.protocol,
        }
    }

    /// Decode one frame, validating before interpreting.
    ///
    /// Order matters and is the E7V order: the version check runs before
    /// everything it governs, because reading a frame of an unknown version
    /// under this version's rules is reading unknown bytes under known rules.
    pub fn decode(bytes: &[u8]) -> Result<StreamFrame, Malformed> {
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(Malformed::TooLarge {
                bytes: bytes.len(),
                limit: MAX_FRAME_BYTES,
            });
        }
        let frame: StreamFrame = serde_json::from_slice(bytes)
            .map_err(|e| Malformed::Undecodable { why: e.to_string() })?;
        if frame.protocol() != CURRENT {
            return Err(Malformed::UnsupportedProtocol {
                saw: frame.protocol(),
            });
        }
        Ok(frame)
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a frame serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entry(n: &str) -> ResourceEntryId {
        use pw_resource::{DevelopmentIdentityKey, EntryIdentity, Partition};
        ResourceEntryId::derive(
            &EntryIdentity::new("Resources.Cart", &[n], Partition::Public),
            &DevelopmentIdentityKey,
        )
    }

    fn address() -> PartAddress {
        PartAddress::new(&TemplateSchemaId("t".into()), LocalPartId(4))
    }

    fn patch(basis: CausalBasis) -> StreamFrame {
        StreamFrame::Patch(Patch {
            protocol: CURRENT,
            basis,
            target: address(),
            operation: PatchOp::ReplaceText { text: "2".into() },
        })
    }

    #[test]
    fn a_frame_round_trips() {
        let frame = patch(CausalBasis::of(entry("a"), Version(15)));
        assert_eq!(StreamFrame::decode(&frame.encode()).unwrap(), frame);
    }

    #[test]
    fn a_frame_from_another_protocol_version_is_incomparable() {
        let mut frame = patch(CausalBasis::of(entry("a"), Version(1)));
        if let StreamFrame::Patch(p) = &mut frame {
            p.protocol = ProtocolVersion(99);
        }
        assert_eq!(
            StreamFrame::decode(&frame.encode()),
            Err(Malformed::UnsupportedProtocol {
                saw: ProtocolVersion(99)
            })
        );
    }

    #[test]
    fn a_frame_larger_than_the_limit_is_refused_before_decoding() {
        // The bound is on BYTES, checked before `serde` sees them: a decoder
        // that parsed first and measured after would already have allocated.
        let huge = vec![b'{'; MAX_FRAME_BYTES + 1];
        assert!(matches!(
            StreamFrame::decode(&huge),
            Err(Malformed::TooLarge { .. })
        ));
    }

    #[test]
    fn garbage_is_refused_with_a_reason() {
        assert!(matches!(
            StreamFrame::decode(b"not a frame"),
            Err(Malformed::Undecodable { .. })
        ));
    }

    #[test]
    fn a_basis_that_advances_is_newer() {
        let mut held = BTreeMap::new();
        held.insert(entry("a"), Version(14));
        assert!(CausalBasis::of(entry("a"), Version(15)).is_newer_than(&held));
    }

    #[test]
    fn a_basis_that_moves_backward_is_not() {
        let mut held = BTreeMap::new();
        held.insert(entry("a"), Version(15));
        assert!(!CausalBasis::of(entry("a"), Version(13)).is_newer_than(&held));
        // And an equal version advances nothing, so it is not newer either.
        assert!(!CausalBasis::of(entry("a"), Version(15)).is_newer_than(&held));
    }

    #[test]
    fn a_multi_resource_basis_is_refused_if_any_entry_moves_backward() {
        // The reason the field is a collection. A patch derived from a fresh
        // cart and a STALE promotion would show a total computed from state the
        // page has already moved past — consistent with nothing.
        let mut held = BTreeMap::new();
        held.insert(entry("cart"), Version(14));
        held.insert(entry("promotion"), Version(9));

        let good = CausalBasis::of(entry("cart"), Version(15)).and(entry("promotion"), Version(9));
        assert!(good.is_newer_than(&held));

        let stale = CausalBasis::of(entry("cart"), Version(15)).and(entry("promotion"), Version(8));
        assert!(
            !stale.is_newer_than(&held),
            "one stale dependency refuses the whole patch"
        );
    }

    #[test]
    fn an_unheld_entry_is_new_state_rather_than_stale() {
        let held: BTreeMap<ResourceEntryId, Version> = BTreeMap::new();
        assert!(CausalBasis::of(entry("a"), Version(1)).is_newer_than(&held));
    }

    #[test]
    fn an_empty_basis_has_no_causal_claim() {
        // "No basis" is not "any basis". A patch that named no resource state
        // could be applied at any time, in any order, which is what the version
        // comparison exists to prevent.
        let held: BTreeMap<ResourceEntryId, Version> = BTreeMap::new();
        assert!(!CausalBasis::default().is_newer_than(&held));
    }

    #[test]
    fn a_resource_change_carries_no_patch() {
        // Subscription and patching are logically separate: a server MAY derive
        // a patch from a change rather than MUST, and a receiver that required
        // one would stall on a change the server chose not to patch.
        let frame = StreamFrame::ResourceChanged {
            protocol: CURRENT,
            entry: entry("a"),
            version: Version(3),
        };
        assert_eq!(StreamFrame::decode(&frame.encode()).unwrap(), frame);
    }

    #[test]
    fn the_protocol_carries_no_transport() {
        // Structural: nothing in this crate names a transport, so a frame
        // cannot acquire a dependency on how it arrived. Checked against the
        // source, because the property is an absence and a passing decode test
        // cannot show one.
        // Everything above the test module. The list of names to look for
        // lives below it, and a scan of the whole file finds itself.
        let src = include_str!("lib.rs");
        let src = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        for transport in ["reqwest", "hyper", "tokio", "WebSocket", "EventSource"] {
            let mentions: Vec<&str> = src
                .lines()
                .filter(|l| l.contains(transport) && !l.trim_start().starts_with("//"))
                .collect();
            assert!(
                mentions.is_empty(),
                "the protocol must not name a transport: {mentions:?}"
            );
        }
    }
}
