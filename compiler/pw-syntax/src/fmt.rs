//! `pw fmt` — canonical formatting (ADR-0013).
//!
//! Works on the Rowan tree, which is why ADR-0013 deferred implementation
//! until ADR-0012 landed: a formatter needs every byte, including the ones a
//! parser normally throws away.
//!
//! # What this normalises
//!
//! Indentation, inter-token spacing, trailing whitespace, runs of blank lines,
//! and policy-column alignment. Every token, in order, with every comment,
//! reaches the output — the formatter never decides what the program *is*,
//! only how it is laid out.
//!
//! # What this deliberately does not do
//!
//! **It does not re-break lines.** Where the author put a line ending, one
//! stays. ADR-0013 makes "minimal line changes outside the reformatted syntax
//! node" non-negotiable and lists line width as at most a configuration knob;
//! width-driven reflowing is the part of a formatter most likely to be wrong in
//! a way that is hard to undo, and it is the part this defers. The honest
//! consequence is recorded in `docs/milestones/E2.md`: two files differing only
//! in line breaks format to two different outputs, so this is canonical
//! *spacing*, not yet a canonical *layout*.
//!
//! **It does not reformat template regions.** Markup is whitespace-sensitive in
//! ways the grammar does not model, so a `TemplateRegion` is copied verbatim.

use crate::kind::{SyntaxKind as K, SyntaxNode, SyntaxToken};

pub const INDENT: &str = "    ";

/// Format a parsed source file.
pub fn format_tree(root: &SyntaxNode) -> String {
    let mut f = Fmt {
        src: root.text().to_string(),
        out: String::new(),
        indent: 0,
        at_line_start: true,
        blank_run: 0,
        pad: Vec::new(),
        last_text: String::new(),
    };
    f.node(root);
    // Exactly one trailing newline, and never a blank line before it.
    while f.out.ends_with(char::is_whitespace) {
        f.out.pop();
    }
    if !f.out.is_empty() {
        f.out.push('\n');
    }
    f.out
}

/// Convenience: parse and format.
pub fn format_source(src: &str) -> String {
    format_tree(&crate::grammar::parse_tree(src).green)
}

struct Fmt {
    /// The original text, used only to ask whether two offsets share a line.
    src: String,
    out: String,
    /// Indentation for the line currently being written, decided when the
    /// line's first token is seen.
    indent: usize,
    at_line_start: bool,
    /// Consecutive newlines already emitted; used to cap blank-line runs.
    blank_run: usize,
    /// Column widths for policy alignment, innermost last.
    pad: Vec<usize>,
    /// Text of the last significant token written. `(` needs it: the last
    /// *character* cannot tell a keyword from a callee.
    last_text: String,
}

