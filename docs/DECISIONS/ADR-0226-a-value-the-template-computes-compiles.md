# ADR-0226: a value the template computes compiles

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0073-a (ADR-0210), which the feed reference app's "likes and counts" needs
(NEXT 24): the host's part. Date: 2026-10-05. Milestone: E14. Amends
ADR-0073, whose refusal it lifts for a text hole and an attribute's value.

## Context

- **A template read each value by path** (ADR-0073): a name, or fields read
  from one.
  - A computed hole, `{n + 1}`, `{counted(List.length(thread.replies), ..)}`
    or `disabled={!item.available}`, checked and did not build: "a template
    hole is read by path, and this is a call".
  - The charter's own sketch writes `disabled={!item.available}` (§8.1), and
    every template system allows it.
- **Ruling 0073-a**: "A computed hole compiles. The compiler lifts each into
  a pure derived function of its inputs, computed by the host per render and
  by the browser's pure emitter (ADR-0044) when the inputs are signals or
  speculated values. The renderer reads it by a compiler-named path. The
  build refusal stays until then."
- **The feed needs one.** A thread page shows how many replies and likes a
  post has, "1 reply · 2 likes", and a count in words is a function of the
  thread.

## Research

- **Vue** accepts an expression "inside text interpolations" and in a
  directive's value. "Functions called inside binding expressions will be
  called every time the component updates, so they should **not** have any
  side effects" ([template
  syntax](https://vuejs.org/guide/essentials/template-syntax.html)).
- **React**: a component must be idempotent; `const time = new Date()` in
  render is marked "Bad: always returns a different result!" ([Components
  and Hooks must be
  pure](https://react.dev/reference/rules/components-and-hooks-must-be-pure)).
- **Phoenix LiveView** is the closest model to a host that patches.
  - "If the `@title` assign changes, then LiveView will execute the dynamic
    parts of the template, `expand_title(@title)`, and send the new content.
    If `@title` is the same, nothing is executed and nothing is sent."
  - "Data loading should never happen inside the template" ([assigns and
    HEEx](https://phoenix-live-view.hexdocs.pm/assigns-eex.html)).
- **So a computed value is a pure function of what it reads.** It is run
  when the page renders, and again when what it reads changes. An effect in
  one would happen at those times, and not when it should: the time shown
  would be that of whatever changed last.

## Decision

1. **A text hole, and an attribute's whole value, may compute**, at the top
   of a page, from one query's value.
   - The compiler lifts the expression into a function of that value, by
     the name its expression reads it: `derived$1`. No declaration has a `$`
     in its name.
   - The function is a component of its own, `feed.app.PostPage.derived_1`,
     with a contract of kind `derived`. It needs no capability, imports
     nothing, and is placed where its page is.
   - The template reads its value by a path the compiler names,
     `#feed.app.PostPage~1`, which no source can write.
   - The page's plan names the binding it reads and the steps to its value,
     the last step the function: `{"derived": "feed.app.PostPage.derived_1"}`.
     A computed attribute is planned beside the parts, and set again as any
     attribute that reads a query's value is (ADR-0171).
   - A view composed in the page computes as the page does: its parameter is
     the page's value (ADR-0136).
2. **The host computes it** when it renders the page, and again when the
   value it reads changes. The page is sent the text, or the attribute,
   that changed, and only that. With scripts off, the page shows it.
3. **Its type is the checker's.**
   - The value relations' typer types the expression, a comparison or an
     `if` as much as a call (ADR-0074).
   - Its WIT is that type: `derived4: func(arg0: t-post) -> bool` for a
     boolean attribute.
4. **It performs nothing: PW0334, revision 2.** A `derived` value's rule now
   covers a value a template computes, anywhere in a template, an effect
   reached through a function named as a value included.
   - **An escape hatch's effect is its audit record's to decide** (PW5010),
     as before.
   - **A page placed at build is exempt.** It renders once, and what it
     reads is its build's input (A-013, `include_markdown`).
   - **An effect a page may not perform at all is said once**, as that
     (PW0401).
   - The build refuses whatever is left: "a host computes only a value that
     performs nothing".
5. **What a host does not compute yet is refused at build, by name**:
   - a value in a block, a loop's row or an arm, or inside a block a signal
     decides;
   - one from several values, or from none;
   - one from a signal, which the browser computes (ADR-0227);
   - one from a value the page speculates on, which the browser would show
     beside a list that shows the speculation (ADR-0227);
   - one from a page's parameter;
   - one in a view that contains itself, an instance made at run time
     (ADR-0228);
   - a computed hole in an attribute's text. Its repair, the attribute's
     whole value, builds.
6. **The feed's thread page shows its counts**:
   `{counted(List.length(thread.replies), "reply", "replies")} ·
   {counted(thread.likes, "like", "likes")}`. Another session's like reaches
   an open thread as its new count.
7. **Corpus C16**:
   - A-032: a page computing a count in words, a `class` and a `disabled`;
   - R-058: "3m ago" computed from the clock (PW0334);
   - `derived_not_pure`'s witnesses: a view's attribute computed through a
     helper that reads the clock, and its valid neighbour;
   - `forbidden_effect`'s valid neighbour, which read the clock in its
     template, reads it in the page's body now.

   Generality is 43 / 43.

## Found

- **A member an imported module lacks resolved.** The qualified call check
  counted an import's own name as a local. So `String.nope(s)` checked clean
  wherever `String` was imported, and the build failed later, or never ran.
  - A-032's first draft called `String.concat`, which no module declares.
    The corpus's ownership test caught it; `pw check` did not.
  - An import binds nothing by its name now, as the name check already read
    it.
  - Two corpus files called a member that does not exist:
    - `task_detached`'s nested witness called `List.each`. It is `List.map`
      now.
    - The cast rule's clean control gave its decoder to `decode.run`. It is
      a decoder function now, as `decode` says decoders are.
  - Their old texts are in `examples/history/C16/`.
  - An optimistic clause's name check had resolved a module, `Carts` in
    `Carts.with_line(..)`, through the same import. It reads a module a
    path starts with as that now.
- **An effectful platform function is compiled code.** `clock.now` answers
  `0` and `include_markdown` `""`: the host's import, or the build's, gives
  the real one. Lifted into a host's function, each would have answered its
  stub. So the build checks what the value performs, with the inference the
  contracts use.
- **A character the lexer has no rule for was `Unknown` between tags.**
  A-032's "1 reply · 2 likes" failed the corpus's lint: `·` was an unknown
  token in the tree the compiler reads, as `—` or an emoji would be. HTML
  reads any character between tags as text. The parser reads it again as
  text there, as ADR-0167 does a `//`, and in code it stays `Unknown`. The
  lint asks the tree now, not the lexer, which does not know markup.
- **`Types` types a name, a field and a call.** `List.length(xs) == 0`, a
  literal or an `if` had no type there, and the first build of A-032 said
  "part 4's value has no type". The value relations' typer is the checker's,
  and types each.

## Acceptance

- **`compiler/pw-core/tests/computed_holes.rs`, 8 tests:**
  - the feed's two counts lifted, planned, typed and compiled with no
    import, and read by their paths; the home page lifts nothing;
  - a computed `class` and `disabled` planned and set again, typed `string`
    and `bool`, with their controls by path;
  - a view composed in the page, given the page's value whole and a field of
    it, each function taking what its view names;
  - each refusal of what a host does not compute, by name, with controls,
    one a lambda whose names are its own;
  - a view that contains itself, and a value the page speculates on;
  - PW0334 in a hole, an attribute and through a function named as a value,
    PW0401 said once, the escape hatch's PW5010, and controls;
  - A-013's build input refused at build and clean at check;
  - a member an imported module lacks, in a body, a template and an
    optimistic clause, and the store's clauses clean.
- **ADR-0073's `template_values.rs`** states what builds now and what does
  not. ADR-0078's `effects_through_values.rs` calls its helper in a `let`,
  where PW0400 is the rule, not in a hole.
- **The development server's tests:**
  - `a_value_the_template_computes_is_the_hosts_and_sent_when_it_changes`:
    three threads' counts, and a like's new count sent alone;
  - `an_attributes_computed_value_is_the_hosts_and_set_again`: a `title` and
    a `hidden` written, and the title set again.
- **`e2e/feed.spec.mjs`, in three engines**: the counts shown with scripts
  off, and another session's like reaching an open thread without a reload.
- **The corpus at C16**: `corpus-check`, `checking_source.rs`,
  `generality.rs` (43 / 43), `corpus_history.rs` (the three old texts) and
  `rule_fixtures.rs`. `pw-syntax`'s `corpus_lossless.rs` asks the tree for an
  unknown token, with a control: `·` is one in code, and text between tags.
  `pw-resource`'s count of the accepted corpus's resources is 10.
- **`scripts/computed_holes_mutations.py`: 26 mutants**, recorded by `just
  e14-computed-holes`. Three older mutants are re-anchored:
  - `optimistic_posts_mutations.py`'s unnamed key;
  - `row_reads_mutations.py`'s attribute that is no read;
  - `template_value_mutations.py`'s computed attribute, which lowers with an
    empty path where it was refused.
- **The workspace, 2,075 tests; the browser suite, 761 in three engines.**

## Not claimed

- **The browser's part** (ADR-0227): a value computed from a signal, such as
  a draft's characters left or a button `disabled` while it is empty, and
  one computed from a value the page speculates on.
- **Where else, and from what else** (ADR-0228):
  - a value in a block, a loop's row, an arm or a view that contains itself;
  - a condition, `{#if n > 0}`, which ruling 0071-a's repair needs;
  - a value from several values, from none, or from a page's parameter;
  - a hole in an attribute's text.
- **"3m ago".** It needs the time as a value the page reads, a query's or a
  signal's, and a value computed from two.
- **A `<select>` bound to a signal**, whose options a computed attribute
  marks `selected` in a row (ADR-0221).
