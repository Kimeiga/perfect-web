# ADR-0132: the browser's resume decision knows what the build compiled

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (E7-L's decision, generalized). Found building ADR-0133.

## Context

A page attaches a handler only after the browser's resume decision
(`pw-resume-wasm`, E7V) says the handler is one this build has code for. The
decision's side of that comparison, the build's handler table, was two
constants: the store page's `add_to_cart` and `clear_cart`, by name, written
in the WebAssembly shim.

So any other handler a program declared, or a benchmark agent added, built
and compiled, was served by identity, and then was refused in the browser
with code 4 ("unknown handler"). Its button never attached, and nothing at
build time said so. Signals (ADR-0133) were the first new handlers anyone
wrote, and none of them attached.

## Decision

1. **The decision's table is the build's.**
   - `pw-resume-wasm` exports `know(table)`, one `identity|capture` line per
     handler the build compiled. It replaces what was known and returns the
     count.
   - The store's two constants are gone. Told nothing, the decision knows
     nothing and refuses every handler, which is the closed side.
2. **The table comes from the build the runtime was served with, never from
   the document.**
   - The development server serves it at `/pw-handlers`, from the build it
     runs, and the runtime fetches it once at boot.
   - A document cached from an older build names handlers this build may not
     have. That is what the decision exists to refuse, so the document cannot
     be the source of what is known.
3. **A handler is known by its identity**, the content hash
   `resume_artifacts` derives, and not by its name.
   - The document's manifest has one entry per identity, with the capture
     schema it presents. The base entry names no handler, so a part with no
     entry of its own authorises nothing.
   - A handler that calls nothing has no name, and two handlers may share
     one; an identity names the code.

## Alternatives

- **Generate the table into the WebAssembly at build time.** Equally sound
  and needs no fetch, but the decision would have to be rebuilt per
  program. Kept for a production deployment, where the runtime is
  content-addressed with the build anyway.
- **Read the table from the document.** Rejected: it makes the check
  circular, and a stale document would vouch for itself.

## Acceptance

- **`runtime/pw-resume-wasm`'s test**
  `the_decision_knows_what_the_build_compiled_and_nothing_else`:
  - told nothing, a handler is refused;
  - told the table, its handlers resume;
  - an identity the build lacks is refused, and so is a name;
  - telling again replaces the table.
- **`e2e/lazy-handler.spec.mjs`**: "a handler this build did not compile is
  refused, whatever the document says". A document rewritten to name an
  identity the build lacks, consistently in its parts and its manifest,
  attaches nothing and fetches no code. The decision before this attached
  it, because it matched by name.
- The own-renderer suite passes in WebKit and Chromium, the store's handlers
  through the new table.
- **Mutation controls:** `scripts/resume_gate_mutations.py`, `just
  e14-resume-gate`, 3 mutants.

## Not claimed

- The build id and the document schema the decision compares are still the
  development server's constants (`B1`, `cart-doc`). A build-derived
  document schema is the same change one field over, and waits for a page
  other than the store's to need it.
