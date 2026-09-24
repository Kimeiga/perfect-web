//! Resolved library signatures: one semantic authority for every consumer.
//!
//! Written annotations remain in HIR as source provenance. A signature contains
//! their resolved identities, or the reason resolution failed, never a second
//! head/argument spelling. Missing and invalid annotations are distinct states.

use std::collections::BTreeMap;

use crate::hir::{Decl, DeclKind, DeclaredType, Hir, Span};
use crate::privacy::{Label, Restriction};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};
use crate::resolved::{self, Builtin, Primitive, ResolvedType, StableTypeId, TypeResolution};

/// Closed set of constructors provided by the selected platform module.
#[derive(Debug, Clone, Copy)]
pub enum PrivacyQualifier {
    Secret,
    Session,
    User,
    Organization,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub path: String,
    pub definition: DefId,
    pub effects: Vec<String>,
    pub label: Label,
    /// No annotation is not the same as an annotation which did not resolve.
    pub returns: Option<TypeResolution>,
    pub params: Vec<Option<TypeResolution>>,
}

impl Signature {
    pub fn capabilities(&self) -> Vec<String> {
        self.effects
            .iter()
            .map(|e| e.split('.').next().unwrap_or(e).to_string())
            .collect()
    }

    pub fn result(&self) -> Option<&ResolvedType> {
        self.returns.as_ref().and_then(TypeResolution::resolved)
    }
}

/// A member lookup asks for the constructor's declared member set. It does not
/// equate two instantiated types; full value compatibility uses `same_as`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Receiver {
    Nominal(DefId),
    Builtin(Builtin),
    Primitive(Primitive),
}

fn receiver(ty: &ResolvedType) -> Option<Receiver> {
    ty.def_id()
        .map(Receiver::Nominal)
        .or_else(|| ty.as_builtin().map(Receiver::Builtin))
        .or_else(|| ty.as_primitive().map(Receiver::Primitive))
}

/// **Facts about one type declaration**, for an identity resolution already
/// established. Never a way to find one: there is no by-name entry point.
///
/// Added 2026-09-24 for the value relations (E9-V1). A construction is a call
/// whose callee is a type, and checking it needs what the declaration says the
/// construction takes.
#[derive(Debug, Clone, Default)]
pub struct TypeDecl {
    /// `type Cart = Cart { lines: .., total: .. }`: its fields, in declaration
    /// order, each resolved from where the declaration is written.
    pub record: Option<Vec<(String, TypeResolution)>>,
    /// `opaque type PositiveInt = Int`: what `PositiveInt(1)` is built from.
    pub representation: Option<TypeResolution>,
}

#[derive(Debug, Default)]
pub struct Signatures {
    by_path: BTreeMap<String, Signature>,
    by_def: BTreeMap<DefId, Signature>,
    by_member: BTreeMap<(Receiver, String), Signature>,
    types: BTreeMap<DefId, TypeDecl>,
    /// Immutable resolver snapshot, mechanically copied from the checked set.
    workspace: Workspace,
    /// Stable declaration paths are a projection of DefId, not a name lookup.
    paths: BTreeMap<DefId, String>,
}

