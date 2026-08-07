//! **Whether a typed value can safely cross a boundary.**
//!
//! Architect ruling, 2026-08-07:
//!
//! > Remote-capable is not equivalent to resume-serializable. But they are two
//! > policies over the same underlying semantic fact: whether a typed value can
//! > safely cross a boundary. Don't build a second `is_remote_capable_type()`
//! > beside `is_serializable_capture()`.
//!
//! ```text
//!                   boundary-transfer analysis        <- this module
//!                      /                 \
//!           resume-capture policy      remote-call policy
//!              resume.rs                  binding.rs
//! ```
//!
//! This project's whole history is one shape: two derivations of one fact,
//! agreeing until the day one of them changes. `docs/RISK_QUEUE.md` is mostly
//! instances of it. A second `is_remote_capable_type()` would be the next one,
//! and it would be worse than most, because the two answers would be about
//! *authority* and would disagree in the permissive direction.
//!
//! # Transferability, not serializability
//!
//! "Serializable" is too weak a word. `OpenTransaction` could be encoded as a
//! handle number and decoded on the other side into a perfectly well-formed
//! value — and the value would be wrong, because the thing it names is held
//! open *here*. The question is not whether bytes can be produced. It is
//! whether the value means the same thing when it arrives.
//!
//! # Three answers, for the reason `PlacementLookup` has three
//!
//! [`Crossing`] is `Proven | Violation | Blocked`, never a bool. The same
//! argument as `placement.rs`: an analysis that cannot tell must not be
//! recorded as an analysis that permits. `World::worlds_for` returned `None`
//! for an effect it did not model and `None` was read as *grants it*, which is
//! how `secret<Payments>` was placeable in the browser for a milestone. A
//! capture whose type this build cannot name is exactly that case, and the
//! corpus has one.

use std::collections::BTreeMap;

use crate::hir::Hir;
use crate::privacy::{Label, Restriction};
use crate::signatures::Signatures;

/// **What the whole program says about a type, gathered once.**
///
/// Both facts are read from DECLARATIONS. A type is a resource because some
/// function declares `resource.acquire<T>` for it, and a type is scoped because
/// the declaration producing it is `session`, `user` or `organization`. There
/// is no list of untransferable types in the compiler, and adding a resource to
/// a library brings every consequence with it.
#[derive(Debug, Default)]
pub struct TypeFacts {
    /// Types some function acquires as a resource, and the path that does.
    resources: BTreeMap<String, String>,
    /// Types produced only by a scoped declaration, with the scope.
    scoped: BTreeMap<String, Restriction>,
}

impl TypeFacts {
    pub fn build(hirs: &[&Hir], sigs: &Signatures) -> TypeFacts {
        let mut f = TypeFacts::default();

        for (path, sig) in sigs.iter() {
            for e in &sig.effects {
                let Some(ty) = e
                    .strip_prefix("resource.acquire<")
                    .and_then(|r| r.strip_suffix('>'))
                else {
                    continue;
                };
                f.resources.insert(ty.to_string(), path.clone());
            }
        }

        for hir in hirs {
            for (_, d) in hir.all_decls() {
                let Some(vis) = d.visibility.as_deref() else {
                    continue;
                };
                let restriction = match vis {
                    "session" => Restriction::Session("SessionId".into()),
                    "user" => Restriction::User("UserId".into()),
                    "organization" => Restriction::Organization("OrganizationId".into()),
                    _ => continue,
                };
                // `session query Cart(..) -> Result<Cart, CartError>` produces a
                // `Cart`. The head is the carrier, so the produced type is its
                // first argument.
                let produced = match d.ret.as_deref() {
                    // A scope attaches to a nominal type, so the head is what
                    // this map is keyed by: `Result<List<X>, E>` scopes `List`,
                    // the same as before `ret_args` began carrying nesting.
                    Some("Result") | Some("Option") | Some("List") => d
                        .ret_args
                        .first()
                        .map(|a| a.split('<').next().unwrap_or(a).trim().to_string()),
                    other => other.map(str::to_string),
                };
                if let Some(ty) = produced {
                    f.scoped.insert(ty, restriction);
                }
            }
        }
        f
    }

    /// **What crossing would cost a value of this type.**
    ///
    /// `None` is a type this build could not determine, and it is a real
    /// answer rather than a missing one — see [`Schema::Undetermined`].
    pub fn profile(&self, ty: Option<&str>) -> TransferProfile {
        let Some(ty) = ty else {
            return TransferProfile {
                schema: Schema::Undetermined,
                resource: None,
                produced_scope: None,
            };
        };
        // The head, because a scope and a resource attach to a nominal type:
        // `List<Cart>` carries whatever `Cart` carries. The one place this
        // module takes a type name apart, for the same reason `ontology.rs` is
        // the one place an effect name is.
        let head = ty.split('<').next().unwrap_or(ty).trim();
        TransferProfile {
            schema: Schema::Named(head.to_string()),
            resource: self.resources.get(head).cloned(),
            produced_scope: self.scoped.get(head).cloned(),
        }
    }

