//! Declaration parser with error recovery.
//!
//! Recovery is the design constraint, not an afterthought. Charter §1.16 wants
//! deterministic compiler feedback for coding agents, and E0's diagnostic spike
//! recorded (finding F-3) that a parser which dies on the first token cannot
//! deliver it. So every failure path here resynchronises to the next
//! declaration boundary and keeps going.
//!
//! Two things this parser deliberately does **not** do:
//!
//! - parse expression or template bodies — it records their balanced token range
//!   and moves on. E2's gate is declarations, spans and recovery;
//! - interpret policy clauses — their grammar differs per keyword and E4 owns
//!   their meaning. They are captured as normalised text with a span.
//!
//! Pretending otherwise would produce a larger green test suite that proves less.

use crate::ast::*;
use crate::lexer::{Kind, Span, Token, lex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

pub struct Parsed {
    pub file: SourceFile,
    pub errors: Vec<ParseError>,
    pub tokens: Vec<Token>,
}

impl Parsed {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Keywords that may begin a declaration. Used both to dispatch and to
/// resynchronise after an error.
const DECL_STARTERS: &[&str] = &[
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
    "replicated",
    "paint",
    "handler_policy",
    "public",
    "session",
    "private",
    "let",
];

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    /// Indices into `toks` of the non-trivia tokens, so the parser never has to
    /// think about whitespace while the token stream stays lossless.
    sig: Vec<usize>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        let toks = lex(src);
        let sig = toks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.kind.is_trivia())
            .map(|(i, _)| i)
            .collect();
        Self {
            src,
            toks,
            sig,
            pos: 0,
            errors: Vec::new(),
        }
    }

    fn tok(&self, n: usize) -> &Token {
        let idx = self.sig.get(n).copied().unwrap_or(self.toks.len() - 1);
        &self.toks[idx]
    }
    fn peek(&self) -> &Token {
        self.tok(self.pos)
    }
    fn peek_at(&self, offset: usize) -> &Token {
        self.tok(self.pos + offset)
    }
    fn at_eof(&self) -> bool {
        self.peek().kind == Kind::Eof
    }
    fn text(&self, t: &Token) -> &'a str {
        t.text(self.src)
    }
    fn peek_text(&self) -> &'a str {
        self.text(self.peek())
    }
    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if !self.at_eof() {
            self.pos += 1;
        }
        t
    }
    fn eat_kw(&mut self, kw: &str) -> Option<Token> {
        if self.peek().kind == Kind::Ident && self.peek_text() == kw {
            Some(self.bump())
        } else {
            None
        }
    }
    fn at_kw(&self, kw: &str) -> bool {
        self.peek().kind == Kind::Ident && self.peek_text() == kw
    }
    fn eat(&mut self, kind: Kind) -> Option<Token> {
        if self.peek().kind == kind {
            Some(self.bump())
        } else {
            None
        }
    }

    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.errors.push(ParseError {
            code,
            message: message.into(),
            span,
            help: None,
        });
    }
    fn error_help(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
        help: impl Into<String>,
    ) {
        self.errors.push(ParseError {
            code,
            message: message.into(),
            span,
            help: Some(help.into()),
        });
    }

    fn ident(&mut self, what: &str) -> Option<Ident> {
        if self.peek().kind == Kind::Ident {
            let t = self.bump();
            Some(Ident {
                name: self.text(&t).to_string(),
                span: t.span,
            })
        } else {
            let found = self.peek().kind.describe();
            let span = self.peek().span.clone();
            self.error("PW0100", format!("expected {what}, found {found}"), span);
            None
        }
    }

    /// A dotted path: `store.pricing`, `database.read`.
    fn dotted(&mut self) -> Option<Ident> {
        let first = self.ident("a name")?;
        let mut name = first.name;
        let mut span = first.span;
        while self.peek().kind == Kind::Dot && self.peek_at(1).kind == Kind::Ident {
            self.bump();
            let part = self.bump();
            name.push('.');
            name.push_str(self.text(&part));
            span = span.start..part.span.end;
        }
        Some(Ident { name, span })
    }

    /// `Name<Arg, Arg>` — generics nest, so this recurses.
    fn type_ref(&mut self) -> Option<TypeRef> {
        // The unit type `()`. Written as a return type all over the corpus
        // (`fn emit(..) -> ()`), so it is not an edge case.
        if self.peek().kind == Kind::LParen && self.peek_at(1).kind == Kind::RParen {
            let open = self.bump();
            let close = self.bump();
            return Some(TypeRef {
                name: "()".to_string(),
                args: Vec::new(),
                span: open.span.start..close.span.end,
            });
        }
        let base = self.dotted()?;
        let mut args = Vec::new();
        let mut end = base.span.end;
        if self.peek().kind == Kind::LAngle {
            self.bump();
            loop {
                if self.peek().kind == Kind::RAngle || self.at_eof() {
                    break;
                }
                if let Some(a) = self.type_ref() {
                    args.push(a);
                } else {
                    break;
                }
                if self.peek().kind == Kind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            match self.eat(Kind::RAngle) {
                Some(t) => end = t.span.end,
                None => {
                    let span = self.peek().span.clone();
                    self.error("PW0101", "unclosed type argument list, expected `>`", span);
                }
            }
        }
        Some(TypeRef {
            name: base.name,
            args,
            span: base.span.start..end,
        })
    }

    /// `!{ a, b<C> }`
    fn effect_row(&mut self) -> Option<EffectRow> {
        let bang = self.eat(Kind::Bang)?;
        let mut effects = Vec::new();
        let mut end = bang.span.end;
        if let Some(open) = self.eat(Kind::LBrace) {
            end = open.span.end;
            loop {
                if self.peek().kind == Kind::RBrace || self.at_eof() {
                    break;
                }
                let Some(t) = self.type_ref() else { break };
                effects.push(EffectRef {
                    name: t.name,
                    args: t.args,
                    span: t.span,
                });
                if self.peek().kind == Kind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            match self.eat(Kind::RBrace) {
                Some(t) => end = t.span.end,
                None => {
                    let span = self.peek().span.clone();
                    self.error("PW0102", "unclosed effect row, expected `}`", span);
                }
            }
        } else {
            let span = self.peek().span.clone();
            self.error_help(
                "PW0103",
                "expected `{` after `!` to open an effect row",
                span,
                "write an empty row as `!{}` to claim purity explicitly",
            );
        }
        Some(EffectRow {
            effects,
            span: bang.span.start..end,
        })
    }

    fn params(&mut self) -> Vec<Param> {
        let mut out = Vec::new();
        if self.eat(Kind::LParen).is_none() {
            return out;
        }
        loop {
            if self.peek().kind == Kind::RParen || self.at_eof() {
                break;
            }
            let Some(name) = self.ident("a parameter name") else {
                self.skip_to_close(Kind::LParen, Kind::RParen);
                break;
            };
            let mut ty = None;
            let mut end = name.span.end;
            if self.eat(Kind::Colon).is_some() {
                ty = self.type_ref();
                if let Some(t) = &ty {
                    end = t.span.end;
                }
            }
            let span = name.span.start..end;
            out.push(Param { name, ty, span });
            if self.peek().kind == Kind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        if self.eat(Kind::RParen).is_none() {
            let span = self.peek().span.clone();
            self.error("PW0104", "unclosed parameter list, expected `)`", span);
        }
        out
    }

    /// Consume a balanced `{ .. }` body, returning its span. Bodies are not
    /// parsed at this milestone.
    fn balanced_body(&mut self) -> Option<Span> {
        let open = self.eat(Kind::LBrace)?;
        let mut depth = 1usize;
        let mut end = open.span.end;
        while !self.at_eof() {
            let t = self.bump();
            end = t.span.end;
            match t.kind {
                Kind::LBrace => depth += 1,
                Kind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(open.span.start..end);
                    }
                }
                _ => {}
            }
        }
        self.error(
            "PW0105",
            "unclosed block, expected `}`",
            open.span.start..end,
        );
        Some(open.span.start..end)
    }

    fn skip_to_close(&mut self, open: Kind, close: Kind) {
        let mut depth = 1usize;
        while !self.at_eof() {
            let k = self.peek().kind;
            if k == open {
                depth += 1;
            } else if k == close {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            self.bump();
        }
    }

    /// Resynchronise to the next thing that could start a declaration. Without
    /// this, one bad line hides every later error in the file.
    fn recover(&mut self) {
        while !self.at_eof() {
            if self.peek().kind == Kind::Ident && DECL_STARTERS.contains(&self.peek_text()) {
                return;
            }
            if self.peek().kind == Kind::LBrace {
                self.balanced_body();
                continue;
            }
            self.bump();
        }
    }

    /// Policy clauses run until the body `{` or the next declaration keyword.
    fn policies(&mut self) -> Vec<Policy> {
        let mut out = Vec::new();
        while !self.at_eof() {
            if self.peek().kind == Kind::LBrace {
                break;
            }
            if self.peek().kind != Kind::Ident || DECL_STARTERS.contains(&self.peek_text()) {
                break;
            }
            let kw_tok = self.bump();
            let keyword = Ident {
                name: self.text(&kw_tok).to_string(),
                span: kw_tok.span.clone(),
            };
            // Value = everything up to the next policy keyword, `{`, or EOF.
            let mut parts: Vec<&str> = Vec::new();
            let mut end = kw_tok.span.end;
            let mut depth = 0i32;
            while !self.at_eof() {
                let k = self.peek().kind;
                if depth == 0 && k == Kind::LBrace {
                    break;
                }
                if depth == 0 && k == Kind::Ident && self.starts_new_policy() {
                    break;
                }
                match k {
                    Kind::LParen | Kind::LBracket | Kind::LAngle => depth += 1,
                    Kind::RParen | Kind::RBracket | Kind::RAngle => depth -= 1,
                    _ => {}
                }
                let t = self.bump();
                end = t.span.end;
                parts.push(self.text(&t));
            }
            out.push(Policy {
                keyword,
                value: parts.join(" ").trim().to_string(),
                span: kw_tok.span.start..end,
            });
        }
        out
    }

    /// A policy value ends when the next identifier is followed by something
    /// that looks like a fresh clause rather than a continuation.
    fn starts_new_policy(&self) -> bool {
        const POLICY_KEYWORDS: &[&str] = &[
            "freshness",
            "consistency",
            "cache",
            "invalidates_on",
            "invalidates",
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
            "on_conflict_unresolved",
            "on_version_mismatch",
            "intrinsic_height",
            "attributes_forced_layout_to",
            "because",
        ];
        POLICY_KEYWORDS.contains(&self.peek_text())
    }

    // --- declarations -------------------------------------------------------

    fn decl(&mut self) -> Option<Decl> {
        let start = self.peek().span.start;

        // Visibility prefix on a resource declaration.
        let visibility = if self.at_kw("public") {
            self.bump();
            Visibility::Public
        } else if self.at_kw("session") {
            self.bump();
            Visibility::Session
        } else if self.at_kw("private") {
            self.bump();
            Visibility::Private
        } else {
            Visibility::Unspecified
        };

        if self.at_kw("module") || self.at_kw("import") {
            let kw = self.bump();
            let is_module = self.text(&kw) == "module";
            let name = self.dotted();
            // `import a.{B, C}` — consume the brace group if present.
            if self.peek().kind == Kind::Dot && self.peek_at(1).kind == Kind::LBrace {
                self.bump();
                self.balanced_body();
            } else if self.peek().kind == Kind::LBrace {
                self.balanced_body();
            }
            let end = name.as_ref().map(|n| n.span.end).unwrap_or(kw.span.end);
            return Some(Decl {
                kind: if is_module {
                    DeclKind::Module
                } else {
                    DeclKind::Import
                },
                name,
                span: start..end,
                body: None,
            });
        }

        if self.at_kw("opaque") {
            self.bump();
            self.eat_kw("type");
            let name = self.ident("a type name");
            let mut representation = None;
            let mut end = name.as_ref().map(|n| n.span.end).unwrap_or(start);
            if self.eat(Kind::Eq).is_some() {
                representation = self.type_ref();
                if let Some(r) = &representation {
                    end = r.span.end;
                }
            }
            return Some(Decl {
                kind: DeclKind::Opaque { representation },
                name,
                span: start..end,
                body: None,
            });
        }

        if self.at_kw("type") {
            self.bump();
            let name = self.ident("a type name");
            // Generic parameters on the declaration: `type Money<Currency> = ..`
            if self.peek().kind == Kind::LAngle {
                self.bump();
                self.skip_to_close(Kind::LAngle, Kind::RAngle);
                self.eat(Kind::RAngle);
            }
            let mut end = name.as_ref().map(|n| n.span.end).unwrap_or(start);
            if self.eat(Kind::Eq).is_none() {
                return Some(Decl {
                    kind: DeclKind::Unrecognised,
                    name,
                    span: start..end,
                    body: None,
                });
            }
            // A record is `Name { .. }`; a union is `| A | B` or `A | B`.
            if self.peek().kind == Kind::Ident && self.peek_at(1).kind == Kind::LBrace {
                self.bump();
                let body = self.balanced_body();
                if let Some(b) = &body {
                    end = b.end;
                }
                return Some(Decl {
                    kind: DeclKind::Record { fields: Vec::new() },
                    name,
                    span: start..end,
                    body,
                });
            }
            let mut variants = Vec::new();
            loop {
                self.eat(Kind::Pipe);
                if self.peek().kind != Kind::Ident {
                    break;
                }
                let vt = self.bump();
                let vname = Ident {
                    name: self.text(&vt).to_string(),
                    span: vt.span.clone(),
                };
                let mut fields = Vec::new();
                let mut vend = vt.span.end;
                if self.peek().kind == Kind::LParen {
                    self.bump();
                    loop {
                        if self.peek().kind == Kind::RParen || self.at_eof() {
                            break;
                        }
                        if let Some(t) = self.type_ref() {
                            vend = t.span.end;
                            fields.push(t);
                        } else {
                            break;
                        }
                        if self.peek().kind == Kind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if let Some(t) = self.eat(Kind::RParen) {
                        vend = t.span.end;
                    }
                }
                end = vend;
                variants.push(Variant {
                    name: vname,
                    fields,
                    span: vt.span.start..vend,
                });
                if self.peek().kind != Kind::Pipe {
                    break;
                }
            }
            return Some(Decl {
                kind: DeclKind::Union { variants },
                name,
                span: start..end,
                body: None,
            });
        }

        // Module-level binding: `let mut cached_handle: Option<MapHandle> = None`.
        if self.at_kw("let") {
            let kw = self.bump();
            self.eat_kw("mut");
            let name = self.ident("a binding name");
            let mut end = name.as_ref().map(|n| n.span.end).unwrap_or(kw.span.end);
            if self.eat(Kind::Colon).is_some()
                && let Some(t) = self.type_ref()
            {
                end = t.span.end;
            }
            // The initialiser runs to the end of the line or to a balanced
            // block; expressions are not parsed at this milestone.
            if self.eat(Kind::Eq).is_some() {
                if self.peek().kind == Kind::LBrace {
                    if let Some(b) = self.balanced_body() {
                        end = b.end;
                    }
                } else {
                    while !self.at_eof() {
                        let k = self.peek().kind;
                        if k == Kind::LBrace
                            || (k == Kind::Ident && DECL_STARTERS.contains(&self.peek_text()))
                        {
                            break;
                        }
                        end = self.bump().span.end;
                    }
                }
            }
            return Some(Decl {
                kind: DeclKind::Unrecognised,
                name,
                span: start..end,
                body: None,
            });
        }

        if self.at_kw("fn") {
            self.bump();
            let name = self.ident("a function name");
            let params = self.params();
            let mut result = None;
            if self.eat(Kind::Arrow).is_some() {
                result = self.type_ref();
            }
            let effects = if self.peek().kind == Kind::Bang {
                self.effect_row()
            } else {
                None
            };
            let body = self.balanced_body();
            let end = body
                .as_ref()
                .map(|b| b.end)
                .or_else(|| effects.as_ref().map(|e| e.span.end))
                .or_else(|| result.as_ref().map(|r| r.span.end))
                .unwrap_or(start);
            return Some(Decl {
                kind: DeclKind::Function {
                    params,
                    result,
                    effects,
                },
                name,
                span: start..end,
                body,
            });
        }

        const UI: &[&str] = &["view", "component", "page"];
        const RES: &[&str] = &[
            "query",
            "command",
            "subscription",
            "resource",
            "materialize",
            "replicated",
            "paint",
        ];

        if self.peek().kind == Kind::Ident && UI.contains(&self.peek_text()) {
            let kw = self.bump();
            let noun = self.text(&kw).to_string();
            let name = self.ident(&format!("a {noun} name"));
            let params = self.params();
            let effects = if self.peek().kind == Kind::Bang {
                self.effect_row()
            } else {
                None
            };
            let policies = self.policies();
            let body = self.balanced_body();
            let end = body.as_ref().map(|b| b.end).unwrap_or(kw.span.end);
            return Some(Decl {
                kind: DeclKind::Ui {
                    noun,
                    params,
                    effects,
                    policies,
                },
                name,
                span: start..end,
                body,
            });
        }

        if self.peek().kind == Kind::Ident && RES.contains(&self.peek_text()) {
            let kw = self.bump();
            let noun = self.text(&kw).to_string();
            let name = self.ident(&format!("a {noun} name"));
            let params = self.params();
            let mut result = None;
            if self.eat(Kind::Arrow).is_some() {
                result = self.type_ref();
            }
            // `paint RatingSpark(..) !{ paint.custom }` — resource-shaped
            // declarations may carry an effect row too.
            if self.peek().kind == Kind::Bang {
                self.effect_row();
            }
            let policies = self.policies();
            let body = self.balanced_body();
            let end = body
                .as_ref()
                .map(|b| b.end)
                .or_else(|| policies.last().map(|p| p.span.end))
                .unwrap_or(kw.span.end);
            return Some(Decl {
                kind: DeclKind::Resource {
                    noun,
                    visibility,
                    params,
                    result,
                    policies,
                },
                name,
                span: start..end,
                body,
            });
        }

        // `handler_policy { .. }` and similar bare blocks.
        if self.peek().kind == Kind::Ident && self.peek_at(1).kind == Kind::LBrace {
            let kw = self.bump();
            let name = Ident {
                name: self.text(&kw).to_string(),
                span: kw.span.clone(),
            };
            let body = self.balanced_body();
            let end = body.as_ref().map(|b| b.end).unwrap_or(kw.span.end);
            return Some(Decl {
                kind: DeclKind::Unrecognised,
                name: Some(name),
                span: start..end,
                body,
            });
        }

        None
    }

    fn attrs(&mut self) -> Vec<(String, String, Span)> {
        let mut out = Vec::new();
        for t in &self.toks {
            if t.kind != Kind::DocAttr {
                continue;
            }
            let text = t.text(self.src);
            let Some(rest) = text
                .strip_prefix("// @")
                .or_else(|| text.strip_prefix("//@"))
            else {
                continue;
            };
            if let Some((k, v)) = rest.split_once(':') {
                out.push((k.trim().to_string(), v.trim().to_string(), t.span.clone()));
            }
        }
        out
    }

    fn run(mut self) -> Parsed {
        let attrs = self.attrs();
        let mut decls = Vec::new();
        let mut module = None;
        let mut guard = 0usize;

        while !self.at_eof() {
            guard += 1;
            if guard > 100_000 {
                let span = self.peek().span.clone();
                self.error("PW0199", "parser made no progress", span);
                break;
            }
            let before = self.pos;
            match self.decl() {
                Some(d) => {
                    if matches!(d.kind, DeclKind::Module) && module.is_none() {
                        module = d.name.clone();
                    }
                    decls.push(d);
                }
                None => {
                    let t = self.peek().clone();
                    let found = t.kind.describe();
                    self.error_help(
                        "PW0106",
                        format!("expected a declaration, found {found}"),
                        t.span.clone(),
                        format!(
                            "declarations start with one of: {}",
                            DECL_STARTERS.join(", ")
                        ),
                    );
                    self.bump();
                    self.recover();
                }
            }
            // Belt and braces: never spin on a token the branches did not consume.
            if self.pos == before && !self.at_eof() {
                self.bump();
            }
        }

        Parsed {
            file: SourceFile {
                module,
                decls,
                attrs,
            },
            errors: self.errors,
            tokens: self.toks,
        }
    }
}

pub fn parse(source: &str) -> Parsed {
    Parser::new(source).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_module_and_a_function_with_an_effect_row() {
        let src = "module store.pricing\n\nfn subtotal(cart: Cart) -> Money<USD> !{} {\n    0\n}\n";
        let p = parse(src);
        assert!(p.ok(), "{:?}", p.errors);
        assert_eq!(p.file.module.as_ref().unwrap().name, "store.pricing");
        let f = p
            .file
            .decls
            .iter()
            .find(|d| matches!(d.kind, DeclKind::Function { .. }))
            .unwrap();
        assert_eq!(f.name.as_ref().unwrap().name, "subtotal");
        let DeclKind::Function {
            params,
            result,
            effects,
        } = &f.kind
        else {
            panic!()
        };
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].ty.as_ref().unwrap().name, "Cart");
        assert_eq!(result.as_ref().unwrap().name, "Money");
        assert_eq!(result.as_ref().unwrap().args[0].name, "USD");
        assert!(
            effects.as_ref().unwrap().effects.is_empty(),
            "!{{}} is an empty row"
        );
    }

    #[test]
    fn an_effect_row_records_each_effect_with_a_span() {
        let src = "fn load(id: StoreId) -> Store !{ database.read<Stores>, trace } { 0 }";
        let p = parse(src);
        assert!(p.ok(), "{:?}", p.errors);
        let DeclKind::Function { effects, .. } = &p.file.decls[0].kind else {
            panic!()
        };
        let row = effects.as_ref().unwrap();
        assert_eq!(row.effects.len(), 2);
        assert_eq!(row.effects[0].name, "database.read");
        assert_eq!(row.effects[0].args[0].name, "Stores");
        assert_eq!(row.effects[1].name, "trace");
        assert_eq!(&src[row.effects[0].span.clone()], "database.read<Stores>");
    }

    #[test]
    fn no_effect_row_is_different_from_an_empty_one() {
        // `!{}` claims purity; writing nothing claims nothing.
        let unannotated = parse("fn f() -> Int { 0 }");
        let DeclKind::Function { effects, .. } = &unannotated.file.decls[0].kind else {
            panic!()
        };
        assert!(effects.is_none());

        let pure = parse("fn f() -> Int !{} { 0 }");
        let DeclKind::Function { effects, .. } = &pure.file.decls[0].kind else {
            panic!()
        };
        assert!(effects.as_ref().unwrap().effects.is_empty());
    }

    #[test]
    fn parses_a_tagged_union_with_payloads() {
        let src = "type OrderState =\n    | Draft\n    | Confirmed(OrderConfirmation)\n    | Cancelled(CancellationReason)\n";
        let p = parse(src);
        assert!(p.ok(), "{:?}", p.errors);
        let DeclKind::Union { variants } = &p.file.decls[0].kind else {
            panic!()
        };
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name.name, "Draft");
        assert!(variants[0].fields.is_empty());
        assert_eq!(variants[1].fields[0].name, "OrderConfirmation");
        assert_eq!(&src[variants[2].name.span.clone()], "Cancelled");
    }

    #[test]
    fn parses_an_opaque_type_with_its_representation() {
        let p = parse("opaque type StoreId = String");
        assert!(p.ok(), "{:?}", p.errors);
        let DeclKind::Opaque { representation } = &p.file.decls[0].kind else {
            panic!()
        };
        assert_eq!(representation.as_ref().unwrap().name, "String");
        assert_eq!(p.file.decls[0].name.as_ref().unwrap().name, "StoreId");
    }

    #[test]
    fn parses_a_query_with_visibility_and_policies() {
        let src = "public query Store(id: StoreId) -> Result<Store, StoreError>\n    freshness      30.seconds\n    consistency    snapshot\n    cache          shared\n{\n    Stores.get(id)\n}\n";
        let p = parse(src);
        let d = &p.file.decls[0];
        let DeclKind::Resource {
            noun,
            visibility,
            params,
            policies,
            ..
        } = &d.kind
        else {
            panic!("{:?}", d.kind)
        };
        assert_eq!(noun, "query");
        assert_eq!(*visibility, Visibility::Public);
        assert_eq!(params.len(), 1);
        let names: Vec<&str> = policies.iter().map(|p| p.keyword.name.as_str()).collect();
        assert!(names.contains(&"freshness"), "{names:?}");
        assert!(names.contains(&"consistency"), "{names:?}");
        assert!(names.contains(&"cache"), "{names:?}");
        let cache = policies.iter().find(|p| p.keyword.name == "cache").unwrap();
        assert_eq!(cache.value, "shared");
    }

    #[test]
    fn recovers_and_reports_more_than_one_error() {
        // E0 finding F-3: a parser that dies on the first token cannot give an
        // agent the whole error batch per compile.
        let src = "module m\n\n@@@\n\nfn good() -> Int !{} { 1 }\n\n###\n\nfn also_good() -> Int !{} { 2 }\n";
        let p = parse(src);
        assert!(
            p.errors.len() >= 2,
            "expected several errors, got {:?}",
            p.errors
        );
        // Both good functions must still be parsed.
        let fns: Vec<&str> = p
            .file
            .decls
            .iter()
            .filter(|d| matches!(d.kind, DeclKind::Function { .. }))
            .filter_map(|d| d.name.as_ref().map(|n| n.name.as_str()))
            .collect();
        assert_eq!(fns, vec!["good", "also_good"]);
    }

    #[test]
    fn errors_carry_real_spans_into_the_source() {
        let src = "module m\n\n%%%\n";
        let p = parse(src);
        let e = &p.errors[0];
        assert_eq!(&src[e.span.clone()], "%");
        assert!(e.span.start > 0);
    }

    #[test]
    fn malformed_input_never_panics_and_always_terminates() {
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
            "module\nmodule\nmodule",
        ] {
            let p = parse(src); // must terminate and not panic
            let _ = p.ok();
        }
    }

    #[test]
    fn the_parser_can_report_success_and_failure() {
        // docs/RISK_QUEUE.md: a check that cannot fail is not a check.
        assert!(parse("module m\n").ok());
        assert!(!parse("module m\n$$$\n").ok());
    }
}
