# Non-goals

Charter §14 Milestone 0 task 12: *"Create `docs/vision/non-goals.md` to resist
scope creep."*

This file exists to be quoted back at anyone — human or agent — who proposes
building one of these. If you want to change a line here, that needs an ADR and a
human decision, not a commit.

---

## Never build (charter §2)

The charter is explicit: *"Do not initially build any of the following."*
Reuse commodity infrastructure instead.

```text
a database engine                    a SQL optimizer
a new TCP or QUIC implementation     a complete browser engine from scratch
a CSS layout engine                  a global CDN
a distributed database               a package registry
a bespoke binary replacement for HTML   a custom router appliance
a local foundation model             a universal CRDT system
a full production framework before proving the semantic model
```

The novel work is only the semantic spine:

```text
value types
+ algebraic effects and capabilities
+ privacy and placement
+ resource lifecycle and consistency
+ cache and invalidation graph
+ UI document-parts graph
+ resumption metadata
+ compiler-generated evidence
```

Everything else is a dependency, and every dependency needs a
reuse/fork/tape/build decision in `docs/research/technology-matrix.md`.

---

## Not yet — sequencing constraints

These are legitimate eventual goals that must not be started early. The charter
orders milestones to answer the highest-risk questions first (§14 preamble:
*"Do not skip directly to a custom browser or full compiler backend"*).

| thing | not before | why |
|---|---|---|
| **Forking Servo** | Milestone 13, and only after profiling identifies a specific overhead | §11.4. First embed *unmodified* Servo. A browser fork is the most expensive commitment available and must be justified by measurement, never by architectural aesthetics. |
| **Any browser fork at all** | same | The compatibility implementation must work in current Safari/Chrome/Firefox first (§11.1). |
| **Forking Koka** | after evidence that no public extension point works | §4 lists five conditions, all of which must hold. Milestone 0 found the `.kki` file already provides machine-readable effects, so the main motivation for a fork is **gone** (ADR-0001). |
| **Writing the own compiler** | Milestone 9 | §14 M9 rationale: *"Only after the semantics and real application are stable is a new compiler justified."* |
| **Writing the own renderer** | Milestone 7 | The golden test suite must exist first, with Marko as one oracle, or we risk building a fast renderer for the wrong model. |
| **A custom transport protocol** | never without evidence | §11.3: use an existing HTTP/3/QUIC implementation. §14 M12 gate: *"No custom transport protocol was introduced without evidence."* |
| **Automatic SQL dependency inference** | after Milestone 6 | §9.4 and ADR-0007: explicit typed events and a transactional outbox are the ground truth. Inference may later be an auditable optimization, never a replacement. |
| **WASI 0.3** | when Rust ships a stable `wasm32-wasip3` | ADR-0008 — measured, not assumed. |
| **PostgreSQL** | Milestone 11 | ADR-0005. SQLite first. |
| **Optimizing syntax** | after the semantics are demonstrated | §7 preamble. `.pw` syntax is provisional (ADR-0010). |
| **Branding** | never, as a task | §1: *"Do not spend time inventing branding."* |

---

## Not this project's problem

- **Complete web compatibility** in any experimental browser work. §14 M13:
  *"Do not spend time chasing complete web compatibility."*
- **Rust-equivalent performance universally.** §14 M10 task 5 forbids promising it.
- **Replacing HTML, URLs, CSS or HTTP.** §11.2 — they provide interoperability,
  accessibility, streaming, indexing, caching and debugging properties we would
  otherwise have to rebuild badly (ADR-0004).
- **Ownership/borrow syntax in ordinary application code.** §5 (Rust row) and
  §14 M10 task 6. Affine annotations appear only for genuinely scarce resources.
- **Optimizing for one AI model.** §19.3.

---

## Process non-goals

- **Do not claim a milestone is complete because code exists.** §3.1: a milestone
  is complete only when its objective gate passes.
- **Do not attempt milestones concurrently.** §0.
- **Do not hide uncertainty.** §3.3. A failed assumption goes in
  `docs/KNOWN_LIMITATIONS.md`, not into a hopeful sentence.
- **Do not create empty directories to look complete.** §12.
- **Do not ask broad design questions that can be resolved experimentally.** §3.2.
  Make a reasonable assumption, record it in `docs/ASSUMPTIONS.md`, validate it.
- **Do not push, publish, or modify external repositories** without explicit
  human authorization. §3.5.

---

## Things Milestone 0 was tempted by and did not do

Recorded because the temptation will recur.

- **Making the `no_std` component allocator good.** It is a 25-line bump
  allocator that never frees. Adequate for a spike; Milestone 8 owns the real one.
- **Adding a browser to the Marko spike.** Interactivity is inferred from
  payload sizes and the emitted resume manifest, not observed. Playwright is
  Milestone 3, and the limitation is stated plainly in the spike README rather
  than papered over.
- **Extending the toy parser toward a real grammar.** It parses one declaration
  form. Milestone 2 owns the grammar; growing the spike would have produced a
  second, competing parser.
- **Writing the `pw` compiler because the corpus looked like it wanted one.**
  The corpus is a specification. Milestone 2 makes it executable.