impl Fmt {
    fn node(&mut self, n: &SyntaxNode) {
        // Markup is copied byte for byte (see module docs).
        if n.kind() == K::TemplateRegion {
            self.raw_region(n);
            return;
        }

        // A policy list is a table: every value starts at the same column,
        // computed from the longest policy name in *this* list. That keeps the
        // reflow inside one syntax node, which is what makes it compatible with
        // ADR-0013's minimal-line-changes rule.
        let pushed = if n.kind() == K::PolicyList {
            self.pad.push(policy_column(n));
            true
        } else {
            false
        };

        for child in n.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Node(c) => self.node(&c),
                rowan::NodeOrToken::Token(t) => self.token(&t, n),
            }
        }

        if pushed {
            self.pad.pop();
        }
    }

    /// Emit a subtree exactly as written, but re-indent its first line.
    fn raw_region(&mut self, n: &SyntaxNode) {
        if let Some(t) = first_significant(n).filter(|_| self.at_line_start) {
            self.indent = self.indent_for(&t);
        }
        self.ensure_space_before_word();
        self.out.push_str(&n.text().to_string());
        self.at_line_start = false;
        self.blank_run = 0;
    }

    fn token(&mut self, t: &SyntaxToken, parent: &SyntaxNode) {
        match t.kind() {
            K::Whitespace => self.whitespace(t.text()),
            K::LineComment | K::DocAttr => {
                if self.at_line_start {
                    self.indent = self.indent_for(t);
                }
                self.ensure_space_before_word();
                self.out.push_str(t.text().trim_end());
                self.at_line_start = false;
                self.blank_run = 0;
            }
            K::Eof => {}
            _ => self.significant(t, parent),
        }
    }

    /// Whitespace never reaches the output verbatim: only the *number of line
    /// breaks* in it survives, and that is capped at one blank line.
    fn whitespace(&mut self, text: &str) {
        let newlines = text.matches('\n').count();
        if newlines == 0 {
            // Horizontal space is regenerated from the spacing rules, so it is
            // dropped here rather than copied — this is what makes the output
            // independent of the author's alignment except where a rule adds it.
            return;
        }
        for _ in 0..newlines.min(2) {
            if self.blank_run < 2 {
                self.trim_trailing_spaces();
                self.out.push('\n');
                self.blank_run += 1;
            }
        }
        self.at_line_start = true;
    }

    fn significant(&mut self, t: &SyntaxToken, parent: &SyntaxNode) {
        let text = t.text();

        if self.at_line_start {
            self.indent = self.indent_for(t);
            self.write_indent();
        } else if self.needs_space_before(t.kind(), parent) {
            self.out.push(' ');
        }

        self.out.push_str(text);
        self.last_text = text.to_string();
        self.at_line_start = false;
        self.blank_run = 0;

        // Policy alignment: pad after the policy's name so every value in the
        // list begins at the same column.
        // A BLOCK policy is not aligned. `acquire { .. }` and
        // `release(handle) { .. }` are headers followed by a block, not a name
        // followed by a value in a column, and padding them produced
        // `acquire   {`. Since 2026-08-11 they are real declarations of a named
        // execution root, and they read like one.
        if let Some(width) = self.pad.last().copied().filter(|_| {
            parent.kind() == K::Policy
                && !has_block(parent)
                && first_significant(parent).is_some_and(|f| f.text_range() == t.text_range())
        }) {
            for _ in text.chars().count()..width {
                self.out.push(' ');
            }
        }
    }

    fn needs_space_before(&self, kind: K, parent: &SyntaxNode) -> bool {
        let prev = self.out.chars().next_back().unwrap_or(' ');
        if prev == ' ' {
            return false;
        }
        // Never a space directly inside a bracket, or before a separator.
        if matches!(prev, '(' | '[') {
            return false;
        }
        if matches!(
            kind,
            K::RParen | K::RBracket | K::Comma | K::Semi | K::Question
        ) {
            return false;
        }
        // An empty block is `{}`, not `{ }` — and an empty effect row is
        // `!{}`, which is how the corpus writes an explicit claim of purity.
        if kind == K::RBrace && prev == '{' {
            return false;
        }
        // `f(x)` and `a[i]` — a call's parenthesis hugs its callee, but a
        // parenthesised expression after a keyword does not.
        if kind == K::LParen {
            // `Result<(), E>` — the unit type opening a type argument list.
            if prev == '<' {
                return false;
            }
            // A keyword is letters too, so "the previous character is a letter"
            // alone turned `for (i, v)` into `for(i, v)`. Deciding by node kind
            // alone was worse: a variant constructor and a policy value both
            // hug their parenthesis without being an `ArgList`.
            if PAREN_TAKES_A_SPACE.contains(&self.last_text.as_str()) {
                return true;
            }
            return !prev.is_alphanumeric() && prev != '_' && prev != ')' && prev != ']';
        }
        if kind == K::LBracket {
            return !prev.is_alphanumeric() && prev != '_' && prev != ')' && prev != ']';
        }
        // `a.b`, `Type<Arg>`, `!{ .. }`, `x: T`, `-1` as a prefix.
        if matches!(kind, K::Dot) || prev == '.' {
            return false;
        }
        if kind == K::Colon {
            return false;
        }
        if kind == K::LBrace && prev == '!' {
            return false;
        }
        if matches!(kind, K::LAngle | K::RAngle) && (in_type_position(parent) || in_policy(parent))
        {
            return false;
        }
        if prev == '<' && (in_type_position(parent) || in_policy(parent)) {
            return false;
        }
        // A prefix operator hugs its operand. The operand is usually a *child*
        // node of the `UnaryExpr`, so asking whether the operand's own parent
        // is a `UnaryExpr` answers the wrong question and yields `! available`.
        if matches!(prev, '!' | '-') && parent.ancestors().take(3).any(|a| a.kind() == K::UnaryExpr)
        {
            return false;
        }
        true
    }

    fn ensure_space_before_word(&mut self) {
        if self.at_line_start {
            self.write_indent();
            self.at_line_start = false;
        } else if !self.out.ends_with(' ') && !self.out.is_empty() {
            self.out.push(' ');
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str(INDENT);
        }
        self.at_line_start = false;
    }

    /// How deep the line beginning with `t` sits.
    ///
    /// Counted from the tree, not from a running brace tally: indentation is
    /// "how many unfinished constructs am I inside", and braces are only some
    /// of them. A brace counter cannot indent a wrapped return type, a policy
    /// list, or a pipeline continuation, because none of those open a brace —
    /// which is exactly what the first version got wrong on 33 corpus files.
    fn indent_for(&self, t: &SyntaxToken) -> usize {
        let start = usize::from(t.text_range().start());
        let mut n = 0;
        // A declaration's header indents its continuation lines, but its body
        // must not indent twice — the body's own block already does that.
        let mut inside_a_body = false;
        // ...and neither must a bracketed group inside the header: a record's
        // field list already indents its fields, so the `type` declaration
        // around it must not add a second level. Tracked per declaration
        // rather than globally, so a nested declaration still indents.
        let mut scope_since_last_decl = false;
        // Brackets opened on the same line indent together, not once each:
        // `Ok(Store {` opens two, and its contents belong one level in, not
        // two. Recorded as the offset each counted scope opened at.
        let mut counted_lines: Vec<usize> = Vec::new();

        for a in t.parent_ancestors() {
            if a.kind() == K::Body {
                inside_a_body = true;
                continue;
            }
            let is_decl_here = is_decl(a.kind());
            if !is_indent_scope(a.kind()) && !is_decl_here {
                continue;
            }
            if is_decl_here && (inside_a_body || scope_since_last_decl) {
                scope_since_last_decl = false;
                continue;
            }
            // Only a construct that began on an earlier line indents this one.
            let open = usize::from(a.text_range().start());
            if open >= start || !self.src[open..start].contains('\n') {
                continue;
            }
            // The token that closes a construct belongs to the outer level:
            // `}` lines up with the line that opened the block, not with its
            // contents.
            if last_significant(&a).is_some_and(|l| l.text_range() == t.text_range()) {
                continue;
            }
            let line = self.src[..open].matches('\n').count();
            if counted_lines.contains(&line) {
                continue;
            }
            counted_lines.push(line);
            n += 1;
            scope_since_last_decl = !is_decl_here;
        }

        // A line that opens with an operator continues the line above it —
        // but only inside a body. A wrapped return type is already indented by
        // its declaration, and adding the bonus there indents it twice.
        if is_continuation_op(t.kind()) && t.parent_ancestors().any(|a| a.kind() == K::Body) {
            n += 1;
        }
        n
    }

    fn trim_trailing_spaces(&mut self) {
        while self.out.ends_with(' ') {
            self.out.pop();
        }
    }
}

