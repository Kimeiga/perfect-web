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
    /// The Rust constant's own name, so a test can ask whether any checker
    /// refers to this code — by constant or by literal — without a second
    /// hand-maintained list.
    pub konst: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Syntax,
    Resolution,
    Effects,
    DeclarationRules,
    Exhaustiveness,
    ScopeGraph,
    Placement,
    Privacy,
    Markup,
    /// Charter §7.5A relations that are not effect-row violations: an
    /// ordering, a cycle, a declared assertion that does not hold.
    Layout,
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
                konst: stringify!($konst),
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

    // --- name resolution (PW002x) -----------------------------------------
    //
    // Allocated through this registry, not chosen in prose. They sit in the
    // syntax range because resolution failures are about the *program text*
    // naming something that is not there, not about what the program means.
    UNRESOLVED_MODULE = "PW0020", Resolution,
        "an imported module must exist in the workspace";
    UNRESOLVED_NAME = "PW0021", Resolution,
        "an imported name must be declared by the module it comes from";
    AMBIGUOUS_NAME = "PW0022", Resolution,
        "a name must resolve to exactly one declaration";
    PRIVATE_ACCESS = "PW0023", Resolution,
        "a private declaration is not visible outside its module";
    DUPLICATE_DECLARATION = "PW0024", Resolution,
        "a module may declare each name once per namespace";
    IMPORT_CYCLE = "PW0025", Resolution,
        "modules must not import each other in a cycle";

    // --- declaration rules (PW01xx-PW03xx) --------------------------------
    RETRY_NOT_IDEMPOTENT = "PW0312", DeclarationRules,
        "a command that retries must be idempotent";
    RETRY_UNBOUNDED = "PW0313", DeclarationRules,
        "a retry policy must be bounded";

    STALE_KEY_POLICY = "PW0325", DeclarationRules,
        "a keyed query must say what happens when its key changes";
    OPTIMISTIC_NO_ROLLBACK = "PW0327", DeclarationRules,
        "an optimistic transition must declare a rollback path";
    CACHE_NO_INVALIDATION = "PW0200", DeclarationRules,
        "a shared cache should declare how it is invalidated";

    // --- effects (PW04xx) -------------------------------------------------
    UNDECLARED_EFFECT = "PW0400", Effects,
        "an effect row must name every effect the body performs";
    FORBIDDEN_EFFECT = "PW0401", Effects,
        "some effects are not permitted where a declaration runs, whatever it declares";
    WRONG_FRAME_PHASE = "PW0402", Effects,
        "each frame phase permits only the work it exists to do";

    // --- layout relations (PW04xx) ----------------------------------------
    OBSERVATION_FEEDBACK_CYCLE = "PW0403", Layout,
        "an observation must not cause the change it observes";
    FALSE_INDEPENDENCE = "PW0404", Layout,
        "a subtree declared independent must not depend on anything outside it";

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
    // Distinct from PW5002 on purpose. PW5002 is the solver finding NO world
    // that can run a declaration; this is a world the author NAMED that cannot
    // grant what the declaration needs. A body that would run fine in the
    // browser, pinned to the origin, is wrong without being unplaceable.
    DECLARED_PLACEMENT_CANNOT_GRANT = "PW5005", Placement,
        "a declared placement must be able to grant every effect it requires";
    VALUE_EXCEEDS_SINK_LEVEL = "PW5006", Privacy,
        "a sink accepts only values its declared privacy level admits";
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

/// What a corpus fixture's `@rule` resolves to.
///
/// Architect ruling, 2026-08-06: model the categories explicitly rather than
/// treating every unregistered code as one generic exception. An unknown code
/// is a typo or a fixture nobody can ever satisfy; a *known gap* is the corpus
/// doing its job, naming an invariant the compiler has not built yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleStatus {
    Registered(Code),
    /// Declared by a fixture, not yet implemented, with an owner.
    KnownGap {
        code: &'static str,
        intended_owner: &'static str,
        missing_analysis: &'static str,
    },
    /// Neither. Always a failure.
    Unknown,
}

/// Invariants the corpus specifies and the compiler has not built.
///
/// Each names the milestone that will own it and the analysis that is missing,
/// so the list is a work plan rather than a list of excuses. It shrinks as
/// milestones land; it grows only when a new specification fixture is added.
pub const KNOWN_GAPS: &[(&str, &str, &str)] = &[
    // (code, intended owner, missing analysis)
    ("PW0306", "E9", "type checking — an ambient null assumption"),
    ("PW0307", "E9", "type checking — an unchecked external cast"),
    (
        "PW0308",
        "E7",
        "serialization analysis — a non-serializable capture",
    ),
    (
        "PW0309",
        "E9C",
        "affine types — a transaction neither committed nor rolled back",
    ),
    (
        "PW0310",
        "E9C",
        "affine types — a resource handle that escapes",
    ),
    (
        "PW0320",
        "E9",
        "type checking — a handler signature that does not match its event",
    ),
    (
        "PW0321",
        "E6",
        "route reachability — a route nothing can reach",
    ),
    (
        "PW0328",
        "E7",
        "resumption manifest — private data crossing into the public shell",
    ),
    (
        "PW3011",
        "E7",
        "resumption manifest — private data crossing into it",
    ),
];

