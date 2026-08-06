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

impl Lowerer<'_> {
    fn decl(&mut self, node: &SyntaxNode) -> Option<DeclId> {
        if !matches!(
            node.kind(),
            K::FnDecl
                | K::UiDecl
                | K::ResourceDecl
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
        let declared_effects = node
            .children()
            .find(|c| c.kind() == K::EffectRow)
            .map(|row| self.effect_row(&row));

        // Reserve the id before lowering the body, so a nested declaration can
        // name its parent and the body can name its owner.
        let id = DeclId(self.hir.decls.alloc(
            Decl {
                name,
                kind,
                params: self.params(node),
                variants: self.variants(node),
                opaque_of: self.opaque_of(node),
                declared_effects,
                body: None,
                children: Vec::new(),
            },
            span_of(node),
        ));

        let (body, children) = match node.children().find(|c| c.kind() == K::Body) {
            Some(b) => self.body(id, &b),
            None => (None, Vec::new()),
        };
        let d = self.hir.decls.get_mut(id.index()).expect("just allocated");
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
                    .map(|p| Param {
                        name: first_name(&p).unwrap_or_default(),
                        ty: p
                            .children()
                            .find(|c| c.kind() == K::TypeRef)
                            .map(|t| type_path(&t)),
                        span: span_of(&p),
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
                // `database.read<Stores>` — the path is the name, the type
                // arguments are not part of the effect's identity.
                let path = e
                    .children()
                    .find(|c| c.kind() == K::TypeRef)
                    .and_then(|t| t.children().find(|c| c.kind() == K::Name))
                    .map(|n| n.text().to_string())
                    .unwrap_or_else(|| text(self.src, &e).trim().to_string());
                EffectRef {
                    path,
                    span: span_of(&e),
                }
            })
            .collect()
    }

    /// Lower a `Body` node, hoisting any nested declarations out of it.
    fn body(&mut self, owner: DeclId, node: &SyntaxNode) -> (Option<BodyId>, Vec<DeclId>) {
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
        let body = Body {
            owner,
            exprs: b.exprs,
            pats: b.pats,
            types: b.types,
            root,
        };
        (
            Some(BodyId(self.hir.bodies.alloc(body, span_of(node)))),
            children,
        )
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
                    Some((K::Str, s)) => Literal::Str(s),
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

            K::MatchExpr => {
                let mut kids = node.children().peekable();
                let scrutinee = match kids.peek().filter(|c| c.kind() != K::MatchArm) {
                    Some(_) => {
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
                let parts = node
                    .descendants()
                    .filter(|c| c.kind() == K::Interpolation)
                    .filter_map(|i| i.children().find(|c| is_expr(c.kind())))
                    .map(|e| self.expr(b, &e))
                    .collect();
                b.expr(Expr::Template { parts }, span)
            }

            K::Interpolation => match node.children().find(|c| is_expr(c.kind())) {
                Some(e) => self.expr(b, &e),
                None => b.expr(Expr::Template { parts: vec![] }, span),
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

        b.expr(
            Expr::Keyword {
                keyword,
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
            K::NameExpr => {
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

/// A type reference's name, without its type arguments.
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
            | K::BinaryExpr
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
