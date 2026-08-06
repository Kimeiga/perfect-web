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
                        .map(|e| e.path.clone())
                        .collect(),
                    label: label_from_return(decl.ret.as_deref(), &decl.ret_args),
                    returns: decl.ret.clone(),
                };
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
