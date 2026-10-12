# ADR-0243: a block's statements are separated, by `;` or a line

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0237.

## Context

- **A block's statements needed no separator.** The block's loop read an
  expression, took a `,` or a `;` if one was there, and read the next. So
  `fn f(a: Int, b: Int) -> Int !{} { a b }` was two statements, `a`
  evaluated and dropped, and checked: its value was `b`. A value meant as
  one expression and written as two was a binding and a dropped statement:
  `let x = f y` bound `f`.
- **A block also keeps some constructs as statements on one line, and its
  readers read them as one.** A first cut of this rule, which refused any two
  statements on one line, refused 171 lines of the repository's programs,
  each of them meant:
  - a `return` and its value, `return Err(e)`: the grammar has no `return`
    expression, and the checker reads the statement after `return` as its
    value (ADR-0038);
  - a clause's head and its value, `scope component`, `identity
    content_address(code, captures)`: the names check reads a head the
    policy table knows and what follows it on its line as one clause
    (ADR-0047), and the analyses read the pair (`check::name_pair`);
  - a block after what precedes it on its line, `view { .. }`,
    `acquire { .. }`, `release(h) { .. }`, `task.spawn(detached) { .. }`:
    the block is that statement's.
- **Whether a word is a clause's head is the names check's to say.** A name
  a scope binds is a use, whatever its spelling, and a function's, a
  query's, a command's or a task's body admits no clause (ADR-0216). The
  grammar cannot see either.
- **A grammar test held a misparse for an operand.**
  `a_query_body_may_begin_with_an_operand` asserted that `items[0]` in a
  query's body parsed. It did: the grammar has no index, so it was `items`
  and then a list, `[0]`, the query's value.
- **The names check read a clause's value to its line's end, through a
  `;`.** So `scope component; nothing_here` in a resource's block read
  `nothing_here` as `scope`'s value, and nothing resolved it: it checked,
  where on a line of its own it was PW0021.

## Research

- **Swift refuses two statements on a line**: its
  `statement_same_line_without_semi` is "consecutive statements on a line
  must be separated by ';'"
  ([DiagnosticsParse.def](https://github.com/swiftlang/swift/blob/main/include/swift/AST/DiagnosticsParse.def)).
- **Go ends a statement with its line**: a semicolon is inserted after a
  line's final token when it is an identifier, a literal, one of `break`,
  `continue`, `fallthrough` or `return`, or one of `++`, `--`, `)`, `]` and
  `}` ([the spec, "Semicolons"](https://go.dev/ref/spec#Semicolons)).
- **Kotlin separates statements by `semis`**, a `;` or a newline,
  `statements : (statement (semis statement)*)? semis?`, and a lambda after
  a call is the call's: `callSuffix : typeArguments? (valueArguments?
  annotatedLambda | valueArguments)`
  ([KotlinParser.g4](https://github.com/Kotlin/kotlin-spec/blob/release/grammar/src/main/antlr/KotlinParser.g4)).
  A block after a statement on its line is read the same way here.

## Decision

1. **Two statements on one line are separated by `;`** (PW0030
   `statements_separated`, its invariant "a block's statements are separated
   by `;` or a line"): "two statements on one line are separated by `;`", at
   the second, with the repair "write `;` between them, or the second on a
   line of its own". A `,` separates too, as before. The repair is advisory:
   whether `;` or a call was meant is the program's to say.
2. **What a block's readers read as one is not two**, and the grammar knows
   each by its shape:
   - a block, after whatever precedes it on its line;
   - the one statement after a `return` alone;
   - the rest of the line after a clause's head alone
     (`pw_syntax::heads_a_clause`, a policy's or a statement clause's
     head), up to a block, which is the clause's body.

   A word's path is not the word: `cache.x y` is two statements.
3. **A clause's head that is no clause where it is written is refused by the
   names check**, which says what it is: a name some scope binds, or a word
   in a code body. What follows it on its line, unless a separator, a line or
   a block comes between, is PW0030 at that statement, with the head
   related: "`key` is no clause's head here".
4. **A `;` ends a clause's value**, as it ends a statement, in the names
   check as in the grammar. A `,` does not: a list is one value,
   `placement browser, edge`.
5. **The second is read**, as rustc reads a missing separator (ADR-0237): the
   block means what was written, and nothing after fails for it.
6. **The grammar and the names check agree on what heads a clause, by
   test**: `every_clause_head_takes_its_value_on_its_line` holds that every
   head the policy table gives a domain, other than a code body's, heads a
   clause in the grammar. The table's two tests read one list of its heads
   now, and that list has the four ADR-0207 added and it lacked: `holds`,
   `transactions`, `reads` and `changes`.
7. **PW0030, not PW0029**: PW002x is name resolution's, and PW001x is full.

## Acceptance

- **`pw-syntax`'s unit test**: each of `a b`, `let x = a x`, `return a b`,
  `cache.a b`, `acquire { a } b`, `release(a) { a } b` and a second
  statement on the line after a clause is one error, at the second; a `;`, a
  `,`, a line and a record's fields separate; and `return Err(a)`, `scope
  component`, `respects prefers_reduced_motion`, `impact layout_write when
  LayoutAffect`, `view { a }`, `acquire { a }`, `release(a) { a }` and
  `task.spawn(detached) { a }` parse clean.
- **`compiler/pw-core/tests/statements_separated.rs`, 4 tests**, each with
  controls:
  - the refusal through `pw check`, at the second, with its repair;
  - a `return` takes the one statement after it;
  - A-019, as the corpus writes it, checks; after `scope component; `, a
    name is resolved, and a list after `placement` is one value; and a
    statement after a block on its line is refused;
  - a binding named `key`, and `scope application` in a function's body,
    are no clauses, and what follows each on its line is refused.
- **`policy.rs`**: every clause head takes its value on its line.
- **The grammar's query-body test** holds `items[0]` refused now, PW0030 at
  its `[`, and not an unknown policy `items`.
- **The repository's programs**: every `.pw` file in it, checked alone,
  reports no PW0030 but one: the C8 text of R-026, kept in
  `examples/history/`, writes `placement edge` in a function's body, which
  ADR-0216 made a pair of names that do not resolve. It is two statements,
  and is said to be; the history test asks only that the text is still
  caught for its own invariant, and it is.
- **`scripts/statements_separated_mutations.py`: 27 mutants**, recorded by
  `just e14-statements-separated`, 18 in the grammar and 9 in the names
  check, against their tests, the corpus's and the names check's own.
- **Run whole, near what this changed**: `names_mutations.py` 20 of 20
  killed, `bind` 14 of 14, `clause_heads` 5 of 5, `control_flow` 13 of 13,
  `provide` 29 of 29, `read_whole` 18 of 18.
- **The workspace, 2,154 tests**: 2,153 at the full check, and the
  query-body test after it, which had held `items[0]` an operand.
- **The browser suite, 782 in three engines**: 781 at the full check; the
  one other, a press on the cart's page in WebKit, read the page again
  before the server had answered, and passed 6 of 6 run again. Its race,
  and the same one in `optimistic.spec.mjs`, are awaited in the commit
  after this.

## Not claimed

- **A clause, a `return` and a block after a statement are statements
  still**, as their readers read them. The grammar knowing each as one node
  is the policy module's "`PolicyExpr` split", and ADR-0038's open ruling
  on a `return` expression. Either changes the HIR every reader walks, and
  is later. Until then the grammar's tables and the policy table are kept
  in step by test.
- **An index has no message of its own.** `items[0]` is refused as two
  statements in a block, and as what an argument list or a hole reads
  elsewhere. Read as an index and refused by name, `List.get(items, 0)`, it
  would say what it is; listed in NEXT under 0047-a.
- **A block after anything** is that statement's, as written: `x + 1 { .. }`
  is not refused. What reads such a block is the reader's to say.
- **What a clause written in a block holds is not judged by its domain**,
  found writing this. `scope bogus` and `scope component page` in a
  resource's block check, as do `respects bogus` in an `animate` block,
  `intrinsic_height bogus` in a `subtree`'s, and `captures bogus`, `load
  bogus` and `on_version_mismatch bogus` in a `handler_policy`.
  KNOWN_LIMITATIONS said such clauses were read by the scope graph and the
  handler rules. The scope graph reads `scope` and refuses no word it
  lacks, and nothing reads the rest: no analysis, generator or runtime
  names `respects`, `intrinsic_height` or a `handler_policy`. Its own ADR,
  next.

## Amended, 2026-10-07: what a parse error already says

Found probing ADR-0247's `let _` and an index: after a statement the parser
could not read, the refusal piled onto its error. `let _ = r()`, before
0099-a, was PW0001 at `_` and then PW0030 at `_`; `g(items[0])` was PW0010,
then PW0030 at `[` and at `)`, then PW0009. After an error, where the
statement ended is the recovery's guess, and a second statement on its line
is the error's, not the program's.

- **No PW0030 after a statement read with an error**: the grammar counts
  its errors across each statement, and the next is taken as separated.
- **No PW0030 at a token no expression begins with**: the expression
  parser refuses it (PW0009), and it is no statement.

The grammar's unit test holds `g(a[0])` to PW0010 and PW0009 alone, and
`scripts/statements_separated_mutations.py` has a mutant for each, 29 in
all.

## Amended, 2026-10-09: markup written on one line is one region, and no operand

Found writing the layouts ADR's refusals (`<h1>Your cart</h1><slot />` was
PW0009): two roots on one line at a view's top did not parse.
`<h1>a</h1><p>b</p>` was PW0009 at the second element's `/`, and
`<h1>a</h1><hr />` at its `>`, while inside an element both were markup. The
template region ended after its first root, and the next `<` was read as a
comparison with it. Probing that, `<p>a</p> + 1` checked with nothing said:
the paragraph rendered, and the sum was dropped.

- **Markup written on one line after a root goes on as one region** when
  what follows begins as markup does: an element, a comment, or a block's
  marker. The space between two roots on the line is text, as it is between
  two elements inside one (`<span>a</span> <span>b</span>`). A root on the
  next line is a statement of its own, as before, and anything else after
  markup on its line is the expression it begins.
- **Markup is no operand** (PW5047): an arithmetic, comparison or logical
  operator over markup is refused where it is written. A value is shown
  inside markup, `<p>{a + 1}</p>`.

The grammar's unit test holds each form and its controls, the checker's
(`tests/statements_separated.rs`) that each checks and that an operator over
markup is PW5047, and `scripts/statements_separated_mutations.py` has three
more mutants, 32 in all.
