//! E2A-S — the minimal static task-scope checker.
//!
//! Why this exists, in the architect's words:
//!
//! > A runtime task tree cannot make compile-fail fixtures fail at compilation.
//! > […] E2A closes only when both halves pass: `E2A = E2A-R + E2A-S`.
//!
//! Koka can represent `task.spawn` in an effect row. That is useful and
//! insufficient — an effect row says *"this function may spawn"*, never *"this
//! handle does not outlive its owner"*. Scope non-escape is a `pw` property.
//!
//! Deliberately **not** the full affine/linear type system of E9C. This is the
//! narrow lexical rule that lets structured-concurrency rejections be real
//! compile failures now:
//!
//! ```text
//! Task<Scope, T>   Subscription<Scope, T>   Resource<Scope, T>
//! ```
//!
//! - `task.spawn` creates a handle owned by the current lexical scope.
//! - A handle may not be returned, stored, or captured into a longer-lived scope.
//! - Ordinary spawned work cannot detach; `durable.spawn` is a separate
//!   capability.
//! - A handle may not be used after its owning scope has exited.

use std::collections::HashMap;

pub type ScopeId = usize;
pub type HandleId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Task,
    Subscription,
    Resource,
}

impl HandleKind {
    fn noun(self) -> &'static str {
        match self {
            HandleKind::Task => "task",
            HandleKind::Subscription => "subscription",
            HandleKind::Resource => "resource",
        }
    }
}

/// Why a scope exists. Used only to make diagnostics readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Application,
    Session,
    Request,
    Route,
    Component,
    Block,
}

