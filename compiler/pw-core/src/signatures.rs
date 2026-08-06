//! E2C — resolved library signatures, as the single source of truth.
//!
//! Architect ruling, 2026-08-06:
//!
//! > No privacy, placement, or effect checker contains a built-in mapping from
//! > library function names to labels, effects, capabilities, or worlds.
//!
//! Before this, three checkers independently hard-coded the same facts:
//! `check.rs` knew `secrets.payments()` yields a secret, `placement.rs` knew
//! `secret.*` is origin-only, and both knew `database.read` is a database
//! effect. Two of those could disagree and nothing would notice.
//!
//! Now one declaration answers all three:
//!
//! ```text
//! secrets.payments : () -> Secret<Payments>  !{ secret.read }
//!                          ^^^^^^^^^^^^^^^^     ^^^^^^^^^^^
//!                          privacy label        effect row, from which the
//!                                               placement solver derives the
//!                                               capability requirement
//! ```

use std::collections::BTreeMap;

use crate::hir::{DeclKind, Hir};
use crate::privacy::{Label, Restriction};
use crate::resolve::{DefId, Workspace};

/// What a resolved declaration promises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// `Stores.get`, as written at the call site.
    pub path: String,
    /// Its declared effect row, as written.
    pub effects: Vec<String>,
    /// The privacy label of what it returns.
    pub label: Label,
    /// The return type's head, without arguments.
    pub returns: Option<String>,
    /// The return type's arguments: `Option<Store>` gives `["Store"]`. Needed
    /// wherever the head alone does not say what a value IS — `Option` is not
    /// a type, `Option<Store>` is.
    pub returns_args: Vec<String>,
    /// Each parameter's declared type head, in order. `None` where the
    /// parameter carries no annotation.
    pub params: Vec<Option<String>>,
}

impl Signature {
    /// Capability families this needs, derived from the effect row. The
    /// placement solver takes these; it does not have its own opinion about
    /// what `secrets.payments` requires.
    pub fn capabilities(&self) -> Vec<String> {
        self.effects
            .iter()
            .map(|e| e.split('.').next().unwrap_or(e).to_string())
            .collect()
    }
}

/// Every signature reachable in a program, by the path a call site writes.
#[derive(Debug, Default)]
pub struct Signatures {
    by_path: BTreeMap<String, Signature>,
    by_def: BTreeMap<DefId, Signature>,
    /// `(receiver type, member)` → signature. A function whose first parameter
    /// is a declared type is that type's member: `offsetWidth(el: ElementRef)`
    /// is what `anchor.offsetWidth` means.
    by_member: BTreeMap<(String, String), Signature>,
}

impl Signatures {
    /// Build from the resolved workspace.
    ///
    /// Keyed by `Module.member` because that is what a call site writes, and by
    /// `DefId` because that is what a resolved call yields. Both point at the
    /// same signature, so a checker holding either sees one truth.
    pub fn build(workspace: &Workspace, units: &[&Hir]) -> Signatures {
        let mut out = Signatures::default();

        for m in &workspace.modules {
            let Some(hir) = units.get(m.unit) else {
                continue;
            };
            for (id, decl) in hir.all_decls() {
                if !matches!(
                    decl.kind,
                    DeclKind::Fn
                        | DeclKind::Query
                        | DeclKind::Command
                        | DeclKind::Subscription
                        | DeclKind::Resource
                        | DeclKind::Task
                ) {
                    continue;
                }
                let sig = Signature {
                    path: format!("{}.{}", m.name, decl.name),
                    effects: decl
                        .declared_effects
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(|e| e.written.clone())
                        .collect(),
                    label: label_from_return(decl.ret.as_deref(), &decl.ret_args),
                    returns: decl.ret.clone(),
                    returns_args: decl.ret_args.clone(),
                    params: decl.params.iter().map(|p| p.ty.clone()).collect(),
                };
                // A function whose first parameter is a declared type reads as
                // that type's member. `offsetWidth(el: ElementRef)` is what
                // `anchor.offsetWidth` resolves to, and its row — not a list of
                // property names in a checker — is what says it measures layout.
                if let Some(receiver) = decl.params.first().and_then(|p| p.ty.clone()) {
                    out.by_member
                        .insert((receiver, decl.name.clone()), sig.clone());
                }
                out.by_path.insert(sig.path.clone(), sig.clone());
                out.by_def.insert(
                    DefId {
                        unit: m.unit,
                        decl: id.0,
                    },
                    sig,
                );
            }
        }
        out
    }