    /// Is this type one a declaration acquires as a resource?
    pub fn resource_producer(&self, ty: &str) -> Option<&str> {
        self.resources.get(ty).map(String::as_str)
    }

    /// The restriction this type carries because of what produces it.
    pub fn produced_scope(&self, ty: &str) -> Option<&Restriction> {
        self.scoped.get(ty)
    }
}

/// Whether a schema can be named for a value at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schema {
    /// A nominal type this build can describe on a wire.
    Named(String),
    /// This build could not determine the type.
    ///
    /// Not "no schema is needed" and not "any schema will do". A resume
    /// manifest carries a hash derived from the capture's type, and a hash
    /// derived from a guess matches nothing — so every resume fails after
    /// deployment, for a reason nobody can diagnose from the deployment.
    Undetermined,
}

/// **What one type would cost to move across a boundary.**
///
/// Per TYPE. What a particular *value* of that type carries — its privacy
/// label — travels in the [`BoundaryContext`], because two values of one type
/// can carry different labels and a profile that folded the label in would be
/// answering about a value while claiming to answer about a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferProfile {
    pub schema: Schema,
    /// The path declaring `resource.acquire<T>` for it, where one does.
    ///
    /// A resource's value IS the thing held open. Encoding it is possible and
    /// meaningless, which is why this is not a serializability question.
    pub resource: Option<String>,
    /// A restriction the type carries because only a scoped declaration
    /// produces it. `Cart` is an ordinary record and is session-scoped because
    /// only a `session query` makes one.
    pub produced_scope: Option<Restriction>,
}

/// Which boundary, and what a value carries across it.
#[derive(Debug, Clone)]
pub struct BoundaryContext {
    pub boundary: Boundary,
    pub direction: Direction,
    /// The label of THIS value, from the dataflow that produced it. Unioned
    /// with the profile's `produced_scope` by [`can_cross`]: neither subsumes
    /// the other, and using only the first silently stopped catching R-030.
    pub label: Label,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Serialized into the resume manifest, which ships with the document.
    ///
    /// There is no "where" to check: the manifest inherits the document's
    /// cacheability, so a private value in it is served to whoever the shell is
    /// served to. Charter §8.5, §7.8.
    ResumeCapture,
    /// Passed across a call between two separately placed components.
    ///
    /// Privacy is NOT decided here. Whether a session value may cross from one
    /// node to another is a question about those nodes, which `World::may_hold`
    /// and the deployment's topology answer — and answering it a second time
    /// from the type would be the duplication this module exists to prevent.
    RemoteCall,
}

/// Which way a value moves. Arguments and results are both checked, because a
/// signature that cannot return a value across a boundary cannot be called
/// across it either — and a rule that checked only one direction would call
/// `with_transaction(fn(OpenTransaction) -> T)` remotable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Into the callee: an argument.
    Inbound,
    /// Back to the caller: a result, or an error.
    Outbound,
}

/// **May this value cross?**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crossing {
    /// It may, and nothing about it is in question.
    Proven,
    /// It may not, and the reason names what would be wrong.
    Violation(Violation),
    /// The analysis has no basis to decide. **Not a yes.**
    Blocked(Blocked),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// The value IS the thing held open. Encoding it produces a well-formed
    /// value that names something on the other side of the boundary.
    Resource { ty: String, producer: String },
    /// Private data crossing a boundary that does not preserve the restriction.
    Private { restriction: Restriction },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// No determinable type, so no schema, so no decision.
    UndeterminedSchema,
}

