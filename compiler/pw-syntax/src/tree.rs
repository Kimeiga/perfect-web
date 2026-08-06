//! Building and inspecting the Rowan green tree.
//!
//! ADR-0012: Rowan owns lossless syntax. The losslessness invariants the
//! hand-rolled representation established are **kept as tests against this
//! tree** rather than retired with it — that is the point of the migration, not
//! a formality.
//!
//! Semantic analysis must not reach in here. It consumes HIR ids and spans.

use rowan::{GreenNode, GreenNodeBuilder, Language};

use crate::kind::{Pw, SyntaxKind, SyntaxNode};
use crate::lexer::{Token, lex};

/// Thin wrapper over `GreenNodeBuilder` that speaks `SyntaxKind` and keeps the
/// source text on hand, so every token pushed carries its real text.
pub struct TreeBuilder<'a> {
    src: &'a str,
    inner: GreenNodeBuilder<'static>,
}

impl<'a> TreeBuilder<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            inner: GreenNodeBuilder::new(),
        }
    }

    pub fn start(&mut self, kind: SyntaxKind) {
        self.inner.start_node(Pw::kind_to_raw(kind));
    }

    pub fn finish_node(&mut self) {
        self.inner.finish_node();
    }

    /// Push a lexed token, taking its text from the source span.
    pub fn token(&mut self, tok: &Token) {
        self.inner.token(
            Pw::kind_to_raw(tok.kind.into()),
            &self.src[tok.span.clone()],
        );
    }

    /// Push a token with explicit text. Used only by tests.
    pub fn raw_token(&mut self, kind: SyntaxKind, text: &str) {
        self.inner.token(Pw::kind_to_raw(kind), text);
    }

    pub fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.inner.checkpoint()
    }

    pub fn start_at(&mut self, cp: rowan::Checkpoint, kind: SyntaxKind) {
        self.inner.start_node_at(cp, Pw::kind_to_raw(kind));
    }

    pub fn finish(self) -> GreenNode {
        self.inner.finish()
    }
}

/// Build a flat green tree from `src`: every token under one `SourceFile` node.
///
/// This is not the parser — it is the losslessness floor. It exists so the
/// round-trip property can be asserted against Rowan **before** the grammar
/// lands, which means a later parse bug cannot be mistaken for a tree bug.
pub fn flat_tree(src: &str) -> SyntaxNode {
    let mut b = TreeBuilder::new(src);
    b.start(SyntaxKind::SourceFile);
    for t in lex(src) {
        if t.kind == crate::lexer::Kind::Eof {
            continue; // Eof has no text; including it would add an empty token
        }
        b.token(&t);
    }
    b.finish_node();
    SyntaxNode::new_root(b.finish())
}

/// The whole source text, reconstructed from the tree.
pub fn tree_text(node: &SyntaxNode) -> String {
    node.text().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::reconstruct;

    fn round_trips(src: &str) -> bool {
        tree_text(&flat_tree(src)) == src
    }

    #[test]
    fn the_green_tree_round_trips_byte_for_byte() {
        let src = "module store.pricing\n\n// a comment\nfn f(x: Int) -> Int !{} {\n    x + 1\n}\n";
        assert_eq!(tree_text(&flat_tree(src)), src);
    }

    #[test]
    fn round_trips_on_the_same_awkward_input_the_lexer_handles() {
        // ADR-0012: the hand-rolled invariants are retained, not retired.
        for src in [
            "",
            "   ",
            "\n\n\n",
            "// just a comment",
            "// @corpus: accepted\n// @id: A-001\n",
            "\"unterminated",
            "\"escaped \\\" quote\"",
            "30.seconds",
            "0.1",
            "a->b=>c",
            "§ unknown bytes ¤",
            "match x { A => 1, _ => 2 }",
            "{#each items as i (i.id)}",
        ] {
            assert!(round_trips(src), "tree did not round-trip: {src:?}");
        }
    }

    #[test]
    fn the_tree_and_the_token_stream_agree() {
        // Two independent reconstructions of the same source. If they diverge,
        // the token->tree mapping dropped or duplicated something.
        let src = "module m\n\nfn f() -> () !{ trace } {\n    // hi\n    g(1, 2)\n}\n";
        let toks = lex(src);
        assert_eq!(reconstruct(src, &toks), tree_text(&flat_tree(src)));
    }

    #[test]
    fn trivia_survives_into_the_tree() {
        let src = "// @id: A-001\nmodule m\n";
        let tree = flat_tree(src);
        let kinds: Vec<SyntaxKind> = tree
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .map(|t| t.kind())
            .collect();
        assert!(kinds.contains(&SyntaxKind::DocAttr), "{kinds:?}");
        assert!(kinds.contains(&SyntaxKind::Whitespace), "{kinds:?}");
    }

    #[test]
    fn token_ranges_match_the_source() {
        let src = "module store.pricing\n";
        let tree = flat_tree(src);
        for t in tree.children_with_tokens().filter_map(|e| e.into_token()) {
            let r = t.text_range();
            let (start, end) = (usize::from(r.start()), usize::from(r.end()));
            assert_eq!(&src[start..end], t.text(), "range does not match text");
        }
    }

    #[test]
    fn nesting_a_node_does_not_change_the_text() {
        // The property that makes the tree usable for a formatter: structure is
        // free, text is preserved.
        let src = "fn f() {}";
        let mut b = TreeBuilder::new(src);
        b.start(SyntaxKind::SourceFile);
        b.start(SyntaxKind::FnDecl);
        for t in lex(src) {
            if t.kind == crate::lexer::Kind::Eof {
                continue;
            }
            b.token(&t);
        }
        b.finish_node();
        b.finish_node();
        let node = SyntaxNode::new_root(b.finish());
        assert_eq!(tree_text(&node), src);
        assert_eq!(node.kind(), SyntaxKind::SourceFile);
        assert_eq!(node.first_child().unwrap().kind(), SyntaxKind::FnDecl);
    }

    #[test]
    fn the_round_trip_property_can_fail() {
        // docs/RISK_QUEUE.md negative control: a builder that skipped trivia
        // would pass every structural assertion above.
        let src = "fn f() { }\n// tail\n";
        let mut b = TreeBuilder::new(src);
        b.start(SyntaxKind::SourceFile);
        for t in lex(src) {
            if t.kind == crate::lexer::Kind::Eof || t.kind.is_trivia() {
                continue; // deliberately lossy
            }
            b.token(&t);
        }
        b.finish_node();
        let lossy = SyntaxNode::new_root(b.finish());
        assert_ne!(tree_text(&lossy), src, "dropping trivia must be detectable");
        assert_eq!(
            tree_text(&flat_tree(src)),
            src,
            "but the real builder keeps it"
        );
    }
}
