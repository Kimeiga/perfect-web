//! **HIR → Backend IR.**
//!
//! E10-A's first stage. Architect ruling, 2026-08-09:
//!
//! > For the first slice, support only the language constructs `add_to_cart`
//! > genuinely needs […] Unsupported constructs should produce an explicit
//! > "backend unsupported" diagnostic, never silently lower differently.
//!
//! and the rule that decides what this file is allowed to compute:
//!
//! > The backend consumes semantics already established upstream. It must not
//! > rediscover effects, capabilities, placement, privacy, or ABI types.
//!
//! # What it consumes rather than derives
//!
//! ```text
//! which calls need authority     the CONTRACT's required_capabilities
//! what a name resolves to        the WORKSPACE
//! what a call returns            SIGNATURES
//! what a type is                 the program's declarations, by DefId
//! ```
//!
//! The one thing it decides is shape: which HIR expression becomes which
//! instruction. Everything a decision depends on arrives already answered, and
//! `no_backend_decision_is_made_from_a_name` is the test that says so.
//!
//! # Why `add_to_cart` gets no special case
//!
//! > Compile exactly one real Pleris command end-to-end first, but do not build
//! > any stage specifically around `add_to_cart`.
//!
//! So the supported set is described as a set of CONSTRUCTS — a call, a name, a
//! literal, a field — and `add_to_cart` is simply a program that stays inside
//! it. `a_declaration_outside_the_supported_set_is_refused_by_name` is what
//! keeps that honest: the refusal names the construct, so widening the backend
//! is a visible act rather than a fixture that started passing.

use std::collections::BTreeMap;

use super::ir::{
    Block, BlockId, CapabilityId, Const, Function, Instr, Lowering, Program, Shape, Terminator,
    Type, TypeDef, ValueId,
};
use crate::contract::ComponentContract;
use crate::hir::{Body, Decl, DeclKind, Expr, ExprId, Hir, Literal, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::signatures::Signatures;

/// **Proof that the program being lowered resolved and checked.**
///
/// Architect ruling, 2026-08-10, step 10: *enforce resolved/checked program
/// input before E10.*
///
/// The reason is E10-A's founding finding. `examples/store/app.pw` called
/// `current_session()` and imported nothing, every upstream analysis produced
/// an answer for it — no effects, no label, no constraint, no capability — and
/// each of those answers is what a harmless call produces too. The backend was
/// the first consumer for which the absence was fatal, and it found out by
/// failing to emit rather than by being told.
///
/// So the precondition is carried by a **type with a private field**. The only
/// way to obtain a `Checked` is `Checked::of`, which runs the checker on the
/// same units, and `program` takes one. A caller cannot hand the backend an
/// unresolved program by forgetting a step: there is no step to forget.
///
/// It is deliberately not a `bool` on `Context`, and not a documented
/// convention. Both are things a caller can be wrong about, which is the shape
/// of every entry in `docs/RISK_QUEUE.md`.
pub struct Checked<'a> {
    cx: Context<'a>,
}

impl<'a> Checked<'a> {
    /// Run the checker, and return the context only if it produced no errors.
    ///
    /// `units` must be the same program `cx` was built from — the borrow makes
    /// that checkable at the call site rather than assumed, because a proof
    /// about a different program is the coincidental correctness of ADR-0022.
    pub fn of(
        units: &[crate::check::Unit],
        cx: Context<'a>,
    ) -> Result<Checked<'a>, Vec<crate::diagnostics::Diagnostic>> {
        assert_eq!(
            units.len(),
            cx.hirs.len(),
            "the checked units and the lowering context describe different \
             programs, so the proof would be about neither"
        );
        let errors: Vec<crate::diagnostics::Diagnostic> = crate::check::check_units(units)
            .into_iter()
            .flat_map(|(_, ds)| ds)
            .filter(|d| d.severity == crate::diagnostics::Severity::Error)
            .collect();
        if errors.is_empty() {
            Ok(Checked { cx })
        } else {
            Err(errors)
        }
    }

    pub fn context(&self) -> &Context<'a> {
        &self.cx
    }
}

/// What the lowering needs from upstream, gathered once.
pub struct Context<'a> {
    pub hirs: &'a [&'a Hir],
    pub ws: &'a Workspace,
    pub sigs: &'a Signatures,
    /// The contracts, which already decided which capabilities each declaration
    /// requires. **Read, never re-derived.**
    pub contracts: &'a [ComponentContract],
}

