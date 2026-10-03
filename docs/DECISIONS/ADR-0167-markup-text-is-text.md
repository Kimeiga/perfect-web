# ADR-0167: markup text is text, and a comment in markup is `<!-- -->`

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Found by ADR-0165, whose first version explained the store's
slots in a `//` line between two elements, and the page showed it.

## Context

The lexer does not know markup. It reads every `//` as a comment that runs
to the line's end, and markup text was put together from its tokens.
Measured on 2026-10-03:

- **A `//` line between elements is page text.** `pw check` said nothing,
  and the page showed the line.
- **A `//` inside an element's text broke the page.** It ran over the closing
  tag. `<p>http://example.com</p>` and `<p>a // b</p>` did not parse.
- **An HTML comment was read as an element with no name.** `<!-- x --><p>one</p>`
  built `< x p>one</>`, and nothing said so.
- **An unquoted attribute value was read as code.** Only one word or number
  could be one, so `<a href=http://x.y>y</a>` built `<a href="http" p>one</a>`.
  Its `//` began a comment that took the rest of the line, and the element
  after it too.
- **Seven rejected-corpus fixtures had shown their own `// ERROR:` notes as
  page text** for as long as they had existed: R-004, R-016, R-018, R-019,
  R-021, R-023 and R-024.

Elsewhere:
- HTML has one comment form, `<!-- -->`, and its unquoted attribute values
  run to whitespace or `>`.
- JSX reads `//` in children as text. `eslint-plugin-react` refuses text
  that begins with `//` or `/*` (`jsx-no-comment-textnodes`), since it is
  almost always a comment that would be shown.
- Svelte and Vue take markup comments as `<!-- -->`, and leave them out of
  what they render: Svelte unless `preserveComments` is set, Vue in
  production unless `compilerOptions.comments` is set.

## Decision

1. **Markup text is text.** Between tags, `//` and `/*` begin no comment.
   - Where the lexer read one in markup content, the parser reads the
     source again from it: as text up to the next `<`, `{` or `}`, and as
     before from there.
   - In code, inside `{ }`, `//` is still a comment.
2. **A comment in markup is `<!-- … -->`.**
   - It is one token in the tree, which no page renders.
   - Its inside is not read: `<!-- a // <p> { here -->` is a comment.
   - One that is not closed runs to the end, as HTML's does, and is refused
     (PW0006).
3. **An unquoted attribute value is read as HTML reads it**, up to a space,
   `>` or `/>`, a value that begins with `//` included. Whitespace may stand
   around `=`.
4. **PW5028: a line of markup text that begins with `//` or `/*` is
   refused.** It reads as a comment, and the page would show it. The repair
   is `<!-- -->`, or the text written as a string, `{"// …"}`. A `<style>`'s
   or a `<script>`'s text is its own language's, where they are comments,
   and is not held to it.
5. **The corpus's seven `// ERROR:` notes are `<!-- ERROR: … -->`.** Each
   fixture now emits its own defect and no other.

## Alternatives

- **`//` a comment in markup too**, as it is in code. A URL, `a // b`, or a
  code sample in `<pre>` would lose text without a word. Text is the
  platform's reading, and the refusal catches the mistake instead.
- **`{/* … */}`, JSX's form.** It needs a comment-only expression, which the
  language does not have, and `<!-- -->` is the markup's own form.
- **Keep comments in the page**, as Svelte and Vue can be told to. The
  runtime addresses parts by comment markers (`<!--pw:s3-->`), and a note in
  a shipped page is read by everyone who opens it.
- **Refuse unquoted attribute values** that are not one word. HTML allows
  them, and Svelte and Vue read them as HTML does. They mis-built silently,
  which a refusal would have fixed too; reading them is what the markup
  means.
- **A lexer that knows markup.** Whether a `<` opens markup or compares
  depends on where the parser is, so the parser asks for the text, and only
  where content holds what the lexer read as code.

## Acceptance

- The parser's tests (`compiler/pw-syntax/src/grammar.rs`):
  - `//` and `/*` in text, a URL among them, on the line of the closing
    tag;
  - a comment in code still a comment;
  - `<!-- -->` one token, its inside not read, an unclosed one refused;
  - unquoted values: a URL, `//cdn…`, spaces around `=`, a void element's
    `/>`.

  Every tree stays lossless.
- `compiler/pw-core/tests/markup_comments.rs`:
  - PW5028 on a `//` or `/*` line, between, after or inside elements;
  - the controls check clean;
  - what a page sends: no comment, text with slashes whole, and unquoted
    values whole.
- The corpus: every rejected fixture emits its own defect and no other.
- `scripts/markup_comments_mutations.py`: 13 mutants, recorded by
  `just e14-markup-comments`.

## Not claimed

- **What a `<style>`'s or a `<script>`'s comments say.** Their text is
  theirs, and PW5023 refuses a script's.
- **HTML's other bogus comments**, `<!x>` and `<?x>`. They are read as
  elements still, as before; only `<!--` begins a comment.
