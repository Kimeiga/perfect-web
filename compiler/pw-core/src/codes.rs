//! The one registry of diagnostic codes.
//!
//! Architect ruling, 2026-08-06:
//!
//! > Raw code strings should no longer be declared independently by parser and
//! > semantic passes. Every checker emits the enum, not a raw `"PW5010"`.
//!
//! The `PW0100` collision is why. It was simultaneously the parser's
//! "expected X, found Y" and `rules.rs`'s shared-cache invariant, so a corpus
//! fixture declaring `PW0100` could be satisfied by an unrelated *syntax* error
//! — the harness would go green while the semantic rule it meant to test had
//! never run.
//!
//! # Ranges
//!
//! ```text
//! PW00xx   lexical and syntax
//! PW01xx+  semantic invariants
//! PW50xx   placement, capability and unsafe boundaries
//! ```
//!
//! Ranges are enforced by a test, not by convention.

use std::fmt;

/// One diagnostic code, with everything the registry knows about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    /// The stable public string, e.g. `"PW5002"`.
    pub id: &'static str,
    /// One sentence naming the invariant, in the developer's vocabulary.
    pub invariant: &'static str,
    /// Which analysis owns it. Metadata — never part of the public code.
    pub owner: Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Syntax,
    DeclarationRules,
    Exhaustiveness,
    ScopeGraph,
    Placement,
    Privacy,
    Markup,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id)
    }
}

macro_rules! codes {
    ($( $konst:ident = $id:literal, $owner:ident, $invariant:literal; )*) => {
        $(
            pub const $konst: Code = Code {
                id: $id,
                invariant: $invariant,
                owner: Owner::$owner,
            };
        )*

        /// Every registered code. The registry's own source of truth.
        pub const ALL: &[Code] = &[ $($konst),* ];
    };
}

codes! {
    // --- syntax (PW00xx) --------------------------------------------------
    EXPECTED = "PW0001", Syntax, "the parser expected a different token here";
    UNCLOSED_TYPE_ARGS = "PW0002", Syntax, "a type argument list must be closed";
    UNCLOSED_EFFECT_ROW = "PW0003", Syntax, "an effect row must be closed";
    EFFECT_ROW_NEEDS_BRACE = "PW0004", Syntax, "an effect row opens with `{`";
    UNCLOSED_PARAMS = "PW0005", Syntax, "a parameter list must be closed";
    UNCLOSED_BLOCK = "PW0006", Syntax, "a block must be closed";
    UNKNOWN_DECLARATION = "PW0007", Syntax, "this does not begin a declaration";
    UNCLOSED_PAREN = "PW0008", Syntax, "a parenthesis must be closed";
    EXPECTED_EXPRESSION = "PW0009", Syntax, "an expression was expected here";
    UNCLOSED_ARGS = "PW0010", Syntax, "an argument list must be closed";
    UNCLOSED_LIST = "PW0011", Syntax, "a list literal must be closed";
    UNCLOSED_MATCH = "PW0012", Syntax, "a match must be closed";
    NO_PROGRESS = "PW0099", Syntax, "the parser made no progress";

    // --- declaration rules (PW01xx-PW03xx) --------------------------------
    RETRY_NOT_IDEMPOTENT = "PW0312", DeclarationRules,
        "a command that retries must be idempotent";
    RETRY_UNBOUNDED = "PW0313", DeclarationRules,
        "a retry policy must be bounded";
    RESOURCE_NO_RELEASE = "PW0323", DeclarationRules,
        "a resource must declare how it is released";
    STALE_KEY_POLICY = "PW0325", DeclarationRules,
        "a keyed query must say what happens when its key changes";
    OPTIMISTIC_NO_ROLLBACK = "PW0327", DeclarationRules,
        "an optimistic transition must declare a rollback path";
    CACHE_NO_INVALIDATION = "PW0200", DeclarationRules,
        "a shared cache should declare how it is invalidated";

    // --- exhaustiveness ---------------------------------------------------
    NON_EXHAUSTIVE_MATCH = "PW0305", Exhaustiveness,
        "a match must cover every value its scrutinee can take";

    // --- structured concurrency (PW20xx) ----------------------------------
    HANDLE_ESCAPES = "PW2001", ScopeGraph,
        "a handle cannot outlive the scope that owns it";
    TASK_DETACHED = "PW2002", ScopeGraph,
        "an ordinary task cannot be detached from its scope";
    HANDLE_USED_LATE = "PW2003", ScopeGraph,
        "a handle cannot be used after its owning scope has exited";
    SCOPE_OUTLIVES_OWNER = "PW2004", ScopeGraph,
        "a subscription cannot declare a scope that outlives its owner";

    // --- privacy, placement, unsafe boundaries (PW50xx) -------------------
    PRIVATE_IN_SHARED_CACHE = "PW5001", Privacy,
        "a value that is not public cannot live in a shared cache";
    NO_FEASIBLE_PLACEMENT = "PW5002", Placement,
        "every declaration must have somewhere it can run";
    SECRET_TO_BROWSER = "PW5003", Privacy,
        "a secret cannot be rendered to the browser";
    CACHE_KEY_OMITS_PARTITION = "PW5004", Privacy,
        "a shared cache key must carry every partition its value depends on";
    UNSAFE_AUDIT_INCOMPLETE = "PW5010", DeclarationRules,
        "an unsafe escape hatch must carry a complete audit record";
    // The architect proposed PW5011 for this. That number was already the
    // unkeyed-list invariant — exactly the collision this registry exists to
    // prevent, caught by the registry on its first day. PW5015 instead.
    UNSAFE_ATTRIBUTION_INVALID = "PW5015", DeclarationRules,
        "an escape hatch's attribution target must name a real owner";
    UNKEYED_LIST = "PW5011", Markup,
        "a list over a mutable collection needs a stable key";
    INVALID_NESTING = "PW5012", Markup,
        "an element may only contain the children HTML permits";
    HANDLER_ON_INERT = "PW5013", Markup,
        "interactive behaviour belongs on an element that can receive it";
    CONTROL_WITHOUT_LABEL = "PW5014", Markup,
        "a form control must have something that names it";
}