/// **The one decision procedure, and the two policies over it.**
///
/// The shared facts — resource, undetermined schema — decide the same way at
/// every boundary, because they are properties of the value rather than of the
/// crossing. Privacy is where the policies differ, and the difference is
/// principled: the resume manifest has no destination to check against, and a
/// remote call has one that something else already checks.
pub fn can_cross(profile: &TransferProfile, ctx: &BoundaryContext) -> Crossing {
    // Order matters, and this order is the existing behaviour: a resource is
    // reported as a resource even where it is also private, because the repair
    // is different and reporting both delivers one defect twice.
    if let Schema::Undetermined = profile.schema {
        return Crossing::Blocked(Blocked::UndeterminedSchema);
    }
    if let Some(producer) = &profile.resource {
        let Schema::Named(ty) = &profile.schema else {
            unreachable!("undetermined schemas returned above")
        };
        return Crossing::Violation(Violation::Resource {
            ty: ty.clone(),
            producer: producer.clone(),
        });
    }

    match ctx.boundary {
        Boundary::ResumeCapture => {
            // Two sources, unioned. The LABEL covers a value that says so in
            // its own type (`Secret<Payments>`) and everything the dataflow
            // carries it through. `produced_scope` covers a type that is
            // private because of the declaration that produces it. Neither
            // subsumes the other, and using only the first silently stopped
            // catching R-030.
            let by_label = ctx.label.restrictions().next().cloned();
            match by_label.or_else(|| profile.produced_scope.clone()) {
                Some(restriction) => Crossing::Violation(Violation::Private { restriction }),
                None => Crossing::Proven,
            }
        }
        // Recorded, not judged. Whether a restricted value may move between
        // two nodes depends on which nodes, and `World::may_hold` plus the
        // deployment's topology is where that is decided. A second answer here
        // would be the pattern this module exists to delete.
        Boundary::RemoteCall => Crossing::Proven,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(ty: &str) -> TransferProfile {
        TransferProfile {
            schema: Schema::Named(ty.to_string()),
            resource: None,
            produced_scope: None,
        }
    }

    fn ctx(boundary: Boundary, label: Label) -> BoundaryContext {
        BoundaryContext {
            boundary,
            direction: Direction::Outbound,
            label,
        }
    }

    #[test]
    fn a_resource_cannot_cross_either_boundary() {
        // The shared fact. `OpenTransaction` could be encoded as a handle
        // number and decoded into a well-formed value naming something held
        // open on the other side — which is why this is transferability and
        // not serializability.
        let p = TransferProfile {
            resource: Some("db.begin".into()),
            ..named("OpenTransaction")
        };
        for boundary in [Boundary::ResumeCapture, Boundary::RemoteCall] {
            assert!(
                matches!(
                    can_cross(&p, &ctx(boundary, Label::public())),
                    Crossing::Violation(Violation::Resource { .. })
                ),
                "{boundary:?}"
            );
        }
    }

    #[test]
    fn privacy_is_where_the_two_policies_differ() {
        // The reason there is one analysis and two policies rather than one of
        // either. A session value may not enter the resume manifest, because
        // the manifest ships with the document and there is no destination to
        // check. It may cross a remote call, because there IS a destination and
        // `World::may_hold` decides against it.
        let p = named("Cart");
        let session = Label::session("SessionId");

        assert!(matches!(
            can_cross(&p, &ctx(Boundary::ResumeCapture, session.clone())),
            Crossing::Violation(Violation::Private { .. })
        ));
        assert_eq!(
            can_cross(&p, &ctx(Boundary::RemoteCall, session)),
            Crossing::Proven
        );
    }

    #[test]
    fn a_scope_from_the_producer_counts_even_when_the_label_is_public() {
        // R-030. `Cart` is an ordinary record with no restriction in its own
        // declaration; it is session-scoped because only a `session query`
        // makes one. An analysis reading only the value's label passes this.
        let p = TransferProfile {
            produced_scope: Some(Restriction::Session("SessionId".into())),
            ..named("Cart")
        };
        assert!(matches!(
            can_cross(&p, &ctx(Boundary::ResumeCapture, Label::public())),
            Crossing::Violation(Violation::Private { .. })
        ));
        // ...and the discriminating half: the same record with no scoped
        // producer crosses freely, or the rule would be "records may not be
        // captured".
        assert_eq!(
            can_cross(
                &named("Cart"),
                &ctx(Boundary::ResumeCapture, Label::public())
            ),
            Crossing::Proven
        );
    }

    #[test]
    fn an_undetermined_schema_is_blocked_and_not_permitted() {
        // The third answer. A bool would have to pick, and picking `true` is
        // exactly the shape that made `secret<Payments>` placeable in the
        // browser: an analysis that could not tell, recorded as one that
        // permitted.
        let p = TransferProfile {
            schema: Schema::Undetermined,
            resource: None,
            produced_scope: None,
        };
        for boundary in [Boundary::ResumeCapture, Boundary::RemoteCall] {
            assert_eq!(
                can_cross(&p, &ctx(boundary, Label::public())),
                Crossing::Blocked(Blocked::UndeterminedSchema)
            );
        }
    }

    #[test]
    fn a_resource_is_reported_as_a_resource_even_when_it_is_also_private() {
        // One defect, one diagnostic. The repairs differ — acquire it again on
        // the other side, versus re-read it where the handler runs — so
        // reporting both would send the reader two ways at once.
        let p = TransferProfile {
            resource: Some("db.begin".into()),
            produced_scope: Some(Restriction::Session("SessionId".into())),
            ..named("OpenTransaction")
        };
        assert!(matches!(
            can_cross(
                &p,
                &ctx(Boundary::ResumeCapture, Label::session("SessionId"))
            ),
            Crossing::Violation(Violation::Resource { .. })
        ));
    }
}
