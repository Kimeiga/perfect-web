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

`HandlerId::derive` takes the normalized implementation, its resolved
references, its capture schema, and the platform ABI. References are sorted
first — the order two references happen to appear in is not a behavioural
difference, and leaving it in would make an unrelated edit reject every resume
in the application.

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

`PW5016` fires when a capture has no type this build can name: the manifest's
schema hash would come from a guess, a guessed hash matches nothing, and every
resume would fail after deployment for a reason unreadable from the deployment.
Named at compile time, where it is still cheap.

`PW3011` — the corpus's reserved code — aliases to `PW5016`. It was the last
`KNOWN_GAP`; there are now none.

## Privacy is checked before anything is decoded

The scope check runs before the schema check, so a widened scope never reaches
the point where bytes are interpreted, even to fail. `PrivacyScope` is
deliberately **not** `Ord`: charter §7.8 says labels are a set of restrictions
and not a total order, and an ordering here would make one session "admit"
another. `a_different_session_is_refused_rather_than_treated_as_stricter` pins
it.

`Public` admits any scope, because public state restricts nothing — the one
widening-adjacent case that is allowed, and it is allowed in the direction that
*adds* restrictions.

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
| 2 | build-time artifact agreement checked | **PASS** — `PW5016` |
| 3 | runtime compatibility returns a typed decision | **PASS** — `Decision`, never a boolean |
| 4 | strict matching works | **PASS** |
| 5 | at least one explicit migration works | **PASS** |
| 6 | privacy weakening rejected | **PASS**, before any decoding |
| 7 | mixed-build streaming rejected | **PASS** |
| 8 | the deployment matrix passes | **PASS** — 27 tests |
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
- **wiring.** `decide` is not called from a running browser. It is a decision
  function with a tested contract, exactly as `pw-tasks` and `pw-resource` are
  behaviour with tested contracts (ADR-0016). No deployed application has ever
  refused a manifest through this code.

So the strongest sentence available is:

> Resume artifacts are accepted only under exact compatible identities or
> explicit checked migrations; incompatible mixed-version artifacts fail closed
> through tested recovery paths.

And not:

> Resumption is safe across deployments.

The second requires the decision to be *in the path*, which is E7's generator
and is not done.

## What implementing it found

**Nothing, and that is worth recording.** Every other milestone in this project
found a defect while being built. E7V found none, because it is new code with
no existing behaviour to contradict — which is itself a reminder that the
defect-finding has come from *challenging existing analyses*, not from writing
new ones. The fuzzer is the next thing that can find something here.
