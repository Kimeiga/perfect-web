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

use std::collections::{BTreeMap, BTreeSet};

use super::ir::{
    Block, BlockId, BuiltinCase, CallableImport, CapabilityId, Const, Function, ImportId, Instr,
    Lowering, MatchArm, Program, Region, Shape, Terminator, Type, TypeDef, ValueId,
};
use crate::contract::ComponentContract;
use crate::hir::{Body, Decl, DeclKind, Expr, ExprId, Hir, Literal, Pattern, Span};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::resolved::{Builtin, Primitive, ResolvedType, TypeResolution};
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

/// **The callables this program imports, with the ABI to call each.**
///
/// One per `ImportId` the IR actually references, and the signature comes from
/// the CALLEE's declaration — not from a call site, and not from the capability.
/// Architect ruling, 2026-08-20:
///
/// > A capability authorizes an operation. It does not identify the operation.
///
/// `Carts.add` and `Carts.clear` share `database.write<Carts>` and have
/// different ABIs, which is what made the previous capability-keyed model
/// unencodable. Here they are two imports, both valid, each with its own
/// signature and both requiring the same authority.
///
/// `required_capabilities` is a SET and may be empty: an operation can need
/// several authorities, one authority can serve many operations, and an
/// operation can be placement-constrained while needing none.
fn host_imports(cx: &Context<'_>, p: &Program) -> Vec<CallableImport> {
    // Every import the lowered code actually calls.
    let mut wanted: BTreeSet<ImportId> = BTreeSet::new();
    for f in &p.functions {
        for b in &f.blocks {
            for i in &b.instrs {
                if let Instr::ImportCall { import, .. } = i {
                    wanted.insert(import.clone());
                }
            }
        }
    }

    let mut out = Vec::new();
    for (unit, hir) in cx.hirs.iter().enumerate() {
        for (id, decl) in hir.all_decls() {
            let def = crate::resolve::DefId { unit, decl: id.0 };
            // **The one canonical definition.** `contract.rs` calls the same
            // function on the same declaration, so the artifact a host reads
            // and the IR the encoder consumes cannot disagree about a
            // callable's ABI or its authority.
            let Some(signature) = cx.sigs.by_def(def) else {
                continue;
            };
            let Some(callable) = crate::backend::callable_of(
                decl,
                def,
                signature,
                |ty| match ty_resolved(cx.sigs, ty, &decl.name_span) {
                    Lowering::Lowered(t) => Some(t),
                    _ => None,
                },
                // The CONTRACT's answer, not a second derivation: whether an
                // effect needs authority is the ontology's, and the contracts
                // already asked. Read off the capability sets they carry.
                |effect| {
                    cx.contracts
                        .iter()
                        .flat_map(|c| c.imports.iter())
                        .flat_map(|i| i.capabilities.iter())
                        .find(|c| c.name() == effect)
                        .cloned()
                },
            ) else {
                continue;
            };
            if !wanted.contains(&callable.id) {
                continue;
            }
            out.push(callable);
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
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
        types: BTreeMap::new(),
    };

    // Parameters first, so a body naming one finds it.
    let Some(signature) = cx.sigs.by_def(def) else {
        return Lowering::Blocked {
            why: "missing resolved signature".into(),
            span,
        };
    };
    let mut params = Vec::new();
    for (index, p) in decl.params.iter().enumerate() {
        let Some(declared) = signature.params.get(index).and_then(Option::as_ref) else {
            return Lowering::Unsupported {
                construct: "an unannotated parameter",
                span: p.span.clone(),
                reason: format!("`{}` has no declared type, and the ABI needs one", p.name),
            };
        };
        let ty = match ty_resolution(cx.sigs, declared, &p.span) {
            Lowering::Lowered(t) => t,
            other => return other.map(|_| unreachable!()),
        };
        let v = f.fresh();
        f.locals.insert(p.name.clone(), v);
        f.types.insert(v, ty.clone());
        params.push((v, ty));
    }

    let ret = match &signature.returns {
        Some(resolution) => match ty_resolution(cx.sigs, resolution, &span) {
            Lowering::Lowered(t) => t,
            other => return other.map(|_| unreachable!()),
        },
        None => Type::Unit,
    };

    let body = cx.hirs[unit].body(body_id);
    let result = match f.expr(body, body.root, Some(&ret)) {
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
    let (program, refusals) = program_by_declaration(checked);
    (program, refusals.into_iter().map(|(_, r)| r).collect())
}

/// The construct a `todo` body is refused as: a declaration its author has
/// not written yet, which is not the backend declining something it could
/// be asked to compile.
pub const PLACEHOLDER: &str = "a `todo` body";

/// [`program`], with each refusal paired with the declaration it refuses, so
/// a caller can say why one component did not build without quoting every
/// other refusal in the program.
pub fn program_by_declaration(
    checked: &Checked<'_>,
) -> (Program, Vec<(DefId, Lowering<Function>)>) {
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
                other => refusals.push((DefId { unit, decl: id.0 }, other)),
            }
        }
    }
    out.types = type_defs(cx, &out);
    out.imports = host_imports(cx, &out);
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
    let hir = cx.hirs[unit];
    hir.all_decls()
        .find(|(_, d)| std::ptr::eq(*d, decl))
        .map(|(id, _)| crate::contract::component_id(hir, id))
        .unwrap_or_else(|| decl.name.clone())
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
    /// Every value's type, recorded where the value is made. A `match` asks
    /// what its scrutinee is, and a field access what record it reads.
    types: BTreeMap<ValueId, Type>,
}

