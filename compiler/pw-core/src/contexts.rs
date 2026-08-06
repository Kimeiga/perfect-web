//! Execution contexts — where work in a body actually runs.
//!
//! # Why this is one concept and not several exceptions
//!
//! Three separate span lists grew here, each added after a false positive:
//! `deferred_spans` (work that does not happen during render), then
//! `delegated_spans` (a clause that *names* a declaration rather than calling
//! it), and then a grammar fix so `query Cart(session)` parsed as one
//! expression. Each was correct and each was local, and a fourth was going to
//! be needed for the next construct.
//!
//! Architect ruling, 2026-08-06:
//!
//! > That prevents future fixes from turning into a growing list of syntax
//! > exceptions.
//!
//! They are all one thing. A body mentions work that executes **somewhere
//! else**, and two questions have separate answers:
//!
//! ```text
//!               effects flow here?     a value flows here?
//! query          no  (query placement)  yes — result and its label
//! command        no  (command placement) yes — result or transition
//! subscription   no  (owned scope)      yes — streamed values
//! <stream>       no  (stream task)      yes — eventually
//! on:press       no  (interaction)      no — the handler is a value, not its result
//! post_paint     no  (later frame)      no
//! frame phases   no  (later frame)      no
//! painter        no  (paint pipeline)   no
//! depends_on     no  (the named decl)   no — it is a declaration, not a use
//! ```
//!
//! Answering "no" to the first is what stopped a page that declares a query
//! dependency from being reported for reaching the database while rendering.
//! Answering "yes" to the second is what keeps a session-scoped query result
//! session-scoped once the page holds it. A model with only one flag would get
//! one of those wrong, which is what each individual patch did.

use crate::hir::{AttrValue, Body, Expr, Node, Span};

/// Where a piece of work runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// The enclosing body itself.
    Render,
    /// A query, at the query's own placement.
    Query,
    /// A command, at the command's own placement.
    Command,
    /// A subscription's owned scope.
    Subscription,
    /// A streamed region's task, after the shell is sent.
    StreamTask,
    /// An event handler, when the user acts.
    Interaction,
    /// A later frame phase.
    FramePhase,
    /// A named declaration, referred to rather than called.
    NamedDeclaration,
}

impl Context {
    /// Does this work belong to a **different declaration**?
    ///
    /// If so its effects are not this body's at all: a query runs at the
    /// query's placement and pays for its own database read.
    ///
    /// Distinct from [`Context::runs_later`], and collapsing the two was a
    /// real regression — a first version answered "not this body's" for every
    /// context, which removed the effects inside `measure { .. }` and
    /// `mutate { .. }` blocks and took the entire frame-phase family down with
    /// it. A frame phase IS this body's work; it happens at a different
    /// *time*, not in a different *declaration*.
    pub fn belongs_to_another_declaration(self) -> bool {
        matches!(
            self,
            Context::Query | Context::Command | Context::Subscription | Context::NamedDeclaration
        )
    }

    /// Does this work happen after the render that mentions it?
    ///
    /// Its effects are still this body's — they go in its row — but a
    /// render-time restriction does not apply, because the render is over.
    pub fn runs_later(self) -> bool {
        matches!(
            self,
            Context::StreamTask | Context::Interaction | Context::FramePhase
        )
    }

    /// Does the enclosing body receive a value from it?
    pub fn yields_value(self) -> bool {
        matches!(
            self,
            Context::Query | Context::Command | Context::Subscription | Context::StreamTask
        )
    }

    pub fn describe(self) -> &'static str {
        match self {
            Context::Render => "this body",
            Context::Query => "the query's own placement",
            Context::Command => "the command's own placement",
            Context::Subscription => "the subscription's scope",
            Context::StreamTask => "a streamed region, after the shell is sent",
            Context::Interaction => "an event handler, when the user acts",
            Context::FramePhase => "a later frame phase",
            Context::NamedDeclaration => "the declaration it names",
        }
    }
}

/// A span of a body whose work runs somewhere other than here.
#[derive(Debug, Clone)]
pub struct Elsewhere {
    pub span: Span,
    pub context: Context,
}

