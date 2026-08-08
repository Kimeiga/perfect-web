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
    ///
    /// **Every nominal component, not just the head.** A scope and a resource
    /// attach to a nominal type, and `List<OpenTransaction>` carries whatever
    /// `OpenTransaction` carries: a list of handles is a list of things held
    /// open here. Reading the head alone looked up `List`, found nothing, and
    /// called it transferable — which is the permissive direction, and exactly
    /// the reading that made `secret<Payments>` placeable in the browser.
    pub fn profile(&self, ty: Option<&str>) -> TransferProfile {
        let Some(ty) = ty else {
            return TransferProfile {
                schema: Schema::Undetermined,
                resource: None,
                produced_scope: None,
            };
        };
        let mut resource = None;
        let mut produced_scope = None;
        let mut carrier = None;
        // The one place this module takes a type name apart, for the same
        // reason `ontology.rs` is the one place an effect name is.
        self.walk(ty, &mut |name| {
            if resource.is_none()
                && let Some(p) = self.resources.get(name)
            {
                resource = Some(p.clone());
                carrier = Some(name.to_string());
            }
            if produced_scope.is_none()
                && let Some(s) = self.scoped.get(name)
            {
                produced_scope = Some(s.clone());
            }
        });
        TransferProfile {
            // The component that carries the cost, where one does, so a
            // diagnostic names `OpenTransaction` rather than
            // `List<OpenTransaction>` — the type the reader has to change is
            // the one held open.
            schema: Schema::Named(
                carrier.unwrap_or_else(|| ty.split('<').next().unwrap_or(ty).trim().to_string()),
            ),
            resource,
            produced_scope,
        }
    }

    /// Every nominal name inside a written type, outermost first.
    fn walk(&self, written: &str, f: &mut impl FnMut(&str)) {
        let (head, args) = match written.split_once('<') {
            Some((h, rest)) => (
                h.trim(),
                rest.trim_end_matches('>')
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>(),
            ),
            None => (written.trim(), Vec::new()),
        };
        f(head);
        for a in args {
            self.walk(a, f);
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

/// Which boundary, what a value carries across it, and what the far side may
/// hold.
#[derive(Debug, Clone)]
pub struct BoundaryContext {
    pub boundary: Boundary,
    pub direction: Direction,
    /// The label of THIS value, from the dataflow that produced it. Joined
    /// with the profile's `produced_scope` by [`can_cross`]: neither subsumes
    /// the other, and using only the first silently stopped catching R-030.
    pub label: Label,
    /// **What the destination is permitted to hold.**
    ///
    /// Architect ruling, 2026-08-08, correcting this module's first version:
    ///
    /// > A resume manifest *does* have a privacy destination: the privacy
    /// > scope/partition of the document or resumable region containing it.
    /// > […] We explicitly wanted private resumable regions to be possible.
    /// > Otherwise any session-private UI state becomes inherently
    /// > non-resumable.
    ///
    /// and, for the other boundary:
    ///
    /// > World alone is not enough to establish privacy. Both `Session<A>` and
    /// > `Session<B>` may be permitted to exist in `Browser`, `Edge` or
    /// > `Origin`. But `Session<A> → Session<B>` must still be forbidden.
    ///
    /// `None` is *the destination scope cannot be established*, which is
    /// [`Blocked::UnknownDestination`] for anything restricted — blocked, not
    /// assumed valid. A public value still crosses, because it flows anywhere.
    pub destination: Option<Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Serialized into a resume manifest, which ships with its document.
    ///
    /// The destination is that document's — or that resumable region's —
    /// privacy scope. Charter §8.5, §7.8: the manifest inherits the document's
    /// cacheability, so a value in it may be exactly as private as the document
    /// is and no more.
    Resume,
    /// Passed across a call between two separately placed components.
    ///
    /// The destination is the far end's scope, which the compiler generally
    /// cannot know: `binding.rs` analyses a signature at build time and a
    /// deployment is not in evidence. So a restricted type on an edge comes
    /// back undetermined rather than transferable, and a host that can
    /// establish the far side may narrow it.
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
    /// The value is restricted and the destination's scope is not known.
    ///
    /// **Not a yes.** Architect ruling, 2026-08-08: *"If the planner cannot
    /// establish the destination privacy scope, the transfer should be
    /// Blocked, not assumed valid."*
    UnknownDestination { carries: Vec<Restriction> },
}

/// **The one decision procedure, and the two policies over it.**
///
/// Every fact decides the same way at both boundaries — **including privacy**,
/// which is the correction of 2026-08-08. What differs between the policies is
/// what each KNOWS about the destination, not what the rule is:
///
/// ```text
/// resume      the document's or region's scope, which the compiler HAS
/// remote      the far node's scope, which a build generally does NOT
/// ```
///
/// so a session value may enter a session-scoped manifest and may not enter a
/// public one, and a session value on a remote edge is undetermined until
/// something can say where it lands.
///
/// The first version of this module said the resume manifest "has no
/// destination to check". That was too strong and contradicted E7V: it would
/// have made any session-private UI state inherently non-resumable.
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

    // **Two sources, JOINED.** The LABEL covers a value that says so in its own
    // type (`Secret<Payments>`) and everything the dataflow carries it through.
    // `produced_scope` covers a type that is private because of the declaration
    // that produces it — `Cart` is an ordinary record and is session-scoped
    // because only a `session query` makes one. Neither subsumes the other, and
    // using only the first silently stopped catching R-030.
    //
    // Joined rather than "whichever is present", which is what this did: a
    // value carrying `Secret<Payments>` by label AND a session scope by
    // producer reported one of them and let the other travel unexamined.
    let carried = match &profile.produced_scope {
        Some(r) => ctx.label.join(&Label::of(r.clone())),
        None => ctx.label.clone(),
    };
    if carried.is_public() {
        // Flows anywhere, including into a destination nobody established.
        return Crossing::Proven;
    }

    // **One flow relation — `Label::flows_into` — and not a second one written
    // here.** `Public → Session<A>` and `Session<A> → Session<A>` pass;
    // `Session<A> → Public` and `Session<A> → Session<B>` do not. A privacy
    // rule reimplemented per boundary is how two boundaries come to disagree
    // about what a session is.
    match &ctx.destination {
        Some(dest) if carried.flows_into(dest) => Crossing::Proven,
        Some(dest) => Crossing::Violation(Violation::Private {
            // The first restriction the destination does not carry.
            // `Label::violations` is the existing answer to "why did this flow
            // fail", so the diagnostic names what the reader has to change.
            restriction: carried
                .violations(dest)
                .into_iter()
                .next()
                .expect("a failed flow has at least one violation"),
        }),
        None => Crossing::Blocked(Blocked::UnknownDestination {
            carries: carried.restrictions().cloned().collect(),
        }),
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

    fn ctx(boundary: Boundary, label: Label, destination: Option<Label>) -> BoundaryContext {
        BoundaryContext {
            boundary,
            direction: Direction::Outbound,
            label,
            destination,
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
        for boundary in [Boundary::Resume, Boundary::RemoteCall] {
            assert!(
                matches!(
                    can_cross(&p, &ctx(boundary, Label::public(), Some(Label::public()))),
                    Crossing::Violation(Violation::Resource { .. })
                ),
                "{boundary:?}"
            );
        }
    }

    // --- the four discriminating rows -------------------------------------
    //
    // Architect ruling, 2026-08-08. The first version of this module said the
    // resume manifest "has no destination to check", which made any
    // session-private UI state inherently non-resumable and contradicted E7V.
    // These four say what the rule actually is, and the two positive controls
    // below keep them from passing for a rule that refuses everything.

    #[test]
    fn resume_session_a_into_session_a_is_allowed() {
        // A private resumable region. THIS is what the correction restored.
        assert_eq!(
            can_cross(
                &named("Cart"),
                &ctx(
                    Boundary::Resume,
                    Label::session("SessionId"),
                    Some(Label::session("SessionId")),
                ),
            ),
            Crossing::Proven
        );
    }

    #[test]
    fn resume_session_a_into_public_is_refused() {
        // R-030's case, and still caught: a manifest that ships with the shared
        // shell is served to whoever the shell is served to.
        assert!(matches!(
            can_cross(
                &named("Cart"),
                &ctx(
                    Boundary::Resume,
                    Label::session("SessionId"),
                    Some(Label::public()),
                ),
            ),
            Crossing::Violation(Violation::Private {
                restriction: Restriction::Session(_)
            })
        ));
    }

    #[test]
    fn remote_session_a_into_session_a_is_allowed() {
        // The same relation at the other boundary. One `flows_into`, not two.
        assert_eq!(
            can_cross(
                &named("Cart"),
                &ctx(
                    Boundary::RemoteCall,
                    Label::session("SessionId"),
                    Some(Label::session("SessionId")),
                ),
            ),
            Crossing::Proven
        );
    }

    #[test]
    fn remote_session_a_into_session_b_is_refused() {
        // **The case `World` cannot see.** Both sessions may exist at the
        // origin, so a rule reading only placement calls this fine.
        assert!(matches!(
            can_cross(
                &named("Cart"),
                &ctx(
                    Boundary::RemoteCall,
                    Label::session("A"),
                    Some(Label::session("B")),
                ),
            ),
            Crossing::Violation(Violation::Private { .. })
        ));
    }

    #[test]
    fn public_flows_into_a_restricted_destination_at_both_boundaries() {
        // The positive control. Without it the four rows above pass for a rule
        // that refuses every crossing into a scope, which would make a private
        // page unable to hold a public value.
        for boundary in [Boundary::Resume, Boundary::RemoteCall] {
            assert_eq!(
                can_cross(
                    &named("Store"),
                    &ctx(boundary, Label::public(), Some(Label::session("SessionId"))),
                ),
                Crossing::Proven,
                "{boundary:?}"
            );
            // ...and into a public one, which is the ordinary case.
            assert_eq!(
                can_cross(
                    &named("Store"),
                    &ctx(boundary, Label::public(), Some(Label::public())),
                ),
                Crossing::Proven,
                "{boundary:?}"
            );
        }
    }

    #[test]
    fn an_unknown_destination_blocks_a_restricted_value_and_not_a_public_one() {
        // Architect ruling: *"If the planner cannot establish the destination
        // privacy scope, the transfer should be Blocked, not assumed valid."*
        //
        // A public value is not blocked, because it flows anywhere including
        // into a destination nobody has established. Without that half, every
        // build-time edge in the program would be undetermined and the answer
        // would carry no information.
        let restricted = can_cross(
            &named("Cart"),
            &ctx(Boundary::RemoteCall, Label::session("SessionId"), None),
        );
        assert!(
            matches!(
                restricted,
                Crossing::Blocked(Blocked::UnknownDestination { .. })
            ),
            "{restricted:?}"
        );
        assert_eq!(
            can_cross(
                &named("Store"),
                &ctx(Boundary::RemoteCall, Label::public(), None)
            ),
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
            can_cross(
                &p,
                &ctx(Boundary::Resume, Label::public(), Some(Label::public()))
            ),
            Crossing::Violation(Violation::Private { .. })
        ));
        // ...and the discriminating half: the same record with no scoped
        // producer crosses freely, or the rule would be "records may not be
        // captured".
        assert_eq!(
            can_cross(
                &named("Cart"),
                &ctx(Boundary::Resume, Label::public(), Some(Label::public()))
            ),
            Crossing::Proven
        );
    }

    #[test]
    fn the_label_and_the_producer_scope_are_joined_rather_than_chosen() {
        // A value that is `Secret<Payments>` by label AND session-scoped by
        // producer carries both. This took whichever was present and let the
        // other travel unexamined — so a destination admitting the session but
        // not the secret would have accepted it.
        let p = TransferProfile {
            produced_scope: Some(Restriction::Session("SessionId".into())),
            ..named("Receipt")
        };
        let session_only = Label::session("SessionId");
        assert!(
            matches!(
                can_cross(
                    &p,
                    &ctx(
                        Boundary::Resume,
                        Label::secret("Payments"),
                        Some(session_only)
                    ),
                ),
                Crossing::Violation(Violation::Private {
                    restriction: Restriction::Secret(_)
                })
            ),
            "the secret is not carried by the destination"
        );
        // Both carried: it crosses.
        let both = Label::secret("Payments").join(&Label::session("SessionId"));
        assert_eq!(
            can_cross(
                &p,
                &ctx(Boundary::Resume, Label::secret("Payments"), Some(both))
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
        for boundary in [Boundary::Resume, Boundary::RemoteCall] {
            assert_eq!(
                can_cross(&p, &ctx(boundary, Label::public(), Some(Label::public()))),
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
                &ctx(
                    Boundary::Resume,
                    Label::session("SessionId"),
                    Some(Label::public())
                )
            ),
            Crossing::Violation(Violation::Resource { .. })
        ));
    }
}
