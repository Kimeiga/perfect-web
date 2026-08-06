# E7V — Resume-version compatibility

**Status: COMPLETE.** All ten gate items pass.

## Question

> A resumable handler's captures survive serializability and privacy checks.
> Can the code that receives them still *mean* what it meant when the document
> was rendered?

**Answered: only under exact compatible identities, or an explicit checked
migration.** Everything else fails closed with a typed decision and a recovery
action.

## The three questions, separated

Charter §8.5 asks three independent things of a resumable capture. Passing one
does not imply passing the others, and E7V exists because only two were being
asked.

| | question | where | since |
|---|---|---|---|
| 1 | can this value be encoded? | `pw-core::resume` | E7 |
| 2 | may it cross this boundary? | `pw-core::resume` | E7 |
| 3 | can **this code** interpret it? | `runtime/pw-resume` | E7V |

A `Cart` is perfectly serializable and may not be in a public manifest. A
public string is serializable and unrestricted and may still be handed to a
handler that no longer reads it the same way.

## Identity is content, not a path

`checkout/add_to_cart` survives a rewrite of the function it names. A manifest
trusting that name hands last week's captures to code that reads them
differently, and the failure is **silent**, because both sides agree about the
name.

`HandlerId::derive` takes four separate types, because each answers a different
question: `ImplementationHash` (order-**sensitive** — `charge(); notify()` is
not `notify(); charge()`), `DependencySet` (order-**insensitive** and
deduplicated — the order two references were discovered in is not behavioural,
and hashing it would make an unrelated edit reject every resume in the
application), `SchemaHash`, and `PlatformAbi`.

## The implementation hash is versioned

```text
1  normalized source text                        SUPERSEDED
2  semantic tokens + resolved reference identities
3  canonical typed IR                            not implemented
```

Scheme 1 was correct while E7V was an isolated decision model — it never
accepts behaviourally changed code — and became a product defect the moment
real tabs would depend on resuming, because a formatter run invalidates every
one of them.

**Scheme 2** hashes token kinds, identifier spellings, operators, literal
values, control-flow syntax and statement order, plus the **resolved identity**
of every reference. Whitespace, indentation, comments and source offsets do not
participate. Spans were the other candidate and are worse: one comment near the
top of a file shifts every span below it while changing no behaviour.

Resolved identities are included because `foo()` can name a different
declaration after an import change with identical tokens. E2B's module graph is
what makes that answerable — the same infrastructure that removed by-name
lookups.

Still invalidating, deliberately: local renames, an `if` rewritten as an
equivalent `match`, two reordered independent pure expressions. Those are false
rejections and they are tolerable. Proving them equivalent is an optimizer, and
an optimizer inside an identity system is a worse hazard than an occasional
reload.

**The scheme is carried in the manifest**, because a digest cannot say what
made it and manifests outlive deployments. A manifest from another scheme is
refused as `UnsupportedHashScheme` — *incomparable*, not different — and the
check runs before everything it governs, because an ABI comparison under an
unknown scheme is meaningless.

`the_scheme_2_identity_matrix_holds` is its falsifiable contract: four changes
that must not alter identity, four that must. Without it, "normalized" is a
word rather than a specification.

**Code identity and capture-schema identity are separate.** Two handlers can
compile to identical behaviour while taking different captures; one handler can
keep its capture schema while changing behaviour. Collapsing them makes one of
those undetectable, so `deployment_matrix.rs` tests each direction alone.

## Strict on purpose

No structural-similarity inference. Two records that look alike are not
evidence that one can be read as the other. Compatibility is an exact identity
match, or an explicit migration tied to **both** schema hashes — asserted by
`a_migration_is_tied_to_both_schemas_and_not_reused`, because a migration that
could be applied to a third schema is structural inference with extra steps.

A migration is pure and fallible by construction: `fn(&[u8]) -> Result<Vec<u8>,
MigrationError>` reaches no capability because the type gives it nothing to
reach one with.

