# Web Failure Census

The active research inventory and all fourteen requested report sections are in
[research/failures](../../research/failures/README.md).

- [Taxonomy and failure table](../../research/failures/CENSUS.md)
- [Compile-fail, runtime, and policy obligations](../../research/failures/OBLIGATIONS.md)
- [DX, performance, security, prior art, priorities, and residual uncertainty](../../research/failures/REQUIREMENTS.md)
- [Implemented tooling changes and evidence](../../research/failures/IMPLEMENTATION.md)

The priority and proposed charter/risk/milestone amendments have one home:
`research/failures/REQUIREMENTS.md`, Parts 12 and 13. Do not copy that backlog into
several independent checklists. Link stable failure IDs from actual implementation
work. These proposals do not displace the locked type-checker repair in `docs/NEXT.md`.

Governance: [ADR-0027](../DECISIONS/ADR-0027-failure-census-and-evidence-validity.md).

The review is pinned to commit `0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45`.
It is broad, not an exhaustive enumeration of all web history. Counts are inventory
counts, not percentages of all possible failures or claims of completed fixes.
