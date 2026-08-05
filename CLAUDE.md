# CLAUDE.md

**Read `PROJECT_CHARTER.md` first.** It is the authoritative project
constitution. The operating rules are summarized in `AGENTS.md` — read that too.
This file adds nothing new; it exists so that Claude Code loads the pointer.

Start of every session:

1. `docs/STATUS.md` — current milestone, open gate items, reproduce commands.
2. `docs/NEXT.md` — the exact next executable task.
3. `AGENTS.md` — operating rules.

Hard rules, repeated because they are the ones most often broken:

- Do not start the next milestone until the current milestone's **gate** passes
  and its evidence is written to `docs/evidence/`.
- Do not claim a gate item passed without a file under `docs/evidence/` that a
  recorded command produced.
- Verify dependency versions and APIs against primary sources before depending
  on them, even if a doc in this repo already states a version.
- Record assumptions in `docs/ASSUMPTIONS.md` and decisions in
  `docs/DECISIONS/ADR-*.md` (indexed from `docs/DECISIONS.md`).
- Never commit secrets or private CA material.
- Local commits only. No push, no publish, no remote changes without explicit
  human authorization.

`just doctor` is read-only. Run it before assuming the environment is broken.
