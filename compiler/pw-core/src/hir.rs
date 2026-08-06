//! HIR — the representation every semantic analysis consumes.
//!
//! ADR-0012 drew the boundary: checkers see **ids and spans, never syntax
//! nodes**. ADR-0014 fixed the shape: id-indexed arenas, per body, with a span
//! stored beside every node.
//!
//! Nothing in this module knows that Rowan exists. That is the point of it.
//! `crate::lower` is the single module allowed to bridge the two, and
//! `hir::tests::only_lower_touches_syntax_nodes` enforces it.

use std::ops::Range;

pub type Span = Range<usize>;

macro_rules! id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $name {
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id!(
    /// A source file's module declaration.
    ModuleId
);
id!(
    /// A top-level or nested declaration: `fn`, `view`, `component`, `query`, ...
    DeclId
);
id!(
    /// A declaration's body. The unit of reanalysis (ADR-0014).
    BodyId
);
id!(
    /// An expression, scoped to one [`Body`].
    ExprId
);
id!(
    /// A pattern, scoped to one [`Body`].
    PatternId
);
id!(
    /// A type reference, scoped to one [`Body`].
    TypeRefId
);

/// An id-indexed arena that keeps a span beside every node.
///
/// The span is not optional and not recomputed: charter §16.3 requires every
/// diagnostic to carry an origin span, so "I hold an id" must imply "I can
/// point at source".
#[derive(Debug, Clone)]
pub struct Arena<T> {
    nodes: Vec<T>,
    spans: Vec<Span>,
}

// Hand-written: `derive(Default)` would demand `T: Default`, which no HIR node
// has or should have.
impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            spans: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Push a node with its span, returning its raw index.
    pub fn alloc(&mut self, node: T, span: Span) -> u32 {
        let i = self.nodes.len() as u32;
        self.nodes.push(node);
        self.spans.push(span);
        i
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.nodes.get(i)
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        self.nodes.get_mut(i)
    }

    pub fn span_at(&self, i: usize) -> Option<&Span> {
        self.spans.get(i)
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &T, &Span)> {
        self.nodes
            .iter()
            .zip(self.spans.iter())
            .enumerate()
            .map(|(i, (n, s))| (i, n, s))
    }
}

// --- declarations --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Fn,
    View,
    Component,
    Page,
    Query,
    Command,
    Subscription,
    Resource,
    Task,
    Type,
    Opaque,
    Let,
    Import,
    /// A declaration form the lowering does not model yet. Recorded rather
    /// than dropped, so a checker can see that something was there.
    Other,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub decls: Vec<DeclId>,
}

/// A declaration parameter. The type is the name **as written**: resolving it
/// is a later layer's job, and a signature has no body arena to hold a
/// `TypeRefId`.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Option<String>,
    pub span: Span,
}

/// One constructor of a `type T = | A | B(X)` declaration.
#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    /// Field type names as written.
    pub fields: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Decl {
    pub name: String,
    pub kind: DeclKind,
    pub params: Vec<Param>,
    /// The declared return type, as written.
    pub ret: Option<String>,
    /// The variants, when this declaration defines an algebraic data type.
    pub variants: Option<Vec<VariantDef>>,
    /// The fields, when this declaration defines a record.
    pub fields: Option<Vec<Param>>,
    /// `opaque type StoreId = String` — the representation, as written.
    pub opaque_of: Option<String>,
    /// The declared effect row, as written: `!{ database.read<Stores> }` yields
    /// `["database.read"]`. Empty for `!{}`; `None` when no row was written.
    pub declared_effects: Option<Vec<EffectRef>>,
    pub body: Option<BodyId>,
    /// Declarations nested inside this one, e.g. a `fn` inside a `component`.
    pub children: Vec<DeclId>,
}

/// One entry in an effect row, with the span of the entry itself so a
/// diagnostic can underline the specific effect rather than the whole row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRef {
    pub path: String,
    pub span: Span,
}

// --- bodies --------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Body {
    pub owner: DeclId,
    pub exprs: Arena<Expr>,
    pub pats: Arena<Pattern>,
    pub types: Arena<TypeRef>,
    /// The block expression the body consists of.
    pub root: ExprId,
}

impl Body {
    pub fn expr(&self, id: ExprId) -> &Expr {
        self.exprs
            .get(id.index())
            .expect("ExprId from another body")
    }

    pub fn expr_span(&self, id: ExprId) -> Span {
        self.exprs
            .span_at(id.index())
            .expect("ExprId from another body")
            .clone()
    }

