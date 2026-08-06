//! Three outcomes, because "safe" and "violation" are not the only answers.
//!
//! Architect ruling, 2026-08-06, after two panics in one phase:
//!
//! > Recovery is not proof. If an earlier phase has produced an invalid value,
//! > every later phase must know that its answer is blocked rather than
//! > silently turning the invalid value into a safe-looking one.
//!
//! The concrete failure this prevents is subtle and was already latent here.
//! `MatchReport::is_exhaustive` returned `missing.is_empty()`. When a
//! constructor pattern's arity disagreed with its declaration, the analysis
//! could not say what was missing — so it reported nothing missing, and
//! "nothing missing" read as *exhaustive*. The defensive fix that stopped the
//! crash converted a panic into a **false proof**, which is worse: a panic is
//! at least loud.
//!
//! So an analysis answers one of three things:
//!
//! ```text
//! Proven(T)      the analysis ran and this is the answer
//! Violation(..)  the analysis ran and found a defect
//! Blocked(..)    an earlier error means there is no answer
//! ```
//!
//! `Blocked` is not a diagnostic. The diagnostic belongs to whichever phase
//! produced the invalid value — reporting it again here is how one defect
//! becomes four. It carries the reason so a developer asking "why did this not
//! get checked" gets an answer, and so `--explain` can say which rules did not
//! run on a file rather than implying they all passed.

use std::fmt;

/// Why an analysis could not reach an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// A constructor pattern binds a different number of fields than its
    /// constructor declares, so the pattern matrix cannot be built.
    PatternArity { ctor: String },
    /// A name did not resolve, so what it refers to is unknown.
    Unresolved { name: String },
    /// A type could not be determined, so the analysis has nothing to
    /// enumerate over.
    UnknownType { of: String },
    /// The node did not parse.
    NotParsed,
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocked::PatternArity { ctor } => {
                write!(f, "`{ctor}` binds the wrong number of fields")
            }
            Blocked::Unresolved { name } => write!(f, "`{name}` does not resolve"),
            Blocked::UnknownType { of } => write!(f, "the type of `{of}` is not known"),
            Blocked::NotParsed => f.write_str("it did not parse"),
        }
    }
}

/// What an analysis concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<T> {
    /// The analysis ran to completion. This is the answer.
    Proven(T),
    /// The analysis ran to completion and the program is wrong.
    Violation(T),
    /// An earlier error means there is no answer. **Not** a violation, and
    /// emphatically not a proof.
    Blocked(Vec<Blocked>),
}

impl<T> Outcome<T> {
    /// Did the analysis actually reach a conclusion?
    ///
    /// The question every caller must ask before treating silence as safety.
    pub fn is_conclusive(&self) -> bool {
        !matches!(self, Outcome::Blocked(_))
    }

    pub fn is_violation(&self) -> bool {
        matches!(self, Outcome::Violation(_))
    }

    /// The reasons, when blocked. Empty otherwise.
    pub fn blockers(&self) -> &[Blocked] {
        match self {
            Outcome::Blocked(b) => b,
            _ => &[],
        }
    }

    /// The answer, when there is one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Outcome::Proven(v) | Outcome::Violation(v) => Some(v),
            Outcome::Blocked(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distinction the whole module exists for.
    #[test]
    fn blocked_is_neither_proven_nor_a_violation() {
        let blocked: Outcome<()> = Outcome::Blocked(vec![Blocked::PatternArity {
            ctor: "Cancelled".into(),
        }]);
        assert!(!blocked.is_conclusive());
        assert!(!blocked.is_violation());
        assert!(blocked.value().is_none());

        // The trap: a caller that treats "not a violation" as "safe". Both of
        // these are not violations, and only one of them is a proof.
        let proven: Outcome<()> = Outcome::Proven(());
        assert!(!proven.is_violation());
        assert!(proven.is_conclusive());
        assert!(
            proven.is_conclusive() != blocked.is_conclusive(),
            "a caller must be able to tell these apart"
        );
    }

    #[test]
    fn a_blocker_says_why_in_the_developers_vocabulary() {
        assert_eq!(
            Blocked::PatternArity {
                ctor: "Cancelled".into()
            }
            .to_string(),
            "`Cancelled` binds the wrong number of fields"
        );
    }
}
