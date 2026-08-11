//! Syntax tree → HIR.
//!
//! **This is the only module in `pw-core` permitted to see a syntax node**
//! (ADR-0014 property 3, enforced by `hir::tests::only_lower_touches_syntax_nodes`).
//!
//! Lowering is total: anything unrecognised becomes [`Expr::Error`] carrying
//! its span, so one bad expression does not cost a checker the rest of the
//! body. It performs no name resolution and no meaning-changing desugaring —
//! see ADR-0014 for why `a.b` stays a field access and `a |> f` stays a
//! binary node.

use pw_syntax::kind::{SyntaxKind as K, SyntaxNode};

use crate::hir::*;

/// Lower a parsed source file.
pub fn lower_file(src: &str, root: &SyntaxNode) -> Hir {
    let mut lo = Lowerer {
        hir: Hir::default(),
        src,
    };
    let mut decls = Vec::new();
    let mut module_name = String::new();
    let mut module_span = 0..0;

    for node in root.children() {
        if node.kind() == K::ModuleDecl {
            module_name = first_name(&node).unwrap_or_default();
            module_span = span_of(&node);
            continue;
        }
        if let Some(id) = lo.decl(&node) {
            decls.push(id);
        }
    }

    let m = Module {
        name: module_name,
        decls,
    };
    lo.hir.modules.alloc(m, module_span);
    lo.hir
}

struct Lowerer<'a> {
    hir: Hir,
    src: &'a str,
}

/// Per-body arenas, filled while walking one body (ADR-0014 property 1).
#[derive(Default)]
struct BodyBuilder {
    exprs: Arena<Expr>,
    pats: Arena<Pattern>,
    types: Arena<TypeRef>,
    nodes: Arena<Node>,
}

impl BodyBuilder {
    fn expr(&mut self, e: Expr, span: Span) -> ExprId {
        ExprId(self.exprs.alloc(e, span))
    }
    fn pat(&mut self, p: Pattern, span: Span) -> PatternId {
        PatternId(self.pats.alloc(p, span))
    }
    fn ty(&mut self, t: TypeRef, span: Span) -> TypeRefId {
        TypeRefId(self.types.alloc(t, span))
    }
    fn node(&mut self, n: Node, span: Span) -> NodeId {
        NodeId(self.nodes.alloc(n, span))
    }
}

fn span_of(n: &SyntaxNode) -> Span {
    let r = n.text_range();
    usize::from(r.start())..usize::from(r.end())
}

fn text(src: &str, n: &SyntaxNode) -> String {
    let s = span_of(n);
    src[s].to_string()
}

/// The first `Name` node's text, which is how the grammar records identifiers.
fn first_name(n: &SyntaxNode) -> Option<String> {
    n.children()
        .find(|c| c.kind() == K::Name)
        .map(|c| c.text().to_string())
}

fn first_name_span(n: &SyntaxNode) -> Option<Span> {
    n.children()
        .find(|c| c.kind() == K::Name)
        .map(|c| span_of(&c))
}

/// Significant (non-trivia) tokens of a node, excluding those inside children.
fn own_tokens(n: &SyntaxNode) -> Vec<pw_syntax::SyntaxToken> {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .collect()
}

fn decl_kind_of(node: &SyntaxNode, src: &str) -> DeclKind {
    match node.kind() {
        K::FnDecl => DeclKind::Fn,
        K::LetDecl => DeclKind::Let,
        K::ImportDecl => DeclKind::Import,
        K::OpaqueDecl => DeclKind::Opaque,
        K::TypeDecl | K::RecordDecl | K::UnionDecl => DeclKind::Type,
        // `view`/`component`/`page` and the resource nouns are distinguished by
        // their leading keyword, which the grammar keeps as a bare token.
        K::EffectDecl => DeclKind::Effect,
        K::PreludeDecl => DeclKind::Prelude,

        K::UiDecl | K::ResourceDecl => {
            let kw = own_tokens(node)
                .iter()
                .map(|t| t.text().to_string())
                .find(|t| {
                    !matches!(
                        t.as_str(),
                        "public" | "session" | "private" | "internal" | "local"
                    )
                })
                .unwrap_or_default();
            match kw.as_str() {
                "view" => DeclKind::View,
                "component" => DeclKind::Component,
                "page" => DeclKind::Page,
                "query" => DeclKind::Query,
                "command" => DeclKind::Command,
                "subscription" => DeclKind::Subscription,
                "resource" => DeclKind::Resource,
                "materialize" => DeclKind::Materialize,
                "event" => DeclKind::Event,
                "task" => DeclKind::Task,
                _ => {
                    let _ = src;
                    DeclKind::Other
                }
            }
        }
        _ => DeclKind::Other,
    }
}

/// The first expression node inside a synthetic wrapper's function body.
fn first_expr(root: &SyntaxNode) -> Option<SyntaxNode> {
    root.descendants()
        .find(|n| n.kind() == K::BlockExpr)?
        .children()
        .find(|c| is_expr(c.kind()))
}

/// The shortest synthetic program that puts an expression in expression
/// position. Padded to the hole's own offset so every span the sub-parse
/// produces already points into the real file.
const HOLE_PREFIX: &str = "module h\nfn h()->(){";

