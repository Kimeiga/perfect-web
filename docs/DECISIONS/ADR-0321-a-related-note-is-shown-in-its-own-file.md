# ADR-0321: a related note is shown in its own file

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Found by W8 (the
store-accounts track, 2026-10-12). Builds on charter §16.3 (a diagnostic names
where the conflicting property originated, not only where its rule fired)
and ADR-0106 (each file reported by its place).

## Context

- **A related note's span was read in the diagnostic's own file, whatever
  file it was in.** `Related` held a span and a label, and every renderer
  read the span in the file the diagnostic was reported for.
- **W8's finding: PW5046 stopped `pw check`.**
  - A page in one module, shown in a layout another module declares for
    fewer eyes, is refused (PW5046). Its note, "`Shell` is declared `user`
    here", carried the span of the layout's name in the layout's file.
  - Read in the page's file, a span past the page's end stopped `pw check`
    with an internal error: "Annotation range `3307..3312` is beyond the end
    of buffer `172`".
  - Within the page's file, it quietly labelled the page's own text: the
    `import` line, in W8's smallest program.
- **The same defect, found by reading every place a note is made**:
  - **PW0338**, a command sent without `idempotent_by`, noted each press that
    sends it. The press is in a page, which may be another file than the
    command's, and its span was read in the command's file.
- **And notes left out because they could not be placed**:
  - **PW5106**, a write that does not reach a reader, noted the reader only
    where it was in the command's own file (`if r.unit == unit`). A reader
    in another file had no note.
  - **PW0351** and **PW0341**, a predicate or a route declared twice, noted
    the second declaration again, or nothing. The first, which may be in
    another file, was named only in the message.
- **Five explanations held runs of spaces mid-sentence**, from lines joined
  without a continuation: the frame phases' four in `effects.rs`, and an
  impact marker's in `ontology.rs`. Found while reading the notes.

## Decision

1. **A note names its file where it is not the diagnostic's own.** `Related`
   gains `file: Option<String>`, the file's path as it was given to the
   checker; `None` is the diagnostic's own file.
   - A detector says which unit a span is in (`Related::in_unit`). The
     checker resolves each to its path once the program is checked
     (`resolve_files`): a unit that is the diagnostic's own names no file.
   - `Related::new(span, label)` is a note in the diagnostic's own file.
     Every note is now built through one of the two, so a field added to a
     note later touches no detector.
2. **Each note found in another file says so**:
   - PW5046's note is in the layout's file;
   - PW0338's notes are each in the file of the page whose press sends it;
   - PW5106 notes a reader in its own file, wherever it is;
   - PW0351 and PW0341 note the first declaration, in its own file.
3. **`pw check` shows a note in its own file**, by that file's lines, beside
   the diagnostic's own.
4. **`pw check` never stops on a span**: one outside its file's text, or off
   a character's edge, is told as a note naming the file and its bytes. It is
   a checker's defect, reported as one rather than as an internal error.
5. **The five explanations read as written**, each wrapped with a
   continuation.

## Alternatives

- **Drop a note in another file, as PW5106 did.** Rejected: charter §16.3
  asks for where the conflicting property originated, and a layout or a
  first declaration in another module is exactly that.
- **Name the other file in the label's text only.** Rejected: a reader, a
  language server and a tool all want a location they can open, and LSP's
  `DiagnosticRelatedInformation` carries one.
- **A detector names the path itself.** A detector holds units, not paths.
  The checker, which holds both, resolves them.

## Acceptance

- `compiler/pw-core/tests/related_files.rs`:
  - PW5046's note is in the layout's file, and the text it marks is the
    layout's name;
  - PW0338's note is in the page's file, at the press;
  - PW5106 names a reader in another file, in it;
  - PW0351 and PW0341 name the first declaration where it is;
  - a control in one file for each, where it can be one;
  - every note made over the rejected corpus and the store and the feed is
    text of the file it names.
- `compiler/pw-cli/tests/related_files.rs`: W8's program, its layout past
  the page's end. `pw check` reports PW5046 at `page.pw:7:5` and its note at
  `shell.pw:24:16`, with no internal error.
- The CLI's own tests:
  - a span outside its file is a note, not a stop;
  - a note in another file is shown in it.
- `scripts/related_files_mutations.py`, 10 mutants, each killed here: the
  checker's two, the five detectors', and `pw check`'s three.
- Re-anchored, their code moved: `refusal_mutations.py`'s and
  `routes_mutations.py`'s declared-twice mutants, each run whole by the
  verify.

## Not claimed

- **A language server.** None exists yet. The note's file is what one would
  carry.
- **Every detector's reach.** A detector that builds a note from another
  unit's span is held to `Related::in_unit` by review. The corpus check
  catches a span read in the wrong file only where it falls out of that
  file's text.