    pub fn by_path(&self, path: &str) -> Option<&Signature> {
        self.by_path.get(path)
    }

    pub fn by_def(&self, def: DefId) -> Option<&Signature> {
        self.by_def.get(&def)
    }

    /// A member of `receiver`, resolved by the receiver's TYPE.
    ///
    /// There is no by-name fallback and deliberately never will be. The
    /// previous version resolved an unknown receiver to "the one declaration
    /// with this member name, if exactly one exists" — conservative in that it
    /// never resolved AMBIGUOUSLY, and unsafe in a way conservatism does not
    /// fix: whether a correctness check ran at all depended on a global
    /// accident. `resolve_corpus` showed it — a sibling declaring a second
    /// `on_press` made the handler rule go silent rather than wrong.
    ///
    /// A correctness analysis never means "use this because it happens to be
    /// the only one with this spelling".
    pub fn member_of(&self, receiver: &str, name: &str) -> Option<&Signature> {
        self.by_member
            .get(&(receiver.to_string(), name.to_string()))
    }

    /// Every signature by path. A checker that needs to ask "which declarations
    /// have this property" — rather than "what does this name do" — reads the
    /// table instead of carrying its own copy of the answer.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Signature)> {
        self.by_path.iter()
    }

    pub fn len(&self) -> usize {
        self.by_def.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_def.is_empty()
    }
}

/// The privacy label a return type carries.
///
/// `Secret<Payments>` yields `Secret(Payments)`. This is the *only* place that
/// mapping exists — it reads the declared type, not a table of function names.
fn label_from_return(head: Option<&str>, args: &[String]) -> Label {
    let Some(head) = head else {
        return Label::public();
    };
    match head {
        "Secret" => Label::of(Restriction::Secret(
            args.first().cloned().unwrap_or_else(|| "?".into()),
        )),
        "Session" => Label::of(Restriction::Session(
            args.first().cloned().unwrap_or_else(|| "SessionId".into()),
        )),
        "User" => Label::of(Restriction::User(
            args.first().cloned().unwrap_or_else(|| "UserId".into()),
        )),
        "Organization" => Label::of(Restriction::Organization(
            args.first()
                .cloned()
                .unwrap_or_else(|| "OrganizationId".into()),
        )),
        _ => Label::public(),
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
            sigs.member_of("ElementRef", "offsetWidth")
                .expect("member")
                .effects,
            ["layout.measure"]
        );
        assert_eq!(
            sigs.member_of("Widget", "offsetWidth")
                .expect("member")
                .effects,
            Vec::<String>::new()
        );

        // A member that exists on exactly ONE type in the whole program is
        // still not reachable without knowing the receiver. That is the point:
        // the old fallback resolved this, so whether a check ran depended on no
        // other type ever declaring an `only_here`. Adding one elsewhere would
        // have silently switched the rule off.
        assert!(sigs.member_of("ElementRef", "only_here").is_none());
        assert!(sigs.member_of("Widget", "only_here").is_some());
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
            "module secrets\n\nfn payments() -> Secret<Payments> !{ secret.read } { 1 }\n",
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
        assert_eq!(sigs.by_def(def), sigs.by_path("Stores.get"));
    }

    #[test]
    fn a_label_comes_from_the_declared_type_not_the_function_name() {
        // The stand-in this replaces keyed off `secrets.*`. A function named
        // anything at all must carry a secret if it RETURNS one, and a function
        // called `secrets.something` must not if it does not.
        let (_, sigs) = build(&["module anything\n\n\
             fn innocuous_name() -> Secret<Signing> !{} { 1 }\n\
             fn scary_sounding_secret() -> Int !{} { 1 }\n"]);
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
