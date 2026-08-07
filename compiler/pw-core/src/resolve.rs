//! E2B — the workspace module graph and name resolution.
//!
//! Architect ruling, 2026-08-06, narrowing assumption A-009:
//!
//! > `pw check` constructs one workspace module graph. It should **not** mean
//! > every declaration in every supplied file is ambiently visible everywhere.
//! > There should be no ambient union of user declarations.
//!
//! and the rule that decides what an external symbol may be:
//!
//! > **External implementation is allowed. Missing declaration is not.**
//!
//! # Why this comes before effect inference
//!
//! Every analysis that follows — privacy, placement, effects — needs to know
//! *what a name refers to*. Before this module they each decided independently,
//! by matching text. Two checkers matching `secrets.payments` by string can
//! disagree about which declaration it is, and nothing would notice.
//!
//! The invariant E2B establishes:
//!
//! > After E2B, semantic analyses consume [`DefId`], not textual names.

use std::collections::{BTreeMap, BTreeSet};

use crate::hir::{Decl, DeclKind, Hir, Span};

/// Which unit a declaration lives in. Index into the checked set.
pub type UnitId = usize;

/// A resolved declaration identity.
///
/// Two declarations with the same name in different modules are different
/// `DefId`s, which is the whole point: a checker holding one cannot silently
/// act on the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefId {
    pub unit: UnitId,
    pub decl: u32,
}

/// Which namespace a name lives in.
///
/// `A-003` declares `query Store` and imports `type Store`; `A-008` declares
/// both `query Recommendations` and `view Recommendations`. Neither is a
/// mistake — a type, a data operation and a rendered view are different kinds
/// of thing, and the corpus names them after what they are about. A single flat
/// namespace reported both as duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Namespace {
    /// `type`, `opaque type`, records, unions.
    Type,
    /// `fn`, `let`, and the data operations: query, command, subscription,
    /// resource, task. These are *called*.
    Term,
    /// `view`, `component`, `page`, `materialize`. These are *rendered*, never
    /// called. A materialization belongs here rather than with the data
    /// operations because nothing in a program invokes one: the materializer
    /// decides when it runs, from the graph.
    Ui,
    /// `effect`. Its own namespace: an effect name appears only in an effect
    /// row, never in an expression, and `database.read` must not collide with
    /// a `fn read` or a `type database`.
    Effect,
    /// `event`. Its own namespace, because an event name appears only in
    /// `emits` and `invalidates_on` — never in an expression — and a
    /// materialization that names an event must not silently resolve to a
    /// query that happens to share the spelling.
    Event,
}

impl Namespace {
    /// Every namespace, in lookup order.
    ///
    /// A constant rather than a literal at each search site. There were two
    /// such sites, both hand-written, and adding `Event` for E6 fixed one and
    /// left the other — so an event was importable and unresolvable, or the
    /// reverse, depending on which path asked.
    pub const ALL: [Namespace; 5] = [
        Namespace::Type,
        Namespace::Term,
        Namespace::Ui,
        Namespace::Effect,
        Namespace::Event,
    ];

    pub fn of(kind: DeclKind) -> Option<Namespace> {
        Some(match kind {
            DeclKind::Type | DeclKind::Opaque => Namespace::Type,
            DeclKind::Fn
            | DeclKind::Let
            | DeclKind::Query
            | DeclKind::Command
            | DeclKind::Subscription
            | DeclKind::Resource
            | DeclKind::Task => Namespace::Term,
            DeclKind::View | DeclKind::Component | DeclKind::Page | DeclKind::Materialize => {
                Namespace::Ui
            }
            DeclKind::Event => Namespace::Event,
            DeclKind::Effect => Namespace::Effect,
            DeclKind::Import | DeclKind::Other => return None,
        })
    }
}

/// One module in the graph.
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub unit: UnitId,
    /// Declarations this module defines, keyed by namespace and name.
    pub defines: BTreeMap<(Namespace, String), DefId>,
    /// Modules it imports, and the names taken from each.
    pub imports: Vec<Import>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    /// Empty means the whole module.
    pub names: Vec<String>,
    pub span: Span,
}

/// What a name resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A declaration in this module.
    Local(DefId),
    /// A declaration reached through an explicit import.
    Imported { def: DefId, from: String },
    /// The name is not visible here.
    Unresolved,
    /// Two imports bring in the same name.
    Ambiguous(Vec<DefId>),
}

