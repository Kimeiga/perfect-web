# Corpus history

The text of every fixture **as it was** before the change that let the compiler
enforce it. See `docs/CORPUS.md` for the per-fixture record.

These are not fixtures. They are not counted, not category-checked, and not run
by `pw check` in CI. They exist so that
`compiler/pw-core/tests/corpus_history.rs` can ask one question:

> Does the *older* text still fail?

Nine of the ten changes added an import or a declaration the fixture called and
the program lacked. That repair is legitimate — a fixture that calls something
which exists nowhere is silent because the program is incomplete, not because
the checker is — but it is exactly the shape of change that could also, by
accident, remove the defect instead of the obstruction. If it did, the old text
would stop being rejected, and this test is what notices.

A fixture whose old text is EXPECTED to pass now (because the change corrected
a genuinely wrong specification) belongs in `EXPECTED_TO_PASS` in that test,
with the reason written next to it. There is currently one.