/// Classify a corpus `@rule`, after alias resolution.
pub fn rule_status(declared: &str) -> RuleStatus {
    let canonical = crate::diagnostics::canonical_code(declared);
    if let Some(c) = lookup(canonical) {
        return RuleStatus::Registered(c);
    }
    if let Some((code, owner, analysis)) = KNOWN_GAPS.iter().find(|(c, _, _)| *c == canonical) {
        return RuleStatus::KnownGap {
            code,
            intended_owner: owner,
            missing_analysis: analysis,
        };
    }
    RuleStatus::Unknown
}

impl Owner {
    /// The code range this owner's diagnostics must sit in, or `""` when the
    /// owner is semantic and may live anywhere at `PW01xx` or above.
    ///
    /// Part of the registry's contract rather than a test helper: a new owner
    /// has to answer this question before it can register a code.
    pub fn range(self) -> &'static str {
        match self {
            Owner::Syntax | Owner::Resolution => "PW00",
            Owner::Placement | Owner::Privacy | Owner::Markup => "PW50",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::DEPRECATED_ALIASES;

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
    fn every_corpus_rule_is_registered_or_an_owned_known_gap() {
        // The ratchet the architect specified:
        //   unknown == 0
        //   known_gaps only shrink, or grow with an approved new fixture
        // An implemented invariant must not silently fall back to "known gap".
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rejected");
        let (mut registered, mut gaps, mut unknown) = (0, 0, Vec::new());

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
            match rule_status(rule) {
                RuleStatus::Registered(_) => registered += 1,
                RuleStatus::KnownGap { .. } => gaps += 1,
                RuleStatus::Unknown => unknown.push(format!(
                    "{}: @rule {rule}",
                    path.file_name().unwrap().to_string_lossy()
                )),
            }
        }

        eprintln!("  Corpus invariants");
        eprintln!("  - registered: {registered}");
        eprintln!("  - known gaps: {gaps}");
        eprintln!("  - unknown:    {}", unknown.len());

        assert!(
            unknown.is_empty(),
            "a fixture names an invariant that is neither built nor planned — a \
             typo, or a rule nobody can ever satisfy:\n{}",
            unknown.join("\n")
        );
        assert!(
            registered >= 19,
            "registered rules regressed to {registered}"
        );
        assert!(
            gaps <= 25,
            "known gaps grew to {gaps} without a new fixture"
        );
    }

    #[test]
    fn every_known_gap_names_an_owner_and_the_missing_analysis() {
        // A gap list without owners is a list of excuses. With them it is a
        // work plan, and `docs/NEXT.md` can be generated from it.
        for (code, owner, analysis) in KNOWN_GAPS {
            assert!(
                lookup(code).is_none(),
                "{code} is registered AND listed as a gap"
            );
            // A gap the alias table already redirects is unreachable: nothing
            // can ever resolve to it, so it is a work item that will never be
            // picked up and a number that reads as unowned when it is not.
            assert_eq!(
                crate::diagnostics::canonical_code(code),
                *code,
                "{code} is listed as a gap but aliased to \
                 {} — the gap entry is dead",
                crate::diagnostics::canonical_code(code)
            );
            assert!(
                owner.starts_with('E') || owner.starts_with('P'),
                "{code}'s owner {owner:?} is not a milestone"
            );
            assert!(
                analysis.len() > 15,
                "{code}'s missing analysis is not described: {analysis:?}"
            );
        }
    }

    /// A registered code must be a code some checker can actually emit.
    ///
    /// `PW0323` was registered as `RESOURCE_NO_RELEASE` — "a resource must
    /// declare how it is released" — while the only thing that ever emitted
    /// `PW0323` was the placement rule, and the corpus fixture declaring it
    /// (R-025) is about placement too. Nothing enforced resource-release, so
    /// the registry asserted an invariant the compiler did not have, on a
    /// number that already meant something else. Had the placement rule and
    /// the fixture met, the ratchet would have counted a correct catch under a
    /// description of a different rule.
    #[test]
    fn every_registered_code_is_one_a_checker_can_emit() {
        // The whole compiler: `pw-syntax` emits the PW00xx codes, `pw-core`
        // the rest. Scanning one crate would call the other's codes dead.
        let compiler = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .expect("compiler/");
        let mut sources = String::new();
        let mut stack = vec![compiler];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("src") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.file_name().is_some_and(|n| n == "codes.rs") {
                    continue;
                }
                if p.extension().is_some_and(|x| x == "rs") {
                    sources.push_str(&std::fs::read_to_string(&p).expect("read"));
                }
            }
        }
        let mut dead = Vec::new();
        for c in ALL {
            // Either by constant (`codes::UNDECLARED_EFFECT.id`) or by the
            // literal, which `rules.rs` still uses.
            if !sources.contains(c.id) && !sources.contains(c.konst) {
                dead.push(format!("{} ({})", c.id, c.invariant));
            }
        }
        assert!(
            dead.is_empty(),
            "registered but unreachable — the registry claims an invariant \
             nothing enforces:\n  {}",
            dead.join("\n  ")
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