/// A resolution failure, reported with both spans charter §16.3 asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    pub kind: ResolveErrorKind,
    /// Where the failing use or import is.
    pub span: Span,
    pub unit: UnitId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveErrorKind {
    /// `import nowhere.{ X }` — no module by that name.
    UnresolvedModule { module: String },
    /// `import domain.{ Missing }` — the module exists, the name does not.
    UnresolvedName { module: String, name: String },
    /// The same name imported from two modules.
    AmbiguousName { name: String, from: Vec<String> },
    /// Two declarations of one name in one module.
    DuplicateDeclaration { name: String },
    /// Reaching a declaration the other module does not make public.
    PrivateAccess { module: String, name: String },
    /// `a` imports `b` imports `a`.
    ImportCycle { modules: Vec<String> },
}

impl ResolveErrorKind {
    pub fn message(&self) -> String {
        match self {
            ResolveErrorKind::UnresolvedModule { module } => {
                format!("no module named `{module}` in this workspace")
            }
            ResolveErrorKind::UnresolvedName { module, name } => {
                format!("`{module}` does not declare `{name}`")
            }
            ResolveErrorKind::AmbiguousName { name, from } => {
                format!("`{name}` is imported from {}", from.join(" and "))
            }
            ResolveErrorKind::DuplicateDeclaration { name } => {
                format!("`{name}` is declared twice in this module")
            }
            ResolveErrorKind::PrivateAccess { module, name } => {
                format!("`{module}.{name}` is not public")
            }
            ResolveErrorKind::ImportCycle { modules } => {
                format!("import cycle: {}", modules.join(" -> "))
            }
        }
    }
}

/// The whole checked set, as one module graph.
#[derive(Debug, Default)]
pub struct Workspace {
    pub modules: Vec<Module>,
    by_name: BTreeMap<String, usize>,
    pub errors: Vec<ResolveError>,
}

impl Workspace {
    /// Build the graph from every unit's HIR.
    pub fn build(units: &[&Hir]) -> Workspace {
        let mut ws = Workspace::default();

        // Pass 1: module identities and what each defines. Two passes, so an
        // import may name a module declared in a file that comes later — file
        // order must not decide whether a program resolves.
        for (unit, hir) in units.iter().enumerate() {
            let name = hir
                .modules
                .iter()
                .next()
                .map(|(_, m, _)| m.name.clone())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let mut defines: BTreeMap<(Namespace, String), DefId> = BTreeMap::new();
            let mut imports = Vec::new();
            for (id, decl) in hir.all_decls() {
                if decl.kind == DeclKind::Import {
                    imports.push(Import {
                        module: decl.name.clone(),
                        names: decl.imports.clone(),
                        span: hir.decl_span(id),
                    });
                    continue;
                }
                let Some(ns) = Namespace::of(decl.kind) else {
                    continue;
                };
                if decl.name.is_empty() {
                    continue;
                }
                let def = DefId { unit, decl: id.0 };
                if defines.insert((ns, decl.name.clone()), def).is_some() {
                    ws.errors.push(ResolveError {
                        kind: ResolveErrorKind::DuplicateDeclaration {
                            name: decl.name.clone(),
                        },
                        span: hir.decl_span(id),
                        unit,
                    });
                }
            }

            // A workspace with two modules of one name has no single graph.
            if ws.by_name.contains_key(&name) {
                ws.errors.push(ResolveError {
                    kind: ResolveErrorKind::DuplicateDeclaration { name: name.clone() },
                    span: 0..0,
                    unit,
                });
            }
            ws.by_name.insert(name.clone(), ws.modules.len());
            ws.modules.push(Module {
                name,
                unit,
                defines,
                imports,
            });
        }

        // Pass 2: check every import against the graph.
        ws.check_imports(units);
        ws.detect_cycles();
        ws
    }

