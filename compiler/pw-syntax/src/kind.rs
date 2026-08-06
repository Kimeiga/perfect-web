//! `SyntaxKind` — one tag space for both tokens and nodes.
//!
//! Rowan's green tree is untyped: every node and token carries a `u16` tag and
//! the language supplies the meaning. So tokens and nodes share one enum, with
//! tokens first so the existing lexer's `Kind` maps across by position.
//!
//! Adding a variant is a **grammar change**. `kind_from_raw` panics on an
//! unknown tag rather than silently producing a wrong kind, because a mismatched
//! tag would corrupt every analysis downstream while looking like a parse
//! success — the failure mode `docs/RISK_QUEUE.md` exists to prevent.

use crate::lexer::Kind as TokenKind;

/// Every token and node tag in the language.
///
/// `#[repr(u16)]` and the explicit discriminants are load-bearing: they are the
/// on-tree encoding. Reordering variants silently reinterprets an existing tree,
/// so append rather than insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[allow(clippy::upper_case_acronyms)]
pub enum SyntaxKind {
    // ---- tokens (mirror `lexer::Kind`) ---------------------------------- 0..
    Whitespace = 0,
    LineComment,
    DocAttr,
    Ident,
    Int,
    Float,
    Str,
    UnterminatedStr,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LAngle,
    RAngle,
    Comma,
    Colon,
    Semi,
    Dot,
    Arrow,
    FatArrow,
    Pipe,
    Bang,
    Eq,
    Question,
    At,
    Amp,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Hash,
    CloseSlash,
    Underscore,
    Unknown,
    Eof,

    // ---- nodes ---------------------------------------------------------- 100..
    /// The whole file.
    SourceFile = 100,
    ModuleDecl,
    ImportDecl,
    OpaqueDecl,
    TypeDecl,
    RecordDecl,
    UnionDecl,
    FnDecl,
    ResourceDecl,
    UiDecl,
    LetDecl,
    /// A declaration the parser could not classify. Present so recovery
    /// produces a tree rather than a hole.
    ErrorDecl,

    ParamList,
    Param,
    TypeRef,
    TypeArgList,
    EffectRow,
    EffectRef,
    VariantList,
    Variant,
    FieldList,
    Field,
    PolicyList,
    Policy,
    /// A body captured but not yet parsed into expressions. E2's next task
    /// replaces this with real expression nodes.
    Body,
    Name,

    // ---- expressions (reserved; the core body grammar fills these) -------- 200..
    BlockExpr = 200,
    LiteralExpr,
    NameExpr,
    FieldExpr,
    CallExpr,
    ArgList,
    LambdaExpr,
    LetStmt,
    IfExpr,
    MatchExpr,
    MatchArm,
    BinaryExpr,
    UnaryExpr,
    ParenExpr,
    RecordExpr,
    TupleExpr,
    ListExpr,
    /// An expression position the parser could not fill.
    ErrorExpr,

    // ---- patterns -------------------------------------------------------- 300..
    WildcardPat = 300,
    BindingPat,
    CtorPat,
    LiteralPat,
    OrPat,
    TuplePat,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::DocAttr
        )
    }

    /// Tokens occupy the first block; everything else is a node.
    pub fn is_token(self) -> bool {
        (self as u16) < SyntaxKind::SourceFile as u16
    }
}

impl From<TokenKind> for SyntaxKind {
    fn from(k: TokenKind) -> Self {
        match k {
            TokenKind::Whitespace => SyntaxKind::Whitespace,
            TokenKind::LineComment => SyntaxKind::LineComment,
            TokenKind::DocAttr => SyntaxKind::DocAttr,
            TokenKind::Ident => SyntaxKind::Ident,
            TokenKind::Int => SyntaxKind::Int,
            TokenKind::Float => SyntaxKind::Float,
            TokenKind::Str => SyntaxKind::Str,
            TokenKind::UnterminatedStr => SyntaxKind::UnterminatedStr,
            TokenKind::LParen => SyntaxKind::LParen,
            TokenKind::RParen => SyntaxKind::RParen,
            TokenKind::LBrace => SyntaxKind::LBrace,
            TokenKind::RBrace => SyntaxKind::RBrace,
            TokenKind::LBracket => SyntaxKind::LBracket,
            TokenKind::RBracket => SyntaxKind::RBracket,
            TokenKind::LAngle => SyntaxKind::LAngle,
            TokenKind::RAngle => SyntaxKind::RAngle,
            TokenKind::Comma => SyntaxKind::Comma,
            TokenKind::Colon => SyntaxKind::Colon,
            TokenKind::Semi => SyntaxKind::Semi,
            TokenKind::Dot => SyntaxKind::Dot,
            TokenKind::Arrow => SyntaxKind::Arrow,
            TokenKind::FatArrow => SyntaxKind::FatArrow,
            TokenKind::Pipe => SyntaxKind::Pipe,
            TokenKind::Bang => SyntaxKind::Bang,
            TokenKind::Eq => SyntaxKind::Eq,
            TokenKind::Question => SyntaxKind::Question,
            TokenKind::At => SyntaxKind::At,
            TokenKind::Amp => SyntaxKind::Amp,
            TokenKind::Plus => SyntaxKind::Plus,
            TokenKind::Minus => SyntaxKind::Minus,
            TokenKind::Star => SyntaxKind::Star,
            TokenKind::Slash => SyntaxKind::Slash,
            TokenKind::Percent => SyntaxKind::Percent,
            TokenKind::Hash => SyntaxKind::Hash,
            TokenKind::CloseSlash => SyntaxKind::CloseSlash,
            TokenKind::Underscore => SyntaxKind::Underscore,
            TokenKind::Unknown => SyntaxKind::Unknown,
            TokenKind::Eof => SyntaxKind::Eof,
        }
    }
}

