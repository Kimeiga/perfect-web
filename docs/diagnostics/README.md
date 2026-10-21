# Diagnostic records

These records preserve failed runs, counterexamples and subsequent repair checks.
They are not accepted milestone evidence. A failed native result remains failed,
even when a later source revision fixes it. Hosted evidence is accepted only
through the canonical fetch/currentness checks into `docs/evidence`.

Train-12's combined local gate at `7952a49f` completed 1006 tests in 271.916s
with one failure and five explicit skips. Its evidence-survivor guard correctly
rejected two mixed historical/repair transcripts in the accepted-evidence tree.
They were moved here byte for byte, with references updated and the guard
unchanged. Original and relocated SHA-256 values are identical:

```text
c2bdbe475cb96fff1e5a3989dd6b592cd65ab88ce373f0806611aff6f8825653  representations-proof.txt
dbfa01b3c03597690e9dfee105708c9befbd0de4ad81db40db542078a45b3172  train12-catalog-and-bound-witnesses.txt
```

The reserved-host mixed diagnostic was subsequently moved with the same
byte-preservation rule (it was not a failing evidence-guard record):

```text
842e83d8e29814ae15137e162a9c938f1af5e77df284ee297af24ed3c1880dad  browser-host-map-reserved-proof.txt
```