    fn check_imports(&mut self, units: &[&Hir]) {
        let mut errors = Vec::new();
        for m in &self.modules {
            // One name imported from two modules. Reported at the import,
            // because that is where the choice was made — resolving it
            // silently to the first would make file order decide meaning.
            let mut seen: BTreeMap<&str, Vec<String>> = BTreeMap::new();
            for imp in &m.imports {
                for name in &imp.names {
                    seen.entry(name).or_default().push(imp.module.clone());
                }
            }
            for (name, from) in seen.into_iter().filter(|(_, f)| f.len() > 1) {
                errors.push(ResolveError {
                    kind: ResolveErrorKind::AmbiguousName {
                        name: name.to_string(),
                        from,
                    },
                    span: m
                        .imports
                        .iter()
                        .find(|i| i.names.iter().any(|n| n == name))
                        .map(|i| i.span.clone())
                        .unwrap_or(0..0),
                    unit: m.unit,
                });
            }

            for imp in &m.imports {
                let Some(&target) = self.by_name.get(&imp.module) else {
                    errors.push(ResolveError {
                        kind: ResolveErrorKind::UnresolvedModule {
                            module: imp.module.clone(),
                        },
                        span: imp.span.clone(),
                        unit: m.unit,
                    });
                    continue;
                };
                let target = &self.modules[target];
                for name in &imp.names {
                    let Some(def) = target.lookup_any(name) else {
                        errors.push(ResolveError {
                            kind: ResolveErrorKind::UnresolvedName {
                                module: imp.module.clone(),
                                name: name.clone(),
                            },
                            span: imp.span.clone(),
                            unit: m.unit,
                        });
                        continue;
                    };
                    // Visibility. A type is importable unless the declaring
                    // module marked it `private`; `session` is a privacy label
                    // on values, not a module-visibility keyword.
                    let decl = declaration(units, def);
                    if decl.is_some_and(|d| d.visibility.as_deref() == Some("private")) {
                        errors.push(ResolveError {
                            kind: ResolveErrorKind::PrivateAccess {
                                module: imp.module.clone(),
                                name: name.clone(),
                            },
                            span: imp.span.clone(),
                            unit: m.unit,
                        });
                    }
                }
            }
        }
        self.errors.append(&mut errors);
    }

    fn detect_cycles(&mut self) {
        // Depth-first, reporting the first cycle found per start node. An
        // import cycle is not always fatal, but it must be visible: it decides
        // initialisation order and it hides which module owns a definition.
        let mut reported: BTreeSet<Vec<String>> = BTreeSet::new();
        let mut errors = Vec::new();

        for start in 0..self.modules.len() {
            let mut path = Vec::new();
            let mut seen = BTreeSet::new();
            if let Some(cycle) = self.walk(start, &mut path, &mut seen) {
                let mut key = cycle.clone();
                key.sort();
                if reported.insert(key) {
                    errors.push(ResolveError {
                        kind: ResolveErrorKind::ImportCycle { modules: cycle },
                        span: 0..0,
                        unit: self.modules[start].unit,
                    });
                }
            }
        }
        self.errors.append(&mut errors);
    }

    fn walk(
        &self,
        at: usize,
        path: &mut Vec<String>,
        seen: &mut BTreeSet<usize>,
    ) -> Option<Vec<String>> {
        if !seen.insert(at) {
            let mut cycle = path.clone();
            cycle.push(self.modules[at].name.clone());
            return Some(cycle);
        }
        path.push(self.modules[at].name.clone());
        for imp in &self.modules[at].imports {
            let Some(&next) = self.by_name.get(&imp.module) else {
                continue;
            };
            if let Some(c) = self.walk(next, path, seen) {
                return Some(c);
            }
        }
        path.pop();
        seen.remove(&at);
        None
    }

    pub fn module_of(&self, unit: UnitId) -> Option<&Module> {
        self.modules.iter().find(|m| m.unit == unit)
    }

    /// Resolve in one namespace. The precise form; [`Workspace::resolve`]
    /// searches all three for callers that do not yet know which they want.
    pub fn resolve_in(&self, unit: UnitId, ns: Namespace, name: &str) -> Resolution {
        let Some(m) = self.module_of(unit) else {
            return Resolution::Unresolved;
        };
        if let Some(def) = m.defines.get(&(ns, name.to_string())) {
            return Resolution::Local(*def);
        }
        let mut hits: Vec<(String, DefId)> = Vec::new();
        for imp in &m.imports {
            let Some(&t) = self.by_name.get(&imp.module) else {
                continue;
            };
            if !(imp.names.is_empty() || imp.names.iter().any(|n| n == name)) {
                continue;
            }
            if let Some(def) = self.modules[t].defines.get(&(ns, name.to_string())) {
                hits.push((imp.module.clone(), *def));
            }
        }
        match hits.len() {
            0 => Resolution::Unresolved,
            1 => Resolution::Imported {
                def: hits[0].1,
                from: hits[0].0.clone(),
            },
            _ => Resolution::Ambiguous(hits.into_iter().map(|(_, d)| d).collect()),
        }
    }

