//! Document identity — re-exported, not defined.
//!
//! Architect ruling, 2026-08-07: the address vocabulary lives in a neutral
//! crate so a browser patch runtime can speak about document addresses without
//! the server renderer entering its dependency closure.
//!
//! > If `pw-protocol` depends on `pw-render` merely because `PartAddress`
//! > happens to live there, then eventually the browser patch runtime can
//! > accidentally drag renderer implementation code into its dependency
//! > closure.
//!
//! That would put E7-R's no-replay structural gate in the position of asserting
//! something a dependency edge contradicts. The renderer USES these types; it
//! does not own them.

pub use pw_document::{
    Anchor, ElementId, IdentityDomain, InstanceFrame, InstancePath, InstanceToken, LocalPartId,
    PartAddress, Partition, TemplateSchemaId,
};