/// Look a code up by its public string.
pub fn lookup(id: &str) -> Option<Code> {
    ALL.iter().copied().find(|c| c.id == id)
}

impl Owner {
    /// The code range this owner's diagnostics must sit in, or `""` when the
    /// owner is semantic and may live anywhere at `PW01xx` or above.
    ///
    /// Part of the registry's contract rather than a test helper: a new owner
    /// has to answer this question before it can register a code.
    pub fn range(self) -> &'static str {
        match self {
            Owner::Syntax => "PW00",
            Owner::Placement | Owner::Privacy | Owner::Markup => "PW50",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{DEPRECATED_ALIASES, canonical_code};

    #[test]
    fn every_code_is_unique() {
        let mut ids: Vec<&str> = ALL.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "a code is registered twice");
    }

    #[test]
    fn every_code_sits_in_its_owner_range() {
        // The `PW0100` collision in one assertion: a syntax code outside PW00xx,
        // or a placement code outside PW50xx, is how two rules end up sharing a
        // number and a corpus fixture goes green for the wrong reason.
        for c in ALL {
            let want = c.owner.range();
            if want.is_empty() {
                assert!(
                    !c.id.starts_with("PW00"),
                    "{} is semantic but sits in the syntax range",
                    c.id
                );
                continue;
            }
            assert!(
                c.id.starts_with(want),
                "{} is owned by {:?} and must start with {want}",
                c.id,
                c.owner
            );
        }
    }

    #[test]
    fn every_alias_resolves_to_a_registered_code() {
        for (alias, canonical) in DEPRECATED_ALIASES {
            assert!(
                lookup(canonical).is_some(),
                "alias {alias} resolves to {canonical}, which is not registered"
            );
            assert!(
                lookup(alias).is_none(),
                "{alias} is both an alias and a registered code"
            );
        }
    }

    #[test]
    fn every_corpus_rule_resolves_to_a_registered_code_or_a_known_gap() {
        // A fixture whose `@rule` names nothing the compiler could ever emit is
        // either a typo or a rule nobody has built. Both are worth knowing, and
        // the second is the corpus doing its job — so unregistered codes are
        // listed, not failed.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rejected");
        let mut unregistered = Vec::new();
        let mut resolved = 0;

        for e in std::fs::read_dir(&root).expect("rejected/") {
            let path = e.expect("entry").path();
            if path.extension().is_none_or(|x| x != "pw") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("read");
            let Some(rule) = src
                .lines()
                .find_map(|l| l.trim().strip_prefix("// @rule:"))
                .map(str::trim)
            else {
                continue;
            };
            let canonical = canonical_code(rule);
            if lookup(canonical).is_some() {
                resolved += 1;
            } else {
                unregistered.push(format!(
                    "{}: @rule {rule} -> {canonical}",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
        }

        assert!(resolved >= 19, "only {resolved} corpus rules resolve");
        // The rest name invariants nobody has implemented yet. That number
        // going DOWN is the project making progress; it must not go up.
        assert!(
            unregistered.len() <= 25,
            "{} corpus rules name no registered code:\n{}",
            unregistered.len(),
            unregistered.join("\n")
        );
    }

    #[test]
    fn an_invariant_reads_as_a_sentence_about_the_program() {
        // A code whose invariant names an implementation pass would teach the
        // developer the wrong thing — the architect's one-code-per-invariant
        // ruling is about vocabulary, not just numbering.
        for c in ALL {
            assert!(!c.invariant.is_empty(), "{} has no invariant", c.id);
            for banned in ["checker", "pass", "analysis", "detector", "algorithm"] {
                assert!(
                    !c.invariant.contains(banned),
                    "{}'s invariant mentions the implementation: {:?}",
                    c.id,
                    c.invariant
                );
            }
        }
    }
}