    /// Resolve `name` as seen from `unit`.
    ///
    /// Local declarations first, then explicit imports. **Nothing else.** A
    /// declaration in an unrelated module is not visible, however convenient
    /// that would be — that convenience was assumption A-009's ambient union,
    /// and it let a file match on a type it never imported.
    pub fn resolve(&self, unit: UnitId, name: &str) -> Resolution {
        for ns in Namespace::ALL {
            match self.resolve_in(unit, ns, name) {
                Resolution::Unresolved => continue,
                other => return other,
            }
        }
        Resolution::Unresolved
    }
}

impl Workspace {
    /// Resolve a qualified path: `Stores.get`, `secrets.payments`.
    ///
    /// The module part must be visible — declared here or imported — and the
    /// member must exist in it. Both halves matter: `Stores.get` where nothing
    /// imports `Stores` is exactly the ambient lookup E2B removed, and
    /// `Stores.missing` is a name the module does not have.
    pub fn resolve_path(&self, unit: UnitId, path: &str) -> Resolution {
        let Some((head, member)) = path.rsplit_once('.') else {
            return self.resolve(unit, path);
        };
        let Some(m) = self.module_of(unit) else {
            return Resolution::Unresolved;
        };
        // The head names a module this file can see.
        let visible = m.name == head || m.imports.iter().any(|i| i.module == head);
        if !visible {
            return Resolution::Unresolved;
        }
        let Some(&t) = self.by_name.get(head) else {
            return Resolution::Unresolved;
        };
        match self.modules[t].lookup_any(member) {
            Some(def) => Resolution::Imported {
                def,
                from: head.to_string(),
            },
            None => Resolution::Unresolved,
        }
    }

    /// Is `name` a module this unit can see?
    pub fn sees_module(&self, unit: UnitId, name: &str) -> bool {
        self.module_of(unit)
            .is_some_and(|m| m.name == name || m.imports.iter().any(|i| i.module == name))
            && self.by_name.contains_key(name)
    }
}

impl Module {
    /// The first namespace that defines `name`. Used by imports, which name a
    /// symbol without saying which namespace they mean.
    pub fn lookup_any(&self, name: &str) -> Option<DefId> {
        Namespace::ALL
            .into_iter()
            .find_map(|ns| self.defines.get(&(ns, name.to_string())).copied())
    }
}

/// Names bound inside a body: parameters, `let`s, lambda parameters, match
/// bindings, and loop variables.
///
/// Collected per body rather than per expression because a `let` is visible to
/// everything after it in the block, and getting that wrong would make the use
/// checker report the program's own locals as undeclared — which is how a
/// checker like this becomes noise and gets turned off.
pub fn local_bindings(body: &crate::hir::Body) -> BTreeSet<String> {
    use crate::hir::{Expr, Node, Pattern};
    let mut out = BTreeSet::new();

    fn pattern_names(
        body: &crate::hir::Body,
        p: crate::hir::PatternId,
        out: &mut BTreeSet<String>,
    ) {
        match body.pat(p) {
            Pattern::Bind { name, .. } => {
                out.insert(name.clone());
            }
            Pattern::Ctor { args, .. } => {
                for a in args {
                    pattern_names(body, *a, out);
                }
            }
            Pattern::Or(ps) => {
                for a in ps {
                    pattern_names(body, *a, out);
                }
            }
            _ => {}
        }
    }

    for id in body.walk() {
        match body.expr(id) {
            Expr::Let { pat: Some(p), .. } => pattern_names(body, *p, &mut out),
            Expr::Lambda { params, .. } => {
                for p in params {
                    pattern_names(body, *p, &mut out);
                }
            }
            Expr::Match { arms, .. } => {
                for a in arms {
                    pattern_names(body, a.pat, &mut out);
                }
            }
            // A `{#each xs as x (k)}` binds `x` for its children.
            Expr::Template { roots, .. } => {
                for n in body.walk_markup(roots) {
                    let Node::Block { directive, .. } = body.node(n) else {
                        continue;
                    };
                    let Some(rest) = directive.split(" as ").nth(1) else {
                        continue;
                    };
                    let name = rest
                        .trim()
                        .split(['(', '}', ' '])
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !name.is_empty() {
                        out.insert(name.to_string());
                    }
                }
            }
            // A body-level statement introduces its modifier words as names:
            // `observe resize` binds nothing, but `use key: T = ..` binds `key`.
            Expr::Keyword { modifiers, .. } => {
                if let Some(first) = modifiers.first() {
                    out.insert(first.clone());
                }
            }
            _ => {}
        }
    }
    out
}