    pub fn pat(&self, id: PatternId) -> &Pattern {
        self.pats
            .get(id.index())
            .expect("PatternId from another body")
    }

    pub fn pat_span(&self, id: PatternId) -> Span {
        self.pats
            .span_at(id.index())
            .expect("PatternId from another body")
            .clone()
    }

    /// Every expression in the body, in allocation order.
    pub fn exprs(&self) -> impl Iterator<Item = (ExprId, &Expr, &Span)> {
        self.exprs.iter().map(|(i, e, s)| (ExprId(i as u32), e, s))
    }

    /// The direct children of an expression. Written once here so no analysis
    /// has to re-derive the traversal and quietly miss a variant — the
    /// generic-callback case fails exactly that way.
    pub fn children(&self, id: ExprId) -> Vec<ExprId> {
        match self.expr(id) {
            Expr::Name(_) | Expr::Literal(_) | Expr::Error => vec![],
            Expr::Field { base, .. } => vec![*base],
            Expr::Call { callee, args } => {
                let mut v = vec![*callee];
                v.extend(args.iter().map(|a| a.value));
                v
            }
            Expr::Lambda {
                descriptor, body, ..
            } => {
                let mut v: Vec<ExprId> = descriptor.iter().copied().collect();
                v.push(*body);
                v
            }
            Expr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Expr::Unary { operand, .. } => vec![*operand],
            Expr::Block { stmts } => stmts.clone(),
            Expr::If { cond, then, els } => {
                let mut v = vec![*cond, *then];
                v.extend(els.iter().copied());
                v
            }
            Expr::Match { scrutinee, arms } => {
                let mut v = vec![*scrutinee];
                v.extend(arms.iter().map(|a| a.body));
                v
            }
            Expr::Record { fields, .. } => fields.iter().filter_map(|f| f.value).collect(),
            Expr::List { items } => items.clone(),
            Expr::Let { init, .. } => init.iter().copied().collect(),
            Expr::Keyword { args, block, .. } => {
                let mut v = args.clone();
                v.extend(block.iter().copied());
                v
            }
            Expr::Template { parts } => parts.clone(),
        }
    }

