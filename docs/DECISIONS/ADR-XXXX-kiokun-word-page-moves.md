# ADR-XXXX: kiokun's word page moves an equivalent simplified form

Status: accepted under the owner's delegation of 2026-10-02, on the
integrator's rulings for track `kiokun` (docs/PARALLEL.md). Date:
2026-10-09. Milestone: E14, kiokun.com in Pleris, step 1 (the word page).

## Context

- **kiokun.com moves a word's page.** Where the word's own file is a
  simplified form that equals its one traditional form in meaning, the page
  answers 308 to that form's page, the query kept (`[word]/+page.ts`,
  415-425):
  - the function is `equivalentTraditionalTarget` (`character-forms.ts`):
    no move for a stub (`redirect`), for an entry with no
    `simplified_form_of`, or where the source or the target is more than
    one character or they are the same;
  - the traditional forms, the source's own left out, must be exactly the
    target;
  - both mnemonic cards must teach one concept, normalized. kiokun.com
    keeps 后/後 and 制/製 apart on purpose;
  - the page moves only where the target is not the word asked for.
- **The rewrite could not say so** until ADR-0295: ADR-0289 declared
  `KiokunError.Moved(String)` and left it unanswered. ADR-0295 gives a page
  `redirect_on Type.Case permanent | temporary`, answered 308 or 307 to the
  page's route filled with the case's value, the query kept, kept by no
  cache.
