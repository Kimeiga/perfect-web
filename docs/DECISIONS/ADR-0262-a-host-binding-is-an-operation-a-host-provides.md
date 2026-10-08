# ADR-0262: a host binding is an operation a host provides

Status: accepted under the owner's delegation of 2026-10-02; found by the
uploads track (ADR-0260) and queued at its merge. Date: 2026-10-07.
Milestone: E14.

## Context

- **A function the host provides names its operation**: `host
  "pw:host/session#read"`, an interface and a name, from which the
  component's WIT world imports it.
- **Any string was taken.** The binding's domain was a quoted string, and
  `backend::host_binding` split it at `#`. The uploads track's `host
  "feed:uploads#claim"` has no interface: WIT generation failed for every
  component of the program, with an error naming no source line (ADR-0260,
  "Found").
- **WIT's shape** (wit-parser 0.257.1, read 2026-10-07 in the copy
  `Cargo.lock` pins): an interface is named `namespace:package/interface`,
  and an identifier is words joined by `-`, each all lowercase or all
  uppercase ASCII letters with digits among them, the first word beginning
  with a letter (`validate_id`).

## Decision

1. **A binding is an operation a host provides**, quoted:
   `"namespace:package/interface#name"`, its namespace, package, interface
   and name each a WIT identifier as wit-parser holds one. The clause has a
   domain of its own, `Domain::HostOp`.
2. **Held where it is written**: anything else is PW0335, a clause's value
   outside its domain, at the clause, saying which part is wrong and why.
   So a program never reaches WIT generation with one.
3. **A version is not taken**: `namespace:package@1.0.0/...` is refused.
   No host operation is versioned yet, and taking one is a ruling of its
   own.

## Acceptance

- **`compiler/pw-core/tests/host_bindings.rs`, 3 tests**: the uploads
  track's binding refused at its clause, with its control; each part held
  to WIT's identifiers (unquoted, no `#`, no namespace, an empty part, a
  word mixing cases, a leading digit, `_`, `--`, `::`), and the controls
  wit-parser takes (an uppercase word, a word of digits after the first,
  the platform's own); and a malformed binding stopping the build at check,
  never at WIT, beside its control, which builds.
- **`scripts/host_binding_mutations.py`, 6 mutants**: the domain undone,
  the interface and the namespace not required, and three of an
  identifier's rules dropped. Recorded by `just e14-host-bindings`.
- **The whole workspace's tests**, the corpus among them: no binding in the
  repository was malformed.

## Not claimed

- **That the operation exists.** A well-formed binding naming an
  interface no host provides is found where it always was: by the host,
  which refuses to serve a program whose imports it cannot supply.