impl<'a> Lower<'a> {
    fn fresh(&mut self) -> ValueId {
        let v = ValueId(self.next_value);
        self.next_value += 1;
        v
    }

    /// Append an instruction, recording its value's type.
    fn push(&mut self, instr: Instr) -> ValueId {
        let v = instr.result();
        self.types.insert(v, instr.ty().clone());
        self.instrs.push(instr);
        v
    }

    /// Is `name` the language's own, here: `Some`, `None`, `Ok`, `Err` where
    /// the program declares no term of that name? The typer's rule, so the
    /// backend cannot read a constructor the checker read as a declaration.
    fn builtin(&self, name: &str) -> Option<BuiltinCase> {
        let case = match name {
            "Some" => BuiltinCase::Some,
            "None" => BuiltinCase::None,
            "Ok" => BuiltinCase::Ok,
            "Err" => BuiltinCase::Err,
            _ => return None,
        };
        if self.locals.contains_key(name) {
            return None;
        }
        matches!(
            self.cx.ws.resolve_in(self.unit, Namespace::Term, name),
            Resolution::Unresolved
        )
        .then_some(case)
    }
}

fn ty_resolution(sigs: &Signatures, resolution: &TypeResolution, span: &Span) -> Lowering<Type> {
    match resolution.resolved() {
        Some(ty) => ty_resolved(sigs, ty, span),
        None => Lowering::Blocked {
            why: resolution.to_string(),
            span: span.clone(),
        },
    }
}

/// ABI projection: semantic identity arrives resolved. Privacy qualification
/// is erased only for the selected qualifier definition, not for its spelling.
fn ty_resolved(sigs: &Signatures, ty: &ResolvedType, span: &Span) -> Lowering<Type> {
    if let Some(p) = ty.as_primitive() {
        return Lowering::Lowered(match p {
            Primitive::Int => Type::Int,
            Primitive::Float => Type::Float,
            Primitive::Bool => Type::Bool,
            Primitive::Str => Type::Str,
            Primitive::Unit => Type::Unit,
        });
    }
    let arg = |i: usize| match ty.args().get(i) {
        Some(t) => ty_resolved(sigs, t, span),
        None => Lowering::Blocked {
            why: "resolved constructor is missing an argument".into(),
            span: span.clone(),
        },
    };
    if let Some(b) = ty.as_builtin() {
        return match b {
            Builtin::Result => match (arg(0), arg(1)) {
                (Lowering::Lowered(a), Lowering::Lowered(b)) => {
                    Lowering::Lowered(Type::Result(Box::new(a), Box::new(b)))
                }
                (Lowering::Lowered(_), other) | (other, _) => other,
            },
            Builtin::Option => arg(0).map(|a| Type::Option(Box::new(a))),
            Builtin::List => arg(0).map(|a| Type::List(Box::new(a))),
            Builtin::Function => Lowering::Unsupported {
                construct: "a function type",
                span: span.clone(),
                reason: format!("`{ty}` is a function; a function value has no layout here"),
            },
        };
    }
    if sigs.privacy_qualifier(ty).is_some() && ty.args().len() == 1 {
        return arg(0);
    }
    if let Some(def) = ty.def_id()
        && ty.args().is_empty()
    {
        return Lowering::Lowered(Type::Nominal(def));
    }
    Lowering::Unsupported {
        construct: "an unspecialized generic type",
        span: span.clone(),
        reason: format!("`{ty}` needs specialization before its layout can be encoded"),
    }
}