/// The column every value in a policy list starts at: longest name + 1.
///
/// The corpus aligns all 8 of its policy blocks, with no exceptions, but two
/// files use different padding. A formatter has to pick one rule; this is it.
/// Does this policy own a block? `acquire { .. }`, `draw(ctx) { .. }`.
fn has_block(policy: &SyntaxNode) -> bool {
    policy.children().any(|c| c.kind() == K::BlockExpr)
}

fn policy_column(list: &SyntaxNode) -> usize {
    // The grammar keeps a policy keyword as a bare token, not a `Name` node.
    // Looking for a `Name` found nothing, so every column came out as 1 and the
    // alignment silently did not happen — the output still parsed, was still
    // idempotent, and was still wrong.
    list.children()
        .filter(|c| c.kind() == K::Policy && !has_block(c))
        .filter_map(|p| first_significant(&p))
        .map(|t| t.text().chars().count())
        .max()
        .unwrap_or(0)
        + 1
}

fn first_significant(n: &SyntaxNode) -> Option<SyntaxToken> {
    n.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !t.kind().is_trivia() && t.kind() != K::Eof)
}

/// Keywords after which `(` opens a grouping, not a call.
const PAREN_TAKES_A_SPACE: &[&str] = &["for", "if", "while", "match", "return", "in"];

/// Node kinds whose contents sit one level deeper than they do.
fn is_indent_scope(k: K) -> bool {
    matches!(
        k,
        K::BlockExpr
            // A policy VALUE that spans lines. `conflict merge_by_field(\n
            // notes = .., ..)` is raw tokens inside the policy — there is no
            // `ArgList` node to indent from — so without this its continuation
            // lines came back flush with the policy head. Harmless until
            // 2026-08-11, when `replicated` began parsing its clauses as
            // policies and `A-011` stopped being canonically formatted.
            | K::Policy
            | K::MatchExpr
            | K::ArgList
            | K::ParamList
            | K::VariantList
            | K::FieldList
            | K::ListExpr
            | K::RecordExpr
            | K::TypeArgList
    )
}