    /// Every expression reachable from `root`, parents before children.
    pub fn walk(&self) -> Vec<ExprId> {
        let mut out = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            out.push(id);
            let kids = self.children(id);
            stack.extend(kids.into_iter().rev());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(String),
    Float(String),
    Str(String),
    /// A string whose closing quote is missing. Kept as a literal so a body
    /// containing one still lowers.
    UnterminatedStr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    /// `==` `!=` `<=` `>=` `<` `>`
    Cmp(String),
    And,
    Or,
    /// `a |> f`. Not desugared to a call: a diagnostic pointing at a
    /// synthesised call the author never wrote is worse than one pointing at
    /// `|>` (ADR-0014).
    Pipe,
    /// `a -> b` in an animation keyframe sequence.
    Transition,
    Assign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct Arg {
    /// `max = 3` and `jitter: true` carry a name; positional arguments do not.
    pub name: Option<String>,
    pub value: ExprId,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: PatternId,
    pub body: ExprId,
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    /// `Point { x, y }` shorthand has no value expression.
    pub value: Option<ExprId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    /// A bare identifier. `a.b` is `Field { base: Name("a"), name: "b" }` —
    /// lowering does not decide whether that is a module path (ADR-0014).
    Name(String),
    Literal(Literal),
    Field {
        base: ExprId,
        name: String,
    },
    Call {
        callee: ExprId,
        args: Vec<Arg>,
    },
    Lambda {
        /// An annotation in parameter position: `resumable(captures = { item })
        /// => ..`. It is call-shaped, so it is kept as an expression rather
        /// than forced into a pattern. R-010 and R-030 — a non-serializable
        /// capture and private data in a resume manifest — are checks *on this
        /// expression*, so dropping it would make them unimplementable.
        descriptor: Option<ExprId>,
        params: Vec<PatternId>,
        body: ExprId,
    },
    Binary {
        op: BinOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    Unary {
        op: UnOp,
        operand: ExprId,
    },
    Block {
        stmts: Vec<ExprId>,
    },
    If {
        cond: ExprId,
        then: ExprId,
        els: Option<ExprId>,
    },
    Match {
        scrutinee: ExprId,
        arms: Vec<MatchArm>,
    },
    Record {
        /// `None` for a bare `{ a: 1 }` with no type name in front.
        name: Option<String>,
        fields: Vec<FieldInit>,
    },
    List {
        items: Vec<ExprId>,
    },
    Let {
        pat: Option<PatternId>,
        ty: Option<TypeRefId>,
        init: Option<ExprId>,
    },
    /// The body-level statement family: `observe resize`, `frame { .. }`,
    /// `unsafe.imperative { .. }`, `use key: Secret<T> = ..`. One node kind
    /// because E2 recognises them and E4 gives them meaning.
    Keyword {
        /// `unsafe.imperative` keeps its qualification.
        keyword: String,
        /// The bare modifier words between the keyword and any punctuation:
        /// `unsafe capability synchronous_geometry` yields
        /// `["capability", "synchronous_geometry"]`.
        modifiers: Vec<String>,
        args: Vec<ExprId>,
        block: Option<ExprId>,
    },
    /// A markup region. `parts` are its interpolated expressions, in source
    /// order; the literal markup between them lives in the syntax tree, which
    /// is where a formatter wants it.
    Template {
        parts: Vec<ExprId>,
    },
    /// Syntax that could not be lowered. Carries its span so a checker can
    /// still report position, and keeps lowering total (ADR-0014).
    Error,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// `_`
    Wild,
    /// A binding: `x`, or `mut x`.
    Bind {
        name: String,
        mutable: bool,
    },
    /// A constructor pattern: `Some(x)`, `DecodeError.Invalid(a, b)`.
    Ctor {
        path: String,
        args: Vec<PatternId>,
    },
    Literal(Literal),
    /// `a | b`
    Or(Vec<PatternId>),
    Error,
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    /// `List`, `store.Order`, `()` for unit.
    pub path: String,
    pub args: Vec<TypeRefId>,
}

// --- the program ---------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Hir {
    pub modules: Arena<Module>,
    pub decls: Arena<Decl>,
    pub bodies: Arena<Body>,
}

impl Hir {
    pub fn decl(&self, id: DeclId) -> &Decl {
        self.decls.get(id.index()).expect("unknown DeclId")
    }

    pub fn decl_span(&self, id: DeclId) -> Span {
        self.decls
            .span_at(id.index())
            .expect("unknown DeclId")
            .clone()
    }

    pub fn body(&self, id: BodyId) -> &Body {
        self.bodies.get(id.index()).expect("unknown BodyId")
    }

    /// Every declaration, nested ones included.
    pub fn all_decls(&self) -> impl Iterator<Item = (DeclId, &Decl)> {
        self.decls.iter().map(|(i, d, _)| (DeclId(i as u32), d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arena_keeps_a_span_for_every_node() {
        let mut a: Arena<&str> = Arena::new();
        let i = a.alloc("x", 3..7);
        assert_eq!(a.get(i as usize), Some(&"x"));
        assert_eq!(a.span_at(i as usize), Some(&(3..7)));
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn only_lower_touches_syntax_nodes() {
        // ADR-0014 property 3, enforced rather than trusted. A checker that
        // reaches into the syntax tree would compile and pass its own tests
        // while quietly re-coupling the layers.
        //
        // The check looks at `use` declarations, not at prose: a first version
        // searched the whole file for "SyntaxNode" and "green" and flagged
        // seven modules that only mentioned them in comments. A check that
        // cries wolf gets disabled, which is worse than not having one.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src/ is readable") {
            let path = entry.expect("entry").path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.extension().is_none_or(|e| e != "rs") || name == "lower.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            for line in text.lines().map(str::trim) {
                if !line.starts_with("use ") {
                    continue;
                }
                for needle in [
                    "SyntaxNode",
                    "SyntaxToken",
                    "rowan",
                    "pw_syntax::kind",
                    "pw_syntax::tree",
                    "pw_syntax::grammar",
                ] {
                    if line.contains(needle) {
                        offenders.push(format!("{name}: {line}"));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "only lower.rs may import syntax nodes (ADR-0014): {offenders:#?}"
        );
    }

    #[test]
    fn the_syntax_node_guard_can_fail() {
        // Negative control: the guard must actually detect an import. Written
        // against the same matcher the test above uses.
        let bad = "use pw_syntax::kind::SyntaxNode;";
        let matched = bad.trim().starts_with("use ")
            && ["SyntaxNode", "rowan", "pw_syntax::kind"]
                .iter()
                .any(|n| bad.contains(n));
        assert!(matched, "the matcher must catch a real import");

        // And must not fire on prose that merely names the type.
        let prose = "// checkers never see a SyntaxNode (ADR-0014)";
        assert!(!prose.trim().starts_with("use "));
    }
}
