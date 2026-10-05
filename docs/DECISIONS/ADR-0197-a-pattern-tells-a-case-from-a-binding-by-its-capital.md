# ADR-0197: a pattern tells a case from a binding by its capital

Status: accepted under the owner's delegation of 2026-10-02. It builds
ADR-0195's ruling 2. Date: 2026-10-05. Milestone: E14.

## Context

- **Six places each decided whether a bare name in a pattern was a case**
  (the parser made it a binding):
  - the checker's exhaustiveness bridge: a case where any type in the
    program had one of that name;
  - the lexical scopes: a case of a type this unit sees, or `true`,
    `false`, `None`;
  - the value relations: the same;
  - the backend's one-level test: the language's four, or any declared case
    in the program;
  - the backend's arms and decision tree: a case of the scrutinee's type;
  - the Koka emitter: a capital.

  No two agreed.
- **So a misspelt case, `Circel`, bound a name** and matched every value
  the arms before it left. This is rustc's E0170: "a pattern binding has
  the same name as an enum variant", which "binds new variables instead of
  matching the actual enum variant".
- **The disagreements had symptoms:**
  - `Pair(Draft, Draft)` reported `Draft` bound twice (PW0028);
  - a template arm `{:Some(Draft)}` bound a name `Draft`.
- **The owner ruled (ADR-0195, ruling 2):**
  - capitalization decides, as in Haskell, OCaml and Elm;
  - a case name begins with an uppercase letter;
  - a pattern name beginning lowercase, or `_`, is always a binding;
  - one beginning uppercase is always a case, resolved in the scrutinee's
    type, and an error where it lacks it.
- **The Haskell 2010 report (§2.4)** separates variable identifiers, which
  begin lowercase, from constructor identifiers, which begin uppercase.
- **No program had to change.** No `.pw` file under `examples/` or
  `packages/`, and no program embedded in a test, names a case in lowercase
  or writes an undeclared capitalized pattern name. The exception is
  `robustness.rs`'s "a constructor that is not one", which checks only that
  the compiler does not panic.

## Decision

1. **The parser decides once.** `pw_syntax::pattern_kind` reads a name's
   first letter: `A`–`Z` is a case, and anything else, `_x` among them, is a
   binding. A pattern's bare name that is a case becomes a constructor
   pattern, as one with arguments or a qualifier already did. `true` and
   `false` are `Bool`'s cases.
2. **Every later reader reads what the parser decided.** A `Pattern::Bind`
   is a binding and a `Pattern::Ctor` a case, everywhere. Each guess is
   deleted, and with them the signatures scoping read to make one:
   `Lexical::build` and `build_in` take a declaration alone, and the three
   rules that passed signatures only to them no longer take any. The
   deleted guesses:
   - the checker's program-wide set of case names (`check::Env`, and its
     threading through every match);
   - the lexical scopes' and the value relations' `names_a_case`;
   - the backend's three;
   - the Koka emitter's.
3. **A capitalized name the scrutinee's type lacks is PW0608**, the
   invariant a qualified pattern was already held to. PW0608 is revision 3.
   Its message names the type and its explanation the risk: "`Circel` is not
   a constructor of `Shape`".
4. **A case is declared with a capital: PW0625**, whose repair names the
   case capitalized.
5. **A template arm's case is one by its capital, and its fields are
   bindings by theirs.** So `{:Some(Draft)}` is not an arm (PW5019): an arm
   takes one case apart, and a field that is a case is a nested pattern no
   arm has.
6. **Not changed:**
   - a `let` name or a parameter binds whatever its first letter, since its
     grammar binds nothing else;
   - a pattern qualified through a module's lowercase name,
     `geometry.Shape.Empty`, is a case by its qualifier.

## Alternatives

- **Keep the program-wide set, in one function.** The six would agree, and
  a misspelt case would still bind. The set is also a whole-program
  question asked of a name that one type answers.
- **Refuse a binding named like any case.** That is rustc's lint (E0170)
  turned into an error. A misspelling no case resembles still binds.
- **Resolve every bare name in the scrutinee's type first, then bind.**
  `Circel` against `Shape` still binds, since `Shape` lacks it.

## Acceptance

- **`compiler/pw-core/tests/case_names.rs`**, 6 tests:
  - a misspelt case refused, the correct spelling and a lowercase binding
    clean;
  - `Pair(Draft, Draft)` clean;
  - a lowercase case refused with PW0625, and its capitalized control clean;
  - `true` and `false` as `Bool`'s cases, and one missing reported;
  - `None` against a `Shape` refused;
  - `{:Some(Draft)}` refused as no arm, and its binding control clean.
- **`compiler/pw-core/tests/sum_types.rs`**: a case alone through a module,
  `geometry.Shape.Empty`, is a case.
- **Corpus C14:**
  - R-055, a case named in lowercase;
  - R-056, a misspelt case;
  - A-030, cases and bindings told apart by their capital;
  - generality witnesses for `case_name_capitalized` and
    `pattern_constructor`, making generality 41 / 41.
- **No program changes.** All of `pw-syntax`'s and `pw-core`'s tests pass,
  as do the conformance suite's 129, and the store's 66 built artifacts are
  byte-identical to `9e53158`'s.
- **`scripts/case_names_mutations.py`**: 5 mutants (`just e14-case-names`).
- **Re-anchored**, each killed:
  - `match_mutations.py`'s "another type's bare constructor reads as a
    binding";
  - `pattern_mutations.py`'s "a case named alone under another is a
    binding";
  - `template_match_mutations.py`'s "a qualified arm does not parse";
  - `sum_type_mutations.py`'s "a pattern through its type is a binding",
    killed by the module test above;
  - ADR-0196's two;
  - "the rule does not run" in `built_page_mutations.py`,
    `handler_capture_mutations.py` and `key_read_mutations.py`, whose rules no
    longer take the signatures.