/// Every region of this body that executes in another context.
pub fn elsewhere(body: &Body) -> Vec<Elsewhere> {
    const NAMING_CLAUSES: &[&str] = &["depends_on", "invalidates_on", "derives_from"];
    let mut out = Vec::new();

    for id in body.walk() {
        match body.expr(id) {
            Expr::Keyword { keyword, .. } => {
                let context = match keyword.as_str() {
                    "query" => Context::Query,
                    "command" => Context::Command,
                    "subscription" | "subscribe" => Context::Subscription,
                    "post_paint" | "frame" | "animate" => Context::FramePhase,
                    "task" | "resource" => Context::Interaction,
                    _ => continue,
                };
                // The whole statement, not its block: `post_paint !{ .. } { .. }`
                // writes a row between the keyword and the braces, so the block
                // is a sibling of the row rather than the statement's only child.
                out.push(Elsewhere {
                    span: body.expr_span(id),
                    context,
                });
            }
            // A clause that NAMES declarations: `depends_on Store(id), Menu(id)`
            // looks exactly like two calls and is not one.
            Expr::Block { stmts } => {
                let mut naming = false;
                for s in stmts {
                    match body.expr(*s) {
                        Expr::Name(n) => naming = NAMING_CLAUSES.contains(&n.as_str()),
                        Expr::Call { .. } if naming => out.push(Elsewhere {
                            span: body.expr_span(*s),
                            context: Context::NamedDeclaration,
                        }),
                        _ => naming = false,
                    }
                }
            }
            Expr::Template { roots, .. } => {
                for n in body.walk_markup(roots) {
                    let Node::Element { tag, attrs, .. } = body.node(n) else {
                        continue;
                    };
                    for a in attrs {
                        let context = if a.name.starts_with("on:") {
                            Context::Interaction
                        } else if tag == "stream" && a.name == "query" {
                            Context::StreamTask
                        } else {
                            continue;
                        };
                        if let AttrValue::Expr(e) = a.value {
                            out.push(Elsewhere {
                                span: body.expr_span(e),
                                context,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The context a span sits inside, or `Render`.
pub fn context_of(regions: &[Elsewhere], span: &Span) -> Context {
    regions
        .iter()
        .filter(|r| r.span.start <= span.start && span.end <= r.span.end)
        // The innermost region wins: a handler inside a frame is an
        // interaction, not a frame phase.
        .min_by_key(|r| r.span.end - r.span.start)
        .map(|r| r.context)
        .unwrap_or(Context::Render)
}

/// Does this span's work belong to the enclosing body's effect row at all?
pub fn effects_belong_here(regions: &[Elsewhere], span: &Span) -> bool {
    !context_of(regions, span).belongs_to_another_declaration()
}

/// Does this span's work happen while the enclosing body renders?
pub fn at_render_time(regions: &[Elsewhere], span: &Span) -> bool {
    !context_of(regions, span).runs_later()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use pw_syntax::parse_tree;

    fn body_of(src: &str) -> (crate::hir::Hir, crate::hir::BodyId) {
        let hir = lower_file(src, &parse_tree(src).green);
        let id = hir
            .all_decls()
            .find_map(|(_, d)| d.body)
            .expect("a declaration with a body");
        (hir, id)
    }

    /// A suppressor is how a check quietly stops measuring, so it gets a
    /// control: naming a declaration is foreign, calling one is not, and the
    /// two appear in the same body with the same text.
    #[test]
    fn naming_a_declaration_is_foreign_but_calling_one_is_not() {
        let src = "module m\n\
                   \n\
                   materialize F(id: Int) {\n\
                       placement edge\n\
                       depends_on Store(id)\n\
                       view { Store(id) }\n\
                   }\n";
        let (hir, id) = body_of(src);
        let body = hir.body(id);
        let regions = elsewhere(body);
        let named: Vec<&Elsewhere> = regions
            .iter()
            .filter(|r| r.context == Context::NamedDeclaration)
            .collect();

        assert_eq!(named.len(), 1, "only the `depends_on` member");
        assert_eq!(&src[named[0].span.start..named[0].span.end], "Store(id)");
        assert!(
            named[0].span.start < src.find("view").expect("view clause"),
            "the foreign span is the one in `depends_on`, not the one in `view`"
        );
    }

    /// The distinction that a first version of this module got wrong.
    ///
    /// A frame phase is this body's work at a different TIME. A query is
    /// another declaration's work entirely. Collapsing them removed the
    /// effects inside `measure { .. }` and took the whole frame-phase family
    /// down with it — the corpus dropped from 44 to 42 in one edit.
    #[test]
    fn a_later_time_is_not_another_declaration() {
        assert!(Context::Query.belongs_to_another_declaration());
        assert!(!Context::Query.runs_later());

        assert!(!Context::FramePhase.belongs_to_another_declaration());
        assert!(Context::FramePhase.runs_later());

        assert!(!Context::Interaction.belongs_to_another_declaration());
        assert!(Context::Interaction.runs_later());

        // Render is neither.
        assert!(!Context::Render.belongs_to_another_declaration());
        assert!(!Context::Render.runs_later());
    }

    /// A value flows out of exactly the contexts that produce one.
    #[test]
    fn only_the_contexts_that_produce_a_value_yield_one() {
        for c in [
            Context::Query,
            Context::Command,
            Context::Subscription,
            Context::StreamTask,
        ] {
            assert!(c.yields_value(), "{c:?} produces a value for this body");
        }
        for c in [
            Context::Render,
            Context::Interaction,
            Context::FramePhase,
            Context::NamedDeclaration,
        ] {
            assert!(!c.yields_value(), "{c:?} does not");
        }
    }

    /// The innermost region wins.
    #[test]
    fn a_handler_inside_a_frame_is_an_interaction() {
        let src = "module m\n\
                   \n\
                   component C() {\n\
                       frame {\n\
                           measure { self.height() }\n\
                       }\n\
                   }\n";
        let (hir, id) = body_of(src);
        let body = hir.body(id);
        let regions = elsewhere(body);
        let inner = src.find("self.height()").expect("the call");
        let span = inner..inner + "self.height()".len();
        assert_eq!(context_of(&regions, &span), Context::FramePhase);
        assert!(!at_render_time(&regions, &span));
        assert!(
            effects_belong_here(&regions, &span),
            "a frame phase's effects are still this declaration's"
        );
    }
}
