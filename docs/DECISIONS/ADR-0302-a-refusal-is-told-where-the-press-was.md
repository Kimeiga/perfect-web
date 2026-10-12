# ADR-0302: a refusal is told where the press was

Status: accepted under the owner's delegation of 2026-10-02, on the owner's
finding of 2026-10-08 23:30 (using the feed by hand). Date: 2026-10-09.
Milestone: E14. The second of three steps (NEXT, "a refusal is never
silent"); the first hides the feed's composer from a reader signed out, and
the third holds a control to what the page shows its predicate holds.

## Context

- **The owner's finding.** Signed out, a reader types into the feed's
  composer and presses Post. The post shows, then goes, with no word. The
  server refuses the command (`requires SignedIn`, 403) and the speculation
  is taken back, so nothing wrong is kept; the press fails silently, the
  dead button this project exists to remove.
- **What the runtime does with a refusal today** (pw-runtime.mjs):
  `command()` reads a body only when the response is OK; on a 403 it throws
  `command X refused: HTTP 403`, the predicate unread. The handler's catch
  sets `data-pw-handler-error="1"` on the control, which nothing reads, and
  logs. No text, no announcement.
- **What the host sends**: 403 `{"committed":false,"refused":"SignedIn"}`,
  the predicate's name alone; an evaluator's error or an unknown predicate,
  202 `{"committed":false}` with nothing.
- **A resent press loses its refusal** (found reading the path, 2026-10-09):
  for a command `idempotent_by` its interaction, a resend is answered from
  the kept outcome without running `requires` again, and the refusal travels
  only in a thread-local set while `requires` runs. The resend is answered
  202 `{"committed":false}`. `retry transport_only` resends exactly so.
- **Predicates are the deployment's** (ADR-0115): `requires SignedIn,
  OwnsPost(post)` names them; the compiler resolves none; the host gives
  each its meaning. No program says anything a reader could be told.
- **What accessibility requires**:
  - WCAG 2.2 SC 4.1.3 (AA): a message that results from an action, or
    reports an error, without a change of context, is exposed by a role or
    property, so assistive technology announces it without focus.
  - ARIA19 and ARIA22: the live container is present, empty, when the page
    loads; a status is polite, an alert assertive and only for what must
    interrupt. ADR-0182: a live region is the same node from the page's
    first byte, through every change.
  - HTML's `<output>` maps to the status role, but its announcements are
    uneven: Firefox's desktop build announced none in 2019, where it honored
    an explicit `role="status"`, and Safari's VoiceOver speaks its role as
    "output" (Scott O'Hara, "output: HTML's native live region element",
    2019; a note of October 2025 still reports updates skipped). An explicit
    `role="status"` is what the techniques test (ARIA22), and what
    a11ysupport.io's NVDA tests announce.
- **What others do**: Laravel's `Response::deny('…')`, Django's
  `PermissionDenied(msg)` and Action Policy carry the words on the rule;
  Next.js, React Router and SvelteKit return them per action, in the
  author's words; Phoenix flashes a per-rule message in a persistent
  `aria-live` group. No framework checks that every rule a control can
  meet has words.

## Decision

1. **Every predicate a refusal can come from has words**, in two layers:
   - **the deployment's**: the deployment that gives a predicate its meaning
     (ADR-0115) gives it words too: `SignedIn` "Sign in to do this.",
     `OwnsPost` "Only its author can do this.", `MayMessage` "You can't send
     this person a message.";
   - **the program's, where it declares them**, told in their place:

     ```
     predicate SignedIn
         says "Sign in to post, reply, like or follow."

     predicate OwnsPost(post: PostId)
         says "Only its author can delete a post."
     ```

   - **PW0351** holds a declared predicate to its words (one string, not
     empty, with no hole, so it names nothing the refusal read), a
     `requires` that names one to its parameters by count and type, and a
     predicate to one declaration in the program. A predicate the program
     does not declare is the deployment's alone, as before.
   - **A host refuses to serve a program** that requires, or declares, a
     predicate it cannot evaluate, naming it: until now each press of such a
     command was answered 202 with nothing to tell.
2. **`pw build` writes the program's words** (`predicates.json`), and **the
   host answers a refusal 403 with them**: `{"committed":false,
   "refused":"SignedIn","says":"Sign in to post, reply, like or follow."}`,
   the deployment's where the program declares none. Never the arguments,
   nor what the predicate read.
3. **A refusal is kept with its interaction**: a resend of a refused press
   is answered as the press was, 403 and the same words.
4. **The runtime tells it where the press was**:
   - the speculation is taken back as now;
   - the words are written, as text, into a message beside the control
     that was pressed (its next sibling, `class="pw-refusal"`), which the
     control's `aria-describedby` names;
   - and said by the page's announcer: one visually hidden, empty element
     with `role="status"` (`pw_render::ANNOUNCER`) that every document
     holds from its first byte, before the runtime, whichever renders it
     (a build's page, a host's page that binds a query, one that binds
     none), so each message is announced (ARIA19, ARIA22), the same node
     through every change (ADR-0182). The runtime adopts it, and adds one
     only where a host served none. Emptied before it is written, so the
     same words said twice are said twice; words told before the last were
     written are not said, the last are.
   - Focus, the control's state and what was typed stay as they are.
   - Pressed again, the control's message is cleared before the press; a
     press that commits clears it. No timer.
   - `data-pw-handler-error` is `refused:SignedIn`, a test's hook, never the
     only word.
5. **Every failed press says so there**, in the platform's words where no
   predicate's apply: a page whose build is out of date, where reading it
   again is not its recovery or has been done already ("This page is out
   of date. Reload it to go on."; a press is never replayed, ADR charter
   §15.6 test 16); a command that could not be reached, its retries spent
   ("This could not be sent. Check the connection and try again."); and
   anything else, a handler that failed to load among them ("This did not
   work. Try again."). `data-pw-handler-error` says which:
   `refused:<predicate>`, `unreachable`, `failed`, or the recovery's name.
6. **The feed declares its three predicates** in its own words, and its
   composer is a signed-in reader's (`hidden={!me.signed_in}`, as W4's
   conversation's is, ADR-0279), so the owner's press cannot be made; a tab
   whose reader signed out elsewhere is refused and told.

## Acceptance

`just e14-refusal` records each (docs/evidence/E14/refusal.txt).

- Compiler (`tests/predicates.rs`): a predicate that says nothing, says two
  strings, empty words or words with a hole, refused; a `requires` of a
  declared predicate with too few or too many arguments, or one of another
  type, refused; `says` on anything but a predicate refused; a predicate the
  program does not declare, or imports, accepted.
- The build's page (`runtime/pw-render/tests/titles.rs`): a page that
  ships the runtime holds the announcer once, before it; a page that ships
  none holds none. Not the browser's to test: the development server renders
  `/StorePage.html` itself, and the build's file is never served there (the
  first run of the mutation controls found its mutant alive).
- Host (`tests/sign_in.rs`, and a page that binds no query): 403 with the
  program's words, and the deployment's where it declares none; a resend of
  a refused press answered the same; a host that cannot evaluate a
  predicate the program requires or declares refuses to start; each kind
  of document holds the announcer once, empty, before its runtime.
- Browser, three engines (`e2e/identity.spec.mjs`): signed out, the feed
  gives no composer; Like, pressed from the keyboard, is refused and its
  words shown beside it, named by its `aria-describedby`, said by the
  announcer; the focus stays and the count is the server's; pressed again,
  the message and the announcer are emptied and told again; a tab whose
  reader signed out in another is told so at its next post, the draft
  kept; a press no answer comes for says so; the page's announcer is in it
  from the first byte. `e2e/recovery.spec.mjs`: a page still stale says
  so; `e2e/lazy-handler.spec.mjs`: a handler that fails to load says so;
  `e2e/accessibility.spec.mjs`: the announcer is in the page a build
  renders and in the page a host does, and it is the page's fourth live
  region, silent where no press failed.
- Mutation controls (`scripts/refusal_mutations.py`): 30 mutants, of the
  checker, the build, the host, the runtime and the feed.

## Consequences

- **A page has one more status region**: a test that asked for "the"
  status of the store's page now names the cart's notice (`#cart-notice`).

## Not claimed

- **Words in one language**: a program has one; a translated program keys
  them by predicate (no i18n yet, ADR-0195).
- **Several predicates failing**: the host stops at the first, in declared
  order (`authorize_export`), and that one is told.
- **A predicate over another's data** may tell, by refusing, that the data
  is there; answering such a refusal as not found is a later option of the
  declaration.
- **Placement**: the message is the control's next sibling; an author
  cannot yet put it elsewhere.
- **No request without the runtime**: commands are JSON requests; a page
  with no script has no command to refuse.

## Alternatives

- **Words per command** (`requires SignedIn else "…"`): the owner asked for
  the predicate's own words, and a predicate guards many commands alike.
- **Words in the deployment alone**: they are general ("Sign in to do
  this."), and a program's copy is in its language and says what the reader
  was doing; so the program may say its own.
- **Words the program must declare for every predicate**: every `requires`
  in the corpus, its generality programs and its history would gain a
  declaration, and the history is kept unchanged (CORPUS.md); the
  deployment's words already tell every refusal.
- **Words the page holds**: a page served before a predicate was added
  could not know it; the host that refused sends them.
- **A message region the compiler writes beside each guarded control**: it
  shifts every such control's layout before any refusal; the message is
  made where a press was refused, and the announcer, which must be there
  first, is there from the first byte.
- **One page-wide region alone**: it is announced but not seen where the
  press was; the message beside the control is.
- **An alert**: assertive interrupts; a refusal of the reader's own press is
  a status (ARIA, "only for what must interrupt").
- **`<output>`**: its implicit status role is announced unevenly (Context);
  an explicit role is announced where it is.
