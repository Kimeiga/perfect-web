//! Where a consequence appears in a live document.
//!
//! Architect ruling, 2026-08-07:
//!
//! > `pw-patch` should depend on **resource identity** and **document-address
//! > identity**, not on `pw-materialize` or the full `pw-render`
//! > implementation. […] If `pw-protocol` depends on `pw-render` merely because
//! > `PartAddress` happens to live there, then eventually the browser patch
//! > runtime can accidentally drag renderer implementation code into its
//! > dependency closure.
//!
//! That would recreate exactly the coupling E7-R's no-replay structural gate
//! exists to prevent: the gate asserts the browser artifact contains no
//! template renderer, and a protocol crate that transitively imported one would
//! make the gate a race between two facts about the same build.
//!
//! So the address vocabulary lives here, without `TemplateIR`, without the
//! renderer, and without any server serialization.
//!
//! ```text
//! E6 defines what data exists          pw-resource
//! E7-R defines where consequences live pw-document   ← this crate
//! E7-P defines how changes travel      pw-protocol
//! ```
//!
//! # The three identities this crate holds
//!
//! ```text
//! TemplateSchemaId   which template, semantically
//! InstancePath       which instance of each repeatable scope encloses it
//! LocalPartId        which part position within the template
//! ```
//!
//! together forming a [`PartAddress`]: *which live location in this document
//! should change*.

use serde::{Deserialize, Serialize};

pub use pw_resource::Partition;

pub mod domain;
pub use domain::{IdentityDomain, InstanceToken};

/// A part's position within its template.
///
/// **Template-scoped and ordinal.** Not a content hash per part: two
/// `<span>{price}</span>` parts have identical structure and are different
/// places in the document, so content identity cannot say which to patch, and
/// adding enough parent context to disambiguate reinvents structural position
/// at a higher price.
///
/// Positional identity is safe because E7V refuses across schemas: local part 3
/// is never read as local part 3 of an incompatible template, so a source edit
/// may renumber freely — it also changes the schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalPartId(pub u32);

impl std::fmt::Display for LocalPartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An element that owns at least one element-local part.
///
/// Separate from `LocalPartId` because one element can own several parts — two
/// dynamic attributes and a handler — and giving each its own range boundary
/// would cost six nodes to say one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ElementId(pub u32);

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A template's semantic identity.
///
/// Over the IR's shape — part kinds, names, contexts, nesting, order — and not
/// over source bytes, so a comment or a reflow does not change it and a
/// reordered attribute does.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TemplateSchemaId(pub String);

impl std::fmt::Display for TemplateSchemaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a part is anchored in the document.
///
/// Two wire encodings of one concept, chosen per part KIND rather than one
/// forced onto all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// Comment boundaries. A range can be zero nodes, one text node, twenty
    /// `<li>`s or a component subtree; an attribute can represent none of those.
    Range,
    /// The owning element carries `data-pw`.
    Element,
}

/// One frame of an instance path: a repeatable scope, and which instance.
///
/// **Generic.** A frame is a repeatable scope, and today that scope is a keyed
/// `{#each}`. The same gap appears with a component used twice, a conditional
/// region recreated after toggling, and a streamed instance, so nothing here is
/// named for loops.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstanceFrame {
    /// The part that opened the scope.
    pub scope: LocalPartId,
    pub instance: InstanceToken,
}

/// The instances enclosing a part, outermost first.
pub type InstancePath = Vec<InstanceFrame>;

/// **Which live location in this document should change.**
///
/// ```text
/// TemplateSchemaId + InstancePath + LocalPartId
/// ```
///
/// A template part *definition* is `TemplateSchemaId + LocalPartId`. A document
/// part *instance* needs the path as well: `docs/RISK_QUEUE.md` instance 25 is
/// what happens without it — three Add buttons sharing one id, a runtime keeping
/// the last, and two thirds of a page silently inert.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartAddress {
    pub template: TemplateSchemaId,
    #[serde(default)]
    pub instances: InstancePath,
    pub part: LocalPartId,
}

impl PartAddress {
    pub fn new(template: &TemplateSchemaId, part: LocalPartId) -> PartAddress {
        PartAddress {
            template: template.clone(),
            instances: Vec::new(),
            part,
        }
    }

    /// Enter a repeatable scope.
    pub fn within(mut self, scope: LocalPartId, instance: InstanceToken) -> PartAddress {
        self.instances.push(InstanceFrame { scope, instance });
        self
    }

    /// The wire form the browser index is keyed by.
    ///
    /// Deterministic and total: every address has exactly one, and two
    /// addresses have the same one only if they are equal. The runtime's index
    /// is a map on this string, so a second spelling would be a second entry
    /// for one location.
    pub fn key(&self) -> String {
        let path: Vec<String> = self
            .instances
            .iter()
            .map(|f| format!("{}@{}", f.scope, f.instance))
            .collect();
        format!("{}/{}|{}", self.template, path.join("/"), self.part)
    }
}

impl std::fmt::Display for PartAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> TemplateSchemaId {
        TemplateSchemaId("0d5ebac8facb8cab".into())
    }

    #[test]
    fn two_instances_of_one_part_are_two_addresses() {
        // RISK_QUEUE 25, as a type-level fact rather than a runtime one.
        let template = schema();
        let a = PartAddress::new(&template, LocalPartId(0))
            .within(LocalPartId(1), InstanceToken::from_wire("aaaa"));
        let b = PartAddress::new(&template, LocalPartId(0))
            .within(LocalPartId(1), InstanceToken::from_wire("bbbb"));
        assert_ne!(a, b);
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn a_different_template_is_a_different_address() {
        let a = PartAddress::new(&schema(), LocalPartId(0));
        let b = PartAddress::new(&TemplateSchemaId("other".into()), LocalPartId(0));
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn nesting_is_ordered_outermost_first() {
        // A path is not a set. `[category, item]` and `[item, category]` name
        // different places, and a key that sorted them would merge two.
        let t = schema();
        let a = PartAddress::new(&t, LocalPartId(9))
            .within(LocalPartId(1), InstanceToken::from_wire("x"))
            .within(LocalPartId(2), InstanceToken::from_wire("y"));
        let b = PartAddress::new(&t, LocalPartId(9))
            .within(LocalPartId(2), InstanceToken::from_wire("y"))
            .within(LocalPartId(1), InstanceToken::from_wire("x"));
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn the_key_round_trips_equality() {
        // The index is a map on this string, so two equal addresses must share
        // one key and two different ones must not.
        let t = schema();
        let make = || {
            PartAddress::new(&t, LocalPartId(4))
                .within(LocalPartId(1), InstanceToken::from_wire("tok"))
        };
        assert_eq!(make().key(), make().key());
        assert_eq!(make(), make());
    }
}