impl<'a> Lower<'a> {
    /// Lower one expression. `expected` is the type its context fixes, if any:
    /// the declaration's result, a parameter's type, the arms' common type.
    /// It is how `None` knows what it is `None` of.
    fn expr(&mut self, body: &Body, e: ExprId, expected: Option<&Type>) -> Lowering<ValueId> {
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
                    // The string's VALUE, not its token: until 2026-09-25 this
                    // held the token, quotes included, which nothing noticed
                    // because the encoder refuses constants that need memory.
                    Literal::Str(s) => match l.string_value() {
                        Some(v) => (Const::Str(v.to_string()), Type::Str),
                        None => {
                            return Lowering::Unsupported {
                                construct: "a string literal whose escapes the language does not define",
                                span,
                                reason: format!("`{s}` means something only under an escape rule"),
                            };
                        }
                    },
                    Literal::UnterminatedStr(_) => {
                        return Lowering::Blocked {
                            why: "an unterminated string; the program did not parse".to_string(),
                            span,
                        };
                    }
                };
                let result = self.fresh();
                Lowering::Lowered(self.push(Instr::Const {
                    result,
                    value,
                    ty: ty.clone(),
                }))
            }
            // A placeholder body. `Unsupported`, not `Blocked`: the program is
            // perfectly well-formed and there is simply nothing to compile.
            Expr::Name(n) if n == "todo" => Lowering::Unsupported {
                construct: PLACEHOLDER,
                span,
                reason: "the declaration is a placeholder".to_string(),
            },
            Expr::Name(n) if self.builtin(n) == Some(BuiltinCase::None) => {
                self.variant(BuiltinCase::None, None, expected, span)
            }
            Expr::Name(n) => match self.locals.get(n) {
                Some(v) => Lowering::Lowered(*v),
                None => Lowering::Blocked {
                    why: format!("`{n}` is not bound here"),
                    span,
                },
            },
            Expr::Block { stmts } => {
                let mut last = None;
                for (i, s) in stmts.iter().enumerate() {
                    let tail = i + 1 == stmts.len();
                    if let Expr::Let { pat, init, .. } = body.expr(*s) {
                        if tail {
                            return Lowering::Unsupported {
                                construct: "a block ending in a binding",
                                span,
                                reason: "the block's value would be the unit value".to_string(),
                            };
                        }
                        match self.bind(body, *pat, *init, span.clone()) {
                            Lowering::Lowered(()) => continue,
                            other => return other.map(|_| unreachable!()),
                        }
                    }
                    match self.expr(body, *s, if tail { expected } else { None }) {
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
            Expr::Call { callee, args } => match body.expr(*callee) {
                Expr::Name(n) if self.builtin(n).is_some_and(BuiltinCase::has_payload) => {
                    let case = self.builtin(n).expect("checked");
                    let [arg] = args.as_slice() else {
                        return Lowering::Blocked {
                            why: format!("`{n}` takes one value"),
                            span,
                        };
                    };
                    let payload_expected = match (case, expected) {
                        (BuiltinCase::Some, Some(Type::Option(t))) => Some((**t).clone()),
                        (BuiltinCase::Ok, Some(Type::Result(t, _))) => Some((**t).clone()),
                        (BuiltinCase::Err, Some(Type::Result(_, e))) => Some((**e).clone()),
                        _ => None,
                    };
                    let payload = match self.expr(body, arg.value, payload_expected.as_ref()) {
                        Lowering::Lowered(v) => v,
                        other => return other,
                    };
                    self.variant(case, Some(payload), expected, span)
                }
                _ => self.call(body, *callee, args, span),
            },
            Expr::Match { scrutinee, arms } => self.matched(body, *scrutinee, arms, expected, span),
            Expr::Field { base, name } => self.field(body, *base, name, span),
            other => Lowering::Unsupported {
                construct: construct_name(other),
                span,
                reason: "the backend lowers calls, names, literals, blocks, bindings, \
                         field reads, and matches over `Option` and `Result`"
                    .to_string(),
            },
        }
    }

    /// `let x = e` and `let _ = e`: the value bound for the rest of the block.
    fn bind(
        &mut self,
        body: &Body,
        pat: Option<crate::hir::PatternId>,
        init: Option<ExprId>,
        span: Span,
    ) -> Lowering<()> {
        let Some(init) = init else {
            return Lowering::Unsupported {
                construct: "a binding with no value",
                span,
                reason: "`let x` without `= e` has nothing to bind".to_string(),
            };
        };
        let v = match self.expr(body, init, None) {
            Lowering::Lowered(v) => v,
            other => return other.map(|_| unreachable!()),
        };
        match pat.map(|p| body.pat(p)) {
            Some(Pattern::Bind { name, .. }) => {
                self.locals.insert(name.clone(), v);
                Lowering::Lowered(())
            }
            Some(Pattern::Wild) | None => Lowering::Lowered(()),
            Some(_) => Lowering::Unsupported {
                construct: "a destructuring binding",
                span,
                reason: "`let` binds a name or `_` here".to_string(),
            },
        }
    }

    /// `Some(x)`, `None`, `Ok(x)`, `Err(e)`, typed by what the context
    /// expects, or by the payload when nothing does.
    fn variant(
        &mut self,
        case: BuiltinCase,
        payload: Option<ValueId>,
        expected: Option<&Type>,
        span: Span,
    ) -> Lowering<ValueId> {
        let payload_ty = payload.and_then(|v| self.types.get(&v).cloned());
        let ty = match (case, expected, payload_ty) {
            (BuiltinCase::Some | BuiltinCase::None, Some(t @ Type::Option(_)), _) => t.clone(),
            (BuiltinCase::Ok | BuiltinCase::Err, Some(t @ Type::Result(..)), _) => t.clone(),
            (BuiltinCase::Some, None, Some(p)) => Type::Option(Box::new(p)),
            _ => {
                return Lowering::Unsupported {
                    construct: "a variant whose type nothing here fixes",
                    span,
                    reason: format!(
                        "`{case:?}` needs the type its context expects, and the context \
                         names none this backend can read"
                    ),
                };
            }
        };
        let result = self.fresh();
        Lowering::Lowered(self.push(Instr::Variant {
            result,
            case,
            payload,
            ty,
        }))
    }

    /// A field of a record value, by the index its declaration gives it.
    fn field(&mut self, body: &Body, base: ExprId, name: &str, span: Span) -> Lowering<ValueId> {
        let of = match self.expr(body, base, None) {
            Lowering::Lowered(v) => v,
            other => return other,
        };
        let Some(Type::Nominal(def)) = self.types.get(&of).cloned() else {
            return Lowering::Unsupported {
                construct: "a field of something that is not a record",
                span,
                reason: format!("`.{name}` is read from a value of no declared record type"),
            };
        };
        let Some(fields) = self.cx.sigs.type_decl(def).and_then(|t| t.record.as_ref()) else {
            return Lowering::Unsupported {
                construct: "a field of something that is not a record",
                span,
                reason: format!("`.{name}` is read from a type with no record fields"),
            };
        };
        let Some((index, (_, declared))) = fields.iter().enumerate().find(|(_, (n, _))| n == name)
        else {
            return Lowering::Blocked {
                why: format!(
                    "the record has no field `{name}`; the checker should have refused it"
                ),
                span,
            };
        };
        let ty = match ty_resolution(self.cx.sigs, declared, &span) {
            Lowering::Lowered(t) => t,
            other => return other.map(|_| unreachable!()),
        };
        let result = self.fresh();
        Lowering::Lowered(self.push(Instr::Project {
            result,
            of,
            field: index as u32,
            ty,
        }))
    }

    /// **A match over `Option` or `Result`**, as a structured [`Instr::Match`].
    /// Every case must have exactly one arm: the encoder emits no fallthrough,
    /// so a missing case is refused here rather than compiled to a trap.
    fn matched(
        &mut self,
        body: &Body,
        scrutinee: ExprId,
        arms: &[crate::hir::MatchArm],
        expected: Option<&Type>,
        span: Span,
    ) -> Lowering<ValueId> {
        let scrutinee = match self.expr(body, scrutinee, None) {
            Lowering::Lowered(v) => v,
            other => return other,
        };
        let (cases, payloads): (Vec<BuiltinCase>, Vec<Option<Type>>) =
            match self.types.get(&scrutinee).cloned() {
                Some(Type::Option(t)) => (
                    vec![BuiltinCase::Some, BuiltinCase::None],
                    vec![Some(*t), None],
                ),
                Some(Type::Result(t, e)) => (
                    vec![BuiltinCase::Ok, BuiltinCase::Err],
                    vec![Some(*t), Some(*e)],
                ),
                other => {
                    return Lowering::Unsupported {
                        construct: "a match over something other than `Option` or `Result`",
                        span,
                        reason: match other {
                            Some(t) => format!("the scrutinee's type is {t:?}"),
                            None => "the scrutinee's type is not known here".to_string(),
                        },
                    };
                }
            };

        let mut lowered: Vec<MatchArm> = Vec::new();
        let mut ty: Option<Type> = expected.cloned();
        for arm in arms {
            let (case, bind) = match body.pat(arm.pat) {
                Pattern::Ctor { path, args } => match (self.builtin(path), args.as_slice()) {
                    (Some(case), [inner]) if case.has_payload() => match body.pat(*inner) {
                        Pattern::Bind { name, .. } => (case, Some(name.clone())),
                        Pattern::Wild => (case, None),
                        _ => {
                            return Lowering::Unsupported {
                                construct: "a nested pattern",
                                span,
                                reason: format!("`{path}(..)` binds a name or `_` here"),
                            };
                        }
                    },
                    (Some(BuiltinCase::None), []) => (BuiltinCase::None, None),
                    _ => {
                        return Lowering::Unsupported {
                            construct: "a pattern this backend does not lower",
                            span,
                            reason: format!("`{path}(..)` is not a case of the scrutinee"),
                        };
                    }
                },
                // `None` parses as a binding of that name; the language's own
                // `None` where nothing else has the name.
                Pattern::Bind { name, .. } if self.builtin(name) == Some(BuiltinCase::None) => {
                    (BuiltinCase::None, None)
                }
                _ => {
                    return Lowering::Unsupported {
                        construct: "a pattern this backend does not lower",
                        span,
                        reason: "an arm names a case: `Some(x)`, `None`, `Ok(x)`, `Err(e)`"
                            .to_string(),
                    };
                }
            };
            let Some(at) = cases.iter().position(|c| *c == case) else {
                return Lowering::Unsupported {
                    construct: "a pattern this backend does not lower",
                    span,
                    reason: format!("`{case:?}` is not a case of the scrutinee"),
                };
            };
            if lowered.iter().any(|a| a.case == case) {
                return Lowering::Unsupported {
                    construct: "a case matched twice",
                    span,
                    reason: format!("`{case:?}` has two arms; the second can never run"),
                };
            }

            // The arm's body, in a region of its own.
            let outer = std::mem::take(&mut self.instrs);
            let binding = match (&payloads[at], &bind) {
                (Some(t), _) => {
                    let v = self.fresh();
                    self.types.insert(v, t.clone());
                    Some(v)
                }
                (None, _) => None,
            };
            let shadowed = bind
                .as_ref()
                .map(|name| (name.clone(), self.locals.get(name).copied()));
            if let (Some(name), Some(v)) = (&bind, binding) {
                self.locals.insert(name.clone(), v);
            }
            let value = self.expr(body, arm.body, ty.as_ref());
            if let Some((name, before)) = shadowed {
                match before {
                    Some(v) => self.locals.insert(name, v),
                    None => self.locals.remove(&name),
                };
            }
            let instrs = std::mem::replace(&mut self.instrs, outer);
            let value = match value {
                Lowering::Lowered(v) => v,
                other => return other,
            };
            let value_ty = self.types.get(&value).cloned();
            match (&ty, value_ty) {
                (None, Some(t)) => ty = Some(t),
                (Some(t), Some(v)) if *t != v => {
                    return Lowering::Blocked {
                        why: format!("the arms produce {t:?} and {v:?}"),
                        span,
                    };
                }
                _ => {}
            }
            lowered.push(MatchArm {
                case,
                binding,
                body: Region { instrs, value },
            });
        }
        if lowered.len() != cases.len() {
            return Lowering::Unsupported {
                construct: "a match that does not cover every case",
                span,
                reason: format!(
                    "{} of {} cases have arms, and a missing case would have no code",
                    lowered.len(),
                    cases.len()
                ),
            };
        }
        let Some(ty) = ty else {
            return Lowering::Blocked {
                why: "a match whose arms produce no typed value".to_string(),
                span,
            };
        };
        let result = self.fresh();
        Lowering::Lowered(self.push(Instr::Match {
            result,
            scrutinee,
            arms: lowered,
            ty,
        }))
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
        //
        // A DOTTED path goes through `resolve_path`, which checks the module is
        // visible and the member exists. It was not asked before 2026-08-20,
        // because the old classification keyed on the effect row and did not
        // need the callee's identity — so `Menus.for_store` had a signature and
        // no `DefId`, and nothing noticed until the identity became the thing
        // that decides.
        let resolution = match path.contains('.') {
            true => self.cx.ws.resolve_path(self.unit, &path),
            false => self.cx.ws.resolve_in(self.unit, Namespace::Term, &path),
        };
        let resolved = match resolution {
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

        // The arguments, each expecting its parameter's declared type.
        let mut lowered = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let expected = match sig.params.get(i).and_then(Option::as_ref) {
                Some(p) => match ty_resolution(self.cx.sigs, p, &span) {
                    Lowering::Lowered(t) => Some(t),
                    _ => None,
                },
                None => None,
            };
            match self.expr(body, a.value, expected.as_ref()) {
                Lowering::Lowered(v) => lowered.push(v),
                other => return other,
            }
        }

        // What the callee returns, from that signature. Not inferred here.
        let ty = match &sig.returns {
            Some(resolution) => match ty_resolution(self.cx.sigs, resolution, &span) {
                Lowering::Lowered(ty) => ty,
                other => return other.map(|_| unreachable!()),
            },
            None => Type::Unit,
        };

        let def = resolved;

        // **Where does this callee's implementation come from?**
        //
        // Architect ruling, 2026-08-20, deleting the rule that used to be here:
        //
        // > "The callee's declared effect row contains a capability required by
        // > this component, therefore it is a HostCall." Delete that
        // > classification. An effectful function can perfectly well be
        // > ordinary compiled Pleris.
        //
        // Three independent facts, and the old rule collapsed the first into
        // the third:
        //
        // ```text
        // implementation   local Pleris / another component / the host
        // effects          database.write<Carts>, session.read, ..
        // authority        the capabilities those effects require
        // ```
        //
        // So the question is the DECLARATION's, not the effect row's: a `fn`
        // with a `host "pw:host/carts#add"` policy is supplied externally, and
        // one without is compiled here however effectful it is.
        let binding = resolved
            .and_then(|d| crate::resolve::declaration(self.cx.hirs, d))
            .and_then(crate::backend::host_binding);

        let result = self.fresh();
        match binding {
            Some(import) => {
                self.push(Instr::ImportCall {
                    result,
                    import,
                    args: lowered,
                    ty,
                });
            }
            None => match def {
                Some(callee) => {
                    self.push(Instr::Call {
                        result,
                        callee,
                        args: lowered,
                        ty,
                    });
                }
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

fn construct_name(e: &Expr) -> &'static str {
    match e {
        Expr::Field { .. } => "a field access",
        Expr::Lambda { .. } => "a lambda",
        Expr::Binary { .. } => "a binary operator",
        Expr::Cast { .. } => "a cast",
        Expr::Try { .. } => "a `?` propagation",
        Expr::Interpolated { .. } => "an interpolated string",
        _ => "this expression",
    }
}