impl Lowerer<'_> {
    /// The `{expr}` holes in a string literal, lowered with the REAL grammar.
    ///
    /// The alternative — a small parser for what may appear in a hole — is the
    /// second-mini-language mistake `docs/NEXT.md` rejected for effect parsing,
    /// and it fails the same way: the two grammars disagree about something and
    /// the checker quietly analyses a different program from the one that runs.
    ///
    /// So the hole is handed to `pw_syntax::parse` inside a synthetic wrapper
    /// whose prefix is space-padded to exactly the hole's offset in the real
    /// source. Every span that comes back is therefore already correct, with no
    /// arithmetic to get wrong.
    fn interpolations(&mut self, b: &mut BodyBuilder, text: &str, at: usize) -> Vec<ExprId> {
        let mut out = Vec::new();
        let mut rest = text;
        let mut base = at;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { break };
            let hole = &after[..close];
            let hole_at = base + open + 1;
            if !hole.trim().is_empty() && hole_at >= HOLE_PREFIX.len() {
                let pad = hole_at - HOLE_PREFIX.len();
                let mut synthetic = String::with_capacity(hole_at + hole.len() + 1);
                synthetic.push_str(HOLE_PREFIX);
                synthetic.push_str(&" ".repeat(pad));
                synthetic.push_str(hole);
                synthetic.push('}');

                let parsed = pw_syntax::parse_tree(&synthetic);
                let mut sub = Lowerer {
                    hir: std::mem::take(&mut self.hir),
                    src: &synthetic,
                };
                if let Some(e) = first_expr(&parsed.green) {
                    let id = sub.expr(b, &e);
                    out.push(id);
                }
                self.hir = sub.hir;
            }
            base = base + open + 1 + close + 1;
            rest = &after[close + 1..];
        }
        out
    }

    fn decl(&mut self, node: &SyntaxNode) -> Option<DeclId> {
        if !matches!(
            node.kind(),
            K::FnDecl
                | K::UiDecl
                | K::ResourceDecl
                | K::EffectDecl
                | K::PreludeDecl
                | K::LetDecl
                | K::TypeDecl
                | K::RecordDecl
                | K::UnionDecl
                | K::OpaqueDecl
                | K::ImportDecl
                | K::ErrorDecl
        ) {
            return None;
        }

        let kind = decl_kind_of(node, self.src);
        let name = first_name(node).unwrap_or_default();
        let name_span = first_name_span(node).unwrap_or_else(|| span_of(node));
        let declared_effects = node
            .children()
            .find(|c| c.kind() == K::EffectRow)
            .map(|row| self.effect_row(&row));

        // Reserve the id before lowering the body, so a nested declaration can
        // name its parent and the body can name its owner.
        let id = DeclId(self.hir.decls.alloc(
            Decl {
                name,
                name_span,
                kind,
                params: self.params(node),
                ret: self.return_type(node),
                ret_args: self.return_type_args(node),
                variants: self.variants(node),
                fields: self.record_fields(node),
                policies: self.policies(node),
                imports: imported_names(node),
                visibility: visibility_of(node),
                opaque_of: self.opaque_of(node),
                type_params: self.type_params(node),
                declared_effects,
                body: None,
                children: Vec::new(),
            },
            span_of(node),
        ));

        // The policies are taken back out of the declaration, lowered against
        // the body's arena, and put back. A policy whose domain is executable
        // code needs an `ExprId`, and that arena does not exist until the body
        // is built.
        let mut policies = std::mem::take(
            &mut self
                .hir
                .decls
                .get_mut(id.index())
                .expect("just allocated")
                .policies,
        );
        let (body, children) = match node.children().find(|c| c.kind() == K::Body) {
            Some(b) => self.body(id, &b, &mut policies),
            None => (None, Vec::new()),
        };
        // Internal invariant, not a claim about the program: `id` was
        // returned by `alloc` four lines up and arenas do not shrink. No `.pw`
        // source can make this absent.
        let d = self.hir.decls.get_mut(id.index()).expect("just allocated");
        d.policies = policies;
        d.body = body;
        d.children = children;
        Some(id)
    }

    fn params(&self, node: &SyntaxNode) -> Vec<Param> {
        node.children()
            .find(|c| c.kind() == K::ParamList)
            .map(|l| {
                l.children()
                    .filter(|c| c.kind() == K::Param)
                    .map(|p| {
                        let t = p.children().find(|c| c.kind() == K::TypeRef);
                        Param {
                            name: first_name(&p).unwrap_or_default(),
                            // The complete type, built once. `DeclaredType`
                            // keeps the head and its arguments together, so a
                            // consumer has to ask for the head alone by name.
                            ty: t
                                .as_ref()
                                .map(|t| crate::hir::DeclaredType::new(type_path(t), type_args(t))),
                            span: span_of(&p),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The type after `->`. It is the `TypeRef` that is a direct child of the
    /// declaration, so it cannot be confused with a parameter's type (those sit
    /// inside `Param`) or a field's.
    fn return_type(&self, node: &SyntaxNode) -> Option<String> {
        node.children()
            .find(|c| c.kind() == K::TypeRef)
            .map(|t| type_path(&t))
    }

    /// The return type's arguments, as written.
    /// The return type's arguments **as written**, nesting included.
    ///
    /// `Result<List<MenuItemId>, StoreError>` gives `["List<MenuItemId>",
    /// "StoreError"]`, not `["List", "StoreError"]`. Heads were enough while
    /// every consumer wanted a carrier's name; they are not enough to answer
    /// "what is one element of this", and a projection back to a head is one
    /// `split('<')` away. The reverse is not recoverable.
    fn return_type_args(&self, node: &SyntaxNode) -> Vec<String> {
        node.children()
            .find(|c| c.kind() == K::TypeRef)
            .and_then(|t| t.children().find(|c| c.kind() == K::TypeArgList))
            .map(|l| {
                l.children()
                    .filter(|c| c.kind() == K::TypeRef)
                    .map(|t| t.text().to_string().trim().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn record_fields(&self, node: &SyntaxNode) -> Option<Vec<Param>> {
        let list = node.children().find(|c| c.kind() == K::FieldList)?;
        Some(
            list.children()
                .filter(|c| c.kind() == K::Field)
                .map(|f| {
                    let t = f.children().find(|c| c.kind() == K::TypeRef);
                    Param {
                        name: first_name(&f).unwrap_or_default(),
                        ty: t
                            .as_ref()
                            .map(|t| crate::hir::DeclaredType::new(type_path(t), type_args(t))),
                        span: span_of(&f),
                    }
                })
                .collect(),
        )
    }

    /// A declaration's policy block, values kept as written.
    /// A declaration's policy clauses.
    ///
    /// Two positions, because the language has two. Most declarations write
    /// their policies between the signature and the body, where the list is a
    /// direct child. A `materialize` block writes them INSIDE its braces —
    /// corpus A-009 and R-017 are the specification — so the list is a
    /// grandchild through `Body > BlockExpr`. Looking only at direct children
    /// found none of a fragment's policies, and an empty policy list reads as
    /// "declared nothing" rather than "not found".
    fn policies(&self, node: &SyntaxNode) -> Vec<Policy> {
        node.children()
            .find(|c| c.kind() == K::PolicyList)
            .or_else(|| {
                node.children()
                    .find(|c| c.kind() == K::Body)?
                    .children()
                    .find(|c| c.kind() == K::BlockExpr)?
                    .children()
                    .find(|c| c.kind() == K::PolicyList)
            })
            .map(|list| {
                list.children()
                    .filter(|c| c.kind() == K::Policy)
                    .map(|p| {
                        let whole = text(self.src, &p);
                        let trimmed = whole.trim_end();
                        // The grammar keeps a policy keyword as a bare token,
                        // so the name is the first word and the value is the
                        // rest — with the run of alignment spaces collapsed,
                        // because `freshness      30.seconds` and
                        // `freshness 30.seconds` declare the same policy.
                        let (name, value) = match trimmed.split_once(char::is_whitespace) {
                            Some((n, v)) => {
                                (n.to_string(), pw_syntax::collapse_policy_whitespace(v))
                            }
                            None => (trimmed.to_string(), String::new()),
                        };
                        Policy {
                            name,
                            value,
                            transition: None,
                            span: span_of(&p),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn variants(&self, node: &SyntaxNode) -> Option<Vec<VariantDef>> {
        let list = node.children().find(|c| c.kind() == K::VariantList)?;
        Some(
            list.children()
                .filter(|c| c.kind() == K::Variant)
                .map(|v| VariantDef {
                    name: first_name(&v).unwrap_or_default(),
                    fields: v
                        .children()
                        .filter(|c| c.kind() == K::TypeRef)
                        .map(|t| type_path(&t))
                        .collect(),
                    span: span_of(&v),
                })
                .collect(),
        )
    }

    fn opaque_of(&self, node: &SyntaxNode) -> Option<String> {
        if node.kind() != K::OpaqueDecl {
            return None;
        }
        node.children()
            .find(|c| c.kind() == K::TypeRef)
            .map(|t| type_path(&t))
    }

    fn effect_row(&self, row: &SyntaxNode) -> Vec<EffectRef> {
        row.children()
            .filter(|c| c.kind() == K::EffectRef)
            .map(|e| {
                // `database.read<Stores>` — the path is the name, and the type
                // arguments come from the `TypeArgList` the grammar already
                // built. Taken from the TREE rather than sliced back out of
                // the text: the parser has answered this question once, and a
                // second answer is how the two come to disagree.
                let head = e.children().find(|c| c.kind() == K::TypeRef);
                let path = head
                    .as_ref()
                    .and_then(|t| t.children().find(|c| c.kind() == K::Name))
                    .map(|n| n.text().to_string())
                    .unwrap_or_else(|| text(self.src, &e).trim().to_string());
                let args = head
                    .as_ref()
                    .and_then(|t| t.children().find(|c| c.kind() == K::TypeArgList))
                    .map(|list| {
                        list.children()
                            .filter(|c| c.kind() == K::TypeRef)
                            .map(|a| type_path(&a))
                            .collect()
                    })
                    .unwrap_or_default();
                EffectRef {
                    path,
                    written: text(self.src, &e).trim().to_string(),
                    args,
                    span: span_of(&e),
                }
            })
            .collect()
    }

    /// The type parameters a declaration binds: the `TypeArgList` that is the
    /// declaration's OWN child, as `type_params()` in the grammar builds it.
    ///
    /// Its own child, deliberately. A `TypeArgList` nested inside a `TypeRef`
    /// is an argument at a use site — `-> Result<Store, StoreError>` — and
    /// reading one of those as a parameter binding would give every function
    /// returning a generic type a set of parameters it never declared.
    fn type_params(&self, node: &SyntaxNode) -> Vec<String> {
        node.children()
            .find(|c| c.kind() == K::TypeArgList)
            .map(|list| {
                list.children()
                    .filter(|c| c.kind() == K::TypeRef)
                    .map(|p| type_path(&p))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Lower a `Body` node, hoisting any nested declarations out of it.
    ///
    /// `policies` are lowered into the SAME arena, so a policy whose domain is
    /// executable code gets a real expression with real spans — see
    /// [`Self::policy_terms`].
    fn body(
        &mut self,
        owner: DeclId,
        node: &SyntaxNode,
        policies: &mut [Policy],
    ) -> (Option<BodyId>, Vec<DeclId>) {
        let Some(block) = node.children().find(|c| c.kind() == K::BlockExpr) else {
            return (None, Vec::new());
        };

        // Nested declarations are declarations, not expressions: a `fn` inside
        // a `component` gets its own DeclId and its own body.
        let mut children = Vec::new();
        for c in block.children() {
            if let Some(child) = self.decl(&c) {
                children.push(child);
            }
        }

        let mut b = BodyBuilder::default();
        let root = self.block(&mut b, &block);
        self.policy_terms(&mut b, policies);
        let body = Body {
            owner,
            exprs: b.exprs,
            pats: b.pats,
            types: b.types,
            nodes: b.nodes,
            root,
        };
        (
            Some(BodyId(self.hir.bodies.alloc(body, span_of(node)))),
            children,
        )
    }

    /// **A policy value that is a transition becomes one.**
    ///
    /// Architect ruling, 2026-08-11 (ADR-0025):
    ///
    /// > An optimistic clause identifies a resource entry and binds its current
    /// > value; its body is an ordinary Pleris transition expression.
    ///
    /// Which heads take this shape is `crate::policy`'s answer — one table, not
    /// a second list here. Both expressions are allocated in the declaration's
    /// own arena and are **not reachable from `root`**, deliberately: they are
    /// terms, so name resolution must see them, and they run in a different
    /// execution context, so the declaration's effect row must not absorb them.
    /// `Decl::transitions` is how a consumer asks for them by name rather than
    /// finding them by walking.
    ///
    /// Spans are offset back into the file. The sub-parse sees only the value
    /// text, so without this a diagnostic would underline column 3 of whatever
    /// line happened to be there.
    fn policy_terms(&mut self, b: &mut BodyBuilder, policies: &mut [Policy]) {
        for p in policies.iter_mut() {
            if crate::policy::domain_of(&p.name) != Some(crate::policy::Domain::Transition) {
                continue;
            }
            if p.value.trim().is_empty() {
                continue;
            }
            // Where the value starts in the file: the policy's span covers
            // `<head> <value>`, and the head plus the whitespace after it is
            // what precedes the value.
            let whole = &self.src[p.span.clone()];
            let offset = p.span.start
                + whole
                    .find(&p.value)
                    .unwrap_or_else(|| p.name.len().min(whole.len()));
            let parsed = pw_syntax::parse_transition_clause(&p.value);
            let Some(clause) = parsed
                .green
                .children()
                .find(|c| c.kind() == K::TransitionClause)
            else {
                continue;
            };
            let exprs: Vec<SyntaxNode> = clause.children().filter(|c| is_expr(c.kind())).collect();
            let (Some(target_node), Some(body_node)) = (exprs.first(), exprs.get(1)) else {
                continue;
            };
            let Some(name_node) = clause.children().find(|c| c.kind() == K::Name) else {
                continue;
            };

            // Lowered by a `Lowerer` over the VALUE's text, because `span_of`
            // reads a node's range and the sub-parse's ranges start at zero.
            // Then every span it allocated is moved back into the file — one
            // shift over the arena suffix, rather than a second span convention
            // every node kind would have to honour.
            let before = (b.exprs.len(), b.pats.len(), b.types.len(), b.nodes.len());
            let mut sub = Lowerer {
                hir: std::mem::take(&mut self.hir),
                src: &p.value,
            };
            let target = sub.expr(b, target_node);
            let body = sub.expr(b, body_node);
            self.hir = std::mem::take(&mut sub.hir);
            b.exprs.shift_spans_from(before.0, offset);
            b.pats.shift_spans_from(before.1, offset);
            b.types.shift_spans_from(before.2, offset);
            b.nodes.shift_spans_from(before.3, offset);

            let bs = span_of(&name_node);
            p.transition = Some(crate::hir::Transition {
                target,
                binder: text(&p.value, &name_node).trim().to_string(),
                binder_span: (bs.start + offset)..(bs.end + offset),
                body,
            });
        }
    }

    fn block(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> ExprId {
        // A nested declaration already has its own DeclId and body; it is not
        // also a statement of the enclosing one.
        let stmts = node
            .children()
            .filter(|c| is_expr(c.kind()))
            .map(|c| self.expr(b, &c))
            .collect();
        b.expr(Expr::Block { stmts }, span_of(node))
    }

    fn expr(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> ExprId {
        let span = span_of(node);
        match node.kind() {
            K::BlockExpr => self.block(b, node),

            K::LiteralExpr => {
                let t = own_tokens(node);
                let lit = match t.first().map(|t| (t.kind(), t.text().to_string())) {
                    Some((K::Int, s)) => Literal::Int(s),
                    Some((K::Float, s)) => Literal::Float(s),
                    Some((K::Str, s)) => {
                        // A string with `{..}` holes is not a literal: the holes
                        // are expressions, and a checker that has to search the
                        // characters for them cannot see into `{token.value}`.
                        if s.contains('{') {
                            let start = span.start;
                            let parts = self.interpolations(b, &s, start);
                            if !parts.is_empty() {
                                return b.expr(Expr::Interpolated { text: s, parts }, span);
                            }
                        }
                        Literal::Str(s)
                    }
                    Some((K::UnterminatedStr, s)) => Literal::UnterminatedStr(s),
                    _ => return b.expr(Expr::Error, span),
                };
                b.expr(Expr::Literal(lit), span)
            }

            K::NameExpr => b.expr(Expr::Name(text(self.src, node).trim().to_string()), span),

            K::FieldExpr => {
                // `base . name` — the base is the only child expression.
                let base = match node.children().next() {
                    Some(c) => self.expr(b, &c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let name = own_tokens(node)
                    .iter()
                    .rev()
                    .find(|t| t.kind() == K::Ident)
                    .map(|t| t.text().to_string())
                    .unwrap_or_default();
                b.expr(Expr::Field { base, name }, span)
            }

            K::CallExpr => {
                let mut kids = node.children();
                let callee = match kids.next() {
                    Some(c) => self.expr(b, &c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let args = node
                    .children()
                    .find(|c| c.kind() == K::ArgList)
                    .map(|l| self.args(b, &l))
                    .unwrap_or_default();
                b.expr(Expr::Call { callee, args }, span)
            }

            K::LambdaExpr => {
                // `x => e`: the body is the last child; everything before it is
                // either a parameter or a call-shaped descriptor.
                let kids: Vec<_> = node.children().collect();
                let Some((last, init)) = kids.split_last() else {
                    let body = b.expr(Expr::Error, span.clone());
                    return b.expr(
                        Expr::Lambda {
                            descriptor: None,
                            params: Vec::new(),
                            body,
                        },
                        span,
                    );
                };
                let mut descriptor = None;
                let mut params = Vec::new();
                for p in init {
                    if is_param_shaped(p.kind()) {
                        params.push(self.param_pattern(b, p));
                    } else {
                        descriptor = Some(self.expr(b, p));
                    }
                }
                let body = self.expr(b, last);
                b.expr(
                    Expr::Lambda {
                        descriptor,
                        params,
                        body,
                    },
                    span,
                )
            }

            K::BinaryExpr => {
                let kids: Vec<_> = node.children().collect();
                let op = own_tokens(node)
                    .first()
                    .map(|t| bin_op(t.kind(), t.text()))
                    .unwrap_or(BinOp::Cmp("?".into()));
                let lhs = match kids.first() {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let rhs = match kids.get(1) {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                b.expr(Expr::Binary { op, lhs, rhs }, span)
            }

            K::CastExpr => {
                let kids: Vec<_> = node.children().collect();
                let value = match kids.first() {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let ty = match kids.iter().find(|c| c.kind() == K::TypeRef) {
                    Some(t) => self.type_ref(b, t),
                    None => b.ty(
                        crate::hir::TypeRef {
                            path: String::new(),
                            args: vec![],
                        },
                        span.clone(),
                    ),
                };
                b.expr(Expr::Cast { value, ty }, span)
            }

            K::UnaryExpr => {
                let op = match own_tokens(node).first().map(|t| t.kind()) {
                    Some(K::Bang) => UnOp::Not,
                    _ => UnOp::Neg,
                };
                let operand = match node.children().next() {
                    Some(c) => self.expr(b, &c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                b.expr(Expr::Unary { op, operand }, span)
            }

            // Parentheses group; the tree already records that (ADR-0014).
            K::ParenExpr => match node.children().next() {
                Some(c) => self.expr(b, &c),
                None => b.expr(Expr::Error, span),
            },

            K::IfExpr => {
                let kids: Vec<_> = node.children().collect();
                let cond = match kids.first() {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let then = match kids.get(1) {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let els = kids.get(2).map(|c| self.expr(b, c));
                b.expr(Expr::If { cond, then, els }, span)
            }

            K::ForExpr => {
                // `pattern`, `iterable`, `body` — the pattern first, because
                // the grammar emits it first and a consumer that guessed by
                // shape would confuse `for x in xs` with `for (i, v) in xs`.
                let kids: Vec<_> = node.children().collect();
                let pat = kids
                    .first()
                    .filter(|c| is_pattern(c.kind()))
                    .map(|c| self.pattern(b, c));
                let rest: Vec<&SyntaxNode> = kids.iter().filter(|c| is_expr(c.kind())).collect();
                let iterable = match rest.first() {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                let body = match rest.get(1) {
                    Some(c) => self.expr(b, c),
                    None => b.expr(Expr::Error, span.clone()),
                };
                b.expr(
                    Expr::For {
                        pat,
                        iterable,
                        body,
                    },
                    span,
                )
            }

            K::MatchExpr => {
                let mut kids = node.children().peekable();
                let scrutinee = match kids.peek().filter(|c| c.kind() != K::MatchArm) {
                    Some(_) => {
                        // Internal invariant: `peek` returned `Some` on the
                        // line above and nothing consumed the iterator between
                        // the two.
                        let c = kids.next().expect("peeked");
                        self.expr(b, &c)
                    }
                    None => b.expr(Expr::Error, span.clone()),
                };
                let arms = node
                    .children()
                    .filter(|c| c.kind() == K::MatchArm)
                    .map(|arm| {
                        let kids: Vec<_> = arm.children().collect();
                        let pat = match kids.first() {
                            Some(p) => self.pattern(b, p),
                            None => b.pat(Pattern::Error, span_of(&arm)),
                        };
                        let body = match kids.get(1) {
                            Some(e) => self.expr(b, e),
                            None => b.expr(Expr::Error, span_of(&arm)),
                        };
                        MatchArm { pat, body }
                    })
                    .collect();
                b.expr(Expr::Match { scrutinee, arms }, span)
            }

            K::RecordExpr => {
                let name = node
                    .children()
                    .find(|c| c.kind() == K::NameExpr)
                    .map(|n| n.text().to_string().trim().to_string());
                let fields = node
                    .children()
                    .filter(|c| c.kind() == K::Field)
                    .map(|f| self.field_init(b, &f))
                    .collect();
                b.expr(Expr::Record { name, fields }, span)
            }

            K::ListExpr | K::TupleExpr => {
                let items = node.children().map(|c| self.expr(b, &c)).collect();
                b.expr(Expr::List { items }, span)
            }

            // A labelled statement (`transform: scale(1.0) -> scale(1.08)`) uses
            // the same node kind as a record field, because it is the same shape.
            K::Field => {
                let f = self.field_init(b, node);
                let fields = vec![f];
                b.expr(Expr::Record { name: None, fields }, span)
            }

            K::LetStmt => self.let_or_keyword(b, node, span),

            K::TemplateRegion => {
                // The element tree, plus a flat list of interpolated
                // expressions. Both are needed: a renderer walks the tree, and
                // an effect or privacy walk wants the expressions without
                // having to descend markup it does not understand.
                // Only the nodes THIS region allocates. Scanning the whole
                // arena would re-collect an earlier region's expressions in a
                // body that has two, and `walk()` would then visit them twice —
                // which is how a checker ends up reporting one defect twice.
                let first = b.nodes.len();
                let roots: Vec<NodeId> = node
                    .children()
                    .filter(|c| is_markup(c.kind()))
                    .map(|c| self.markup(b, &c))
                    .collect();
                let mut parts: Vec<ExprId> = Vec::new();
                for (_, n, _) in b.nodes.iter().skip(first) {
                    match n {
                        Node::Interpolation(e) => parts.push(*e),
                        Node::Element { attrs, .. } => {
                            parts.extend(attrs.iter().filter_map(|a| match a.value {
                                AttrValue::Expr(e) => Some(e),
                                _ => None,
                            }));
                        }
                        _ => {}
                    }
                }
                parts.sort_by_key(|e| b.exprs.span_at(e.index()).map(|s| s.start).unwrap_or(0));
                b.expr(Expr::Template { parts, roots }, span)
            }

            K::Interpolation => match node.children().find(|c| is_expr(c.kind())) {
                Some(e) => self.expr(b, &e),
                None => b.expr(
                    Expr::Template {
                        parts: vec![],
                        roots: vec![],
                    },
                    span,
                ),
            },

            _ => b.expr(Expr::Error, span),
        }
    }

    /// `LetStmt` covers both `let x = e` and the keyword-statement family; they
    /// are told apart by their first token.
    fn let_or_keyword(&mut self, b: &mut BodyBuilder, node: &SyntaxNode, span: Span) -> ExprId {
        let toks = own_tokens(node);
        let head = toks
            .first()
            .map(|t| t.text().to_string())
            .unwrap_or_default();

        if head == "let" {
            let pat = node.children().find(|c| c.kind() == K::Name).map(|n| {
                let mutable = toks.iter().any(|t| t.text() == "mut");
                b.pat(
                    Pattern::Bind {
                        name: n.text().to_string(),
                        mutable,
                    },
                    span_of(&n),
                )
            });
            let ty = node
                .children()
                .find(|c| c.kind() == K::TypeRef)
                .map(|t| self.type_ref(b, &t));
            let init = node
                .children()
                .find(|c| is_expr(c.kind()))
                .map(|e| self.expr(b, &e));
            return b.expr(Expr::Let { pat, ty, init }, span);
        }

        // `unsafe.imperative`, `observe resize`, `use key: Secret<T> = ..`.
        // The keyword absorbs `.ident` while the tokens stay dotted; every
        // bare identifier after that is a modifier word.
        let mut keyword = head;
        let mut modifiers = Vec::new();
        let mut i = 1;
        while i + 1 < toks.len() && toks[i].kind() == K::Dot && toks[i + 1].kind() == K::Ident {
            keyword.push('.');
            keyword.push_str(toks[i + 1].text());
            i += 2;
        }
        while i < toks.len() {
            if toks[i].kind() == K::Ident {
                modifiers.push(toks[i].text().to_string());
            }
            i += 1;
        }

        let args: Vec<ExprId> = node
            .children()
            .find(|c| c.kind() == K::ArgList)
            .map(|l| self.args(b, &l).into_iter().map(|a| a.value).collect())
            .unwrap_or_default();
        let block = node
            .children()
            .find(|c| c.kind() == K::BlockExpr)
            .map(|blk| self.expr(b, &blk));
        // An initialiser (`use key: T = expr`) is an argument to the statement.
        let mut args = args;
        if let Some(init) = node
            .children()
            .find(|c| is_expr(c.kind()) && c.kind() != K::BlockExpr)
        {
            let id = self.expr(b, &init);
            args.push(id);
        }

        // The grammar keeps a `because "…"` string as a bare token, so it is
        // recovered from the statement's own tokens rather than from `args`.
        let justification = toks
            .iter()
            .find(|t| matches!(t.kind(), K::Str | K::UnterminatedStr))
            .map(|t| t.text().to_string());

        // `attributes_forced_layout_to VendorMap` — the word ending in `_to`
        // names the clause, and the identifier after it names the owner. Read
        // from the tokens rather than a fixed keyword so a new attribution
        // clause does not need a parser change to be visible.
        let attribution = toks
            .iter()
            .position(|t| {
                t.kind() == K::Ident && (t.text().ends_with("_to") || t.text() == "attributed")
            })
            .and_then(|i| toks.get(i + 1))
            .filter(|t| t.kind() == K::Ident)
            .map(|t| t.text().to_string());

        b.expr(
            Expr::Keyword {
                keyword,
                justification,
                attribution,
                modifiers,
                args,
                block,
            },
            span,
        )
    }

    fn field_init(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> FieldInit {
        let name = first_name(node).unwrap_or_default();
        let value = node
            .children()
            .find(|c| is_expr(c.kind()))
            .map(|e| self.expr(b, &e));
        FieldInit {
            name,
            value,
            span: span_of(node),
        }
    }

    fn args(&mut self, b: &mut BodyBuilder, list: &SyntaxNode) -> Vec<Arg> {
        // The grammar consumes `max =` / `jitter:` as bare tokens before the
        // value, so a name is recovered by looking at what precedes each value.
        let mut named: Vec<String> = Vec::new();
        let mut pending: Option<String> = None;
        for e in list.children_with_tokens() {
            if let Some(t) = e.as_token() {
                match t.kind() {
                    K::Ident => pending = Some(t.text().to_string()),
                    K::Comma => pending = None,
                    _ => {}
                }
            } else if e.as_node().is_some_and(|n| is_expr(n.kind())) {
                named.push(pending.take().unwrap_or_default());
            }
        }
        list.children()
            .filter(|c| is_expr(c.kind()))
            .enumerate()
            .map(|(i, c)| Arg {
                name: named.get(i).filter(|s| !s.is_empty()).cloned(),
                value: self.expr(b, &c),
            })
            .collect()
    }

    /// A lambda parameter, which the grammar emits as an expression node.
    fn param_pattern(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> PatternId {
        let span = span_of(node);
        match node.kind() {
            // `x => e` writes the parameter as an expression; `fn(x) e`
            // writes it as a declaration-shaped `Param` holding a `Name`.
            // Both are parameters, and only the first was handled: `fn(el) ..`
            // lowered its parameter to `Pattern::Error`, so `el` was bound to
            // nothing and every rule that needed its type saw an untyped
            // receiver.
            //
            // Nothing complained, because the effect checker had a by-spelling
            // fallback that found `getBoundingClientRect` without needing a
            // receiver at all. R-037 was caught for a reason unrelated to what
            // it tests — see `docs/RISK_QUEUE.md`.
            K::NameExpr | K::Name => {
                let name = text(self.src, node).trim().to_string();
                if name == "_" {
                    b.pat(Pattern::Wild, span)
                } else {
                    b.pat(
                        Pattern::Bind {
                            name,
                            mutable: false,
                        },
                        span,
                    )
                }
            }
            K::Param => {
                // `el` or `el: T` — the binding is the first child; a written
                // type annotation is a sibling this pattern does not carry.
                match node.children().next() {
                    Some(c) => self.param_pattern(b, &c),
                    None => b.pat(Pattern::Error, span),
                }
            }
            K::ParamList | K::TupleExpr | K::ListExpr => {
                let args = node.children().map(|c| self.param_pattern(b, &c)).collect();
                b.pat(
                    Pattern::Ctor {
                        path: String::new(),
                        args,
                    },
                    span,
                )
            }
            _ => b.pat(Pattern::Error, span),
        }
    }

    /// Lower one markup node.
    fn markup(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> NodeId {
        let span = span_of(node);
        match node.kind() {
            K::Element => {
                let open = node.children().find(|c| c.kind() == K::OpenTag);
                let tag = open.as_ref().and_then(first_name).unwrap_or_default();
                let attrs = open
                    .as_ref()
                    .map(|o| {
                        o.children()
                            .filter(|c| c.kind() == K::Attr)
                            .map(|a| self.attr(b, &a))
                            .collect()
                    })
                    .unwrap_or_default();
                let children: Vec<NodeId> = node
                    .children()
                    .filter(|c| is_markup(c.kind()))
                    .map(|c| self.markup(b, &c))
                    .collect();
                let self_closing = node.children().all(|c| c.kind() != K::CloseTag);
                b.node(
                    Node::Element {
                        tag,
                        attrs,
                        children,
                        self_closing,
                    },
                    span,
                )
            }
            K::Text => {
                let t = text(self.src, node);
                b.node(Node::Text(t), span)
            }
            K::MarkupBlock => {
                // The opening marker is the block's first interpolation; its
                // children are everything between it and the closing one.
                let directive = node
                    .children()
                    .find(|c| c.kind() == K::Interpolation)
                    .map(|i| text(self.src, &i))
                    .unwrap_or_default();
                // The block's own markers are children of it too — the opening
                // one first and the closing one last. Filtering by what they
                // ARE rather than by position also drops a `{:else}` in the
                // middle, which a positional skip would have kept as content.
                let children = node
                    .children()
                    .filter(|c| is_markup(c.kind()) && !is_block_marker(self.src, c))
                    .map(|c| self.markup(b, &c))
                    .collect();
                b.node(
                    Node::Block {
                        directive,
                        children,
                    },
                    span,
                )
            }
            K::Interpolation => {
                // A bare `{/each}` or `{:else}` outside a block: keep it, so
                // recovery still yields something walkable.
                let raw = text(self.src, node);
                if raw.starts_with("{#") || raw.starts_with("{/") || raw.starts_with("{:") {
                    return b.node(
                        Node::Block {
                            directive: raw,
                            children: vec![],
                        },
                        span,
                    );
                }
                match node.children().find(|c| is_expr(c.kind())) {
                    Some(e) => {
                        let id = self.expr(b, &e);
                        b.node(Node::Interpolation(id), span)
                    }
                    None => b.node(Node::Text(raw), span),
                }
            }
            _ => b.node(Node::Text(text(self.src, node)), span),
        }
    }

    fn attr(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> Attr {
        let name = node
            .children()
            .find(|c| c.kind() == K::AttrName)
            .map(|n| n.text().to_string())
            .unwrap_or_default();
        let value = match node.children().find(|c| c.kind() == K::AttrValue) {
            None => AttrValue::None,
            Some(v) => match v.children().find(|c| c.kind() == K::Interpolation) {
                Some(i) => match i.children().find(|c| is_expr(c.kind())) {
                    Some(e) => AttrValue::Expr(self.expr(b, &e)),
                    None => AttrValue::None,
                },
                None => AttrValue::Static(v.text().to_string()),
            },
        };
        Attr {
            name,
            value,
            span: span_of(node),
        }
    }

    fn pattern(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> PatternId {
        let span = span_of(node);
        match node.kind() {
            K::WildcardPat => b.pat(Pattern::Wild, span),
            K::BindingPat => b.pat(
                Pattern::Bind {
                    name: text(self.src, node).trim().to_string(),
                    mutable: false,
                },
                span,
            ),
            K::CtorPat => {
                let path =
                    first_name(node).unwrap_or_else(|| text(self.src, node).trim().to_string());
                let args = node
                    .children()
                    .filter(|c| is_pattern(c.kind()))
                    .map(|c| self.pattern(b, &c))
                    .collect();
                b.pat(Pattern::Ctor { path, args }, span)
            }
            K::LiteralPat => {
                let t = own_tokens(node);
                let lit = match t.first().map(|t| (t.kind(), t.text().to_string())) {
                    Some((K::Int, s)) => Literal::Int(s),
                    Some((K::Float, s)) => Literal::Float(s),
                    Some((K::Str, s)) => Literal::Str(s),
                    _ => return b.pat(Pattern::Error, span),
                };
                b.pat(Pattern::Literal(lit), span)
            }
            K::OrPat | K::TuplePat => {
                let args: Vec<_> = node
                    .children()
                    .filter(|c| is_pattern(c.kind()))
                    .map(|c| self.pattern(b, &c))
                    .collect();
                if node.kind() == K::OrPat {
                    b.pat(Pattern::Or(args), span)
                } else {
                    b.pat(
                        Pattern::Ctor {
                            path: String::new(),
                            args,
                        },
                        span,
                    )
                }
            }
            _ => b.pat(Pattern::Error, span),
        }
    }

    fn type_ref(&mut self, b: &mut BodyBuilder, node: &SyntaxNode) -> TypeRefId {
        let span = span_of(node);
        let path = first_name(node).unwrap_or_else(|| text(self.src, node).trim().to_string());
        let args = node
            .children()
            .find(|c| c.kind() == K::TypeArgList)
            .map(|l| {
                l.children()
                    .filter(|c| c.kind() == K::TypeRef)
                    .map(|c| self.type_ref(b, &c))
                    .collect()
            })
            .unwrap_or_default();
        b.ty(TypeRef { path, args }, span)
    }
}

/// The names an `import ... { A, B }` brings into scope.
///
/// The grammar skips the brace list as a balanced range, so the names are read
/// from the declaration's own tokens. That is enough for resolution and avoids
/// a grammar change; E2B's gate is about the *graph*, not about how the list is
/// spelled.
fn imported_names(node: &SyntaxNode) -> Vec<String> {
    if node.kind() != K::ImportDecl {
        return Vec::new();
    }
    let toks = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .collect::<Vec<_>>();
    let Some(open) = toks.iter().position(|t| t.kind() == K::LBrace) else {
        return Vec::new();
    };
    toks[open + 1..]
        .iter()
        .take_while(|t| t.kind() != K::RBrace)
        .filter(|t| t.kind() == K::Ident)
        .map(|t| t.text().to_string())
        .collect()
}

/// The visibility keyword a declaration opens with, if any.
///
/// The grammar keeps it as a bare token, so it is the declaration's first
/// significant token when it is one of the three.
fn visibility_of(node: &SyntaxNode) -> Option<String> {
    let first = own_tokens(node).first()?.text().to_string();
    matches!(first.as_str(), "public" | "session" | "private").then_some(first)
}

/// A type reference's name, without its type arguments.
/// The type arguments of a `TypeRef`, **as written**: `Result<List<MenuItem>,
/// E>` gives `["List<MenuItem>", "E"]`.
///
/// Written form rather than heads, because a head loses exactly what the
/// element rule needs. `Menus.for_store` returns `Result<List<MenuItemId>, _>`
/// and `{#each menu as item}` iterates what is inside BOTH wrappers — heads
/// alone stop at `List` and the element is gone.
fn type_args(t: &SyntaxNode) -> Vec<String> {
    t.children()
        .find(|c| c.kind() == K::TypeArgList)
        .map(|l| {
            l.children()
                .filter(|c| c.kind() == K::TypeRef)
                .map(|a| a.text().to_string().trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn type_path(t: &SyntaxNode) -> String {
    t.children()
        .find(|c| c.kind() == K::Name)
        .map(|n| n.text().to_string())
        .unwrap_or_else(|| t.text().to_string().trim().to_string())
}

fn bin_op(kind: K, text: &str) -> BinOp {
    match kind {
        K::Plus => BinOp::Add,
        K::Minus => BinOp::Sub,
        K::Star => BinOp::Mul,
        K::Slash => BinOp::Div,
        K::Percent => BinOp::Rem,
        K::Amp => BinOp::And,
        K::Pipe => BinOp::Or,
        K::PipeGt => BinOp::Pipe,
        K::Arrow => BinOp::Transition,
        K::Eq => BinOp::Assign,
        K::Cmp | K::LAngle | K::RAngle => BinOp::Cmp(text.to_string()),
        _ => BinOp::Cmp(text.to_string()),
    }
}

fn is_expr(k: K) -> bool {
    matches!(
        k,
        K::BlockExpr
            | K::LiteralExpr
            | K::NameExpr
            | K::FieldExpr
            | K::CallExpr
            | K::LambdaExpr
            | K::LetStmt
            | K::IfExpr
            | K::MatchExpr
            | K::ForExpr
            | K::BinaryExpr
            | K::CastExpr
            | K::UnaryExpr
            | K::ParenExpr
            | K::RecordExpr
            | K::TupleExpr
            | K::ListExpr
            | K::ErrorExpr
            | K::TemplateRegion
            | K::Field
    )
}

/// Is this node one of a block's own `{#..}` / `{/..}` / `{:..}` markers?
fn is_block_marker(src: &str, n: &SyntaxNode) -> bool {
    n.kind() == K::Interpolation && {
        let t = text(src, n);
        let t = t.trim_start();
        t.starts_with("{#") || t.starts_with("{/") || t.starts_with("{:")
    }
}

/// One markup node and everything under it.
fn is_markup(k: K) -> bool {
    matches!(k, K::Element | K::Text | K::Interpolation | K::MarkupBlock)
}

/// Can this node stand in a lambda's parameter position as a binding?
/// Anything else there is an annotation, kept as an expression.
fn is_param_shaped(k: K) -> bool {
    matches!(k, K::NameExpr | K::ParamList | K::TupleExpr | K::ListExpr)
}

fn is_pattern(k: K) -> bool {
    matches!(
        k,
        K::WildcardPat | K::BindingPat | K::CtorPat | K::LiteralPat | K::OrPat | K::TuplePat
    )
}