## Build time and runtime are different failures

| | what disagrees | reported as |
|---|---|---|
| build | artifacts within **one** build | `PW5016`, a compile error |
| runtime | independently cached artifacts from **different** releases | a typed `Decision`, never a source diagnostic |

Two build-time codes, because they are two invariants:

- `PW5016` — a capture has no type this build can name, so the schema hash
  would come from a guess and a guessed hash matches nothing.
- `PW5017` — the manifest and the handler artifact, generated by two different
  walks, describe different contracts.

`PW3011` aliases to `PW5007`, not to either of these. It was registered as
"resumption manifest — private data crossing into it", which is
`private_in_resume_manifest` and already enforced; an earlier alias onto
`PW5016` would have implied coverage that does not exist. There are no
`KNOWN_GAP`s.

## Privacy is checked before anything is decoded

The scope check runs before the schema check, so a widened scope never reaches
the point where bytes are interpreted, even to fail. `PrivacyScope` is
deliberately **not** `Ord`: charter §7.8 says labels are a set of restrictions
and not a total order, and an ordering here would make one session "admit"
another. `a_different_session_is_refused_rather_than_treated_as_stricter` pins
it.

`can_flow_to` names the relation rather than deriving it from an order:

```text
Public     -> Session<A>   allowed     adds a restriction
Session<A> -> Public       forbidden   removes one
Session<A> -> Session<A>   allowed
Session<A> -> Session<B>   forbidden   different principals
```

`Public` is the empty set of restrictions, so it is a subset of every
destination. **It authorises the data, not the code** — a public capture
resumed into a session page may still invoke a handler needing `session.read`,
which remains subject to capability, placement and handler-identity checks.

## Recovery is per construct

The runtime chooses only among actions legal for what is being resumed.

| construct | recoveries |
|---|---|
| public region | refetch region, reload |
| private slot | re-render private slot, reload |
| unsaved input | ask the user, reload |
| pending command | retry interaction, ask the user |
| open resource | irrecoverable |

A `PendingCommand` contains no automatic replay, and a test enumerates its
recoveries to keep it that way: re-running a mutation the user did not
re-request is how a version mismatch becomes a double charge. An `OpenResource`
is irrecoverable because a handle is not a description of a resource — it *is*
the resource, and nothing on the other side can be reconnected to it.

## Mixed-build streaming

`patch_applies` is separate from `decide` because it is a different question
with a different shape: a patch carries no captures and attaches no handler,
but it must belong to the generation of artifacts the document came from.
Otherwise a redeploy mid-stream sends the second half of a page from a build
whose markup the first half does not match.

Content addressing identifies each *piece*; `BuildId` identifies the coherent
*set*, which is what a patch must agree with.

## Gate

| # | item | result |
|---|---|---|
| 1 | manifest identity model documented | **PASS** — this file and the module docs |
| 2 | build-time artifact agreement checked | **PASS** — `PW5017`, with six mutation controls. `PW5016` is the narrower, separate check that a capture has a nameable type at all |
| 3 | runtime compatibility returns a typed decision | **PASS** — `Decision`, never a boolean |
| 4 | strict matching works | **PASS** |
| 5 | at least one explicit migration works | **PASS** |
| 6 | privacy weakening rejected | **PASS**, before any decoding |
| 7 | mixed-build streaming rejected | **PASS** |
| 8 | the deployment matrix passes | **PASS** — 34 rows, plus 6 fuzz targets over 22,000 cases |
| 9 | malformed inputs produce no panic | **PASS** |
| 10 | evidence distinguishes `pw` from Marko | **below** |

## What is `pw`'s and what is Marko's

This distinction is the one most likely to be overstated, so it is stated
narrowly.

**`pw`'s, in this repository:**
- the identity model and its derivation;
- the compatibility decision and every row of the matrix;
- the recovery policy per construct;
- the build-time capture-schema check.

