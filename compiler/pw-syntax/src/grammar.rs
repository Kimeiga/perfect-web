//! The tree-emitting parser: declarations **and** core expression bodies.
//!
//! ADR-0012 sequenced this deliberately — Rowan first, then body parsing — so
//! the body grammar is written once, against the durable representation.
//!
//! Architect ruling on scope: build the **real** expression syntax now, and
//! stage how much *semantic meaning* is implemented over it. Both shortcuts were
//! rejected with reasons worth restating, because they will look tempting again:
//!
//! - an *effect-only* grammar creates a second mini-language. The real parser
//!   eventually sees one program and the effect parser currently sees another;
//!   it can misassociate lambda bodies, call arguments, operator precedence,
//!   nested blocks or match arms while still producing plausible effect results.
//! - *declarations alone* cannot analyse
//!   `List.map(items, item => database.read(item.id))` — the higher-order case
//!   the corpus exists to catch.
//!
//! Trivia is attached before each significant token, so the tree stays lossless
//! by construction rather than by a later repair pass.

use crate::kind::{SyntaxKind as K, SyntaxNode};
use crate::lexer::{Kind, Span, Token, lex};
use crate::tree::TreeBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

pub struct Parse {
    pub green: SyntaxNode,
    pub errors: Vec<SyntaxError>,
}

