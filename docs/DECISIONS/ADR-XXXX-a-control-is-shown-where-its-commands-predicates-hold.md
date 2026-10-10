# ADR-XXXX: a control is shown where its command's predicates hold

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-10. Milestone: E14. The third step of NEXT's "a refusal is never
silent" (the owner's finding, using the feed by hand, relayed 2026-10-08):
"the compiler holds what the page can tell: a control whose handler sends a
command `requires P` is rendered only where the page shows `P` holds ... or
it is refused. Ruled with research, the witness's form first." ADR-0302 told
a refusal where the press was; this keeps the press from being offered where
the page can already tell it would be refused.

## Context

- **What a page could tell, it did not ask.** The feed guarded its composer
  with `me.signed_in`, a field of its own `Viewer`, which the deployment's
  `feed:data/users#viewer` computes as "the session has a principal": the
  same fact `requires SignedIn` asks, computed a second time, and agreeing
  only because one implementation wrote both. Its Like, follow, reply and
  mark-as-read controls had no guard at all: signed out, each was a press
  the server refused.
- **The research** (2026-10-10): Pundit's views ask `policy(@post).update?`
  and its controllers `authorize @post`, one policy method; CASL's `<Can>`
  evaluates the rules the server enforces (`packRules`); Cedar's "UI
  filtering" asks the same authorizer which actions to show. The rule they
  share: the button and the action ask one policy, the hidden button is the
  interface's courtesy, and the server's check stays. None checks
  statically that a control is shown only where its action's policy holds
  (UrFlow verifies policies written as queries; Ur/Web types forms against
  their handlers): the rule here is the compiler's.

## Decision

1. **A page asks a predicate of its reader by naming it**: `{#if SignedIn}`,
   `hidden={!SignedIn}`. A declared predicate with no parameters, named in a
   page's, a view's or a layout's markup, is a `Bool` the deployment answers
   for the document's principal, as it answers `requires` (`Identity::
   answers`, the one evaluator). The template reads it as `SignedIn~holds`,
   a name no source writes; the plan names what the page asks
   (`predicates`); the host answers each when it reads the document's
   values, and a value computed from an answer is computed by the host from
   it; the document carries the answers to the browser (`holds`), which
   renders a speculated region with them. Not a witness the program
   declares (`me.signed_in`): a second fact that can disagree with the
   first, as a deployment of guests, where every session is `SignedIn`,
   shows.
2. **A control whose handler sends a command that `requires` a predicate
   with no parameters stands where the page asks it and it holds**
   (PW5048): in the first branch of an `{#if}` whose condition has it as a
   conjunct (`&`), or under `hidden={!P}` (a disjunct of `|`) on the
   control or an element around it. An `{:else}` asserts nothing of the
   condition; neither does a disjunction's side. A view's control is held
   in the view's own markup.
3. **`|refusable`**, an event modifier the runtime does nothing for: the
   control is shown to every reader and its refusal told (ADR-0302). For a
   control no reader's answer can decide: the store's Add, in a menu every
   reader is served alike (E7-P), and every control of a program that asks
   no predicate it requires (one the deployment alone declares). So no
   press is speculated that the page knows will be refused: a guarded
   control is shown only where its predicate held when the page was
   rendered, and a `|refusable` one is a press the page cannot judge,
   speculated as any is and, if refused, taken back and told (ADR-0302).
4. **A name a template reads is a value** (PW0629): a binding, a parameter,
   a signal, a constant, a function or a command, a case, a resource a
   `resource={..}` mounts, or a predicate the page asks. A type, a page, a
   view, a query, an event and a predicate with parameters are not; a
   handler asks no predicate (it runs in the browser, after the document).
5. **A page that asks its reader anything is its reader's** (PW5049): a
   page served to everyone asks none.

## Found

1. **A name that holds no value checked and built, and the page failed
   where it was served**: `{#if InteractionId}` (a type), `{#if P}` (a page)
   and `{#if SignedIn}` passed `pw check` and `pw build`, the plan naming
   nothing, and the renderer answered "no value for `SignedIn`". Only a name
   of a function's type was caught (PW0609).
2. **The feed showed fourteen controls a signed-out reader's press was
   refused for**: Like (three pages), Delete (two), the composer and its
   remove-image, reply, follow, unfollow, mark all read, a conversation's
   mark-read and its composer. Each is now shown where `SignedIn` holds.
3. **A block decided by a value fixed for the document is never rendered
   again**, so a count inside one is stale: moving the home page's reader
   block (`{waiting} unread`) from `me.signed_in` to `SignedIn` lost its
   telling (the host's notification and message tests found it). Only the
   controls are guarded by the predicate; the blocks that show live values
   keep the query that decides them.

## Alternatives

- **A witness the program declares** (`predicate SignedIn witness
  me.signed_in`), NEXT's first wording: the checker could not know the
  witness agrees with the deployment, and the frameworks above ask one
  policy for both.
- **Hiding by the runtime**, the browser asking the host before showing a
  control: a round trip per control, and nothing the compiler can hold.
- **A predicate with parameters asked of each row** (`{#if
  OwnsPost(post.id)}`): a read for every row through the deployment's
  evaluator, the feed's Delete fifty reads a page. The next step, ruled with
  how a deployment answers many at once.

## Not claimed

- **A predicate with parameters is not asked yet** (PW0629): a control whose
  command requires one (`OwnsPost(post)`, `MayMessage(to)`) is not held to
  where it stands; the feed's Delete is shown where the program's own
  `mine` says.
- **A view's control is held in the view's own markup**: a page's `{#if}`
  around a view's use does not count.
- **An answer is the document's for its life**: a reader signed out in
  another tab is told at the next press (ADR-0302); documents rendered again
  when a principal changes are W8's telling by principal.
- **A computed value from an answer and another value** is one value's
  (ruling 0073-a): nested `{#if}`s and an element's `hidden` each read one.

## Acceptance

`just e14-holds` (docs/evidence/E14/holds.txt):

- `compiler/pw-core/tests/controls_where_they_hold.rs`: a control under its
  predicate, by `{#if}`, a conjunct, nesting and `hidden={!P}`, accepted;
  one with no guard, in an `{:else}`, under `hidden={P}` or a disjunction,
  refused, and a command that requires nothing needing none; a type's, a
  page's and a parameterised predicate's name refused where a template
  reads it, a predicate's answered; a handler asking one refused; a page
  served to everyone asking refused; `|refusable` shown to each; an answer
  a `Bool`; the plan naming `SignedIn` and computing from its answer;
- the host: a post shown without its Like, and the composer hidden, to a
  reader signed out, and both to one signed in, with the answer carried in
  the document (`a_control_is_shown_where_its_commands_predicate_holds`);
  the sign-in tests over a feed that asks nothing (`|refusable`) and one
  whose command requires a predicate the deployment cannot evaluate;
- the feed and the store in three engines;
- `scripts/holds_mutations.py`: 21 mutants.

The benchmark's store (ADR-0156) changes as ADR-0159 changed it: its two
controls are `|refusable`, the least that keeps it a program the compiler
accepts.
