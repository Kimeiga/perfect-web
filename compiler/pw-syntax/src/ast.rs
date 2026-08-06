//! The declaration AST.
//!
//! Every node carries a byte span, because charter §16.3 requires diagnostics to
//! name *where a value originated* as well as where a rule fired — and a span
//! that is reconstructed later is a span that is wrong.
//!
//! This is a *declaration-level* AST. Function bodies and template regions are
//! captured as balanced token ranges rather than parsed into expressions: E2's
//! gate is about declarations, spans and recovery, and pretending to parse
//! expressions we cannot yet check would be scope creep with a green test suite.

use crate::lexer::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// A type as written. Not resolved, not checked — that is `pw-core`'s job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub name: String,
    pub args: Vec<TypeRef>,
    pub span: Span,
}

/// One entry in an effect row: `database.read<Stores>`, `layout.measure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRef {
    /// Dotted family name, e.g. `database.read`.
    pub name: String,
    pub args: Vec<TypeRef>,
    pub span: Span,
}

/// `!{ ... }`. `None` means no row was written at all, which is different from
/// an empty row `!{}` — the first is unannotated, the second is an explicit
/// claim of purity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRow {
    pub effects: Vec<EffectRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub name: Ident,
    pub fields: Vec<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

/// A policy clause inside a `query`/`command`/`materialize` block, e.g.
/// `freshness 30.seconds`, `cache shared`, `invalidates_on StoreChanged(id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub keyword: Ident,
    /// Raw token text of the value, normalised to single spaces. Kept as text
    /// because policy grammars differ per keyword and E4 owns their meaning.
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Session,
    Private,
    /// No visibility keyword written.
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclKind {
    Module,
    Import,
    /// `opaque type StoreId = String`
    Opaque {
        representation: Option<TypeRef>,
    },
    /// `type OrderState = | Draft | Confirmed(..)`
    Union {
        variants: Vec<Variant>,
    },
    /// `type Store = Store { id: StoreId, .. }`
    Record {
        fields: Vec<Field>,
    },
    Function {
        params: Vec<Param>,
        result: Option<TypeRef>,
        effects: Option<EffectRow>,
    },
    /// `query`, `command`, `subscription`, `resource`, `materialize`, `paint`
    Resource {
        /// Which keyword introduced it.
        noun: String,
        visibility: Visibility,
        params: Vec<Param>,
        result: Option<TypeRef>,
        policies: Vec<Policy>,
    },
    /// `view`, `component`, `page`
    Ui {
        noun: String,
        params: Vec<Param>,
        effects: Option<EffectRow>,
        policies: Vec<Policy>,
    },
    /// Something the parser could not classify. Kept so recovery can continue
    /// and so `pw check` reports one error rather than cascading.
    Unrecognised,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub kind: DeclKind,
    pub name: Option<Ident>,
    /// The whole declaration, including its body.
    pub span: Span,
    /// The body's token range, if it had one. Unparsed by design.
    pub body: Option<Span>,
}

/// A parsed `.pw` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub module: Option<Ident>,
    pub decls: Vec<Decl>,
    /// `// @key: value` header entries, in source order.
    pub attrs: Vec<(String, String, Span)>,
}

impl SourceFile {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, v, _)| v.as_str())
    }

    pub fn attrs_named<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> {
        self.attrs
            .iter()
            .filter(move |(k, _, _)| k == key)
            .map(|(_, v, _)| v.as_str())
    }

    /// Every declaration that introduces a name, for E2's name-resolution step.
    pub fn named_decls(&self) -> impl Iterator<Item = (&Ident, &Decl)> {
        self.decls
            .iter()
            .filter_map(|d| d.name.as_ref().map(|n| (n, d)))
    }
}
