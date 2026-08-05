# AGENTS.md — operating rules

`PROJECT_CHARTER.md` is the authoritative project constitution. Read it before
changing anything. This file summarizes only the operating rules; when the two
disagree, the charter wins.

## Before you touch code

1. Read `PROJECT_CHARTER.md`, `docs/ARCHITECTURE.md`, and the ADRs in
   `docs/DECISIONS/` that cover the area you are changing.
2. Read `docs/STATUS.md` for the current milestone, the last passing commit, and
   which gate items are open.
3. Verify upstream APIs and versions against **primary** documentation before
   depending on them. Do not trust versions quoted in the charter or in any doc
   in this repo without rechecking (`docs/research/version-verification.md`
   records when each was last verified).

## Non-negotiable rules

- **Execute, do not speculate.** Charter §3.1. A milestone is complete only when
  its objective gate passes — not when code exists.
- **Stay in the current milestone.** Do not build features assigned to later
  milestones. `docs/STATUS.md` names the active one.
- **One coherent architecture.** No parallel competing designs. Consequential
  design changes require an ADR *before* the change.
- **Never hide uncertainty.** Charter §3.3. If an upstream project is
  experimental, an API moved, or an assumption failed, write it into
  `docs/KNOWN_LIMITATIONS.md` and `docs/ASSUMPTIONS.md`. Do not invent APIs.
- **Evidence or it did not happen.** Every gate claim points at a file under
  `docs/evidence/<milestone>/` produced by a command recorded in `docs/STATUS.md`.
- **No credentials, tokens, private certs, or user data in the repo.** Charter §3.7.
- **No push, no publish, no remote PR, no external repo modification** without
  explicit human authorization. Local commits only.
- **Inspect remote install scripts before executing them.** Record the installed
  version in `tools/versions.lock`.

## Document map

| File | Purpose |
|---|---|
| `PROJECT_CHARTER.md` | Constitution. Immutable except by explicit human decision. |
| `docs/STATUS.md` | Current milestone, gate state, reproduce commands, next 3 tasks. |
| `docs/NEXT.md` | The next executable tasks in order. |
| `docs/DECISIONS.md` | Index of ADRs + short decision log. |
| `docs/DECISIONS/ADR-*.md` | One accepted decision each, with context and consequences. |
| `docs/ASSUMPTIONS.md` | Every unverified assumption, with how/when it gets validated. |
| `docs/KNOWN_LIMITATIONS.md` | Things that do not work, honestly stated. |
| `docs/RISK_REGISTER.md` | Live risks with fallbacks. |
| `docs/ARCHITECTURE.md` | How the pieces fit today and where they are going. |
| `docs/SEMANTICS.md` | The language/platform semantics being validated. |
| `docs/BENCHMARKS.md` | Measured results with reproduction metadata. |
| `docs/milestones/M*.md` | Per-milestone question, gate, and measured result. |
| `docs/evidence/M*/` | Raw command output backing each gate claim. |

## Commands

`just doctor` is read-only and explains what is missing. Everything else:
`just bootstrap`, `just fmt`, `just lint`, `just test`, `just ci`, `just spikes`.
See `justfile`.

## Multi-agent discipline (charter §3.5)

- One integrator owns architecture and merges.
- Never let two agents edit overlapping files at once.
- Give each subagent a separate Git worktree and a non-overlapping assignment.
- Subagents must read the charter, `docs/ARCHITECTURE.md`, and the relevant ADRs
  before changing code.
- Delete abandoned worktrees after integrating or rejecting them.
