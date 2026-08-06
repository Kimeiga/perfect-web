# `unserializable_capture` and `private_in_resume_manifest`

Two invariants over the same construct, and the reason they are separate is
that a value can pass one and fail the other. Charter §8.5:

```text
Can this value be serialized?        -> its TYPE     (is it a resource?)
May it cross this privacy boundary?  -> its LABEL    (is it private?)
```

A `Cart` is perfectly serializable and may not be in the manifest. A
`DatabaseConnection` is not private in any interesting sense and cannot be
serialized at all. Answering both from one check would make one of them a
special case of the other, which they are not.

| axis | file | which question |
|---|---|---|
| direct invalid | `direct.pw` | serializable? — a live connection |
| indirect invalid | `via-field.pw` | serializable? — a resource reached as a field |
| indirect invalid | `../private_in_resume_manifest/direct.pw` | privacy? — a session-scoped cart |
| indirect invalid | `../private_in_resume_manifest/rebound.pw` | privacy? — the same, renamed first |
| neighbour | `identifier-capture.pw` | an id, which is both serializable and public |
| neighbour | `public-value.pw` | a public query result — serializable and unrestricted |

**A third question exists and is NOT checked:** can this value be resumed
safely under *this code version*? A capture that satisfies both of the above
can still be restored into a handler whose code has changed. Nothing here
addresses it, `PW3011` is the reserved code, and it is the one remaining
KNOWN_GAP.