impl Signatures {
    pub fn build(workspace: &Workspace, units: &[&Hir]) -> Signatures {
        let mut out = Signatures {
            workspace: workspace.clone(),
            ..Signatures::default()
        };
        for m in &workspace.modules {
            let Some(hir) = units.get(m.unit) else {
                continue;
            };
            for (id, decl) in hir.all_decls() {
                let def = DefId {
                    unit: m.unit,
                    decl: id.0,
                };
                let path = if m.name.is_empty() {
                    decl.name.clone()
                } else {
                    format!("{}.{}", m.name, decl.name)
                };
                out.paths.insert(def, path);
            }
        }
        for m in &workspace.modules {
            let Some(hir) = units.get(m.unit) else {
                continue;
            };
            for (id, decl) in hir.all_decls() {
                let def = DefId {
                    unit: m.unit,
                    decl: id.0,
                };
                if decl.kind == DeclKind::Opaque
                    && let Some(rep) = &decl.opaque_of
                {
                    // The representation is kept as a spelling by lowering,
                    // so only a head without arguments can be resolved from
                    // it faithfully. A generic one is Blocked, not re-parsed.
                    let representation = match rep.contains('<') {
                        false => resolved::resolve(
                            workspace,
                            m.unit,
                            Some(def),
                            &decl.type_params,
                            &DeclaredType::new(rep.clone(), Vec::new()),
                            decl.name_span.clone(),
                        ),
                        true => TypeResolution::Blocked {
                            why: format!(
                                "the representation `{rep}` is kept as a spelling, not a tree"
                            ),
                        },
                    };
                    out.types.entry(def).or_default().representation = Some(representation);
                }
                if decl.kind == DeclKind::Type
                    && let Some(fields) = &decl.fields
                {
                    let record: Vec<(String, TypeResolution)> = fields
                        .iter()
                        .map(|field| {
                            let resolution = match &field.ty {
                                Some(written) => resolved::resolve(
                                    workspace,
                                    m.unit,
                                    Some(def),
                                    &decl.type_params,
                                    written,
                                    field.span.clone(),
                                ),
                                None => TypeResolution::Blocked {
                                    why: format!("the field `{}` has no written type", field.name),
                                },
                            };
                            (field.name.clone(), resolution)
                        })
                        .collect();
                    out.types.entry(def).or_default().record = Some(record);
                    // The declaration applied to its own parameters: `Box<T>`,
                    // not `Box`. Resolution checks a declared constructor's
                    // arity, so the bare name is not a type here.
                    let nominal = DeclaredType::new(
                        decl.name.clone(),
                        decl.type_params
                            .iter()
                            .map(|p| DeclaredType::new(p.clone(), Vec::new()))
                            .collect(),
                    );
                    let recv = resolved::resolve(
                        workspace,
                        m.unit,
                        Some(def),
                        &decl.type_params,
                        &nominal,
                        decl.name_span.clone(),
                    );
                    for field in fields {
                        let Some(written) = &field.ty else { continue };
                        let result = resolved::resolve(
                            workspace,
                            m.unit,
                            Some(def),
                            &decl.type_params,
                            written,
                            field.span.clone(),
                        );
                        let label = result.resolved().map(|t| out.label(t)).unwrap_or_default();
                        if let Some(key) = recv.resolved().and_then(receiver) {
                            out.by_member.insert(
                                (key, field.name.clone()),
                                Signature {
                                    path: format!("{}.{}", out.paths[&def], field.name),
                                    definition: def,
                                    effects: vec![],
                                    label,
                                    returns: Some(result),
                                    params: vec![Some(recv.clone())],
                                },
                            );
                        }
                    }
                }
                if !matches!(
                    decl.kind,
                    DeclKind::Fn
                        | DeclKind::Query
                        | DeclKind::Command
                        | DeclKind::Subscription
                        | DeclKind::Resource
                        | DeclKind::Task
                        | DeclKind::View
                        | DeclKind::Component
                        | DeclKind::Page
                        | DeclKind::Materialize
                ) {
                    continue;
                }
                let sig = out.signature_of(def, decl);
                if Namespace::of(decl.kind) == Some(Namespace::Term) {
                    if let Some(key) = sig
                        .params
                        .first()
                        .and_then(Option::as_ref)
                        .and_then(TypeResolution::resolved)
                        .and_then(receiver)
                    {
                        out.by_member.insert((key, decl.name.clone()), sig.clone());
                    }
                    out.by_path.insert(sig.path.clone(), sig.clone());
                }
                // UI declarations need interfaces too, but are not term-callable.
                out.by_def.insert(def, sig);
            }
        }
        out
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn unit_of(&self, module: Option<&str>) -> Option<usize> {
        let name = module.unwrap_or_default();
        self.workspace
            .modules
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.unit)
    }