fn is_decl(k: K) -> bool {
    matches!(
        k,
        K::FnDecl
            | K::UiDecl
            | K::ResourceDecl
            | K::TypeDecl
            | K::RecordDecl
            | K::UnionDecl
            | K::OpaqueDecl
            | K::LetDecl
            | K::ImportDecl
            | K::ModuleDecl
    )
}

/// An operator that cannot begin a statement, so a line starting with one is
/// continuing the line above: `|> List.map(..)`, `-> Result<..>`.
fn is_continuation_op(k: K) -> bool {
    matches!(
        k,
        K::PipeGt
            | K::Arrow
            | K::FatArrow
            | K::Dot
            | K::Plus
            | K::Star
            | K::Slash
            | K::Percent
            | K::Amp
            | K::Cmp
    )
}

fn last_significant(n: &SyntaxNode) -> Option<SyntaxToken> {
    n.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia() && t.kind() != K::Eof)
        .last()
}

/// Inside a type, `<` and `>` delimit arguments and must not be spaced like
/// comparison operators.
fn in_type_position(n: &SyntaxNode) -> bool {
    n.kind() == K::TypeArgList
        || n.kind() == K::TypeRef
        || n.ancestors()
            .take(3)
            .any(|a| matches!(a.kind(), K::TypeArgList | K::TypeRef))
}

