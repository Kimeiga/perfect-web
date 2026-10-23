# E14-E: a decision brief for the owner

**Status: a proposal, 2026-10-21. Nothing here is decided, and no model was
called.** The integrator prepared it from a read-only pass over the harness
at `e7664d2e` (drafted with a research subagent; its claims carry file:line
references, and the changed-lines defect below was reproduced and repaired
before this was recorded). The three decisions are the owner's
([E14 plan](E14.md), "Decisions the owner holds"). Line references are
`benchmarks/harness/bench.mjs` unless they name another file.

## 1. What exists, and what E14-E still needs

**The agent contract.** `--agent` is `noop`, `replay:<reference|unsafe>` or a
shell command (:210-229). A real agent runs as `bash -c <cmd>` in the
sandbox's `app/`, a git repository (:180-191), with `BENCH_PROMPT` (the path
of a verbatim copy of the task's `prompt.md`, :192) and `BENCH_STACK`, under
`BENCH_AGENT_SECONDS`, 1,800 by default (:225-226). Success is exit 0; the
last 40 lines of output are kept (:228). Nothing tells an agent how to run a
checker: the Pleris sandbox holds only `domain.pw`, `lib/` and `store/`
(:56-60), and `pw check` runs from the repository (:244-252, :267-269); the
frameworks ship `pnpm typecheck|build` (`baselines/*/package.json:6-8`).
Isolation compares bytes (:141-147, :194-200); a process can still read the
repository by path (ADR-0124:84-86, `EVIDENCE_LEDGER.md:132`, open).

