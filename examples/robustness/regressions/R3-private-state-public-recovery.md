# R3 — private state offered a public recovery region

```text
@regression: R3
@phase:      resume compatibility (runtime/pw-resume)
@found:      2026-08-06, by the FIRST run of the E7V fuzzer
@symptom:    a Session-scoped manifest in a PublicRegion construct was refused
             with recovery `RefetchRegion`
@impact:     recovering private state by re-rendering a PUBLIC region, which
             renders it for whoever asks
@rate:       611 of 4000 generated cases
@fix:        `Construct::recovery_for(scope)` — the scope narrows the
             construct's list, and the scope wins
```

## Minimized reproducer

```rust
let d = decide(
    &ResumeEntry {
        privacy_scope: PrivacyScope::Session("s-1".into()),
        ..entry_with_no_matching_handler()
    },
    &runtime_with_no_handlers(),
    Construct::PublicRegion,
);
assert_eq!(recovery_of(&d), Recovery::RefetchRegion);   // before the fix
```

Two lines: a private scope, and a construct that says public.

## Why the deployment matrix missed it

Every matrix row pairs a construct with a *consistent* scope — a
`PrivateSlot` holds session state, a `PublicRegion` holds public state. The
defect needs the pair to **disagree**, which is a contradiction no
hand-written scenario would think to write, and exactly what a generator
produces by taking the cross product.

The pairing is arguably impossible in a well-formed artifact: a public region
should never carry session state. But "should never happen" is not a recovery
policy, and a malformed or forged manifest is precisely when the recovery
matters.

## Negative control

Reverting `recovery_for` to `recoveries().first()` makes
`fuzz_recovery_planning` fail at seed 7 and 610 others. The deployment matrix
stays green — its 29 rows never construct the disagreeing pair.

## What this says about the method

E7V found no defects while being written. Its first independent adversary
found one in its first run, in the code path least exercised by the
hand-written scenarios. The architect predicted this before it happened:

> New code without an adversarial neighbor is unchallenged, not demonstrated
> clean.