impl Parse {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Keywords that may begin a declaration; also the resynchronisation set.
///
/// `pub(crate)` because `parser.rs` had its own copy of this list and they had
/// to agree. They stopped agreeing the moment E6 added `event`: the tree
/// grammar accepted the declaration and the declaration parser reported
/// "expected a declaration", for the same file, in the same build. Two lists
/// that must be identical are one list.
pub const DECL_STARTERS: &[&str] = &[
    "module",
    "import",
    "opaque",
    "type",
    "fn",
    "view",
    "component",
    "page",
    "query",
    "command",
    "subscription",
    "resource",
    "materialize",
    "event",
    "effect",
    "prelude",
    "replicated",
    "paint",
    "handler_policy",
    "public",
    "session",
    "private",
    "let",
];

/// Elements HTML forbids a closing tag on.
///
/// `<img src="x">` has no `</img>` and takes no children, and writing either is
/// a parse error the browser silently repairs. The grammar has to know the same
/// list the serializer does, or a page written correctly does not parse.
///
/// E7 task 2 gate 4 is what surfaced this: `examples/render/tricky.pw` is
/// ordinary HTML and `pw check` reported "unclosed block" on it.
pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub const UI_NOUNS: &[&str] = &["view", "component", "page"];

/// Statement keywords that may appear inside a body and take a
/// `kw [name] [(args)] [-> Type] [{ block }]` shape.
///
/// These are the §7.5A and resource-lifecycle forms. They are recognised
/// syntactically here; their *meaning* is staged (E4 for resources, E7 for the
/// frame phases), which is exactly the split the architect asked for.
const STMT_KEYWORDS: &[&str] = &[
    // `let cart = query Cart(session)` is ONE expression. Without this the
    // parser stopped after `query`, leaving the call as a sibling statement —
    // so a page that DECLARED a dependency on a query looked like a page that
    // CALLED into the database, and every such page was reported for reaching
    // the database while rendering. The corpus never showed it because the one
    // fixture with that shape was rejected for another reason anyway.
    "query",
    "command",
    "subscription",
    "use",
    "observe",
    "animate",
    "frame",
    "measure",
    "mutate",
    "post_paint",
    "subtree",
    "resource",
    "subscribe",
    "unsafe",
];
pub const RESOURCE_NOUNS: &[&str] = &[
    "query",
    "command",
    "subscription",
    "resource",
    "materialize",
    // E6. A typed event is a declaration because `invalidates_on
    // InventoryChanged(id, _item: MenuItemId)` has to be checkable against
    // something — otherwise a materialization can name an event that does not
    // exist and nothing notices until the materializer never fires.
    "event",
    "replicated",
    "paint",
];

/// Policy clause keywords. A value runs until the next one, a `{`, or a
/// declaration keyword.
/// Words that continue a statement onto the next line rather than beginning a
/// new one.
///
/// The newline rule that ends a statement is right for almost everything and
/// wrong for these: charter §14 M5 task 6 mandates a `because "…"` on an
/// escape hatch, and it is written on its own line. Without this the statement
/// ended early, `because` became a statement of its own, and the justification
/// never reached the declaration that needed it — so an audited escape hatch
/// was reported as unjustified.
/// Declaration node kinds — the ones a visibility keyword may precede.
fn is_decl_kind(k: K) -> bool {
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

const STMT_CLAUSE_KEYWORDS: &[&str] =
    &["because", "attributes_forced_layout_to", "when", "respects"];

pub const POLICY_KEYWORDS: &[&str] = &[
    // E8's effect ontology: `capability database.read<T>` and
    // `host "pw:host/database#read"` inside an `effect` block. Policy clauses
    // rather than expressions, because `database.read<T>` is a NAME and the
    // expression grammar reads `<` as a comparison — the same reason a
    // `materialize` block's policies live inside its braces (E6).
    "capability",
    "host",
    "freshness",
    "consistency",
    "cache",
    "invalidates_on",
    "invalidates",
    // E6 task 4: a command emits typed events in the same transaction as its
    // state change. Distinct from `invalidates`, which names a RESOURCE — an
    // event names a fact about the world, and which resources it affects is
    // the graph's answer rather than the command's.
    "emits",
    "fallback",
    "retry",
    "concurrency",
    "timeout",
    "requires",
    "idempotent_by",
    "transaction",
    "optimistic",
    "rollback",
    "placement",
    "privacy",
    "storage",
    "offline",
    "sync",
    "conflict",
    "scope",
    "transport",
    "reconnect",
    "dedupe_by",
    "on_scope_exit",
    "delivery",
    "partition",
    "depends_on",
    "regenerate",
    "stampede",
    "code_version",
    "locale",
    // E6 cache-key dimensions (charter §14 M6 task 1). Without these in the
    // table a policy value ran on past the newline and swallowed the next
    // clause, so `locale included_in_key` and `tenant included_in_key` became
    // one policy named `locale` — and the tenant dimension was simply absent
    // from the key audit that exists to notice absences.
    "tenant",
    "policy_version",
    "affine",
    "acquire",
    "release",
    "identity",
    "captures",
    "load",
    "revision",
    "inputs",
    "isolated",
    "draw",
    "key",
    "on_key_change",
    "on_conflict_unresolved",
    "on_version_mismatch",
    "intrinsic_height",
    "attributes_forced_layout_to",
    "because",
];

struct P<'a> {
    src: &'a str,
    toks: Vec<Token>,
    /// Index into `toks`, including trivia.
    pos: usize,
    b: TreeBuilder<'a>,
    errors: Vec<SyntaxError>,
    /// Guards against a grammar rule that consumes nothing in a loop.
    fuel: u32,
    /// A checkpoint taken before a visibility keyword, so the declaration that
    /// follows can re-parent it (see [`P::start`]).
    pending_vis: Option<rowan::Checkpoint>,
}

impl<'a> P<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            toks: lex(src),
            pos: 0,
            b: TreeBuilder::new(src),
            errors: Vec::new(),
            fuel: 0,
            pending_vis: None,
        }
    }

    // --- token access, skipping trivia ------------------------------------

    fn nth(&self, n: usize) -> &Token {
        let mut i = self.pos;
        let mut seen = 0;
        while i < self.toks.len() {
            if !self.toks[i].kind.is_trivia() {
                if seen == n {
                    return &self.toks[i];
                }
                seen += 1;
            }
            i += 1;
        }
        self.toks.last().expect("lex always emits Eof")
    }
    fn cur(&self) -> Kind {
        self.nth(0).kind
    }
    fn cur_text(&self) -> &'a str {
        let t = self.nth(0);
        &self.src[t.span.clone()]
    }
    fn cur_span(&self) -> Span {
        self.nth(0).span.clone()
    }
    fn at_eof(&self) -> bool {
        self.cur() == Kind::Eof
    }
    fn at(&self, k: Kind) -> bool {
        self.cur() == k
    }
    fn at_kw(&self, kw: &str) -> bool {
        self.cur() == Kind::Ident && self.cur_text() == kw
    }
    fn nth_is(&self, n: usize, k: Kind) -> bool {
        self.nth(n).kind == k
    }

    /// Is there a line break between the last consumed token and the next
    /// significant one?
    ///
    /// Statements are newline-terminated, so any loop that consumes bare
    /// identifiers must stop here. Without this guard a trailing-modifier loop
    /// swallows the first token of the *next* statement and the error surfaces
    /// one line later, pointing at innocent code.
    /// Is there unconsumed trivia before the next significant token?
    fn trivia_pending(&self) -> bool {
        self.toks.get(self.pos).is_some_and(|t| t.kind.is_trivia())
    }

    fn newline_ahead(&self) -> bool {
        self.toks[self.pos..]
            .iter()
            .take_while(|t| t.kind.is_trivia())
            .any(|t| self.src[t.span.clone()].contains('\n'))
    }

    /// Emit pending trivia into the current node, so the tree stays lossless.
    fn eat_trivia(&mut self) {
        while self.pos < self.toks.len() && self.toks[self.pos].kind.is_trivia() {
            let t = self.toks[self.pos].clone();
            self.b.token(&t);
            self.pos += 1;
        }
    }

    /// Consume the current significant token into the tree.
    fn bump(&mut self) {
        self.eat_trivia();
        if self.pos < self.toks.len() && self.toks[self.pos].kind != Kind::Eof {
            let t = self.toks[self.pos].clone();
            self.b.token(&t);
            self.pos += 1;
        }
    }

    fn eat(&mut self, k: Kind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn start(&mut self, k: K) {
        // A declaration preceded by a visibility keyword re-parents that
        // keyword into itself, so `private type X` is ONE declaration whose
        // first token says `private`. Without it the keyword became a
        // declaration of its own and the type looked public.
        if let Some(cp) = self.pending_vis.take() {
            if is_decl_kind(k) {
                self.b.start_at(cp, k);
                return;
            }
            self.pending_vis = Some(cp);
        }
        // Trivia belongs *outside* a node it merely precedes, so it is flushed
        // before the node opens. Without this, a comment before `fn` would end
        // up inside the FnDecl and the formatter would move it.
        self.eat_trivia();
        self.b.start(k);
    }

    /// Open a node **without** flushing trivia first.
    ///
    /// The one exception to the rule above, and it is markup: whitespace
    /// between two inline elements is content, not formatting. Flushing it
    /// first put it outside the `Text` node that was created to hold it, so the
    /// node came out empty and the space never reached HIR — which renders
    /// `<span>a</span> <span>b</span>` with the words run together.
    fn start_keeping_trivia(&mut self, k: K) {
        self.b.start(k);
    }
    fn finish(&mut self) {
        self.b.finish_node();
    }

    fn error(&mut self, code: &'static str, msg: impl Into<String>) {
        let span = self.cur_span();
        self.errors.push(SyntaxError {
            code,
            message: msg.into(),
            span,
            help: None,
        });
    }
    fn error_help(&mut self, code: &'static str, msg: impl Into<String>, help: impl Into<String>) {
        let span = self.cur_span();
        self.errors.push(SyntaxError {
            code,
            message: msg.into(),
            span,
            help: Some(help.into()),
        });
    }

    /// Consume `k` or report a diagnostic and keep going. Recovery is
    /// deliberate: a missing closer should not truncate the rest of the file.
    fn expect(&mut self, k: Kind, ctx: &str) -> bool {
        if self.eat(k) {
            return true;
        }
        let found = self.cur().describe();
        let want = k.describe();
        self.error("PW0001", format!("expected {want} {ctx}, found {found}"));
        false
    }

    fn name(&mut self, what: &str) -> bool {
        if self.at(Kind::Ident) {
            self.start(K::Name);
            self.bump();
            self.finish();
            true
        } else {
            let found = self.cur().describe();
            self.error("PW0001", format!("expected {what}, found {found}"));
            false
        }
    }

    /// A dotted path consumed as one `Name` node: `store.pricing`, `database.read`.
    fn dotted_name(&mut self, what: &str) -> bool {
        if !self.at(Kind::Ident) {
            let found = self.cur().describe();
            self.error("PW0001", format!("expected {what}, found {found}"));
            return false;
        }
        self.start(K::Name);
        self.bump();
        while self.at(Kind::Dot) && self.nth_is(1, Kind::Ident) {
            self.bump();
            self.bump();
        }
        self.finish();
        true
    }

    // --- types --------------------------------------------------------------

    fn type_ref(&mut self) -> bool {
        // The unit type `()`.
        if self.at(Kind::LParen) && self.nth_is(1, Kind::RParen) {
            self.start(K::TypeRef);
            self.bump();
            self.bump();
            self.finish();
            return true;
        }
        if !self.at(Kind::Ident) {
            let found = self.cur().describe();
            self.error("PW0001", format!("expected a type, found {found}"));
            return false;
        }
        self.start(K::TypeRef);
        self.dotted_name("a type name");
        if self.at(Kind::LAngle) {
            self.start(K::TypeArgList);
            self.bump();
            loop {
                if self.at(Kind::RAngle) || self.at_eof() {
                    break;
                }
                if !self.type_ref() {
                    break;
                }
                if !self.eat(Kind::Comma) {
                    break;
                }
            }
            if !self.eat(Kind::RAngle) {
                self.error("PW0002", "unclosed type argument list, expected `>`");
            }
            self.finish();
        }
        self.finish();
        true
    }

    /// `!{ a, b<C> }`
    fn effect_row(&mut self) {
        self.start(K::EffectRow);
        self.bump(); // `!`
        if self.eat(Kind::LBrace) {
            loop {
                if self.at(Kind::RBrace) || self.at_eof() {
                    break;
                }
                self.start(K::EffectRef);
                let ok = self.type_ref();
                self.finish();
                if !ok {
                    break;
                }
                if !self.eat(Kind::Comma) {
                    break;
                }
            }
            if !self.eat(Kind::RBrace) {
                self.error("PW0003", "unclosed effect row, expected `}`");
            }
        } else {
            self.error_help(
                "PW0004",
                "expected `{` after `!` to open an effect row",
                "write an empty row as `!{}` to claim purity explicitly",
            );
        }
        self.finish();
    }

    /// `<C>` on a declaration. Wrapped in a `TypeArgList` node rather than
    /// skipped as a balanced range, so the formatter knows the angle brackets
    /// delimit types and does not space them like comparisons —
    /// `opaque type Secret<C>`, not `opaque type Secret < C >`.
    fn type_params(&mut self) {
        if !self.at(Kind::LAngle) {
            return;
        }
        self.start(K::TypeArgList);
        self.bump();
        loop {
            if self.at(Kind::RAngle) || self.at_eof() {
                break;
            }
            if self.at(Kind::Ident) {
                self.start(K::TypeRef);
                self.name("a type parameter");
                self.finish();
            } else {
                self.bump();
            }
            if !self.eat(Kind::Comma) {
                break;
            }
        }
        self.eat(Kind::RAngle);
        self.finish();
    }

    fn param_list(&mut self) {
        if !self.at(Kind::LParen) {
            return;
        }
        self.start(K::ParamList);
        self.bump();
        loop {
            if self.at(Kind::RParen) || self.at_eof() {
                break;
            }
            self.start(K::Param);
            if !self.name("a parameter name") {
                self.finish();
                self.skip_balanced(Kind::LParen, Kind::RParen);
                break;
            }
            if self.eat(Kind::Colon) {
                self.type_ref();
            }
            self.finish();
            if !self.eat(Kind::Comma) {
                break;
            }
        }
        if !self.eat(Kind::RParen) {
            self.error("PW0005", "unclosed parameter list, expected `)`");
        }
        self.finish();
    }

    fn skip_balanced(&mut self, open: Kind, close: Kind) {
        let mut depth = 1i32;
        while !self.at_eof() {
            if self.at(open) {
                depth += 1;
            } else if self.at(close) {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            self.bump();
        }
    }

    // --- expressions --------------------------------------------------------

    /// Binding powers. Higher binds tighter. Real precedence, because an
    /// approximation here would misassociate operands while still producing a
    /// tree that looks parsed.
    fn infix_power(k: Kind, text: &str) -> Option<(u8, u8)> {
        let _ = text;
        let bp = match k {
            Kind::PipeGt => 1, // `a |> f |> g`, lowest precedence, left-assoc
            // `100.px -> 120.px -> 100.px`: an animation keyframe sequence.
            // Only reachable in expression position; a return type or a
            // statement arrow is consumed by the type and statement grammars
            // before `expr` ever sees it.
            Kind::Arrow => 2,
            Kind::Eq => 2,
            Kind::Amp => 3,
            Kind::Pipe => 3,
            // `==` `!=` `<=` `>=` sit with `<` and `>`: same precedence class,
            // and `<`/`>` are only comparisons here because a type argument
            // list is parsed by the type grammar, never by `expr`.
            Kind::Cmp | Kind::LAngle | Kind::RAngle => 5,
            Kind::Plus | Kind::Minus => 7,
            Kind::Star | Kind::Slash | Kind::Percent => 8,
            _ => return None,
        };
        Some((bp, bp + 1))
    }

    fn expr(&mut self, min_bp: u8) {
        // The checkpoint MUST be taken before the left operand is emitted.
        // Taking it afterwards makes `start_at` wrap nothing, so `a + b * c`
        // still produces BinaryExpr nodes — just empty ones in the wrong place.
        // The precedence test caught that; a test that only asserted
        // "a BinaryExpr exists" would not have.
        // Flush leading trivia BEFORE the checkpoint, so whitespace preceding an
        // expression stays outside its node. Otherwise every expression node
        // begins with the spaces in front of it, and a formatter reading node
        // boundaries would reflow them.
        self.eat_trivia();
        let cp = self.b.checkpoint();
        self.expr_lhs(cp);
        loop {
            if self.at_eof() {
                break;
            }
            // `=>` binds looser than every operator and is right-associative,
            // so the parameter is whatever was just parsed and the body is the
            // rest. Handling it here rather than as an `ident =>` lookahead is
            // what makes `(a, b) => e` and `resumable(captures = { .. }) => e`
            // parse with the same shape as `x => e`.
            if self.at(Kind::FatArrow) && min_bp == 0 {
                self.b.start_at(cp, K::LambdaExpr);
                self.bump();
                self.expr(0);
                self.finish();
                continue;
            }
            let k = self.cur();
            // `<`, `-` and `!` are both infix and prefix. At the start of a
            // line they begin a new statement — `<main>` opens markup, it does
            // not compare the previous line to `main`. Without this rule
            // `let s = f(id)` followed by `<main>` silently parses as one
            // comparison and the error surfaces three lines later.
            if matches!(k, Kind::LAngle | Kind::Minus | Kind::Bang) && self.newline_ahead() {
                break;
            }
            let text = self.cur_text();
            // `as` is a keyword operator whose right operand is a TYPE. It
            // binds tighter than every arithmetic operator, so `a as T + b`
            // is `(a as T) + b`.
            if k == Kind::Ident && text == "as" && !self.newline_ahead() {
                if 9 < min_bp {
                    break;
                }
                self.b.start_at(cp, K::CastExpr);
                self.bump();
                if !self.type_ref() {
                    self.error("PW0009", "expected a type after `as`");
                }
                self.finish();
                continue;
            }
            let Some((lbp, rbp)) = Self::infix_power(k, text) else {
                break;
            };
            if lbp < min_bp {
                break;
            }
            self.b.start_at(cp, K::BinaryExpr);
            self.bump(); // operator
            self.expr(rbp);
            self.finish();
        }
    }

    fn expr_lhs(&mut self, cp: rowan::Checkpoint) {
        match self.cur() {
            Kind::Int | Kind::Float | Kind::Str | Kind::UnterminatedStr => {
                self.start(K::LiteralExpr);
                self.bump();
                self.finish();
            }
            Kind::Minus | Kind::Bang => {
                self.start(K::UnaryExpr);
                self.bump();
                self.eat_trivia();
                let inner = self.b.checkpoint();
                self.expr_lhs(inner);
                self.finish();
            }
            Kind::LParen => {
                self.start(K::ParenExpr);
                self.bump();
                if !self.at(Kind::RParen) {
                    self.expr(0);
                }
                if !self.eat(Kind::RParen) {
                    self.error("PW0008", "unclosed parenthesis, expected `)`");
                }
                self.finish();
            }
            Kind::LBracket => {
                self.start(K::ListExpr);
                self.bump();
                loop {
                    if self.at(Kind::RBracket) || self.at_eof() {
                        break;
                    }
                    self.expr(0);
                    if !self.eat(Kind::Comma) {
                        break;
                    }
                }
                self.expect(Kind::RBracket, "to close a list literal");
                self.finish();
            }
            // `{#each ..}` / `{:else}` in expression position opens a template
            // region; a plain `{` is a block.
            Kind::LBrace
                if self.nth_is(1, Kind::Hash)
                    || self.nth_is(1, Kind::Slash)
                    || self.nth_is(1, Kind::Colon) =>
            {
                self.template_region()
            }
            Kind::LBrace => self.block_expr(),
            // HTML-like markup in a view or component body.
            Kind::LAngle => self.template_region(),
            Kind::Underscore => {
                self.start(K::NameExpr);
                self.bump();
                self.finish();
            }
            Kind::Ident => {
                if self.at_kw("if") {
                    return self.if_expr();
                }
                if self.at_kw("match") {
                    return self.match_expr();
                }
                if self.at_kw("let") {
                    return self.let_stmt();
                }
                if STMT_KEYWORDS.contains(&self.cur_text()) {
                    return self.keyword_stmt();
                }
                if self.at_kw("fn") && self.nth_is(1, Kind::LParen) {
                    self.start(K::LambdaExpr);
                    self.bump();
                    self.param_list();
                    self.expr(0);
                    self.finish();
                    return;
                }
                // A bare name. `a.b` is NOT absorbed here: in expression
                // position the parser cannot tell a module path from a field
                // access, and it must not pretend to. `postfix` gives every
                // `.ident` the same shape and name resolution folds the
                // leading segments that turn out to be a module path.
                self.start(K::NameExpr);
                self.bump();
                self.finish();
            }
            _ => {
                self.start(K::ErrorExpr);
                let found = self.cur().describe();
                self.error("PW0009", format!("expected an expression, found {found}"));
                self.bump();
                self.finish();
                return;
            }
        }
        self.postfix(cp);
    }

    /// Field access and calls, left-associatively: `a.b(c).d`.
    ///
    /// `cp` marks the position *before* the receiver was emitted, so each
    /// postfix step wraps everything to its left. Same trap as `expr`.
    fn postfix(&mut self, cp: rowan::Checkpoint) {
        loop {
            if self.at(Kind::Dot) && self.nth_is(1, Kind::Ident) {
                self.b.start_at(cp, K::FieldExpr);
                self.bump();
                self.bump();
                self.finish();
                continue;
            }
            // `Store { id: .. }` and `store.Inner { .. }` — a record literal.
            // Guarded by `record_body_ahead` so `if x { .. }` is unaffected.
            if self.record_body_ahead() {
                self.b.start_at(cp, K::RecordExpr);
                self.record_fields();
                self.finish();
                continue;
            }
            if self.at(Kind::LParen) {
                self.b.start_at(cp, K::CallExpr);
                self.start(K::ArgList);
                self.bump();
                loop {
                    if self.at(Kind::RParen) || self.at_eof() {
                        break;
                    }
                    // Named arguments: `max = 3`, `jitter: true`. The name is
                    // consumed here so the value parses as an ordinary
                    // expression rather than tripping on `=` or `:`.
                    if self.at(Kind::Ident)
                        && (self.nth_is(1, Kind::Eq) || self.nth_is(1, Kind::Colon))
                    {
                        self.bump();
                        self.bump();
                    }
                    self.expr(0);
                    if !self.eat(Kind::Comma) {
                        break;
                    }
                }
                if !self.eat(Kind::RParen) {
                    self.error("PW0010", "unclosed argument list, expected `)`");
                }
                self.finish(); // ArgList
                self.finish(); // CallExpr
                continue;
            }
            if self.at(Kind::Question) {
                self.bump(); // `?` propagation, no node of its own yet
                continue;
            }
            break;
        }
    }

    /// Does the brace ahead look like `{ field: ..` or `{}` rather than a block?
    fn record_body_ahead(&self) -> bool {
        if !self.at(Kind::LBrace) {
            return false;
        }
        self.nth_is(1, Kind::RBrace) || (self.nth_is(1, Kind::Ident) && self.nth_is(2, Kind::Colon))
    }

    fn record_fields(&mut self) {
        self.bump(); // `{`
        loop {
            if self.at(Kind::RBrace) || self.at_eof() {
                break;
            }
            self.start(K::Field);
            self.name("a field name");
            if self.eat(Kind::Colon) {
                self.expr(0);
            }
            self.finish();
            if !self.eat(Kind::Comma) {
                break;
            }
        }
        if !self.eat(Kind::RBrace) {
            self.error("PW0012", "unclosed record literal, expected `}`");
        }
    }

    /// A template region: `<main>...</main>`, `{#each ..}`, `{/each}`.
    ///
    /// Charter §14 M2 task 9 asks for "only enough template parsing to
    /// represent static HTML, expressions, conditions, and keyed loops". The
    /// markup itself is a region; its `{...}` interpolations are parsed as real
    /// expressions, which is what matters for effect analysis — a forbidden
    /// call inside `{store.name}` must be visible.
    ///
    /// Deliberately NOT a full HTML grammar: nesting validity, ARIA rules and
    /// keyed-loop checking are E3 and are not pretended here.
    /// A markup region, as a **tree**: elements, their attributes, their text.
    ///
    /// The first version consumed tags as a run of tokens. That round-tripped
    /// and parsed, and it was still wrong: E3 has to lower this to a renderer
    /// and E5 has to know which attribute carries which value, and neither can
    /// read a token soup. Losslessness was never the hard part — structure is.
    fn template_region(&mut self) {
        self.start(K::TemplateRegion);
        let mut depth = 0i32;
        let mut guard = 0;
        // Elements are opened and closed by separate tokens, so the tree is
        // built by starting a node on `<tag` and finishing it on `</tag`.
        // `open` counts how many `Element` nodes are waiting to be finished.
        let mut open = 0usize;
        // ...and how many `{#..}` blocks are waiting for their `{/..}`.
        let mut blocks = 0usize;
        while !self.at_eof() {
            guard += 1;
            if guard > 50_000 {
                self.error("PW0099", "template made no progress");
                break;
            }
            // Whitespace between markup is CONTENT. It is what separates two
            // inline elements, and leaving it as trivia drops it from HIR — so
            // `<span>a</span> <span>b</span>` renders without the space.
            if depth > 0 && self.trivia_pending() {
                self.start_keeping_trivia(K::Text);
                self.eat_trivia();
                self.finish();
            }
            match self.cur() {
                Kind::LAngle if self.nth_is(1, Kind::Slash) => {
                    depth -= 1;
                    self.start(K::CloseTag);
                    while !self.at_eof() && !self.at(Kind::RAngle) {
                        self.bump();
                    }
                    self.eat(Kind::RAngle);
                    self.finish(); // CloseTag
                    if open > 0 {
                        open -= 1;
                        self.finish(); // Element
                    }
                    if depth <= 0 {
                        break;
                    }
                }
                Kind::LAngle => {
                    depth += 1;
                    self.start(K::Element);
                    let self_closing = self.open_tag();
                    if self_closing {
                        depth -= 1;
                        self.finish(); // Element
                    } else {
                        open += 1;
                    }
                    if depth <= 0 {
                        break;
                    }
                }
                // `{/each}` closes the block it belongs to.
                Kind::LBrace if self.nth_is(1, Kind::Slash) => {
                    depth -= 1;
                    self.interpolation();
                    if blocks > 0 {
                        blocks -= 1;
                        self.finish(); // MarkupBlock
                    }
                    if depth <= 0 {
                        break;
                    }
                }
                // `{#each ..}` opens one. Nesting it — rather than leaving the
                // marker and its contents as siblings — is what lets a renderer
                // emit the children *inside* the loop.
                Kind::LBrace if self.nth_is(1, Kind::Hash) => {
                    depth += 1;
                    self.start_keeping_trivia(K::MarkupBlock);
                    self.interpolation();
                    blocks += 1;
                }
                Kind::LBrace => {
                    self.interpolation();
                }
                // A `}` at depth 0 belongs to the enclosing block, not to us.
                Kind::RBrace if depth <= 0 => break,
                _ => self.text_run(),
            }
        }
        // An unclosed element or block must still produce a well-formed tree.
        // Recovery that leaves nodes open would corrupt every ancestor's text
        // range, and the losslessness suite would then fail far from the cause.
        for _ in 0..(open + blocks) {
            self.finish();
        }
        self.finish(); // TemplateRegion
    }

    /// `<tag attr="v" on:press={h} />`. Returns whether it self-closed.
    ///
    /// A void element self-closes whether or not a slash was written, because
    /// HTML says so — the slash is ignored by every browser parser and a
    /// closing tag is discarded.
    fn open_tag(&mut self) -> bool {
        self.start(K::OpenTag);
        self.bump(); // `<`
        let mut void = false;
        // The tag name, which may be dotted for a component: `<store.Card />`.
        if self.at(Kind::Ident) {
            void = VOID_ELEMENTS.contains(&self.cur_text());
            self.start(K::Name);
            self.bump();
            while self.at(Kind::Dot) && self.nth_is(1, Kind::Ident) {
                self.bump();
                self.bump();
            }
            self.finish();
        }
        let mut self_closing = false;
        while !self.at_eof() && !self.at(Kind::RAngle) {
            if self.at(Kind::Slash) && self.nth_is(1, Kind::RAngle) {
                self_closing = true;
                self.bump();
                continue;
            }
            if self.at(Kind::Ident) {
                self.attribute();
                continue;
            }
            self.bump(); // stray punctuation; kept so the tree stays lossless
        }
        self.eat(Kind::RAngle);
        self.finish(); // OpenTag
        self_closing || void
    }

    /// `class="x"`, `on:press={handler}`, `style:width={w}`, `disabled`.
    fn attribute(&mut self) {
        self.start(K::Attr);
        self.start(K::AttrName);
        self.bump();
        // A namespaced attribute: `on:press`, `style:width`.
        while self.at(Kind::Colon) && self.nth_is(1, Kind::Ident) {
            self.bump();
            self.bump();
        }
        // `aria-label` lexes as three tokens; the name is all of them.
        while self.at(Kind::Minus) && self.nth_is(1, Kind::Ident) {
            self.bump();
            self.bump();
        }
        self.finish(); // AttrName
        if self.eat(Kind::Eq) {
            self.start(K::AttrValue);
            if self.at(Kind::LBrace) {
                self.interpolation();
            } else if self.at(Kind::Str) || self.at(Kind::Int) || self.at(Kind::Ident) {
                self.bump();
            }
            self.finish(); // AttrValue
        }
        self.finish(); // Attr
    }

    /// Character data between tags, as one node per run.
    fn text_run(&mut self) {
        self.start(K::Text);
        let mut moved = false;
        while !self.at_eof()
            && !self.at(Kind::LAngle)
            && !self.at(Kind::LBrace)
            && !self.at(Kind::RBrace)
        {
            self.bump();
            moved = true;
        }
        if !moved {
            self.bump(); // never spin
        }
        self.finish();
    }

    /// `{ expr }`, `{#each items as x (x.id)}`, `{/each}` inside a template.
    ///
    /// Returns the nesting delta: `{#..}` opens, `{/..}` closes, everything
    /// else is neutral.
    fn interpolation(&mut self) -> i32 {
        self.start(K::Interpolation);
        self.bump(); // `{`
        let delta = match self.cur() {
            Kind::Hash => 1,
            Kind::Slash => -1,
            _ => 0,
        };
        if self.at(Kind::Hash) || self.at(Kind::Slash) || self.at(Kind::Colon) {
            // A block marker: `{#each ..}`, `{/each}`, `{:else}`. Its contents
            // are directives, not a single expression, so they are consumed as
            // a region — except the parenthesised key, which is an expression.
            while !self.at(Kind::RBrace) && !self.at_eof() {
                self.bump();
            }
        } else if !self.at(Kind::RBrace) {
            self.expr(0);
        }
        self.expect(Kind::RBrace, "to close an interpolation");
        self.finish();
        delta
    }

    fn block_expr(&mut self) {
        self.start(K::BlockExpr);
        self.bump(); // `{`
        let mut guard = 0;
        while !self.at(Kind::RBrace) && !self.at_eof() {
            guard += 1;
            if guard > 20_000 {
                self.error("PW0099", "block made no progress");
                break;
            }
            let before = self.pos;
            // A named `fn` nested in a component or view body is a
            // declaration, not an expression. `fn(` with no name is still a
            // lambda. The nested-declaration set is deliberately this small:
            // it grows when a corpus file needs it, not on speculation.
            if self.at_kw("fn") && self.nth_is(1, Kind::Ident) {
                self.eat_trivia();
                self.decl();
            } else if self.at(Kind::Ident)
                && self.nth_is(1, Kind::Colon)
                && !STMT_KEYWORDS.contains(&self.cur_text())
            {
                // A labelled statement: `transform: scale(1.0) -> scale(1.08)`,
                // `duration: 180.milliseconds`. Same shape as a record field,
                // so it gets the same node kind — an `animate` block really is
                // a set of named property bindings.
                self.eat_trivia();
                self.start(K::Field);
                self.name("a property name");
                self.bump(); // `:`
                self.expr(0);
                self.finish();
            } else {
                self.expr(0);
            }
            self.eat(Kind::Comma);
            self.eat(Kind::Semi);
            if self.pos == before {
                self.bump(); // never spin
            }
        }
        if !self.eat(Kind::RBrace) {
            self.error("PW0006", "unclosed block, expected `}`");
        }
        self.finish();
    }

    /// `use key: Secret<Payments> = secrets.payments()`
    /// `observe resize(self) -> Float`
    /// `frame { measure { .. } mutate { .. } }`
    /// `unsafe capability synchronous_geometry because "..."`
    ///
    /// One syntactic shape covering the body-level statement family. Their
    /// semantics are staged: recognising them is E2's job, checking them is E4
    /// and E7's.
    fn keyword_stmt(&mut self) {
        self.start(K::LetStmt);
        self.bump(); // the keyword
        // A qualified keyword: `unsafe.imperative`, `unsafe.lifecycle`.
        while self.at(Kind::Dot) && self.nth_is(1, Kind::Ident) {
            self.bump();
            self.bump();
        }
        // An optional chain of modifier words before any punctuation:
        // `unsafe capability synchronous_geometry`, `observe resize`.
        while self.at(Kind::Ident)
            && (!self.newline_ahead() || STMT_CLAUSE_KEYWORDS.contains(&self.cur_text()))
            && !STMT_KEYWORDS.contains(&self.cur_text())
        {
            self.bump();
            if self.at(Kind::LParen)
                || self.at(Kind::Colon)
                || self.at(Kind::Eq)
                || self.at(Kind::Arrow)
                || self.at(Kind::LBrace)
                || self.at(Kind::Str)
            {
                break;
            }
        }
        if self.at(Kind::Str) {
            self.bump(); // a `because "..."` justification
        }
        // A statement may declare its own effect row: `post_paint !{ post_paint,
        // trace } { .. }`. Without this the row terminated the statement and the
        // block that followed became a sibling — so the work inside it looked
        // like it happened during render.
        if self.at(Kind::Bang) {
            self.effect_row();
        }
        if self.at(Kind::LParen) {
            self.start(K::ArgList);
            self.bump();
            loop {
                if self.at(Kind::RParen) || self.at_eof() {
                    break;
                }
                self.expr(0);
                if !self.eat(Kind::Comma) {
                    break;
                }
            }
            self.expect(Kind::RParen, "to close a statement argument list");
            self.finish();
        }
        if self.eat(Kind::Colon) {
            self.type_ref();
        }
        if self.eat(Kind::Arrow) {
            self.type_ref();
        }
        if self.eat(Kind::Eq) {
            self.expr(0);
        }
        // Trailing modifiers before a block: `resource map when visible { .. }`.
        while self.at(Kind::Ident)
            && (!self.newline_ahead() || STMT_CLAUSE_KEYWORDS.contains(&self.cur_text()))
        {
            self.bump();
        }
        if self.at(Kind::LBrace) {
            self.block_expr();
        }
        self.finish();
    }

    fn let_stmt(&mut self) {
        self.start(K::LetStmt);
        self.bump(); // let
        self.eat_kw("mut");
        self.name("a binding name");
        if self.eat(Kind::Colon) {
            self.type_ref();
        }
        if self.eat(Kind::Eq) {
            self.expr(0);
        }
        self.finish();
    }

    fn if_expr(&mut self) {
        self.start(K::IfExpr);
        self.bump(); // if
        self.expr(0);
        if self.at(Kind::LBrace) {
            self.block_expr();
        }
        while self.at_kw("elif") || self.at_kw("else") {
            let is_else = self.at_kw("else");
            self.bump();
            if is_else && self.at_kw("if") {
                self.bump();
                self.expr(0);
            } else if !is_else {
                self.expr(0);
            }
            if self.at(Kind::LBrace) {
                self.block_expr();
            }
        }
        self.finish();
    }

    fn match_expr(&mut self) {
        self.start(K::MatchExpr);
        self.bump(); // match
        self.expr(0);
        if self.eat(Kind::LBrace) {
            let mut guard = 0;
            while !self.at(Kind::RBrace) && !self.at_eof() {
                guard += 1;
                if guard > 5_000 {
                    self.error("PW0099", "match made no progress");
                    break;
                }
                let before = self.pos;
                self.start(K::MatchArm);
                self.pattern();
                if self.eat(Kind::FatArrow) {
                    self.expr(0);
                }
                self.finish();
                self.eat(Kind::Comma);
                if self.pos == before {
                    self.bump();
                }
            }
            if !self.eat(Kind::RBrace) {
                self.error("PW0006", "unclosed match, expected `}`");
            }
        }
        self.finish();
    }

    fn pattern(&mut self) {
        self.eat_trivia();
        let cp = self.b.checkpoint();
        self.pattern_atom();
        while self.at(Kind::Pipe) {
            self.b.start_at(cp, K::OrPat);
            self.bump();
            self.pattern_atom();
            self.finish();
        }
    }

    fn pattern_atom(&mut self) {
        match self.cur() {
            Kind::Underscore => {
                self.start(K::WildcardPat);
                self.bump();
                self.finish();
            }
            Kind::Int | Kind::Float | Kind::Str => {
                self.start(K::LiteralPat);
                self.bump();
                self.finish();
            }
            Kind::LParen => {
                self.start(K::TuplePat);
                self.bump();
                loop {
                    if self.at(Kind::RParen) || self.at_eof() {
                        break;
                    }
                    self.pattern();
                    if !self.eat(Kind::Comma) {
                        break;
                    }
                }
                self.eat(Kind::RParen);
                self.finish();
            }
            Kind::Ident => {
                // A constructor pattern if it takes arguments; a binding otherwise.
                let is_ctor = self.nth_is(1, Kind::LParen);
                self.start(if is_ctor { K::CtorPat } else { K::BindingPat });
                self.dotted_name("a pattern");
                if is_ctor {
                    self.bump(); // (
                    loop {
                        if self.at(Kind::RParen) || self.at_eof() {
                            break;
                        }
                        self.pattern();
                        if !self.eat(Kind::Comma) {
                            break;
                        }
                    }
                    self.eat(Kind::RParen);
                }
                self.finish();
            }
            _ => {
                self.start(K::WildcardPat);
                let found = self.cur().describe();
                self.error("PW0011", format!("expected a pattern, found {found}"));
                self.bump();
                self.finish();
            }
        }
    }

    // --- declarations -------------------------------------------------------

    fn policies(&mut self) {
        if !(self.at(Kind::Ident) && POLICY_KEYWORDS.contains(&self.cur_text())) {
            return;
        }
        self.start(K::PolicyList);
        while self.at(Kind::Ident) && POLICY_KEYWORDS.contains(&self.cur_text()) {
            self.start(K::Policy);
            self.bump(); // keyword
            let mut depth = 0i32;
            let mut took_value = false;
            while !self.at_eof() {
                let k = self.cur();
                if depth == 0 && k == Kind::LBrace {
                    break;
                }
                // A policy list inside a `materialize` block ends at the
                // block's own `}`. Without this the last clause's value
                // swallowed the closing brace, so the declaration looked
                // unclosed to everything downstream.
                if depth == 0 && k == Kind::RBrace {
                    break;
                }
                // A policy value must take at least one token. `cache private`
                // and `scope component` would otherwise end immediately,
                // because `private` and `component` also start declarations.
                // A policy value ends where the next clause or declaration
                // begins — and one only begins at the START OF A LINE.
                //
                // Without that condition, `key store, session` ended after the
                // comma, because `session` also starts a `session query`
                // declaration. Everything after it stopped being a policy, so
                // `cache shared` was invisible and the shared-cache rule did
                // not run at all. Silently: the query simply had no cache
                // policy as far as any checker could tell.
                //
                // `newline_ahead` is the right test even though the name reads
                // backwards: `nth()` skips trivia WITHOUT advancing `pos`, so
                // the whitespace between the last consumed token and this one
                // is still ahead of `pos`. A hand-rolled backwards scan looked
                // before that whitespace, always answered false, and silently
                // disabled the rule for two corpus fixtures.
                if took_value
                    && depth == 0
                    && k == Kind::Ident
                    && self.newline_ahead()
                    && (POLICY_KEYWORDS.contains(&self.cur_text())
                        || DECL_STARTERS.contains(&self.cur_text()))
                {
                    break;
                }
                took_value = true;
                match k {
                    Kind::LParen | Kind::LBracket | Kind::LAngle => depth += 1,
                    Kind::RParen | Kind::RBracket | Kind::RAngle => depth -= 1,
                    _ => {}
                }
                self.bump();
            }
            self.finish();
        }
        self.finish();
    }

    fn body(&mut self) {
        if !self.at(Kind::LBrace) {
            return;
        }
        self.start(K::Body);
        self.block_expr();
        self.finish();
    }

    /// A `materialize` block: policy clauses, then whatever else the block has.
    ///
    /// `materialize` is the one declaration whose policies live INSIDE its
    /// braces — corpus A-009 and R-017 both write them that way, and they are
    /// the specification. Parsing the block as an ordinary expression body read
    /// `placement` and `edge` as two unrelated bare names, so a fragment that
    /// declared where it runs, what it depends on and which events concern it
    /// produced a node with **no policy at all**. An empty dependency list is
    /// indistinguishable from "depends on nothing", so the E6 graph would have
    /// shown a fragment nothing invalidates and no rule would have objected.
    ///
    /// After the policies the rest of the block is parsed normally, because
    /// R-017 puts a `view { .. }` there and its wall-clock read has to be
    /// visible to the effect checker.
    fn materialize_body(&mut self) {
        if !self.at(Kind::LBrace) {
            return;
        }
        self.start(K::Body);
        self.start(K::BlockExpr);
        self.bump(); // `{`
        self.policies();
        let mut guard = 0;
        while !self.at(Kind::RBrace) && !self.at_eof() {
            guard += 1;
            if guard > 20_000 {
                self.error("PW0099", "block made no progress");
                break;
            }
            let before = self.pos;
            self.expr(0);
            self.eat(Kind::Comma);
            self.eat(Kind::Semi);
            if self.pos == before {
                self.bump(); // never spin
            }
        }
        if !self.eat(Kind::RBrace) {
            self.error("PW0006", "unclosed block, expected `}`");
        }
        self.finish();
        self.finish();
    }

    fn decl(&mut self) -> bool {
        if self.at_kw("module") || self.at_kw("import") {
            let is_module = self.at_kw("module");
            self.start(if is_module {
                K::ModuleDecl
            } else {
                K::ImportDecl
            });
            self.bump();
            self.dotted_name("a module name");
            if self.at(Kind::Dot) && self.nth_is(1, Kind::LBrace) {
                self.bump();
            }
            if self.at(Kind::LBrace) {
                self.bump();
                self.skip_balanced(Kind::LBrace, Kind::RBrace);
                self.eat(Kind::RBrace);
            }
            self.finish();
            return true;
        }

        // A visibility keyword may precede any declaration, not only the UI and
        // resource nouns. `private type Secretive` parsed as TWO declarations —
        // a bare word and an unqualified type — so the type looked public and a
        // module could import it.
        let vis_prefix = (self.at_kw("public") || self.at_kw("session") || self.at_kw("private"))
            && matches!(
                &self.src[self.nth(1).span.clone()],
                "type" | "opaque" | "fn" | "let"
            );

        if vis_prefix {
            // Flush trivia, take a checkpoint, then consume the keyword. The
            // branch below re-parents both under the declaration's real kind
            // via `start_at`, so the visibility is the declaration's first
            // token — which is where `lower::visibility_of` looks.
            self.eat_trivia();
            self.pending_vis = Some(self.b.checkpoint());
            self.bump();
        }

        if self.at_kw("opaque") {
            self.start(K::OpaqueDecl);
            self.bump();
            self.eat_kw("type");
            self.name("a type name");
            // Type parameters, as `type` already accepts: `opaque type
            // Secret<C>`. Without them `Secret<Payments>` could not be
            // *declared*, only written — so a privacy label naming a capability
            // had to be invented by a checker instead of read from a signature.
            self.type_params();
            if self.eat(Kind::Eq) {
                self.type_ref();
            }
            self.finish();
            return true;
        }

        if self.at_kw("type") {
            let is_record = {
                // `type X = Name {` is a record; otherwise a union.
                let mut i = 1;
                while !matches!(self.nth(i).kind, Kind::Eq | Kind::Eof) {
                    i += 1;
                }
                self.nth(i + 1).kind == Kind::Ident && self.nth(i + 2).kind == Kind::LBrace
            };
            self.start(if is_record {
                K::RecordDecl
            } else {
                K::UnionDecl
            });
            self.bump();
            self.name("a type name");
            if self.at(Kind::LAngle) {
                self.bump();
                self.skip_balanced(Kind::LAngle, Kind::RAngle);
                self.eat(Kind::RAngle);
            }
            if self.eat(Kind::Eq) {
                if is_record {
                    self.name("a record constructor");
                    if self.at(Kind::LBrace) {
                        self.start(K::FieldList);
                        self.bump();
                        loop {
                            if self.at(Kind::RBrace) || self.at_eof() {
                                break;
                            }
                            self.start(K::Field);
                            self.name("a field name");
                            if self.eat(Kind::Colon) {
                                self.type_ref();
                            }
                            self.finish();
                            if !self.eat(Kind::Comma) {
                                break;
                            }
                        }
                        self.eat(Kind::RBrace);
                        self.finish();
                    }
                } else {
                    self.start(K::VariantList);
                    loop {
                        self.eat(Kind::Pipe);
                        if !self.at(Kind::Ident) {
                            break;
                        }
                        self.start(K::Variant);
                        self.name("a variant name");
                        if self.at(Kind::LParen) {
                            self.bump();
                            loop {
                                if self.at(Kind::RParen) || self.at_eof() {
                                    break;
                                }
                                if !self.type_ref() {
                                    break;
                                }
                                if !self.eat(Kind::Comma) {
                                    break;
                                }
                            }
                            self.eat(Kind::RParen);
                        }
                        self.finish();
                        if !self.at(Kind::Pipe) {
                            break;
                        }
                    }
                    self.finish();
                }
            }
            self.finish();
            return true;
        }

        if self.at_kw("let") {
            self.start(K::LetDecl);
            self.bump();
            self.eat_kw("mut");
            self.name("a binding name");
            if self.eat(Kind::Colon) {
                self.type_ref();
            }
            if self.eat(Kind::Eq) {
                self.expr(0);
            }
            self.finish();
            return true;
        }

        if self.at_kw("fn") {
            self.start(K::FnDecl);
            self.bump();
            self.name("a function name");
            self.param_list();
            if self.eat(Kind::Arrow) {
                self.type_ref();
            }
            if self.at(Kind::Bang) {
                self.effect_row();
            }
            self.body();
            self.finish();
            return true;
        }

        let vis = self.at_kw("public") || self.at_kw("session") || self.at_kw("private");
        let after_vis = if vis { self.nth(1) } else { self.nth(0) };
        let after_vis_text = &self.src[after_vis.span.clone()];

        if after_vis.kind == Kind::Ident && UI_NOUNS.contains(&after_vis_text) {
            self.start(K::UiDecl);
            if vis {
                self.bump();
            }
            self.bump(); // noun
            self.name("a name");
            self.param_list();
            if self.at(Kind::Bang) {
                self.effect_row();
            }
            self.policies();
            self.body();
            self.finish();
            return true;
        }

        // E8 — `effect database.read<T> { capability .. host .. }`.
        //
        // Its own branch, ahead of the generic resource nouns, because it is
        // the ONE declaration whose name is a dotted path. Architect ruling,
        // 2026-08-07:
        //
        // > Give effect declarations their own path grammar […] That keeps the
        // > parser change local to the language feature that actually requires
        // > it. Given everything this project has discovered about syntax
        // > features accidentally widening unrelated grammar, I would strongly
        // > prefer that.
        //
        // So `type foo.bar` and `fn foo.bar()` stay invalid: `name()` is
        // untouched and only this branch reaches `dotted_name`.
        //
        // The type parameters reuse `type_params` — the same binder
        // `opaque type Secret<C>` uses — rather than an effect-specific one.
        // `prelude Effect` — the package saying which of its namespaces are
        // ambient. One keyword and one namespace name; no path, because the
        // module declaring it is the module whose declarations are exported.
        //
        // Architect ruling, 2026-08-07, sketched it as `prelude Effect from
        // web.effects` in a package manifest. Pleris has no manifest format —
        // a package is a directory of `.pw` files — so the declaration lives
        // in the module that owns the declarations. Same semantics for the one
        // case that exists, one fewer invention on the way, and it generalises
        // to `prelude Type` unchanged. What it cannot express is a package
        // electing a module it does not own; nothing needs that today, and a
        // manifest is where it belongs when packages get one.
        if after_vis.kind == Kind::Ident && after_vis_text == "prelude" {
            self.start(K::PreludeDecl);
            if vis {
                self.bump();
            }
            self.bump(); // `prelude`
            self.name("a namespace name");
            self.finish();
            return true;
        }

        if after_vis.kind == Kind::Ident && after_vis_text == "effect" {
            self.start(K::EffectDecl);
            if vis {
                self.bump();
            }
            self.bump(); // `effect`
            self.dotted_name("an effect name");
            self.type_params();
            self.materialize_body();
            self.finish();
            return true;
        }

        if after_vis.kind == Kind::Ident && RESOURCE_NOUNS.contains(&after_vis_text) {
            let materialize = after_vis_text == "materialize";
            self.start(K::ResourceDecl);
            if vis {
                self.bump();
            }
            self.bump(); // noun
            self.name("a name");
            self.param_list();
            if self.eat(Kind::Arrow) {
                self.type_ref();
            }
            if self.at(Kind::Bang) {
                self.effect_row();
            }
            self.policies();
            if materialize {
                self.materialize_body();
            } else {
                self.body();
            }
            self.finish();
            return true;
        }

        // `handler_policy { .. }` and similar bare named blocks.
        if self.at(Kind::Ident) && self.nth_is(1, Kind::LBrace) {
            self.start(K::ErrorDecl);
            self.name("a declaration name");
            self.body();
            self.finish();
            return true;
        }

        false
    }

    fn recover(&mut self) {
        while !self.at_eof() {
            if self.at(Kind::Ident) && DECL_STARTERS.contains(&self.cur_text()) {
                return;
            }
            if self.at(Kind::LBrace) {
                self.bump();
                self.skip_balanced(Kind::LBrace, Kind::RBrace);
                self.eat(Kind::RBrace);
                continue;
            }
            self.bump();
        }
    }

    fn run(mut self) -> Parse {
        self.b.start(K::SourceFile);
        while !self.at_eof() {
            self.fuel += 1;
            if self.fuel > 200_000 {
                self.error("PW0099", "parser made no progress");
                break;
            }
            let before = self.pos;
            if !self.decl() {
                self.start(K::ErrorDecl);
                let found = self.cur().describe();
                self.error_help(
                    "PW0007",
                    format!("expected a declaration, found {found}"),
                    format!(
                        "declarations start with one of: {}",
                        DECL_STARTERS.join(", ")
                    ),
                );
                self.bump();
                self.recover();
                self.finish();
            }
            if self.pos == before && !self.at_eof() {
                self.bump();
            }
        }
        // Trailing trivia must land inside the file node, or the tree loses it.
        self.eat_trivia();
        self.b.finish_node();

        Parse {
            green: SyntaxNode::new_root(self.b.finish()),
            errors: self.errors,
        }
    }
}

pub fn parse_tree(src: &str) -> Parse {
    P::new(src).run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::tree_text;

    fn parse_ok(src: &str) -> Parse {
        let p = parse_tree(src);
        assert!(p.ok(), "unexpected errors: {:?}", p.errors);
        p
    }

    /// Every test asserts losslessness too: a grammar that drops a token would
    /// otherwise pass every structural assertion.
    fn assert_lossless(src: &str, p: &Parse) {
        assert_eq!(tree_text(&p.green), src, "parse was not lossless");
    }

    fn kinds(node: &SyntaxNode) -> Vec<K> {
        node.descendants().map(|n| n.kind()).collect()
    }

    /// Text of the first node of `kind`, if any.
    fn first_text(p: &Parse, kind: K) -> Option<String> {
        p.green
            .descendants()
            .find(|n| n.kind() == kind)
            .map(|n| n.text().to_string())
    }

    /// Text of every node of `kind`, in document order.
    fn texts(p: &Parse, kind: K) -> Vec<String> {
        p.green
            .descendants()
            .filter(|n| n.kind() == kind)
            .map(|n| n.text().to_string())
            .collect()
    }

    #[test]
    fn markup_nests_elements_rather_than_listing_them() {
        // The token-run version round-tripped and parsed and was still useless:
        // a renderer cannot read a token soup. Assert the NESTING, because a
        // flat list of Element nodes would satisfy every other check here.
        let src = "view V() !{} {\n    <main><h1>Title</h1><p>Body</p></main>\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);

        let main = p
            .green
            .descendants()
            .find(|n| n.kind() == K::Element)
            .expect("an element");
        assert!(main.text().to_string().starts_with("<main>"));
        assert!(main.text().to_string().ends_with("</main>"));

        let inner: Vec<String> = main
            .children()
            .filter(|c| c.kind() == K::Element)
            .map(|c| c.text().to_string())
            .collect();
        assert_eq!(
            inner,
            ["<h1>Title</h1>", "<p>Body</p>"],
            "h1 and p must be CHILDREN of main, not siblings"
        );
    }

    #[test]
    fn a_self_closing_element_has_no_children() {
        let src = "view V() !{} {\n    <section><img />after</section>\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let img = p
            .green
            .descendants()
            .find(|n| n.kind() == K::Element && n.text().to_string().starts_with("<img"))
            .expect("the img");
        assert_eq!(img.text().to_string(), "<img />");
        assert!(
            img.children().all(|c| c.kind() != K::Element),
            "a self-closing element must not swallow what follows it"
        );
    }

    #[test]
    fn attributes_keep_their_names_and_values_apart() {
        let src = "view V() !{} {\n    <button aria-label=\"Add\" on:press={handler} disabled>Go</button>\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);

        assert_eq!(
            texts(&p, K::AttrName),
            ["aria-label", "on:press", "disabled"],
            "a namespaced or hyphenated name is ONE name"
        );
        let values = texts(&p, K::AttrValue);
        assert_eq!(
            values,
            ["\"Add\"", "{handler}"],
            "a bare attribute has no value"
        );

        // The interpolated handler must be a real expression, not text: an
        // effect checker has to be able to look inside it.
        assert!(
            p.green
                .descendants()
                .any(|n| n.kind() == K::NameExpr && n.text() == "handler"),
            "the attribute value must parse as an expression"
        );
        assert_eq!(texts(&p, K::Text), ["Go"]);
    }

    #[test]
    fn a_component_tag_keeps_its_qualified_name() {
        let src = "view V() !{} {\n    <store.Card id={x} />\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let name = p
            .green
            .descendants()
            .find(|n| n.kind() == K::Name && n.text().to_string().contains('.'))
            .expect("a dotted tag name");
        assert_eq!(name.text().to_string(), "store.Card");
    }

    #[test]
    fn an_unclosed_element_still_produces_a_well_formed_tree() {
        // Recovery that left nodes open would corrupt every ancestor's text
        // range, and the losslessness suite would then fail for a reason that
        // has nothing to do with the missing tag.
        let src = "view V() !{} {\n    <main><p>oops\n}\n";
        let p = parse_tree(src);
        assert_eq!(tree_text(&p.green), src, "still lossless");
        assert!(
            p.green
                .descendants()
                .filter(|n| n.kind() == K::Element)
                .count()
                >= 1,
            "the elements that did open must still be nodes"
        );
    }

    #[test]
    fn a_visibility_keyword_belongs_to_the_declaration_it_precedes() {
        // Permanent fixture, at the architect's request. `private type X`
        // parsed as TWO declarations — a bare word, then an unqualified type —
        // so the type came out with NO visibility and any module could import
        // it. That is not merely a parser bug: it silently widened a
        // module boundary, which is exactly the kind of claim the project makes.
        for src in [
            "private type Secretive = Secretive { y: Int }\n",
            "private type Alias = String\n",
            "public opaque type Id = String\n",
            "session fn f() -> Int !{} { 1 }\n",
            "private let x = 1\n",
        ] {
            let p = parse_ok(src);
            assert_lossless(src, &p);

            let decls: Vec<K> = p
                .green
                .children()
                .map(|c| c.kind())
                .filter(|k| *k != K::Whitespace)
                .collect();
            // ONE declaration, not two. Which flavour of declaration it is is
            // not what this test is about — that the visibility belongs to it
            // is.
            assert_eq!(
                decls.len(),
                1,
                "{src:?} must be ONE declaration, got {decls:?}"
            );
            assert!(
                is_decl_kind(decls[0]),
                "{src:?} produced {:?}, not a declaration",
                decls[0]
            );

            // ...and the visibility must be INSIDE it, or lowering cannot see it.
            let first = p
                .green
                .first_child()
                .expect("a declaration")
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| !t.kind().is_trivia())
                .expect("a first token");
            assert!(
                matches!(first.text(), "private" | "public" | "session"),
                "{src:?}: the declaration's first token is {:?}",
                first.text()
            );
        }

        // Control: a bare visibility word with no declaration after it must not
        // silently swallow whatever follows.
        let stray = parse_tree("private\n\ntype T = T { x: Int }\n");
        assert_eq!(
            tree_text(&stray.green),
            "private\n\ntype T = T { x: Int }\n"
        );
    }

    #[test]
    fn compound_comparisons_are_one_operator() {
        // Before `Cmp`, `opens >= closes` lexed as `>` then `=` and parsed as
        // `opens > (= closes)` — an error three tokens later.
        let src = "fn f() { if opens >= closes { 1 } else { 2 } }";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let bin = first_text(&p, K::BinaryExpr).expect("a comparison");
        assert_eq!(bin.trim(), "opens >= closes");

        // Negative control: `>` and `=` written apart must NOT join.
        let split = parse_tree("fn f() { a > = b }");
        assert!(!split.ok(), "`> =` must not lex as `>=`");
    }

    #[test]
    fn a_multi_line_string_is_a_single_token() {
        let src = "fn f() {\n    unsafe capability g\n        because \"\"\"one\n                  two\"\"\"\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let strs: Vec<_> = p
            .green
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == K::Str)
            .collect();
        assert_eq!(strs.len(), 1, "expected one string token, got {strs:?}");
        assert!(strs[0].text().contains('\n'), "it must span lines");

        // Negative control: a single-quoted string still ends at the newline,
        // which is the recovery property the triple-quoted form exists to keep.
        let one = parse_tree("fn f() { let s = \"unclosed\n }");
        assert!(
            one.green
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| t.kind() == K::UnterminatedStr),
            "a single-quoted string must not swallow the newline"
        );
    }

    #[test]
    fn markup_on_the_next_line_is_not_a_comparison() {
        // The bug this prevents is silent: `f(id)` and `<main>` parse as one
        // `<` comparison, and the error lands on `</main>` two lines later.
        let src = "view V() !{} {\n    let store = fetch(id)\n    <main><h1>{store.name}</h1></main>\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        assert!(
            first_text(&p, K::TemplateRegion)
                .is_some_and(|t| t.starts_with("<main>") && t.ends_with("</main>")),
            "the markup must be its own region: {:?}",
            first_text(&p, K::TemplateRegion)
        );
        let comparisons: Vec<_> = p
            .green
            .descendants()
            .filter(|n| n.kind() == K::BinaryExpr)
            .map(|n| n.text().to_string())
            .collect();
        assert!(
            comparisons.is_empty(),
            "no comparison here: {comparisons:?}"
        );

        // Positive control: on the SAME line `<` is still a comparison.
        let cmp = parse_ok("fn f() { a < b }");
        assert_eq!(first_text(&cmp, K::BinaryExpr).unwrap().trim(), "a < b");
    }

    #[test]
    fn a_template_region_balances_over_block_markers() {
        let src = "view V() !{} {\n    {#each items as i (i.id)}<li>{i.name}</li>{/each}\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let region = first_text(&p, K::TemplateRegion).expect("a region");
        assert!(
            region.contains("{/each}"),
            "the region must reach its closing marker, got {region:?}"
        );
        // The keyed expression inside `(i.id)` is real syntax, not skipped text.
        assert!(
            p.green
                .descendants()
                .any(|n| n.kind() == K::FieldExpr && n.text() == "i.name"),
            "interpolated expressions must be parsed, not consumed as tokens"
        );
    }

    #[test]
    fn a_qualified_keyword_statement_owns_its_block() {
        let src = "fn f() {\n    acquire { unsafe.imperative { Sdk.mount(self) } }\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let stmt = p
            .green
            .descendants()
            .find(|n| n.kind() == K::LetStmt && n.text().to_string().starts_with("unsafe."))
            .expect("the unsafe statement");
        assert!(
            stmt.descendants().any(|n| n.kind() == K::BlockExpr),
            "its block must be nested inside it, not a sibling"
        );
        assert!(stmt.text().to_string().contains("Sdk.mount"));
    }

    #[test]
    fn a_property_transition_is_a_named_field_holding_a_chain() {
        let src = "fn f() {\n    animate pulse {\n        transform: scale(1.0) -> scale(1.08) -> scale(1.0)\n    }\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let field = p
            .green
            .descendants()
            .find(|n| n.kind() == K::Field)
            .expect("a property binding");
        assert!(field.text().to_string().starts_with("transform:"));
        // Left-associative: the outer step covers the whole chain.
        let chain = field
            .descendants()
            .find(|n| n.kind() == K::BinaryExpr)
            .expect("a transition chain");
        assert_eq!(
            chain.text().to_string().trim(),
            "scale(1.0) -> scale(1.08) -> scale(1.0)"
        );
    }

    #[test]
    fn a_nested_fn_is_a_declaration_not_a_lambda() {
        let src = "component C() {\n    placement browser\n    fn load() -> Store !{ database.read<Stores> } {\n        Stores.get(id)\n    }\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let f = p
            .green
            .descendants()
            .find(|n| n.kind() == K::FnDecl)
            .expect("a nested fn declaration");
        // The effect row must belong to the nested fn, with its type argument
        // intact — this is the row an effect checker will read.
        let row = f
            .descendants()
            .find(|n| n.kind() == K::EffectRow)
            .expect("its effect row");
        assert_eq!(row.text().to_string().trim(), "!{ database.read<Stores> }");
        assert!(
            row.descendants().any(|n| n.kind() == K::TypeArgList),
            "`<Stores>` must parse as a type argument list"
        );
        // Negative control: an unnamed `fn(` is still a lambda.
        let l = parse_ok("fn g() { h(fn(x: Int) x + 1) }");
        assert!(l.green.descendants().any(|n| n.kind() == K::LambdaExpr));
    }

    #[test]
    fn a_function_with_a_body_produces_expression_nodes() {
        let src = "fn f(x: Int) -> Int !{} {\n    g(x, 1) + 2\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let ks = kinds(&p.green);
        assert!(ks.contains(&K::FnDecl), "{ks:?}");
        assert!(ks.contains(&K::EffectRow), "{ks:?}");
        assert!(ks.contains(&K::BlockExpr), "{ks:?}");
        assert!(ks.contains(&K::CallExpr), "{ks:?}");
        assert!(ks.contains(&K::BinaryExpr), "{ks:?}");
    }

    #[test]
    fn operator_precedence_is_real_not_approximated() {
        // `a + b * c` must nest as `a + (b * c)`. An approximation that got this
        // wrong would still produce a tree that "parsed".
        let src = "fn f() { a + b * c }";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let outer = p
            .green
            .descendants()
            .find(|n| n.kind() == K::BinaryExpr)
            .expect("a binary expression");
        // The outer operator is `+`, and its right operand is another BinaryExpr.
        assert!(outer.text().to_string().contains('+'));
        let inner = outer
            .descendants()
            .filter(|n| n.kind() == K::BinaryExpr)
            .nth(1)
            .expect("a nested binary expression");
        assert_eq!(inner.text().to_string().trim(), "b * c");
    }

    #[test]
    fn a_lambda_inside_a_call_is_parsed_as_a_lambda() {
        // R-037's shape: the effect travels through a callback. If the lambda
        // body were misassociated, an effect checker would look at the wrong
        // expression and still report something plausible.
        let src = "fn f() { List.map(items, item => database.read(item.id)) }";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let lambda = p
            .green
            .descendants()
            .find(|n| n.kind() == K::LambdaExpr)
            .expect("a lambda");
        let body = lambda.text().to_string();
        assert!(body.contains("database.read"), "lambda body: {body}");
        // The lambda must be INSIDE the argument list, not a sibling of the call.
        assert!(
            lambda.ancestors().any(|a| a.kind() == K::ArgList),
            "lambda is not inside the call's arguments"
        );
    }

    #[test]
    fn match_arms_and_patterns_are_structured() {
        let src = "fn f(s: OrderState) -> String {\n    match s {\n        Draft => \"d\",\n        Confirmed(c) => c,\n        _ => \"other\",\n    }\n}\n";
        let p = parse_ok(src);
        assert_lossless(src, &p);
        let ks = kinds(&p.green);
        assert!(ks.contains(&K::MatchExpr), "{ks:?}");
        assert_eq!(
            p.green
                .descendants()
                .filter(|n| n.kind() == K::MatchArm)
                .count(),
            3
        );
        assert!(ks.contains(&K::CtorPat), "{ks:?}");
        assert!(ks.contains(&K::WildcardPat), "{ks:?}");
    }

    #[test]
    fn unclosed_brackets_are_reported_not_swallowed() {
        // Before `expect` was wired in, each of these parsed silently: the
        // closer was consumed with a bare `eat` whose false was discarded.
        for (src, want) in [
            ("fn f() { [1, 2 }", "to close a list literal"),
            ("view v() { <p>{name</p> }", "to close an interpolation"),
        ] {
            let p = parse_tree(src);
            assert!(
                p.errors.iter().any(|e| e.message.contains(want)),
                "{src:?} should report {want:?}, got {:?}",
                p.errors
            );
            assert_lossless(src, &p);
        }
    }

    #[test]
    fn field_access_chains_left_to_right() {
        let src = "fn f() { a.b.c(1).d }";
        let p = parse_ok(src);
        assert_lossless(src, &p);

        // Asserting only that the node KINDS exist would pass even when each
        // wraps nothing — which is exactly the bug the precedence test caught.
        // So assert the nesting: the outermost postfix step must cover the
        // whole chain, and each inner one a strict prefix of it.
        let outer = p
            .green
            .descendants()
            .find(|n| matches!(n.kind(), K::FieldExpr | K::CallExpr))
            .expect("a postfix expression");
        assert_eq!(outer.text().to_string().trim(), "a.b.c(1).d");
        let inner: Vec<String> = outer
            .descendants()
            .filter(|n| matches!(n.kind(), K::FieldExpr | K::CallExpr))
            .map(|n| n.text().to_string())
            .collect();
        // Every prefix of the chain is its own node: `.c` and `.d` must have
        // the same shape even though a call intervenes between them.
        for want in ["a.b.c(1)", "a.b.c", "a.b"] {
            assert!(
                inner.contains(&want.to_string()),
                "missing {want:?} in {inner:?}"
            );
        }
    }

    #[test]
    fn declarations_still_parse_as_they_did() {
        for src in [
            "module store.pricing\n",
            "opaque type StoreId = String\n",
            "type OrderState =\n    | Draft\n    | Confirmed(OrderConfirmation)\n",
            "public query Store(id: StoreId) -> Store\n    freshness 30.seconds\n    cache shared\n{\n    Stores.get(id)\n}\n",
            "view V(x: Int) !{} {\n    x\n}\n",
        ] {
            let p = parse_tree(src);
            assert!(p.ok(), "{src:?} -> {:?}", p.errors);
            assert_eq!(tree_text(&p.green), src, "not lossless: {src:?}");
        }
    }

    #[test]
    fn recovery_reports_more_than_one_error_and_keeps_later_declarations() {
        let src =
            "module m\n\n$$$\n\nfn good() -> Int !{} { 1 }\n\n%%%\n\nfn also() -> Int !{} { 2 }\n";
        let p = parse_tree(src);
        assert!(p.errors.len() >= 2, "{:?}", p.errors);
        assert_eq!(tree_text(&p.green), src, "recovery must stay lossless");
        assert_eq!(
            p.green
                .descendants()
                .filter(|n| n.kind() == K::FnDecl)
                .count(),
            2,
            "both good functions must survive recovery"
        );
    }

    #[test]
    fn malformed_input_never_panics_and_stays_lossless() {
        for src in [
            "",
            "{",
            "}",
            "((((",
            "fn",
            "fn f(",
            "fn f(x:",
            "type",
            "type T =",
            "opaque type",
            "public query",
            "!{",
            "fn f() -> Int !{ database.read<",
            "\"unterminated",
            "§§§",
            "fn f() { match",
            "fn f() { a => }",
            "let",
            "fn f() { [1, 2",
        ] {
            let p = parse_tree(src);
            assert_eq!(tree_text(&p.green), src, "not lossless: {src:?}");
        }
    }

    #[test]
    fn the_grammar_can_accept_and_reject() {
        // docs/RISK_QUEUE.md negative control.
        assert!(parse_tree("module m\n").ok());
        assert!(!parse_tree("module m\n$$$\n").ok());
    }
}