/// Lower one declaration.
pub fn function(cx: &Context<'_>, unit: usize, decl: &Decl, span: Span) -> Lowering<Function> {
    let def = match decl_def(cx, unit, decl) {
        Some(d) => d,
        None => {
            return Lowering::Blocked {
                why: format!("`{}` has no resolved identity", decl.name),
                span,
            };
        }
    };

    let Some(body_id) = decl.body else {
        return Lowering::Unsupported {
            construct: "a declaration with no body",
            span,
            reason: "there is nothing to lower; a bodyless declaration is an \
                     interface, and E10-A compiles implementations"
                .to_string(),
        };
    };

    // The contract for this declaration, by the id `contracts()` built. Its
    // `required_capabilities` are what this function may call across — derived
    // once, upstream, from inference.
    let component_id = component_id(cx, unit, decl);
    let capabilities: Vec<CapabilityId> = cx
        .contracts
        .iter()
        .find(|c| c.component_id == component_id)
        .map(|c| {
            c.required_capabilities
                .iter()
                .cloned()
                .map(CapabilityId)
                .collect()
        })
        .unwrap_or_default();

    let mut f = Lower {
        cx,
        unit,
        next_value: 0,
        instrs: Vec::new(),
        locals: BTreeMap::new(),
        capabilities: &capabilities,
    };

    // Parameters first, so a body naming one finds it.
    let mut params = Vec::new();
    for p in &decl.params {
        let Some(declared) = &p.ty else {
            return Lowering::Unsupported {
                construct: "an unannotated parameter",
                span: p.span.clone(),
                reason: format!("`{}` has no declared type, and the ABI needs one", p.name),
            };
        };
        let ty = match f.ty_of(declared, &p.span) {
            Lowering::Lowered(t) => t,
            other => return other.map(|_| unreachable!()),
        };
        let v = f.fresh();
        f.locals.insert(p.name.clone(), v);
        params.push((v, ty));
    }

    let ret = match &decl.ret {
        Some(head) => {
            let written = crate::hir::DeclaredType::new(head.clone(), decl.ret_args.clone());
            match f.ty_of(&written, &span) {
                Lowering::Lowered(t) => t,
                other => return other.map(|_| unreachable!()),
            }
        }
        None => Type::Unit,
    };

    let body = cx.hirs[unit].body(body_id);
    let result = match f.expr(body, body.root) {
        Lowering::Lowered(v) => v,
        other => return other.map(|_| unreachable!()),
    };

    let instrs = std::mem::take(&mut f.instrs);
    Lowering::Lowered(Function {
        def,
        export: decl.name.clone(),
        params,
        ret,
        blocks: vec![Block {
            id: BlockId(0),
            instrs,
            terminator: Terminator::Return(result),
        }],
        capabilities,
    })
}

/// Lower every component declaration in a checked program.
///
/// Returns the functions that lowered and the refusals, both — a program where
/// half the declarations are outside E10-A's supported set is the ordinary
/// case, and reporting only the successes would make the backend look finished.
/// **Lower a program that checked.**
///
/// Takes `Checked`, not `Context`. See the type's docstring: the precondition
/// is the parameter, so it cannot be skipped.
pub fn program(checked: &Checked<'_>) -> (Program, Vec<Lowering<Function>>) {
    let cx = checked.context();
    let mut out = Program::default();
    let mut refusals = Vec::new();
    for (unit, hir) in cx.hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            if !matches!(decl.kind, DeclKind::Command | DeclKind::Query) {
                continue;
            }
            match function(cx, unit, decl, hir.decl_span(id)) {
                Lowering::Lowered(f) => out.functions.push(f),
                other => refusals.push(other),
            }
        }
    }
    out.types = type_defs(cx, &out);
    (out, refusals)
}