    fn signature_of(&self, def: DefId, decl: &Decl) -> Signature {
        let resolve = |ty: &DeclaredType, span: Span| {
            resolved::resolve(
                &self.workspace,
                def.unit,
                Some(def),
                &decl.type_params,
                ty,
                span,
            )
        };
        let returns = decl
            .ret
            .as_ref()
            .map(|t| resolve(t, decl.name_span.clone()));
        Signature {
            path: self
                .paths
                .get(&def)
                .cloned()
                .unwrap_or_else(|| decl.name.clone()),
            definition: def,
            effects: decl
                .declared_effects
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|e| e.written.clone())
                .collect(),
            label: returns
                .as_ref()
                .and_then(TypeResolution::resolved)
                .map(|t| self.label(t))
                .unwrap_or_default(),
            returns,
            params: decl
                .params
                .iter()
                .map(|p| p.ty.as_ref().map(|t| resolve(t, p.span.clone())))
                .collect(),
        }
    }

    pub fn resolve_type(
        &self,
        module: Option<&str>,
        decl: &Decl,
        ty: &DeclaredType,
        span: Span,
    ) -> TypeResolution {
        let Some(unit) = self.unit_of(module) else {
            return TypeResolution::Blocked {
                why: "the annotation's declaring module is unavailable".into(),
            };
        };
        let binder = Namespace::of(decl.kind).and_then(|ns| {
            match self.workspace.resolve_in(unit, ns, &decl.name) {
                Resolution::Local(def) | Resolution::Imported { def, .. } => Some(def),
                _ => None,
            }
        });
        resolved::resolve(&self.workspace, unit, binder, &decl.type_params, ty, span)
    }

    /// Language bindings such as the implicit UI receiver and event protocol
    /// name a defining module explicitly. This is not ambient application lookup.
    pub fn language_type(&self, module: &str, name: &str) -> Option<ResolvedType> {
        let unit = self.unit_of(Some(module))?;
        resolved::resolve(
            &self.workspace,
            unit,
            None,
            &[],
            &DeclaredType::new(name, vec![]),
            0..0,
        )
        .resolved()
        .cloned()
    }

    pub fn by_path(&self, path: &str) -> Option<&Signature> {
        self.by_path.get(path)
    }

    /// What a type declaration says about itself, by its identity.
    pub fn type_decl(&self, def: DefId) -> Option<&TypeDecl> {
        self.types.get(&def)
    }

    /// The qualified path of a declaration, for a diagnostic. A projection of
    /// the identity, never a way to reach one.
    pub fn path_of(&self, def: DefId) -> Option<&str> {
        self.paths.get(&def).map(String::as_str)
    }
    pub fn by_def(&self, def: DefId) -> Option<&Signature> {
        self.by_def.get(&def)
    }

    pub fn in_module(&self, module: Option<&str>, path: &str) -> Option<&Signature> {
        let unit = self.unit_of(module)?;
        let found = if path.contains('.') {
            self.workspace.resolve_path_in(unit, Namespace::Term, path)
        } else {
            self.workspace.resolve_in(unit, Namespace::Term, path)
        };
        match found {
            Resolution::Local(def) | Resolution::Imported { def, .. } => self.by_def(def),
            _ => None,
        }
    }

    pub fn member_of(&self, ty: &ResolvedType, name: &str) -> Option<&Signature> {
        self.by_member.get(&(receiver(ty)?, name.to_string()))
    }

    /// The same lookup, from a receiver's constructor alone. For a value
    /// relation holding a partly inferred type: `List<?>` still has `List`'s
    /// members, and which one is meant does not depend on the hole.
    pub fn member_by(&self, receiver: Receiver, name: &str) -> Option<&Signature> {
        self.by_member.get(&(receiver, name.to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Signature)> {
        self.by_path.iter()
    }
    pub fn all_signatures(&self) -> impl Iterator<Item = &Signature> {
        self.by_def.values().chain(self.by_member.values())
    }
    pub fn len(&self) -> usize {
        self.by_path.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    pub fn stable_type(&self, ty: &ResolvedType) -> Option<StableTypeId> {
        resolved::stable_with(ty, &|def| self.paths.get(&def).cloned())
    }

    /// Whether this is the selected platform's privacy constructor, by DefId.
    pub fn privacy_qualifier(&self, ty: &ResolvedType) -> Option<PrivacyQualifier> {
        self.privacy_kind(ty.def_id()?)
    }

    pub fn privacy_kind(&self, def: DefId) -> Option<PrivacyQualifier> {
        let module = self
            .workspace
            .modules
            .iter()
            .find(|m| m.name == "capability")?;
        [
            ("Secret", PrivacyQualifier::Secret),
            ("Session", PrivacyQualifier::Session),
            ("User", PrivacyQualifier::User),
            ("Organization", PrivacyQualifier::Organization),
        ]
        .into_iter()
        .find_map(|(name, kind)| {
            (module.defines.get(&(Namespace::Type, name.to_string())) == Some(&def)).then_some(kind)
        })
    }

    pub fn label(&self, ty: &ResolvedType) -> Label {
        let mut label = Label::public();
        if let Some(qualifier) = self.privacy_qualifier(ty)
            && let Some(arg) = ty.args().first()
            && let Some(identity) = self.stable_type(arg)
        {
            // Preserve the legacy names of the selected platform's intrinsic
            // principals. Every other identity stays fully qualified.
            let key = match &identity {
                StableTypeId::Declared { path, args }
                    if args.is_empty()
                        && matches!(
                            path.as_str(),
                            "capability.SessionId"
                                | "capability.UserId"
                                | "capability.OrganizationId"
                                | "capability.Payments"
                                | "capability.Signing"
                        ) =>
                {
                    path.trim_start_matches("capability.").to_string()
                }
                _ => format!("type:{}", identity),
            };
            label = Label::of(match qualifier {
                PrivacyQualifier::Secret => Restriction::Secret(key),
                PrivacyQualifier::Session => Restriction::Session(key),
                PrivacyQualifier::User => Restriction::User(key),
                PrivacyQualifier::Organization => Restriction::Organization(key),
            });
        }
        for arg in ty.args() {
            label = label.join(&self.label(arg));
        }
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use pw_syntax::parse_tree;

    fn build(sources: &[&str]) -> (Vec<Hir>, Signatures) {
        let owned: Vec<Hir> = sources
            .iter()
            .map(|s| lower_file(s, &parse_tree(s).green))
            .collect();
        let refs: Vec<&Hir> = owned.iter().collect();
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        (owned, sigs)
    }

    #[test]
    fn a_member_resolves_by_receiver_type_and_ambiguity_resolves_to_nothing() {
        let (_, sigs) = build(&[
            "module browser\n\n             opaque type ElementRef = String\n             fn offsetWidth(el: ElementRef) -> Float !{ layout.measure } { 0.0 }\n",
            "module other\n\n             opaque type Widget = String\n             fn offsetWidth(w: Widget) -> Float !{} { 0.0 }\n             fn only_here(w: Widget) -> Float !{ network.fetch } { 0.0 }\n",
        ]);

        // Each receiver gets its own member, with its own row.
        assert_eq!(
            sigs.member_of(
                &sigs
                    .language_type("browser", "ElementRef")
                    .expect("declared receiver"),
                "offsetWidth"
            )
            .expect("member")
            .effects,
            ["layout.measure"]
        );
        assert_eq!(
            sigs.member_of(
                &sigs
                    .language_type("other", "Widget")
                    .expect("declared receiver"),
                "offsetWidth"
            )
            .expect("member")
            .effects,
            Vec::<String>::new()
        );

        // A member that exists on exactly ONE type in the whole program is
        // still not reachable without knowing the receiver. That is the point:
        // the old fallback resolved this, so whether a check ran depended on no
        // other type ever declaring an `only_here`. Adding one elsewhere would
        // have silently switched the rule off.
        assert!(
            sigs.member_of(
                &sigs
                    .language_type("browser", "ElementRef")
                    .expect("declared receiver"),
                "only_here"
            )
            .is_none()
        );
        assert!(
            sigs.member_of(
                &sigs
                    .language_type("other", "Widget")
                    .expect("declared receiver"),
                "only_here"
            )
            .is_some()
        );
    }

    /// The fallback stays deleted.
    ///
    /// It is easy to reintroduce — "just resolve it when there's only one" is a
    /// reasonable-sounding sentence — and the failure it causes is silence, not
    /// a wrong answer, so nothing goes red when it comes back.
    #[test]
    fn there_is_no_by_name_member_lookup_in_the_compiler() {
        // Assembled at runtime so this test does not match its own source.
        let needles = [format!("unique{}member", "_"), format!("member(N{}", "one")];
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../");
        let mut offenders = Vec::new();
        let mut stack = vec![src_dir];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("compiler/") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).expect("read");
                for (n, line) in text.lines().enumerate() {
                    // A doc comment may name the thing it explains why we do
                    // not do.
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    if needles.iter().any(|needle| line.contains(needle)) {
                        offenders.push(format!("{}:{}", p.display(), n + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "by-name member lookup is back — a correctness check would depend \
             on a name being unique across the program:\n  {}",
            offenders.join("\n  ")
        );
    }

    #[test]
    fn no_checker_carries_a_table_of_library_names() {
        // The E2C deletion gate, verbatim:
        //
        //   No privacy, placement, or effect checker contains a built-in
        //   mapping from library function names to labels, effects,
        //   capabilities, or worlds.
        //
        // Enforced, because a table is easy to reintroduce "just for this one
        // case" and impossible to notice afterwards.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();

        for (file, banned) in [
            // The exact symbols that were deleted, plus the library names they
            // keyed off. `signatures.rs` itself may name types — it reads
            // declared return types — but no checker may name a FUNCTION.
            (
                "check.rs",
                &["introduced_restriction", "current_organization", "secrets."][..],
            ),
            (
                "placement.rs",
                &["secrets", "current_", "Stores", "Carts"][..],
            ),
            ("privacy.rs", &["secrets", "current_", "database.read"][..]),
        ] {
            let path = dir.join(file);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                // Comments may discuss what was removed; code may not do it.
                let code = line.split("//").next().unwrap_or("");
                for needle in banned {
                    if code.contains(needle) {
                        offenders.push(format!("{file}: {}", line.trim()));
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "a checker has regrown a library-name table:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn one_declaration_answers_privacy_effects_and_placement() {
        // The architect's worked example, as a test. Three questions, one
        // source — not three tables that can disagree.
        let (_, sigs) = build(&[
            include_str!("../../../packages/pw-platform-web/capability.pw"),
            "module secrets\nimport capability.{Secret, Payments}\n\nfn payments() -> Secret<Payments> !{ secret.read } { 1 }\n",
        ]);
        let s = sigs.by_path("secrets.payments").expect("the signature");

        assert_eq!(s.effects, ["secret.read"], "effect inference reads this");
        assert_eq!(
            s.label.to_string(),
            "Secret<Payments>",
            "privacy propagation reads this"
        );
        assert_eq!(
            s.capabilities(),
            ["secret"],
            "placement derives the capability from the row"
        );
    }

    #[test]
    fn a_pure_function_carries_no_effects_and_no_label() {
        // The control. If every signature came back restricted, the checks
        // above would pass while meaning nothing.
        let (_, sigs) = build(&["module List\n\nfn map(xs: Int, f: Int) -> Int !{} { 0 }\n"]);
        let s = sigs.by_path("List.map").expect("the signature");
        assert!(s.effects.is_empty());
        assert!(s.label.is_public());
        assert!(s.capabilities().is_empty());
    }

    #[test]
    fn the_same_signature_is_reachable_by_path_and_by_identity() {
        // A checker that resolved a call holds a DefId; one that has only the
        // written text holds a path. They must not be able to see different
        // answers.
        let (hirs, sigs) =
            build(&["module Stores\n\nfn get(id: Int) -> Int !{ database.read } { 0 }\n"]);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let ws = Workspace::build(&refs);
        let def = ws.modules[0].lookup_any("get").expect("get");
        assert_eq!(
            sigs.by_def(def).map(|s| s.definition),
            sigs.by_path("Stores.get").map(|s| s.definition)
        );
    }

    #[test]
    fn a_label_comes_from_the_declared_type_not_the_function_name() {
        // The stand-in this replaces keyed off `secrets.*`. A function named
        // anything at all must carry a secret if it RETURNS one, and a function
        // called `secrets.something` must not if it does not.
        let (_, sigs) = build(&[
            include_str!("../../../packages/pw-platform-web/capability.pw"),
            "module anything\nimport capability.{Secret, Signing}\n\n\
             fn innocuous_name() -> Secret<Signing> !{} { 1 }\n\
             fn scary_sounding_secret() -> Int !{} { 1 }\n",
        ]);
        assert_eq!(
            sigs.by_path("anything.innocuous_name")
                .unwrap()
                .label
                .to_string(),
            "Secret<Signing>"
        );
        assert!(
            sigs.by_path("anything.scary_sounding_secret")
                .unwrap()
                .label
                .is_public(),
            "the name must not decide the label"
        );
    }
}
