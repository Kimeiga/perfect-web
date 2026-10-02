# ADR-0129: a value's label follows it through calls, bodies, branches and assignments

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (E14-L); charter §7.8. Settles the open ruling of ADR-0085,
whose cache half ADR-0128 settled.

## Context

ADR-0128 made the cache rules read everything a declaration observes. The
sinks read a *value's* label, from `labels.rs`, and ADR-0128's probes found
it lost in five ways. A sixth turned up writing this one. At `ea65917` each
of these checks clean:

1. **A parameter that states a label keeps it out of the result** (ADR-0085's
   contract). `fn mine(s: Session<SessionId>) -> String { "{s}" }` returns a
   public string, and so does `Carts.current(current_session())`. A body
   given a secret by a parameter that states it returns it in a `String`,
   and the caller logs it publicly.
2. **A call's value is its callee's signature.** A helper that reads
   `secrets.payments()` and returns it interpolated is public to its caller.
   ADR-0118 followed reads for the coarse label the cache rules read, not for
   values.
3. **A branch's condition is not joined.** `if token == token { "yes" } else
   { "no" }` is public.
4. **Neither is a sink's.** `if token == token { log.public("yes") }`
   passes.
5. **An assigned binding keeps its `let`'s label.** `let mut shown = "none"`
   then `shown = "{token}"`, logged publicly, passes.
6. **`log<Public>` refuses only a secret.** A session id logged publicly
   passes, though a public log's contents "may be read by anyone"
   (`packages/pw-platform-web/log.pw`).
7. **PW5003 reads only a label's first restriction.** A value that is both a
   session's and a secret is ordered `Session` first, and renders into markup
   with nothing reported.

## Decision

1. **An argument's label comes out of the call**, whatever its parameter
   states (Jif's default). A label a parameter states bounds what it
   accepts; it does not stop the value flowing on.
2. **Except a secret the parameter states.** That is a key the call uses,
   not a value it holds, and it is left out of the result
   (`Label::given_to`). Only a secret can be a key:
   - a partition (`Session`, `User`, `Organization`, `Device`) says whose a
     value is, and what is made from a session's id is that session's;
   - a secret is "a capability's secret material" (`capability.pw`): using
     it is what it is for.
3. **A call carries what its callee's body makes** (`check::summaries`):
   - the label of the body's value, and of each failure a `?` in it may
     return, with what each call in it makes, to a fixed point;
   - a host binding has no body, so its signature is its contract, and the
     platform answers for it;
   - so a Pleris body that returns the key it was given, or a secret it
     read, returns a secret. What decides whether a secret's use is a key's
     is the body, not the signature: `Payments.capture(key, ..)`'s receipt
     is public, and `"{key}"` is not.
4. **A branch carries its condition**, and so does everything it decides
   (charter §7.8: "joins through data flow", with conditions as data):
   - `if` and `match` values join their condition's label;
   - each expression has the conditions it runs under: the `if` conditions,
     `match` subjects and loop collections that enclose it, a short-circuit's
     left operand, a lambda's driving arguments, and each earlier `?` in its
     block that could have returned instead;
   - a sink reached under a non-public condition is refused: reaching it
     tells the condition.
5. **An assigned binding carries what it is assigned**, and the conditions
   the assignment runs under. `if secret { x = "a" }` makes `x` depend on
   `secret`.
6. **`log<Public>` takes only a public value.** A session's, a user's or a
   device's value is refused there as a secret is. The message names the
   value's whole label.
7. **PW5003 reads every restriction**, and runs wherever there is markup:
   a parameter can bring a secret that nothing the body calls declares.

## Alternatives

- **Label polymorphism written in signatures** (Jif's `{a}`, Paragon's
  read effects). Rejected for now. Inference over bodies gives Pleris
  functions the same precision without new syntax. A signature must speak
  only where there is no body: a host binding. There the rule "a secret it
  states is a key, everything else flows" covers every binding in the
  corpus. Revisit when a host binding must say its result does *not* depend
  on a partition argument.
- **An audited `declassify(e, to: L) because "…"`.** Deferred, not rejected.
  - No program in the corpus lowers a value's label. ADR-0085's digest and
    masked number have no instance.
  - Until one does, a label is lowered only where Flow Caml lowers one: at a
    foreign implementation. Here that is a host binding, whose contract
    records its owner (ADR-0123).
  - The first instance takes the charter's escape-hatch form: the `unsafe`
    namespace, a justification, and a semantic-diff entry (§14 M5 task 6).
- **The coarse label for values** (LIO's floating label, ADR-0128's
  `observed`). Rejected for values. Every helper's result would carry
  everything the helper read, including a session it logged or a permission
  it checked. The coarse label is right for decisions over a whole result:
  caches and placement.
- **Leave conditions out** (ADR-0064's "a label is a value's"). Rejected.
  `if secret { "a" } else { "b" }` is the secret, one bit at a time, and the
  charter asks for a conservative system.

## Consequences

- No accepted, store or kiokun program gains a diagnostic. No rejected
  fixture's diagnostics change. The generality and rule neighbours of the
  public-log rule log only public values, and still check clean.
- The checker computes each callable declaration's summary to a fixed point
  before the unit rules run. The generality suite takes 18.8 s against about
  17 s before, and checking the accepted corpus takes 0.2 s either way.
- `resume::check` and `Labels::of_decl` take the summaries; a caller with
  none passes an empty map and reads each callee by its signature, as
  before.

## Acceptance

- **`compiler/pw-core/tests/labels_follow_values.rs`**, 6 tests. Each case
  fails at `ea65917` and has a control:
  - an argument through a stated partition, through a host binding, and a
    secret returned by a body (control: a key a call uses);
  - a helper's body, directly and through a relay (control: a helper that
    reads and does not return);
  - branches: an `if` and a `match` value, a sink in a branch, a `?`, and a
    lambda (control: a branch on a public value);
  - an assignment (control: a public value assigned);
  - a public log of a session id (control: `log.private`);
  - PW5003 with a session's value beside a secret (control: the session's
    value alone).
- **PW5003 blames the name that brings the secret.** In `"{s}{key}"` it
  named `s`, the first labelled name.
- **Mutation controls:** `scripts/value_labels_mutations.py`, `just
  e14-value-labels`, 12 mutants, one per piece above. Evidence:
  `docs/evidence/E14/value-labels.txt`.
- **Earlier controls, re-anchored** to the lines this rewrote, each still
  killed: `label_call_mutations.py`'s "a declared call's result keeps its
  contract alone", `label_plain_mutations.py`'s "a declared call carries
  nothing it is given", and `label_value_mutations.py`'s "a module's
  function named as a value is public".
- **One retired:** `label_plain_mutations.py`'s "a parameter that states a
  label carries its argument too". A stated partition carries by rule now,
  and that a stated secret does not is the new "a key a parameter states
  comes out".

## Not claimed

- **A write is not a sink.** A database write inside a secret branch stores
  what the branch decided. The charter's sinks are serialization, logging,
  placement and caching (§7.8), and a stored value's label is not tracked
  through storage.
- **A function value called later.** A lambda given straight to a call runs
  under its driving arguments. One bound to a name and called elsewhere
  carries what it computes (ADR-0079), not where it is called.
- **Timing.** How long a branch takes is not a label.