**Not `pw`'s, and not demonstrated here:**
- the resumption *mechanism*. Marko resumes documents (ADR-0002, ADR-0017), and
  E0/RQ-1 measured that behaviour in Chrome and Safari. Nothing in E7V changes
  who implements resumption.
- the resumption *mechanism* itself. Marko resumes documents; `decide` sits in
  front of it and says whether a manifest may attach at all.

## In the browser

`runtime/pw-resume-wasm` compiles the decision to `wasm32-unknown-unknown` —
73 kB, no `wasm-bindgen`, a four-function ABI — and the store page's Add button
calls it before mutating. **Not a JavaScript port**: a second implementation of
a security decision is two things that can disagree, and the disagreement is
silent because both return a boolean and only one is right.

Four browser tests in Chromium, Firefox and WebKit:

| manifest | result |
|---|---|
| compatible | attaches, the mutation runs |
| hash scheme 1 | `refused: unsupported hash scheme`, no mutation, `refetch-region` |
| session scope into public | `refused: privacy widened`, no mutation, `rerender-private-slot` |
| changed handler identity | `refused: unknown handler`, no mutation |

The third is R3 in a real browser: private state is not offered a public region
refetch.

The gate **fails closed**. If the decision has not loaded, the handler refuses
rather than running — a handler that ran because the gate was still loading
would be the bypass this design exists to prevent.

So the sentence available is:

> The compatibility policy governs the actual handler-attachment path used by
> the application, in all three browser-engine families.

And still not:

> Resumption is safe across deployments.

Marko owns the resumption mechanism (ADR-0002, ADR-0017); this decides whether
a manifest may reach it.

## Build-time artifact agreement

`resume_artifacts.rs` generates the manifest and the handler artifact by two
**different walks** — the manifest from the `captures` list as written, the
artifact from what the lambda body reads — and compares them (`PW5017`).

Two sources for one fact is normally a smell. Here it is the point: a generator
bug that changes one walk and not the other is exactly what the comparison
exists to catch, and a single source of truth would make it undetectable by
construction. That is also why a stub generator was refused — it would produce
`stub says A, stub says A, A == A`.

The test with teeth is not a successful generation. It is six mutations, each
of exactly one field of one record — manifest capture hash, artifact capture
hash, document schema, handler identity, platform ABI, build identity — each of
which must be caught **and named**. Without them, `a_valid_pair_agrees` passes
even if `compare` returns `None` unconditionally.

## Attachment cannot be bypassed

`Authorised` has a private field, so its only constructor is `authorise`, and
`attach` takes one by value. A caller cannot reach `attach` without holding the
result of `decide`. A structural test that greps for a bypass would be a lint;
this is the same idea in the type system, where it cannot be forgotten — the
same move as deleting the by-name member fallback rather than documenting that
it should not be used.

A migration authorises the bytes it **produced**, never the manifest's
originals. Handing the originals to a handler expecting the new schema would be
the mismatch the migration exists to prevent, arriving one step later in a place
nothing checks.

## What implementing it found

**Nothing while being written, and one thing the moment it was attacked.**

E7V found no defect during construction — the first milestone here that did
not. Its first independent adversary found one in its first run:

> **R3.** A `Session`-scoped manifest in a `PublicRegion` construct was refused
> with `RefetchRegion` — recovering private state by re-rendering a public
> region. 611 of 4000 generated cases.

The deployment matrix could not have found it. Every hand-written row pairs a
construct with a *consistent* scope, because the disagreeing pair is a
contradiction no scenario would think to write. The generator produced it by
taking the cross product.

Recorded in `examples/robustness/regressions/R3-*.md` with the minimized
reproducer and the negative control. The architect predicted the shape of this
before it happened:

> New code without an adversarial neighbor is unchallenged, not demonstrated
> clean.

The artifact comparison then found two more — both in this repository's own
generality witnesses, which declared captures their bodies never read.