**Each run records** (:387-399) `task`, `stack`, `agent` (the command),
`score`, `failed_at` (agent, check, build, serve, contract or hidden),
`changed_lines`, `agent_run` (ok, seconds, the output's tail), `stages[]`
(counts and failing tests' titles) and `commit`. Results are gitignored
(`.gitignore:38`); committed traces are `docs/evidence/E14/harness-T*.txt`.

**Not built** (searched `benchmarks/`, `justfile`, `scripts/`):
- an agent adapter; a model id; tokens, cost, turns; tool, turn and token
  budgets (only wall-clock seconds and a time cap exist; charter M14 task 7
  asks for the "same time/tool budget");
- seeds, a matrix runner and aggregation (`run()` is one task, stack and
  agent, :371);
- teaching conditions (`docs/guide/` is written for "an AI agent", but omits
  `<stream>` and `<dialog>`; `examples/store/app.pw` and
  `examples/demo/dialog.pw` hold T05, T10 and T11's answers);
- counts of compiler iterations and warnings (the checker runs once, after
  the agent, :267-274);
- a "not expressible" status: `run()` throws for an omitted stack (:374).
  All twelve tasks are gradable on Pleris today (E14.md).

**Defects to repair before any paid run:**
- **`changed_lines` missed every file an agent makes** (repaired, and
  landed with train-13 on 2026-10-23; `just e14-bench-footprint`). It read `git diff
  --numstat`, which lists only tracked files. Measured on the committed
  patches: Next.js's T01 reference 27 lines, really 77; T06 3, really 48; T08
  32, really 52; T11 10, really 37 (their unsafe patches alike). Pleris's and
  SvelteKit's patches only edit, and are unchanged. The footprint row of
  every comparison so far understated Next.js's changes in four of twelve
  tasks.
- An agent's non-zero exit or timeout scores 0 at `failed_at: "agent"`,
  ungraded (:384-386): an infrastructure failure reads as incapacity.
- Pleris builds write the shared `spikes/own-renderer/template-ir.json`
  (`run.sh:22,67`): grade Pleris runs one at a time. `commit` records no
  dirty flag and no toolchain digest (:398).

## 2. A bounded study design

**One scaffold in every cell:** one minimal agent loop; tools to read, edit,
list and `check` (`tsc`, `svelte-check`, `pw check`); one prompt; the same
turn, token and time caps.

**Five arms per model, task and seed:** next-react with no teaching,
sveltekit with none, pleris with none, pleris with the docs in context, and
pleris with a read-only docs tool. The frameworks get nothing extra.

| phase | models | tasks | seeds | runs |
|---|---|---|---|---|
| probe | 1, the cheaper | T04 | 1 | 5 |
| pilot | 2 | T03, T04, T09 | 3 | 90 |
| full | 2 | all 12 | 5 | 600, and 90 for a check-off ablation |

The pilot's tasks span E14.md's three classes: T03 refused whole (PW5001),
T04 in part (PW0305), T09 on no stack (the null control). Nine runs per
model and arm (about ±27 points, Wilson) measure cost, variance and grader
flake; they are not a result. Sixty per cell in the full run give about
±12 points at 50%, before clustering by task. Seeds are replicate indices,
not a provider's seeds.

**Gate item 4, capability against harness.** Capability is the contrast
between models on the framework arms. Harness effects are the teaching
delta (each docs arm minus pleris with none, within a model), a scripted
tool-path control, the grader's flake rate (references replayed ten times
per task and stack, no API), the infrastructure failure rate and,
optionally, the check-on/off delta. The stack effect is within a model,
paired by task and seed. Report per task, model and arm: passes over runs
with a Wilson 95% interval, Pleris-minus-framework differences, the
`failed_at` split, and tokens and cost per pass. Pool by a task-clustered
bootstrap. Never one score; pre-register the headline arm and decision rule.
Pleris starts valid on ten of twelve tasks, so `pw check` acts only when an
agent writes the unsafe form: log each check, to count "refused, then
repaired".

## 3. The three decisions

**1. Models, agents and budget.**
- A, recommended: one provider, two tiers, the custom loop above: Claude
  Sonnet 5.5 (`claude-sonnet-5-5`) and Haiku 5.5 (`claude-haiku-5-5`).
  Cheap, with a controlled scaffold. The drafting assistant is an Anthropic
  model; the vendor is the owner's choice.
- B: A, and a non-Anthropic or a local model in the full run: across
  vendors; charter M14 task 9 defers small agents; names and prices
  unverified here.
- C: a vendor's agent as is, e.g. `claude -p --output-format json`: least
  code, but its own prompts and tools confound the comparison, and its shell
  makes the open isolation item a prerequisite.
- In any case: approve the five-run probe first, with a spending cap per
  phase that the orchestrator enforces.

**2. How Pleris is taught.**
- Docs in context: simple and constant; the narrative guide is about 65 KB,
  and under-covers T05, T10 and T11.
- A docs tool: pays per page read, and gives Pleris a tool the frameworks
  lack.
- Both: the costliest, and confounds the two.
- Recommended: none, docs-in-context and docs-tool in the pilot, with
  docs-in-context as the pre-registered headline and teaching tokens
  reported apart. Any option needs a frozen, allow-listed corpus that
  excludes `examples/store`, `examples/demo/dialog.pw`, ADR-0120 to
  ADR-0158, `docs/milestones/E14.md` and `docs/evidence/E14/`, with a leak
  scan.

**3. A task Pleris cannot express.**
- As a failure in the headline: pessimistic, and mixes language gaps with
  agent ability.
- Beside it: the headline over tasks with a passing reference, the excluded
  count in the same row.
- Recommended: beside it, with the strict figure shown too. Moot today (0 of
  12), but tasks have become expressible by changing the language
  (ADR-0148), so it governs T13 onwards, and applies to the frameworks too.

## 4. Budget arithmetic

```text
cost = (1 + r) * runs * mean(I_new * p_in + I_cw * p_cw + I_cr * p_cr + O * p_out) / 1e6
```

`I_new`, `I_cw`, `I_cr` and `O` are a run's uncached input, cache-write,
cache-read and output tokens over all its turns (thinking counts as output);
`p_*` is the price per million of each; `r` is the rerun overhead for
infrastructure failures. Runs: probe 5, pilot 90, full 600 at two models and
five seeds (900 at three), ablation 90. Tokens per run are unmeasured; the
probe measures them. Prices as the drafting pass read them on 2026-10-21 at
https://platform.claude.com/docs/en/about-claude/pricing, per million
tokens (input / output / cache read / 5-minute cache write): Sonnet 5.5
$2 / $10 / $0.10 / $2.50; Haiku 5.5 $0.10 / $0.50 / $0.01 / $0.125 up to
100K-token prompts. Re-read them before spending.

## 5. Zero-cost work before the decisions

1. **A usage sidecar and a stub adapter**: the harness reads `$BENCH_USAGE`
   JSON into `agent_run.usage`. Accept: a stub applying the reference scores
   1 on T08 for Pleris with its usage recorded; `just e14-pleris-controls`
   unchanged.
2. **A scripted tool-path control**: the reference applied through the
   adapter's own tools scores 1 on all 36 task and stack pairs; no-op 0.
3. **`changed_lines`**: done, landed with train-13 (above).
4. **An infrastructure class**: `failed_at: "infra"` for a flagged failure,
   a reserved exit code or a timeout, excluded from denominators.
5. **A matrix runner**: `--dry-run`, resume, randomized block order with a
   recorded shuffle seed, Pleris graded one at a time, a spending ledger
   with a hard cap. Accept: a dry run prints exactly 5, 90 and 600; a
   simulated overspend halts it.
6. **The "not expressible" path** (a task field and a status, not a throw)
   and a report with Wilson intervals, paired differences, and strict and
   expressible-only headlines.
7. **The teaching corpus and its leak scan**: fails on a corpus holding
   `examples/store`, passes on the curated set.
8. **Provenance** (toolchain digest, dirty flag, prompt hash) and the
   grader's flake baseline.

One ruling precedes these: equal starting text (ADR-0124:87-90, below).

## 6. Threats to validity

- **Familiarity**: React and Svelte are heavily represented in training
  data; no model has seen Pleris. Never average across teaching arms.
- **Starting text**: the Pleris baseline is 39.9 KB with 41% comment lines,
  against 6.4-7.3 KB and 6-7% for the frameworks, and T01's sandbox comments
  name its answer (`baselines/pleris/lib/Carts.pw:60-72`), as reported by
  the drafting pass.
- **Leakage**: an inherited environment and a readable repository; `check`
  output must not print repository paths.
- **An in-sample checker**: rules were added after tasks exposed holes, so
  nine of twelve refused is partly by construction. Agent-made Pleris
  failures that pass `pw check` give the out-of-sample miss rate; add
  held-out tasks.
- **Nondeterminism**: replicates are samples; grading is timing-sensitive
  (T08's unsafe patch fails the contract's "two quick presses" test in some
  runs, ADR-0124:73-76).
- **Tool asymmetry**: `tsc` and `svelte-check` refuse none of the twelve
  wrong fixes and `pw check` nine (E14.md), which is the claim under test;
  Pleris's diagnostics also carry repair text; only the frameworks can run a
  server in their sandbox.