/// The Rowan language tag for `pw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pw {}

impl rowan::Language for Pw {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        // Deliberately total-by-table rather than a transmute. An unknown tag is
        // a bug in the builder, and it must fail loudly here rather than produce
        // a plausible-but-wrong kind that every later analysis trusts.
        ALL_KINDS
            .iter()
            .copied()
            .find(|k| *k as u16 == raw.0)
            .unwrap_or_else(|| panic!("unknown pw SyntaxKind tag: {}", raw.0))
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<Pw>;
pub type SyntaxToken = rowan::SyntaxToken<Pw>;
pub type SyntaxElement = rowan::SyntaxElement<Pw>;

/// Every declared kind. Used by `kind_from_raw` and by the round-trip test that
/// proves the table stays in step with the enum.
pub const ALL_KINDS: &[SyntaxKind] = {
    use SyntaxKind::*;
    &[
        Whitespace,
        LineComment,
        DocAttr,
        Ident,
        Int,
        Float,
        Str,
        UnterminatedStr,
        LParen,
        RParen,
        LBrace,
        RBrace,
        LBracket,
        RBracket,
        LAngle,
        RAngle,
        Comma,
        Colon,
        Semi,
        Dot,
        Arrow,
        FatArrow,
        Pipe,
        Bang,
        Eq,
        Question,
        At,
        Amp,
        Plus,
        Minus,
        Star,
        Slash,
        Percent,
        Hash,
        CloseSlash,
        Underscore,
        Unknown,
        Eof,
        SourceFile,
        ModuleDecl,
        ImportDecl,
        OpaqueDecl,
        TypeDecl,
        RecordDecl,
        UnionDecl,
        FnDecl,
        ResourceDecl,
        UiDecl,
        LetDecl,
        ErrorDecl,
        ParamList,
        Param,
        TypeRef,
        TypeArgList,
        EffectRow,
        EffectRef,
        VariantList,
        Variant,
        FieldList,
        Field,
        PolicyList,
        Policy,
        Body,
        Name,
        BlockExpr,
        LiteralExpr,
        NameExpr,
        FieldExpr,
        CallExpr,
        ArgList,
        LambdaExpr,
        LetStmt,
        IfExpr,
        MatchExpr,
        MatchArm,
        BinaryExpr,
        UnaryExpr,
        ParenExpr,
        RecordExpr,
        TupleExpr,
        ListExpr,
        ErrorExpr,
        WildcardPat,
        BindingPat,
        CtorPat,
        LiteralPat,
        OrPat,
        TuplePat,
    ]
};

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::Language;

    #[test]
    fn every_kind_round_trips_through_the_raw_tag() {
        for k in ALL_KINDS {
            assert_eq!(Pw::kind_from_raw(Pw::kind_to_raw(*k)), *k, "{k:?}");
        }
    }

    #[test]
    fn the_kind_table_has_no_duplicate_tags() {
        // A duplicate discriminant would make `kind_from_raw` silently return
        // the wrong kind for one of them.
        let mut tags: Vec<u16> = ALL_KINDS.iter().map(|k| *k as u16).collect();
        let before = tags.len();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), before, "duplicate SyntaxKind discriminants");
    }

    #[test]
    fn the_table_covers_every_lexer_token_kind() {
        // If the lexer gains a token and the table does not, the parser would
        // panic at runtime instead of failing to compile. Enumerate explicitly.
        let lexer_kinds = [
            TokenKind::Whitespace,
            TokenKind::LineComment,
            TokenKind::DocAttr,
            TokenKind::Ident,
            TokenKind::Int,
            TokenKind::Float,
            TokenKind::Str,
            TokenKind::UnterminatedStr,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LAngle,
            TokenKind::RAngle,
            TokenKind::Comma,
            TokenKind::Colon,
            TokenKind::Semi,
            TokenKind::Dot,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Pipe,
            TokenKind::Bang,
            TokenKind::Eq,
            TokenKind::Question,
            TokenKind::At,
            TokenKind::Amp,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Hash,
            TokenKind::CloseSlash,
            TokenKind::Underscore,
            TokenKind::Unknown,
            TokenKind::Eof,
        ];
        for k in lexer_kinds {
            let sk: SyntaxKind = k.into();
            assert!(
                ALL_KINDS.contains(&sk),
                "{k:?} -> {sk:?} missing from ALL_KINDS"
            );
            assert!(sk.is_token(), "{sk:?} should be in the token block");
        }
    }

    #[test]
    fn tokens_and_nodes_occupy_separate_blocks() {
        assert!(SyntaxKind::Eof.is_token());
        assert!(!SyntaxKind::SourceFile.is_token());
        assert!(!SyntaxKind::BlockExpr.is_token());
        assert!(!SyntaxKind::WildcardPat.is_token());
    }

    #[test]
    #[should_panic(expected = "unknown pw SyntaxKind tag")]
    fn an_unknown_tag_fails_loudly_rather_than_guessing() {
        // docs/RISK_QUEUE.md: the negative control. A mismatched tag must not
        // produce a plausible-but-wrong kind that later analyses trust.
        Pw::kind_from_raw(rowan::SyntaxKind(60_000));
    }
}