- **The oracle standard** (the integrator's ruling of 2026-10-09): kiokun's
  own code, run locally over a stated sample, every difference named, and a
  committed fixture for CI.

## Decision

1. **The word page declares the move**:
   `redirect_on KiokunError.Moved permanent`, kiokun.com's 308.
2. **The word query answers `Moved(t)`** where `equivalent_target` of the
   word's own file names a `t` other than the word. The file decides before
   a stub is followed or forms merged, as kiokun.com's does.
   `equivalent_target` is the port that ADR-0289's merge already uses.
   The query still makes the word's other reads before it answers the
   move, where kiokun.com moves first. The response is the same, and three
   words in the whole dictionary move; a query that answers early would
   need its reads split by the move, for no reader's gain.
3. **The moves are held to kiokun.com's own `equivalentTraditionalTarget`**
   (`just e14-kiokun-moves`, local), and each move's target must be a page
   (200), not another move.
4. **ADR-0295's tests on the development server keep their five tests and
   every assertion.** The program now declares the clause and answers the
   case itself, so the helpers change, each asserting that `app.pw` declares
   what it patches before patching it (the integrator's condition):
   - `moving` replaces the line that decides the move, choosing `old`,
     `same` and `dots`, and otherwise keeping kiokun's own answer;
   - `permanent` is that alone;
   - `temporary` changes the clause's word;
   - the unnamed case takes the clause away.

## Differences from kiokun.com

Numbered after ADR-0299's 19.

20. **A conjugated form is not moved to its dictionary form.** kiokun.com
    answers 307 to the deinflected form, with `from`, `conj` and `alt` in
    the query (`+page.ts:363-378`). The rewrite answers 404. Deinflection
    is step 2's, and a move that adds to the query needs a page's typed
    query parameters (ADR-0295, Not claimed).

## Found

1. **Three words move in the whole dictionary**: 余→餘, 宁→寧 and 霡→霢,
   among 107 one-character entries with a `simplified_form_of` (94,908
   one-character entries read). Each target is a page, not another move.
2. **The repository's sample moves nothing.** 面 is its one candidate and
   the hard negative: the simplified form of 麵, with traditional forms 面
   and 麵 (麵 alone once its own is left out) and no card, so no concept
   to compare. CI therefore also holds hand cases built from kiokun's own
   movers' deciding fields.
3. **ADR-0295's tests pinned text that 1d had changed.** A clean textual
   merge left all five failing on this track; the pinned strings were
   updated, and `e14-redirects` was recorded again (ADR-0299).
4. **`e14-kiokun-word` took 4 h 01 m on CI** (verify 37961821792, a shard
   of its own; the job's limit is 345 min), about 220 s a mutant: each
   mutant runs every kiokun test, and each test compiled the program
   into its own `TempDir`. On the integrator's ruling, each distinct
   program is now compiled once per test process. Its files are kept as
   bytes, keyed by a hash of the sources compiled, and written into each
   test's own `TempDir`, so nothing on disk outlives its test (ADR-0158).
   `Build::write` writes only paths relative to its directory, and no
   build id, so the bytes are the same files.
   - Measured back to back with `/usr/bin/time -l`, 30 tests, 12 cores at
     a load near 16:

     | | one thread per core | four threads |
     |---|---|---|
     | before | 92.8 s, 3.61 GB, 487 CPU-s | 138.7 s, 2.09 GB, 437 CPU-s |
     | after | 76.0 s, 2.71 GB, 261 CPU-s | 99.0 s, 2.39 GB, 251 CPU-s |

   - One test's own time is about 7.1 s to compile and 7.7 s for the
     server to load the components (Wasmtime compiling them in a debug
     build); the cache removes the first.
   - `scripts/kiokun_word_mutations.py` runs one test thread per core
     again: 2.71 GB is 1.6 GB under ADR-0292's bound.
   - CI runs on four cores, where CPU time decides, so the shard should
     take about half its time.

## Acceptance

- **The server's tests**:
  - `an_equivalent_simplified_form_is_moved_to_its_traditional_page`:
    宁 is answered 308 to `/word/%E5%AF%A7`, the query kept; 余 to
    `/word/%E9%A4%98`, its own form not counted among its traditional
    ones; 寧, 餘, 後, 后 (another concept) and 甲 (a file naming the word
    itself) are each answered 200;
  - `the_moves_match_kiokuns_answers_for_the_sample` (CI): the fixture's 28
    answers, 面 among them;
  - `the_moves_match_kiokuns_own_on_a_sample` (local, ignored);
  - the timing sample counts a 308 as a page moved, not as a defect.
- **ADR-0295's tests**, `tests::redirects`: five, every assertion kept.
- **The oracle** (`just e14-kiokun-moves`, local): 1,643 words (107
  candidates, 1,536 controls), 3 moves, no named difference, 0 unnamed
  (`docs/evidence/E14/kiokun-moves.txt`).
- **Mutation controls**: five more in `scripts/kiokun_word_mutations.py`:
  - the word's own file never moves it;
  - a word is moved to itself;
  - the move is temporary;
  - the page names no move;
  - a character's own form counts among its traditional forms.

  `scripts/redirects_mutations.py` was run whole again
  (`docs/evidence/E14/redirects.txt`), and so was
  `scripts/kiokun_inventory_mutations.py`, since its test names the new
  local recipe.

## Questions asked

- **Who changes ADR-0295's tests' helpers?** (W6, 2026-10-09.) The
  integrator: W6, on condition that each helper asserts the clause is
  declared before it patches it, all five tests and their assertions are
  kept, and `redirects_mutations.py` is run whole and `e14-redirects`
  recorded again.
- **The sample** (W6's plan): accepted as stated. Every one-code-point
  entry with `simplified_form_of`, every 64th of the others, hand cases for
  a real mover and 面, and a committed fixture. The integrator added the
  check that a move's target is itself a page.

## Not claimed

- **Deinflection's 307** (Difference 20).
- **A canonical link** on the page a word moves to: the integrator's,
  after navigate (ADR-0293's Q2).

## Alternatives

- **A temporary move.** Refused: kiokun.com sends 308, and the two forms
  teach one concept for good; a search index then keeps the traditional
  page as the canonical one, as kiokun.com intends.
- **Deciding on the page's merged data.** Refused: kiokun.com decides on
  the word's own file, before a stub is followed or forms merged, and a
  merged entry's `simplified_form_of` is no longer the word's.
- **A sample of every 256th file, as the head's oracle takes.** Refused:
  three words move in the whole dictionary, so a stride would miss them.
  Every candidate is cheap to read (107 of them), and the controls show
  that the rest stay.
