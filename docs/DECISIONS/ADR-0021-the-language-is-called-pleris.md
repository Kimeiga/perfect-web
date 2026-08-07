# ADR-0021 — The language is called Pleris; the project stays Perfect Web

**Status:** Accepted
**Date:** 2026-08-07
**Milestone:** E8 (naming; no code depends on it)

## Context

The language had no name of its own. It was "the Perfect Web language", which
reads as a claim rather than a name, and searching for it lands on DreamBerd,
TodePond, and general argument about whether software can be perfect. That is
the wrong developer intent to compete for.

## Decision

```text
Perfect Web    the research project and its question
Pleris         the language and the developer-facing toolchain
.pw            source files
pw-resource, pw-document, pw-protocol, …   internal implementation crates
```

**Pleris** is a normalized spelling of Ancient/Modern Greek **πλήρης**
(*plḗrēs / plíris*) — *full, complete, lacking nothing*.

That is the actual thesis. The language exists because modern web programs leave
load-bearing facts outside the program:

```text
effects              left out
privacy              left out
placement            left out
cache dependencies   left out
resource lifecycle   left out
```

Perfect Web is the aspiration; Pleris is the language that makes a program
complete enough for a compiler to reason about it.

## Clearance, as measured

Checked 2026-08-07. Commands are recorded so the result can be re-derived rather
than believed.

| where | result |
|---|---|
| crates.io | `?q=pleris` → `"total": 0` |
| npm | `registry.npmjs.org/pleris` → `{"error":"Not found"}` |
| PyPI | `pypi.org/pypi/pleris/json` → `Not Found` |
| GitHub repository search | `total_count: 1`, and it matches `pleri`, not `pleris` |
| Web search | no language, compiler, framework or developer product |
| `pleris.dev` `.org` `.io` | no A record, no NS — unregistered |

**Two things are NOT clear, and are recorded as such:**

- **`github.com/pleris` is taken.** An empty user account created 2026-07-25:
  0 repositories, 0 followers, no name, no bio. The organization name is
  unavailable; `pleris-lang` or the existing `perfect-web` remain.
- **Trademark status is unverified.** The USPTO endpoint tried returned 404 and
  no WIPO check was made. This ADR does not claim trademark clearance.

## Consequences

- Nothing in the build depends on this. No crate is renamed, `.pw` is unchanged,
  and `pw check` still works — so this ADR can be reversed by editing prose.
- The CLI is still `pw`. Whether the developer-facing binary becomes `pleris`
  is a separate decision with a real mechanical cost across `justfile`, the
  docs and the evidence scripts; it is recorded in `docs/NEXT.md` rather than
  done here.
- A formal clearance pass — GitHub organization, trademark registers, domain
  registration — should happen before the name appears anywhere public. The
  measurements above cover developer search intent and package namespaces only.

## What would falsify this

A language, compiler or developer tool named Pleris appearing before this
project is public. The registries above are cheap to re-check, and re-checking
them before any public announcement is the whole mitigation.
