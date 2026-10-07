# The identity track's recipes (ADR-0253, docs/PARALLEL.md): accounts,
# sign-in and the `requires` evaluator. The track adds its recipes here and
# nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).