/// A policy clause's value, where `<` and `>` also delimit type arguments.
///
/// `capability database.read<T>` — E8's effect declaration. A policy value is
/// consumed as raw tokens (`grammar::policies`), so the tree cannot say "this
/// is a type" the way it can inside a `TypeRef`, and the enclosing clause says
/// it instead. Sound because a policy value is a declarative name and never an
/// expression: there is no policy in the language whose value is a comparison.
///
/// Without this, `pw fmt` rewrote `capability database.read<T>` as
/// `capability database.read < T >` — the exact defect `grammar::type_params`
/// exists to prevent one layer up, arriving through the one construct whose
/// value is not a parsed type.
fn in_policy(n: &SyntaxNode) -> bool {
    n.kind() == K::Policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::parse_tree;
    use crate::lexer::{Kind, lex};

    /// The significant tokens of a source, as text. Two sources with the same
    /// list mean the same program.
    fn significant(src: &str) -> Vec<String> {
        lex(src)
            .into_iter()
            .filter(|t| !t.kind.is_trivia() && t.kind != Kind::Eof)
            .map(|t| src[t.span.clone()].to_string())
            .collect()
    }

    fn comments(src: &str) -> Vec<String> {
        lex(src)
            .into_iter()
            .filter(|t| matches!(t.kind, Kind::LineComment | Kind::DocAttr))
            .map(|t| src[t.span.clone()].trim_end().to_string())
            .collect()
    }

    #[test]
    fn formatting_is_idempotent() {
        let src = "module m\n\nfn  f( x:Int )->Int !{ } {\n  g(x,1)+2\n}\n";
        let once = format_source(src);
        let twice = format_source(&once);
        assert_eq!(once, twice, "fmt(fmt(x)) must equal fmt(x)");
    }

    #[test]
    fn formatting_preserves_every_significant_token() {
        // ADR-0013: semantics-preserving. Asserted by comparing token streams,
        // not by eyeballing output.
        let src = "fn f(x: Int) -> Int !{ trace } {\n    match x {\n        A(y) => y,\n        _ => 0,\n    }\n}\n";
        assert_eq!(significant(src), significant(&format_source(src)));
    }

    #[test]
    fn formatting_preserves_every_comment() {
        let src = "// @id: A-001\n// leading\nmodule m\n\nfn f() {\n    // inside\n    g() // trailing\n}\n";
        let out = format_source(src);
        assert_eq!(comments(src), comments(&out), "a comment was lost:\n{out}");
    }

    #[test]
    fn the_comment_check_can_fail() {
        // Negative control: a formatter that dropped comments would pass every
        // structural assertion above.
        let src = "fn f() {\n    // kept?\n    g()\n}\n";
        let lossy: String = lex(src)
            .into_iter()
            .filter(|t| !matches!(t.kind, Kind::LineComment | Kind::DocAttr))
            .map(|t| src[t.span.clone()].to_string())
            .collect();
        assert_ne!(
            comments(src),
            comments(&lossy),
            "the check must detect a drop"
        );
        assert_eq!(comments(src), comments(&format_source(src)));
    }

    #[test]
    fn indentation_is_four_spaces_and_braces_line_up() {
        let src = "fn f() {\nif a {\ng()\n}\n}\n";
        let out = format_source(src);
        assert_eq!(
            out, "fn f() {\n    if a {\n        g()\n    }\n}\n",
            "got:\n{out}"
        );
    }

    #[test]
    fn runs_of_blank_lines_collapse_to_one() {
        let src = "module m\n\n\n\n\nfn f() {}\n";
        let out = format_source(src);
        assert_eq!(out, "module m\n\nfn f() {}\n", "got:\n{out}");
    }

    #[test]
    fn trailing_whitespace_is_removed() {
        let src = "fn f() {   \n    g()   \n}\n";
        let out = format_source(src);
        assert!(
            !out.lines().any(|l| l.ends_with(' ')),
            "trailing space survived:\n{out:?}"
        );
    }

    #[test]
    fn a_policy_list_is_aligned_to_its_longest_name() {
        let src = "public query q() -> Int\n    cache 30.seconds\n    invalidates_on Order\n{\n    1\n}\n";
        let out = format_source(src);
        let cache = out.lines().find(|l| l.contains("cache")).expect("cache");
        let inval = out
            .lines()
            .find(|l| l.contains("invalidates_on"))
            .expect("invalidates_on");
        let col = |l: &str, name: &str| l.find(name).unwrap() + name.len() + 1;
        assert_eq!(
            col(cache, "cache") + (14 - 5),
            col(inval, "invalidates_on"),
            "values must start at the same column:\n{out}"
        );
        // And the whole thing is still idempotent with the padding in place.
        assert_eq!(out, format_source(&out));
    }

    #[test]
    fn a_template_region_is_copied_verbatim() {
        // Markup whitespace is not the formatter's to normalise.
        let src = "view V() !{} {\n    <p>  spaced   text  </p>\n}\n";
        let out = format_source(src);
        assert!(
            out.contains("<p>  spaced   text  </p>"),
            "markup was reflowed:\n{out}"
        );
    }

    #[test]
    fn calls_and_types_are_spaced_canonically() {
        let src = "fn f(a:List<Int>,b:Int)->Map<K,V> !{} {\n    g( a , b )\n}\n";
        let out = format_source(src);
        assert!(out.contains("g(a, b)"), "call spacing:\n{out}");
        assert!(
            out.contains("List<Int>"),
            "type args must not be spaced:\n{out}"
        );
        assert!(
            out.contains("!{}"),
            "an empty effect row is `!{{}}`:\n{out}"
        );
        assert_eq!(out, format_source(&out));
    }

    #[test]
    fn a_capability_clauses_type_argument_is_not_spaced_like_a_comparison() {
        // E8's effect declaration. A policy value is consumed as raw tokens,
        // so `<` reached `needs_space_before` with a `Policy` parent and was
        // spaced as an operator: `capability database.read < T >`. The
        // declaration then no longer looked like the effect row it describes,
        // which is the one property the flat-dotted form exists for.
        let src = "module p\n\neffect database.read<T> {\n    \
                   capability database.read<T>\n    \
                   host \"pw:host/database#read\"\n}\n";
        let out = format_source(src);
        assert!(
            out.contains("capability database.read<T>"),
            "a capability's type argument must not be spaced:\n{out}"
        );
        assert!(
            out.contains("effect database.read<T>"),
            "and neither must the declaration's own binder:\n{out}"
        );
        assert_eq!(out, format_source(&out), "idempotent");
    }

    #[test]
    fn a_comparison_in_an_expression_is_still_spaced() {
        // The control. If `<` stopped being spaced everywhere, the test above
        // would pass while the formatter had been broken for ordinary code.
        let out = format_source("module p\n\nfn f(a: Int) -> Bool !{} {\n    a<1\n}\n");
        assert!(out.contains("a < 1"), "a comparison is an operator:\n{out}");
    }

    #[test]
    fn formatting_a_broken_file_still_produces_output() {
        // ADR-0013: stable under partially invalid syntax where possible.
        let src = "fn f() {\n    let a = @@@\n    g()\n}\n";
        let p = parse_tree(src);
        assert!(!p.ok());
        let out = format_tree(&p.green);
        assert_eq!(
            significant(src),
            significant(&out),
            "no token may be lost from a broken file"
        );
    }
}