impl ScopeKind {
    fn noun(self) -> &'static str {
        match self {
            ScopeKind::Application => "application",
            ScopeKind::Session => "session",
            ScopeKind::Request => "request",
            ScopeKind::Route => "route",
            ScopeKind::Component => "component",
            ScopeKind::Block => "block",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Handle {
    pub id: HandleId,
    pub kind: HandleKind,
    /// The scope that owns this handle's lifetime.
    pub owner: ScopeId,
    /// Granted the `durable_job.enqueue` capability, so it is *allowed* to
    /// outlive its creating scope.
    pub durable: bool,
    pub name: String,
    pub span: std::ops::Range<usize>,
}

/// One thing a program does with a handle.
#[derive(Debug, Clone)]
pub enum Op {
    /// The handle is returned from, stored in, or captured by `into` — a scope
    /// which may outlive the owner.
    Escape {
        handle: HandleId,
        into: ScopeId,
        how: &'static str,
        span: std::ops::Range<usize>,
    },
    /// Explicitly detached from its owner with no durable capability.
    Detach {
        handle: HandleId,
        span: std::ops::Range<usize>,
    },
    /// Used at a point where `at` is the innermost live scope.
    Use {
        handle: HandleId,
        at: ScopeId,
        span: std::ops::Range<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeViolation {
    pub code: &'static str,
    pub message: String,
    /// Where the violation happened.
    pub span: std::ops::Range<usize>,
    /// Where the handle was created — charter §16.3's "where the value originated".
    pub origin_span: std::ops::Range<usize>,
    pub help: String,
}

#[derive(Debug, Default)]
pub struct ScopeGraph {
    scopes: Vec<Scope>,
    handles: Vec<Handle>,
    ops: Vec<Op>,
    by_name: HashMap<String, ScopeId>,
}

impl ScopeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scope(&mut self, name: &str, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            id,
            kind,
            parent,
            name: name.to_string(),
        });
        self.by_name.insert(name.to_string(), id);
        id
    }

    pub fn spawn(
        &mut self,
        name: &str,
        kind: HandleKind,
        owner: ScopeId,
        span: std::ops::Range<usize>,
    ) -> HandleId {
        self.spawn_inner(name, kind, owner, false, span)
    }

    /// `durable.spawn` — explicitly permitted to outlive its creating scope,
    /// because the capability was granted.
    pub fn spawn_durable(
        &mut self,
        name: &str,
        kind: HandleKind,
        owner: ScopeId,
        span: std::ops::Range<usize>,
    ) -> HandleId {
        self.spawn_inner(name, kind, owner, true, span)
    }

    fn spawn_inner(
        &mut self,
        name: &str,
        kind: HandleKind,
        owner: ScopeId,
        durable: bool,
        span: std::ops::Range<usize>,
    ) -> HandleId {
        let id = self.handles.len();
        self.handles.push(Handle {
            id,
            kind,
            owner,
            durable,
            name: name.to_string(),
            span,
        });
        id
    }

    pub fn op(&mut self, op: Op) {
        self.ops.push(op);
    }

    /// Is `outer` an ancestor of `inner` (or the same scope)? Ancestors outlive
    /// their descendants, so escaping *into* an ancestor is the dangerous
    /// direction.
    fn outlives(&self, outer: ScopeId, inner: ScopeId) -> bool {
        let mut cur = Some(inner);
        while let Some(c) = cur {
            if c == outer {
                return true;
            }
            cur = self.scopes[c].parent;
        }
        false
    }

    fn path(&self, id: ScopeId) -> String {
        let s = &self.scopes[id];
        format!("{} `{}`", s.kind.noun(), s.name)
    }

    /// Run the checker. An empty result means the program is scope-safe.
    pub fn check(&self) -> Vec<ScopeViolation> {
        let mut out = Vec::new();

        for op in &self.ops {
            match op {
                Op::Escape {
                    handle,
                    into,
                    how,
                    span,
                } => {
                    let h = &self.handles[*handle];
                    if h.durable {
                        continue; // capability granted; this is the legal path
                    }
                    // Escaping into the owner or a descendant is fine — those
                    // die no later than the handle does.
                    if !self.outlives(*into, h.owner) || *into == h.owner {
                        continue;
                    }
                    out.push(ScopeViolation {
                        code: "PW2001",
                        message: format!(
                            "{} handle `{}` escapes the {} that owns it",
                            h.kind.noun(),
                            h.name,
                            self.path(h.owner)
                        ),
                        span: span.clone(),
                        origin_span: h.span.clone(),
                        help: format!(
                            "the handle is {how} into {}, which outlives {}. Keep it inside its \
                             owning scope, or declare it `durable` and supply the \
                             `durable_job.enqueue` capability",
                            self.path(*into),
                            self.path(h.owner)
                        ),
                    });
                }

                Op::Detach { handle, span } => {
                    let h = &self.handles[*handle];
                    if h.durable {
                        continue;
                    }
                    out.push(ScopeViolation {
                        code: "PW2002",
                        message: format!(
                            "ordinary {} `{}` cannot be detached from its scope",
                            h.kind.noun(),
                            h.name
                        ),
                        span: span.clone(),
                        origin_span: h.span.clone(),
                        help: "every ordinary task belongs to a scope and is cancelled when that \
                               scope ends. Use `durable.spawn` with the \
                               `durable_job.enqueue` capability for work that must outlive the \
                               request"
                            .to_string(),
                    });
                }

                Op::Use { handle, at, span } => {
                    let h = &self.handles[*handle];
                    // Using a handle is legal only inside its owner scope or a
                    // descendant of it — anywhere else, the owner may already
                    // have exited.
                    if self.outlives(h.owner, *at) {
                        continue;
                    }
                    out.push(ScopeViolation {
                        code: "PW2003",
                        message: format!(
                            "{} handle `{}` is used outside the {} that owns it",
                            h.kind.noun(),
                            h.name,
                            self.path(h.owner)
                        ),
                        span: span.clone(),
                        origin_span: h.span.clone(),
                        help: format!(
                            "the use is in {}, which is not inside {}. A result arriving after \
                             the owning scope has exited must not be committed",
                            self.path(*at),
                            self.path(h.owner)
                        ),
                    });
                }
            }
        }

        out
    }

    /// A subscription whose *declared* scope outlives the component it is
    /// created in. Charter §7.5's rejected case "subscription outlives component
    /// scope" (corpus R-028).
    pub fn check_declared_scope(
        &self,
        handle: HandleId,
        declared: ScopeId,
        span: std::ops::Range<usize>,
    ) -> Option<ScopeViolation> {
        let h = &self.handles[handle];
        if declared == h.owner || !self.outlives(declared, h.owner) {
            return None;
        }
        Some(ScopeViolation {
            code: "PW2004",
            message: format!(
                "{} `{}` declares {} scope inside a {}",
                h.kind.noun(),
                h.name,
                self.scopes[declared].kind.noun(),
                self.scopes[h.owner].kind.noun()
            ),
            span,
            origin_span: h.span.clone(),
            help: format!(
                "{} outlives {}, so the {} would keep pushing updates into a scope that no \
                 longer exists",
                self.path(declared),
                self.path(h.owner),
                h.kind.noun()
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// route
    ///   └── component
    ///         └── block
    fn nested() -> (ScopeGraph, ScopeId, ScopeId, ScopeId, ScopeId) {
        let mut g = ScopeGraph::new();
        let app = g.scope("app", ScopeKind::Application, None);
        let route = g.scope("/stores/:id", ScopeKind::Route, Some(app));
        let component = g.scope("StorePage", ScopeKind::Component, Some(route));
        let block = g.scope("menu-loop", ScopeKind::Block, Some(component));
        (g, app, route, component, block)
    }

    // --- accepted -----------------------------------------------------------

    #[test]
    fn a_task_used_inside_its_owning_scope_is_fine() {
        let (mut g, _app, _route, component, block) = nested();
        let t = g.spawn("load_menu", HandleKind::Task, component, 0..10);
        g.op(Op::Use {
            handle: t,
            at: block,
            span: 20..30,
        });
        assert!(g.check().is_empty());
    }

    #[test]
    fn escaping_into_a_descendant_scope_is_fine() {
        let (mut g, _app, _route, component, block) = nested();
        let t = g.spawn("load_menu", HandleKind::Task, component, 0..10);
        g.op(Op::Escape {
            handle: t,
            into: block,
            how: "captured",
            span: 20..30,
        });
        assert!(g.check().is_empty());
    }

    #[test]
    fn a_durable_job_may_outlive_its_creating_scope() {
        let (mut g, app, _route, component, _block) = nested();
        let j = g.spawn_durable("send_receipt", HandleKind::Task, component, 0..10);
        g.op(Op::Escape {
            handle: j,
            into: app,
            how: "returned",
            span: 20..30,
        });
        g.op(Op::Detach {
            handle: j,
            span: 40..50,
        });
        assert!(g.check().is_empty(), "{:?}", g.check());
    }

    // --- rejected -----------------------------------------------------------

    #[test]
    fn task_handle_escaping_its_owner_is_rejected() {
        let (mut g, _app, route, component, _block) = nested();
        let t = g.spawn("load_menu", HandleKind::Task, component, 5..15);
        g.op(Op::Escape {
            handle: t,
            into: route,
            how: "returned",
            span: 40..50,
        });
        let v = g.check();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].code, "PW2001");
        assert!(v[0].message.contains("escapes"), "{}", v[0].message);
        // charter §16.3: the diagnostic must point at where the handle came from
        assert_eq!(v[0].origin_span, 5..15);
        assert!(v[0].help.contains("durable"), "{}", v[0].help);
    }

    #[test]
    fn detached_task_without_durable_capability_is_rejected() {
        let (mut g, _app, _route, component, _block) = nested();
        let t = g.spawn("record_analytics", HandleKind::Task, component, 5..15);
        g.op(Op::Detach {
            handle: t,
            span: 40..50,
        });
        let v = g.check();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].code, "PW2002");
        assert!(v[0].help.contains("durable.spawn"), "{}", v[0].help);
    }

    #[test]
    fn a_dead_component_cannot_accept_a_late_result() {
        // The task belongs to a component; the use happens in a sibling route
        // scope that is NOT inside it.
        let (mut g, app, _route, component, _block) = nested();
        let other = g.scope("/other", ScopeKind::Route, Some(app));
        let t = g.spawn("load_menu", HandleKind::Task, component, 5..15);
        g.op(Op::Use {
            handle: t,
            at: other,
            span: 60..70,
        });
        let v = g.check();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].code, "PW2003");
        assert!(v[0].help.contains("must not be committed"), "{}", v[0].help);
    }

    #[test]
    fn subscription_declaring_application_scope_inside_a_component_is_rejected() {
        let (mut g, app, _route, component, _block) = nested();
        let s = g.spawn("OrderTracking", HandleKind::Subscription, component, 5..15);
        let v = g
            .check_declared_scope(s, app, 30..40)
            .expect("should be rejected");
        assert_eq!(v.code, "PW2004");
        assert!(v.message.contains("application scope"), "{}", v.message);
    }

    #[test]
    fn subscription_declaring_component_scope_inside_a_component_is_fine() {
        let (mut g, _app, _route, component, _block) = nested();
        let s = g.spawn("OrderTracking", HandleKind::Subscription, component, 5..15);
        assert!(g.check_declared_scope(s, component, 30..40).is_none());
    }

    // --- the checker must be able to go red AND green ------------------------

    #[test]
    fn the_checker_can_distinguish_safe_from_unsafe() {
        // docs/RISK_QUEUE.md: a check that cannot fail is not a check.
        let (mut g, _app, route, component, block) = nested();
        let safe = g.spawn("safe", HandleKind::Task, component, 0..1);
        g.op(Op::Escape {
            handle: safe,
            into: block,
            how: "captured",
            span: 2..3,
        });
        assert!(g.check().is_empty());

        let unsafe_handle = g.spawn("unsafe", HandleKind::Task, component, 4..5);
        g.op(Op::Escape {
            handle: unsafe_handle,
            into: route,
            how: "returned",
            span: 6..7,
        });
        assert_eq!(g.check().len(), 1);
    }

    #[test]
    fn resources_and_subscriptions_obey_the_same_rule_as_tasks() {
        for kind in [HandleKind::Resource, HandleKind::Subscription] {
            let (mut g, _app, route, component, _block) = nested();
            let h = g.spawn("h", kind, component, 0..1);
            g.op(Op::Escape {
                handle: h,
                into: route,
                how: "stored",
                span: 2..3,
            });
            let v = g.check();
            assert_eq!(v.len(), 1, "{kind:?} should be rejected");
            assert!(v[0].message.contains(kind.noun()));
        }
    }
}