/// The declaration a `DefId` points at.
pub fn declaration<'a>(units: &'a [&Hir], def: DefId) -> Option<&'a Decl> {
    units
        .get(def.unit)?
        .all_decls()
        .find(|(id, _)| id.0 == def.decl)
        .map(|(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use pw_syntax::parse_tree;

    fn hirs(sources: &[&str]) -> Vec<Hir> {
        sources
            .iter()
            .map(|s| lower_file(s, &parse_tree(s).green))
            .collect()
    }

    fn workspace(sources: &[&str]) -> (Vec<Hir>, Workspace) {
        let owned = hirs(sources);
        let refs: Vec<&Hir> = owned.iter().collect();
        let ws = Workspace::build(&refs);
        (owned, ws)
    }

    const DOMAIN: &str =
        "module domain\n\ntype Store = Store { name: String }\ntype Hidden = Hidden { x: Int }\n";

    #[test]
    fn an_imported_name_resolves_and_an_unimported_one_does_not() {
        // The ruling in one test. Both files are in the checked set; only the
        // import makes `Store` visible.
        let with = "module page\n\nimport domain.{ Store }\n\nfn f(s: Store) -> Int !{} { 1 }\n";
        let (_, ws) = workspace(&[DOMAIN, with]);
        assert!(ws.errors.is_empty(), "{:?}", ws.errors);
        assert!(matches!(
            ws.resolve(1, "Store"),
            Resolution::Imported { from, .. } if from == "domain"
        ));

        let without = "module page\n\nfn f(s: Store) -> Int !{} { 1 }\n";
        let (_, ws) = workspace(&[DOMAIN, without]);
        assert_eq!(
            ws.resolve(1, "Store"),
            Resolution::Unresolved,
            "a declaration in another module must NOT be ambiently visible"
        );
    }

    #[test]
    fn a_local_declaration_shadows_nothing_and_resolves_first() {
        let local = "module page\n\nimport domain.{ Store }\n\ntype Store = Store { local: Int }\n";
        let (_, ws) = workspace(&[DOMAIN, local]);
        assert!(
            matches!(ws.resolve(1, "Store"), Resolution::Local(d) if d.unit == 1),
            "a module's own declaration wins"
        );
    }

    #[test]
    fn an_unresolved_module_and_an_unresolved_name_are_different_errors() {
        let (_, ws) = workspace(&[DOMAIN, "module page\n\nimport nowhere.{ Store }\n"]);
        assert!(
            matches!(
                ws.errors.as_slice(),
                [ResolveError {
                    kind: ResolveErrorKind::UnresolvedModule { module },
                    ..
                }] if module == "nowhere"
            ),
            "{:?}",
            ws.errors
        );

        let (_, ws) = workspace(&[DOMAIN, "module page\n\nimport domain.{ Missing }\n"]);
        assert!(
            matches!(
                ws.errors.as_slice(),
                [ResolveError {
                    kind: ResolveErrorKind::UnresolvedName { name, .. },
                    ..
                }] if name == "Missing"
            ),
            "{:?}",
            ws.errors
        );
    }

    #[test]
    fn one_name_from_two_modules_is_ambiguous() {
        let a = "module a\n\ntype Thing = Thing { x: Int }\n";
        let b = "module b\n\ntype Thing = Thing { y: Int }\n";
        let use_both = "module page\n\nimport a.{ Thing }\nimport b.{ Thing }\n";
        let (_, ws) = workspace(&[a, b, use_both]);
        assert!(
            matches!(ws.resolve(2, "Thing"), Resolution::Ambiguous(v) if v.len() == 2),
            "{:?}",
            ws.resolve(2, "Thing")
        );
    }

    #[test]
    fn a_duplicate_declaration_is_reported_once() {
        let dup = "module m\n\ntype Thing = Thing { x: Int }\ntype Thing = Thing { y: Int }\n";
        let (_, ws) = workspace(&[dup]);
        assert_eq!(
            ws.errors
                .iter()
                .filter(|e| matches!(e.kind, ResolveErrorKind::DuplicateDeclaration { .. }))
                .count(),
            1,
            "{:?}",
            ws.errors
        );
    }

    #[test]
    fn an_import_cycle_is_visible() {
        let a = "module a\n\nimport b.{ T }\n\ntype S = S { x: Int }\n";
        let b = "module b\n\nimport a.{ S }\n\ntype T = T { y: Int }\n";
        let (_, ws) = workspace(&[a, b]);
        assert!(
            ws.errors
                .iter()
                .any(|e| matches!(e.kind, ResolveErrorKind::ImportCycle { .. })),
            "{:?}",
            ws.errors
        );

        // Control: an acyclic graph reports none, or every program would.
        let (_, ws) = workspace(&[DOMAIN, "module page\n\nimport domain.{ Store }\n"]);
        assert!(
            !ws.errors
                .iter()
                .any(|e| matches!(e.kind, ResolveErrorKind::ImportCycle { .. })),
            "{:?}",
            ws.errors
        );
    }

    #[test]
    fn file_order_does_not_decide_whether_a_program_resolves() {
        // Two passes exist for this. With one pass, importing a module declared
        // in a later file would fail — and the corpus is read in directory
        // order, which nobody controls.
        let page = "module page\n\nimport domain.{ Store }\n\nfn f(s: Store) -> Int !{} { 1 }\n";
        let (_, forward) = workspace(&[DOMAIN, page]);
        let (_, backward) = workspace(&[page, DOMAIN]);
        assert!(forward.errors.is_empty(), "{:?}", forward.errors);
        assert!(backward.errors.is_empty(), "{:?}", backward.errors);
        assert!(matches!(
            backward.resolve(0, "Store"),
            Resolution::Imported { .. }
        ));
    }

    #[test]
    fn a_qualified_path_needs_both_the_module_and_the_member() {
        let lib = "module Stores\n\nfn get(id: Int) -> Int !{ database.read } { 0 }\n";
        let user = "module page\n\nimport Stores\n\nfn f() -> Int !{} { Stores.get(1) }\n";
        let (_, ws) = workspace(&[lib, user]);
        assert!(
            matches!(
                ws.resolve_path(1, "Stores.get"),
                Resolution::Imported { .. }
            ),
            "{:?}",
            ws.resolve_path(1, "Stores.get")
        );

        // A member the module does not have.
        assert_eq!(ws.resolve_path(1, "Stores.missing"), Resolution::Unresolved);

        // ...and the ambient case E2B exists to remove: the module is in the
        // checked set, this file does not import it.
        let no_import = "module page\n\nfn f() -> Int !{} { Stores.get(1) }\n";
        let (_, ws) = workspace(&[lib, no_import]);
        assert_eq!(
            ws.resolve_path(1, "Stores.get"),
            Resolution::Unresolved,
            "a module in the workspace is not visible without an import"
        );
    }

    #[test]
    fn resolution_yields_an_identity_not_a_name() {
        // The architectural invariant E2B establishes: after this, analyses
        // consume a DefId. Two modules declaring `Store` are two identities,
        // so a checker holding one cannot act on the other.
        let other = "module other\n\ntype Store = Store { different: Int }\n";
        let page = "module page\n\nimport domain.{ Store }\n";
        let (_, ws) = workspace(&[DOMAIN, other, page]);
        let Resolution::Imported { def, .. } = ws.resolve(2, "Store") else {
            panic!("expected an import");
        };
        assert_eq!(def.unit, 0, "it must be domain's Store, not other's");

        let other_store = ws.modules[1].lookup_any("Store").expect("other::Store");
        assert_ne!(def, other_store, "same name, different identity");
    }
}
