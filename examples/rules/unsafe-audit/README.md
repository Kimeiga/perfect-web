# Rule fixtures

`.pw` programs that exercise **one rule** from several angles. Distinct from
`examples/{accepted,rejected}`, which is the charter §16 corpus and is policed
for category coverage by `tools/corpus-check`.

Architect ruling, 2026-08-06:

> Add or split fixtures so the rule cannot accidentally pass through only one
> half. At least one valid case should prove the checker is not merely banning
> every unsafe escape.

Each file declares `// @expect:` — either `clean` or the canonical code it must
report. `compiler/pw-core/tests/rule_fixtures.rs` runs them.