/// Every nominal type the lowered functions reach, resolved to its shape.
fn type_defs(cx: &Context<'_>, p: &Program) -> Vec<TypeDef> {
    let mut wanted: Vec<DefId> = Vec::new();
    for f in &p.functions {
        for (_, t) in &f.params {
            wanted.extend(t.nominals());
        }
        wanted.extend(f.ret.nominals());
        for b in &f.blocks {
            for i in &b.instrs {
                wanted.extend(i.ty().nominals());
            }
        }
    }
    wanted.sort();
    wanted.dedup();

    let mut out = Vec::new();
    for def in wanted {
        let Some(hir) = cx.hirs.get(def.unit) else {
            continue;
        };
        let Some(decl) = hir
            .all_decls()
            .find(|(id, _)| id.0 == def.decl)
            .map(|(_, d)| d)
        else {
            continue;
        };
        let shape = if let Some(of) = &decl.opaque_of {
            Shape::Alias(Box::new(primitive(of).unwrap_or(Type::Str)))
        } else if let Some(variants) = &decl.variants
            && variants.len() > 1
        {
            Shape::Variant {
                cases: variants.iter().map(|v| (v.name.clone(), None)).collect(),
            }
        } else {
            Shape::Record {
                fields: decl
                    .fields
                    .as_ref()
                    .map(|fs| {
                        fs.iter()
                            .filter_map(|f| {
                                f.ty.as_ref().map(|t| {
                                    (
                                        f.name.clone(),
                                        primitive(t.constructor_head_only()).unwrap_or(Type::Str),
                                    )
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        };
        out.push(TypeDef {
            def,
            name: decl.name.clone(),
            shape,
        });
    }
    out
}

/// A written type into its head and arguments, respecting nesting.
///
/// `Result<List<MenuItem>, StoreError>` gives `("Result", ["List<MenuItem>",
/// "StoreError"])`. Splitting on every comma would give three, and the middle
/// one would resolve against nothing.
fn split(written: &str) -> (&str, Vec<&str>) {
    let w = written.trim();
    let Some(open) = w.find('<') else {
        return (w, Vec::new());
    };
    let head = w[..open].trim();
    let inner = w[open + 1..].trim_end();
    let inner = inner.strip_suffix('>').unwrap_or(inner);
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in inner.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        args.push(last);
    }
    (head, args)
}

fn primitive(name: &str) -> Option<Type> {
    Some(match name {
        "Int" => Type::Int,
        "Float" => Type::Float,
        "Bool" => Type::Bool,
        "String" | "Str" => Type::Str,
        _ => return None,
    })
}

fn component_id(cx: &Context<'_>, unit: usize, decl: &Decl) -> String {
    let module = cx.hirs[unit]
        .all_decls()
        .find(|(_, d)| std::ptr::eq(*d, decl))
        .and_then(|(id, _)| cx.hirs[unit].module_of(id))
        .unwrap_or_default()
        .to_string();
    if module.is_empty() {
        decl.name.clone()
    } else {
        format!("{module}.{}", decl.name)
    }
}

fn decl_def(cx: &Context<'_>, unit: usize, decl: &Decl) -> Option<DefId> {
    cx.hirs[unit]
        .all_decls()
        .find(|(_, d)| std::ptr::eq(*d, decl))
        .map(|(id, _)| DefId { unit, decl: id.0 })
}

struct Lower<'a> {
    cx: &'a Context<'a>,
    unit: usize,
    next_value: u32,
    instrs: Vec<Instr>,
    locals: BTreeMap<String, ValueId>,
    capabilities: &'a [CapabilityId],
}

impl<'a> Lower<'a> {
    fn fresh(&mut self) -> ValueId {
        let v = ValueId(self.next_value);
        self.next_value += 1;
        v
    }

    /// A declared type, resolved.
    ///
    /// **Through the workspace**, so `Cart` in one module and `Cart` in another
    /// are two types here exactly as they are everywhere else. A backend that
    /// matched on the spelling would be the fourth place in this project to do
    /// that, and the first three each cost a milestone.
    fn ty_of(&self, declared: &crate::hir::DeclaredType, span: &Span) -> Lowering<Type> {
        self.ty_written_in(self.unit, &declared.written(), span)
    }

    /// A type as written, resolved.
    ///
    /// **One splitter, and it respects nesting.** `Result<List<MenuItem>,
    /// StoreError>` has two arguments, not three, and a version that split on
    /// every comma would resolve `List<MenuItem` against nothing.
    /// A type as written, resolved from a chosen unit.
    ///
    /// **A callee's return type is written in the CALLEE's module.**
    /// `current_session()` returns `Session<SessionId>`, and `Session` is the
    /// platform's — a caller that imports the function and not the type is
    /// correct under A-009, so resolving the return type in the caller's scope
    /// asks the wrong module and finds nothing. That blocked every command in
    /// the store.
    fn ty_written_in(&self, unit: usize, written: &str, span: &Span) -> Lowering<Type> {
        let (head, args) = split(written);
        if let Some(p) = primitive(head) {
            return Lowering::Lowered(p);
        }
        let arg = |i: usize| -> Lowering<Type> {
            match args.get(i) {
                Some(a) => self.ty_written_in(unit, a, span),
                None => Lowering::Unsupported {
                    construct: "a carrier with no argument",
                    span: span.clone(),
                    reason: format!("`{head}` needs a type argument and was written bare"),
                },
            }
        };
        match head {
            "Result" => match (arg(0), arg(1)) {
                (Lowering::Lowered(a), Lowering::Lowered(b)) => {
                    Lowering::Lowered(Type::Result(Box::new(a), Box::new(b)))
                }
                (Lowering::Lowered(_), other) | (other, _) => other.map(|_: Type| unreachable!()),
            },
            "Option" => arg(0).map(|a| Type::Option(Box::new(a))),
            "List" => arg(0).map(|a| Type::List(Box::new(a))),
            _ => match self.cx.ws.resolve_in(unit, Namespace::Type, head) {
                Resolution::Local(def) | Resolution::Imported { def, .. } => {
                    Lowering::Lowered(Type::Nominal(def))
                }
                _ => Lowering::Blocked {
                    why: format!("`{head}` names no type this program declares"),
                    span: span.clone(),
                },
            },
        }
    }

    fn expr(&mut self, body: &Body, e: ExprId) -> Lowering<ValueId> {
        let span = body.expr_span(e);
        match body.expr(e) {
            Expr::Literal(l) => {
                // The HIR keeps a literal as its SOURCE TEXT, so a number that
                // does not fit is a refusal here rather than a silent wrap —
                // the backend is where "as written" stops being enough.
                let (value, ty) = match l {
                    Literal::Int(n) => match n.parse::<i64>() {
                        Ok(v) => (Const::Int(v), Type::Int),
                        Err(_) => {
                            return Lowering::Unsupported {
                                construct: "an integer literal this backend cannot represent",
                                span,
                                reason: format!("`{n}` does not fit in the ABI's `s64`"),
                            };
                        }
                    },
                    Literal::Float(n) => match n.parse::<f64>() {
                        Ok(v) => (Const::Float(v), Type::Float),
                        Err(_) => {
                            return Lowering::Unsupported {
                                construct: "a float literal this backend cannot represent",
                                span,
                                reason: format!("`{n}` is not an `f64`"),
                            };
                        }
                    },
                    Literal::Str(s) => (Const::Str(s.clone()), Type::Str),
                    Literal::UnterminatedStr(_) => {
                        return Lowering::Blocked {
                            why: "an unterminated string; the program did not parse".to_string(),
                            span,
                        };
                    }
                };
                let result = self.fresh();
                self.instrs.push(Instr::Const {
                    result,
                    value,
                    ty: ty.clone(),
                });
                Lowering::Lowered(result)
            }
            // A placeholder body. `Unsupported`, not `Blocked`: the program is
            // perfectly well-formed and there is simply nothing to compile.
            Expr::Name(n) if n == "todo" => Lowering::Unsupported {
                construct: "a `todo` body",
                span,
                reason: "the declaration is a placeholder".to_string(),
            },
            Expr::Name(n) => match self.locals.get(n) {
                Some(v) => Lowering::Lowered(*v),
                None => Lowering::Blocked {
                    why: format!("`{n}` is not bound here"),
                    span,
                },
            },
            Expr::Block { stmts } => {
                let mut last = None;
                for s in stmts {
                    match self.expr(body, *s) {
                        Lowering::Lowered(v) => last = Some(v),
                        other => return other,
                    }
                }
                match last {
                    Some(v) => Lowering::Lowered(v),
                    None => Lowering::Unsupported {
                        construct: "an empty block",
                        span,
                        reason: "a function must produce a value".to_string(),
                    },
                }
            }
            Expr::Call { callee, args } => self.call(body, *callee, args, span),
            other => Lowering::Unsupported {
                construct: construct_name(other),
                span,
                reason: "E10-A lowers what `add_to_cart` needs: calls, names, \
                         literals and blocks"
                    .to_string(),
            },
        }
    }

    /// A call — the one place the host boundary is decided.
    ///
    /// **From the contract, not from the callee's name.** If the enclosing
    /// declaration requires a capability and this call is the thing that needs
    /// it, it is a `HostCall`. The alternative — matching `Carts.add` against a
    /// list — is exactly the by-spelling resolution E2C deleted.
    fn call(
        &mut self,
        body: &Body,
        callee: ExprId,
        args: &[crate::hir::Arg],
        span: Span,
    ) -> Lowering<ValueId> {
        let mut lowered = Vec::new();
        for a in args {
            match self.expr(body, a.value) {
                Lowering::Lowered(v) => lowered.push(v),
                other => return other,
            }
        }

        let path = crate::infer::path_of(body, callee);

        // **Through `Signatures`, keyed by the path a call site writes.**
        // `Carts.add` is a qualified path across a module boundary, and
        // `Workspace::resolve_in` answers about a single name — asking it for a
        // dotted path returned nothing and blocked every real command. `by_path`
        // is the answer E2C built for exactly this question, and `by_def` is
        // what a caller holding a `DefId` uses instead.
        // A BARE name resolves through the workspace, which knows this unit's
        // imports and the platform prelude. A DOTTED path is already qualified,
        // and `by_path` is keyed by exactly the form a call site writes.
        //
        // Neither is a spelling match: one goes through resolution and the
        // other through a map E2C builds from resolved declarations. Reading
        // the last segment would be the third thing, and it is the one
        // `last_segment.rs` forbids.
        let resolved = match self.cx.ws.resolve_in(self.unit, Namespace::Term, &path) {
            Resolution::Local(d) | Resolution::Imported { def: d, .. } => Some(d),
            _ => None,
        };
        let Some(sig) = resolved
            .and_then(|d| self.cx.sigs.by_def(d))
            .or_else(|| self.cx.sigs.by_path(&path))
            .cloned()
        else {
            return Lowering::Blocked {
                why: format!("`{path}` names no declaration this program contains"),
                span,
            };
        };

        // What the callee returns, from that signature. Not inferred here.
        let ty = match &sig.returns {
            Some(head) => {
                let written = crate::hir::DeclaredType::new(head.clone(), sig.returns_args.clone());
                // From the CALLEE's unit. See `ty_written_in`.
                let at = resolved.map(|d| d.unit).unwrap_or(self.unit);
                match self.ty_written_in(at, &written.written(), &span) {
                    Lowering::Lowered(t) => t,
                    other => return other.map(|_| unreachable!()),
                }
            }
            None => Type::Unit,
        };

        let def = resolved;

        // **Does this call cross the capability boundary?** The callee's own
        // declared row says what it performs; the enclosing contract says what
        // authority the enclosing declaration was granted. A call whose callee
        // performs a capability the contract requires is the host call.
        let performs: Vec<&CapabilityId> = self
            .capabilities
            .iter()
            .filter(|c| sig.effects.iter().any(|e| effect_is(e, &c.0)))
            .collect();

        let result = self.fresh();
        match performs.first() {
            Some(cap) => self.instrs.push(Instr::HostCall {
                result,
                capability: (*cap).clone(),
                args: lowered,
                ty,
            }),
            None => match def {
                Some(callee) => self.instrs.push(Instr::Call {
                    result,
                    callee,
                    args: lowered,
                    ty,
                }),
                // A call that needs no authority and whose callee has no
                // resolved identity here — a platform declaration reached
                // through the prelude. Refused rather than emitted as a call to
                // nothing, which is what an invented `DefId` would be.
                None => {
                    return Lowering::Unsupported {
                        construct: "a call to a declaration with no resolved identity",
                        span,
                        reason: format!("`{path}` has a signature but no `DefId` visible here"),
                    };
                }
            },
        }
        Lowering::Lowered(result)
    }
}

/// Does this written effect correspond to this capability?
///
/// Compared through the CONTRACT's own parse, so the two spellings cannot
/// drift: `Capability::parse` is what produced the capability in the first
/// place, and asking it again is asking the same function rather than writing a
/// second matcher.
fn effect_is(effect: &str, cap: &crate::contract::Capability) -> bool {
    crate::contract::Capability::parse(effect) == *cap
}

fn construct_name(e: &Expr) -> &'static str {
    match e {
        Expr::Field { .. } => "a field access",
        Expr::Lambda { .. } => "a lambda",
        Expr::Binary { .. } => "a binary operator",
        Expr::Cast { .. } => "a cast",
        Expr::Interpolated { .. } => "an interpolated string",
        _ => "this expression",
    }
}
