#!/usr/bin/env node
// The bug museum (ADR-0342, "the claim, measured").
//
//   node museum.mjs                    # every exhibit
//   node museum.mjs --exhibit J08,F01  # the exhibits whose id starts so
//   node museum.mjs --checks-only      # Pleris's `pw check` halves alone, for
//                                      # the mutation controls (scripts/proof_mutations.py)
//
// An exhibit is a failure from the census (research/failures/), with a dated
// primary source, shown in two halves, each with its control:
//
//   the bug happening    in Next.js or SvelteKit: ADR-0120's baseline with
//                        the exhibit's patches, in place, on the dependencies
//                        the workspace installed (no download), checked by the
//                        stack's own checker, built, served, and
//                        `bug.spec.mjs` run on it, which passes when the bug
//                        happens. Control: the idiomatic repair, on which the
//                        same spec must fail.
//   Pleris's answer      the same change to the benchmark's Pleris store,
//                        given to `pw check`: refused by a named code, whose
//                        neighbour (the repair the code asks for) checks
//                        clean; or a named runtime test that passes; or, for a
//                        miss, the change checking clean.
//
// What an exhibit expects is its own (`exhibit.json`'s "expect"); the run
// fails when any half comes out otherwise, so a refusal that stops being made
// fails the recipe. A miss that is shown is a result, not a failure.

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const MUSEUM = path.dirname(fileURLToPath(import.meta.url));
const BENCH = path.resolve(MUSEUM, "..");
const REPO = path.resolve(BENCH, "..");
const HARNESS = path.join(BENCH, "harness");
const PLERIS_STORE = path.join(BENCH, "baselines/pleris");
const PW = path.join(REPO, "target/debug/pw");

// The brief's Playwright ports (8941-9140); each reproduction's server takes
// the next one from PORT + 140, in a gap the own renderer's suite leaves
// between its hosts' ports (PORT + 120..122 and PORT + 160..182), which a
// runtime test starts. Two servers an exhibit: twenty ports, ten exhibits.
const PORT = Number(process.env.PORT ?? 8941);
const CHECKS_ONLY = process.argv.includes("--checks-only");
let nextPort = PORT + 140;

const STACKS = {
  "next-react": {
    baseline: path.join(BENCH, "baselines/next-react"),
    storePath: "/stores/blue-bottle",
    serve: (port) => ["pnpm", ["--silent", "start"], { PORT: String(port) }],
  },
  sveltekit: {
    baseline: path.join(BENCH, "baselines/sveltekit"),
    storePath: "/stores/blue-bottle",
    serve: (port) => [
      "node",
      ["build"],
      { PORT: String(port), HOST: "127.0.0.1", ORIGIN: `http://127.0.0.1:${port}` },
    ],
  },
};

// --- small helpers -----------------------------------------------------------

function sh(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { encoding: "utf8", maxBuffer: 64 << 20, ...opts });
  return { ok: r.status === 0, status: r.status, out: `${r.stdout ?? ""}${r.stderr ?? ""}` };
}

function tail(text, n = 15) {
  return text.trim().split("\n").slice(-n).join("\n");
}

/**
 * Text as a reader sees it: a terminal's escape sequences (colours, and the
 * cursor moves Playwright's line reporter writes even into a pipe, with
 * FORCE_COLOR=0) taken out. J01's "1 passed" came after `ESC[1A ESC[2K`, and
 * a `^\s*1 passed` read the passing run as a failure.
 */
function plain(text) {
  return text.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "");
}

function files(dir) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...files(p));
    else if (e.isFile()) out.push(p);
  }
  return out;
}

/** A patch named by an exhibit: its own file, or one of the repository's. */
function patchPath(ex, name) {
  return name.startsWith("benchmarks/") ? path.join(REPO, name) : path.join(ex.root, name);
}

/**
 * Apply (or, `reverse`, take back) one patch to the files under `dir`, a
 * patch whose paths are relative to `dir`. Run from the top of `dir`'s own
 * repository with `--directory`: `git apply` run inside a repository's
 * subdirectory ignores, silently, every path outside that subdirectory, and
 * reads a patch's paths from the repository's top.
 */
function apply(ex, name, dir, reverse = false) {
  const top = sh("git", ["rev-parse", "--show-toplevel"], { cwd: dir }).out.trim();
  const rel = path.relative(top, dir);
  const args = ["apply", ...(reverse ? ["-R"] : []), ...(rel ? [`--directory=${rel}`] : []), patchPath(ex, name)];
  const r = sh("git", args, { cwd: top });
  if (!r.ok) throw new Error(`${name} does not ${reverse ? "come off" : "apply"}: ${tail(r.out)}`);
}

async function waitFor(url, ms) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    try {
      const r = await fetch(url);
      if (r.status < 500) return true;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

// --- the exhibits --------------------------------------------------------------

function exhibits(prefix) {
  return fs
    .readdirSync(MUSEUM, { withFileTypes: true })
    .filter((d) => d.isDirectory() && !d.name.startsWith(".") && (!prefix || prefix.split(",").some((x) => d.name.startsWith(x))))
    .map((d) => {
      const root = path.join(MUSEUM, d.name);
      const ex = JSON.parse(fs.readFileSync(path.join(root, "exhibit.json"), "utf8"));
      if (ex.id !== d.name) throw new Error(`${d.name}/exhibit.json names itself ${ex.id}`);
      return { ...ex, root };
    })
    .sort((a, b) => a.id.localeCompare(b.id));
}

// --- the bug, happening ----------------------------------------------------------

/** Check, build and serve the baseline as it stands, run the spec: its counts. */
async function showOnce(ex, app) {
  const s = STACKS[ex.repro.stack];
  // The stack's own checker first: the bug is one its checker accepts.
  const check = sh("pnpm", ["--silent", "typecheck"], { cwd: app });
  if (!check.ok) return { stage: "typecheck", out: tail(check.out) };
  const build = sh("pnpm", ["--silent", "build"], { cwd: app });
  if (!build.ok) return { stage: "build", out: tail(build.out) };

  const port = nextPort++;
  const [cmd, args, env] = s.serve(port);
  const server = spawn(cmd, args, {
    cwd: app,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  let serverOut = "";
  server.stdout.on("data", (d) => (serverOut += d));
  server.stderr.on("data", (d) => (serverOut += d));
  let specs = null;
  try {
    const base = `http://127.0.0.1:${port}`;
    // Up when it answers at all: a path no route serves, so that the probe
    // reads nothing of the store. Reading the store page here would be the
    // first read of everything it reads, and a store that keeps a read (T09's
    // notice, five minutes) would then keep the probe's, not the spec's.
    if (!(await waitFor(`${base}/__pw-museum-ready`, 60_000))) {
      return { stage: "serve", out: tail(serverOut) };
    }
    // The spec and the contract's fixtures, beside the harness so they
    // resolve its `@playwright/test`, as `bench.mjs` grades.
    fs.mkdirSync(path.join(HARNESS, ".grading"), { recursive: true });
    specs = fs.mkdtempSync(path.join(HARNESS, ".grading", `museum-${ex.id}-`));
    fs.copyFileSync(path.join(HARNESS, "contract/fixtures.mjs"), path.join(specs, "fixtures.mjs"));
    fs.copyFileSync(path.join(ex.root, "bug.spec.mjs"), path.join(specs, "bug.spec.mjs"));
    const report = path.join(specs, "report.json");
    sh("pnpm", ["exec", "playwright", "test", "--config", path.join(HARNESS, "grade.config.mjs")], {
      cwd: HARNESS,
      env: {
        ...process.env,
        BENCH_SPECS: specs,
        BENCH_REPORT: report,
        BENCH_STACK: ex.repro.stack,
        BENCH_BASE_URL: base,
        BENCH_STORE_PATH: s.storePath,
      },
    });
    if (!fs.existsSync(report)) return { stage: "spec", out: "no report" };
    const stats = JSON.parse(fs.readFileSync(report, "utf8")).stats ?? {};
    const failures = [];
    const walk = (suite) => {
      for (const spec of suite.specs ?? []) {
        for (const t of spec.tests ?? []) {
          if (t.status !== "expected") {
            const last = t.results?.[t.results.length - 1];
            const why = plain(last?.error?.message ?? "").split("\n")[0];
            failures.push(`${spec.title}: ${why}`.slice(0, 300));
          }
        }
      }
      for (const inner of suite.suites ?? []) walk(inner);
    };
    for (const suite of JSON.parse(fs.readFileSync(report, "utf8")).suites ?? []) walk(suite);
    return {
      stage: "spec",
      passed: stats.expected ?? 0,
      failed: (stats.unexpected ?? 0) + (stats.flaky ?? 0),
      failures,
    };
  } finally {
    if (specs) fs.rmSync(specs, { recursive: true, force: true });
    try {
      process.kill(-server.pid, "SIGTERM");
    } catch {
      /* already gone */
    }
  }
}

/**
 * The bug, shown: the spec passes on the bug and fails on its repair. Each a
 * spec that ran: a stage of no tests proves nothing either way.
 *
 * In place, in this worktree's baseline, with the dependencies the workspace
 * installed for it: the brief allows the baselines' existing dependencies and
 * nothing new, and `bench.mjs`'s sandbox installs its own pinned lockfile,
 * which on this host needs a package the store does not hold (nanoid 3.3.19;
 * the workspace's is 3.3.18). `scripts/bench_contract_mutations.py` builds its
 * mutants in place the same way. A baseline with changes git has not
 * committed is refused, so nothing of anyone's is overwritten; every patch is
 * taken back, last first, and the baseline built again as it is.
 */
async function showBug(ex) {
  const app = STACKS[ex.repro.stack].baseline;
  const dirty = sh("git", ["status", "--porcelain", "--", "."], { cwd: app }).out.trim();
  if (dirty) {
    throw new Error(`${path.relative(REPO, app)} has changes git has not committed; the museum patches it in place:\n${dirty}`);
  }
  const applied = [];
  try {
    for (const name of [...(ex.repro.base ?? []), ...(ex.repro.bug ? [ex.repro.bug] : [])]) {
      apply(ex, name, app);
      applied.push(name);
    }
    const bug = await showOnce(ex, app);
    apply(ex, ex.repro.fixed, app);
    applied.push(ex.repro.fixed);
    const fixed = await showOnce(ex, app);
    const ran = (r) => r.stage === "spec" && r.passed + r.failed > 0;
    return {
      happens: ran(bug) && bug.failed === 0,
      control: ran(fixed) && fixed.failed > 0,
      bug,
      fixed,
    };
  } finally {
    for (const name of applied.reverse()) apply(ex, name, app, true);
    sh("pnpm", ["--silent", "build"], { cwd: app });
  }
}

// --- Pleris's answer ---------------------------------------------------------------

/** `pw check` on the benchmark's Pleris store with these patches: its codes. */
function plerisCheck(ex, patches) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), `pw-museum-${ex.id}-`));
  try {
    for (const part of ["domain.pw", "lib", "store"]) {
      fs.cpSync(path.join(PLERIS_STORE, part), path.join(root, part), { recursive: true });
    }
    sh("git", ["init", "-q"], { cwd: root });
    for (const name of patches) apply(ex, name, root);
    const sources = [
      ...files(path.join(REPO, "packages/pw-std")),
      ...files(path.join(REPO, "packages/pw-platform-web")),
      ...files(root).filter((f) => !f.includes(`${path.sep}.git${path.sep}`)),
    ].filter((f) => f.endsWith(".pw"));
    const r = sh(PW, ["check", "--plain", ...sources]);
    const codes = [...new Set([...r.out.matchAll(/^error: \[(PW\d{4})\]/gm)].map((m) => m[1]))];
    const first = r.out.split("\n").find((l) => l.startsWith("error: ")) ?? "";
    return { ok: r.ok, codes, first };
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

/** A named test of the Pleris runtime: a command and what it must print. */
function plerisRuntime(ex) {
  const t = ex.pleris.test;
  const r = sh("bash", ["-c", t.command], {
    cwd: REPO,
    env: { ...process.env, PORT: String(PORT) },
  });
  const out = plain(r.out);
  return { ok: r.ok && new RegExp(t.expect, "m").test(out), out: tail(out, 6) };
}

/**
 * Pleris's answer: the change given to `pw check` (when the exhibit has one),
 * its neighbour (the repair the code asks for, on the change; with no repair
 * patch, the program without the change), and a runtime test (when it has
 * one). A diagnostic exhibit may have a runtime test too: the code refuses the
 * program, and the test shows the repaired one does what the code promised.
 */
function answer(ex) {
  const p = ex.pleris;
  const base = p.base ?? [];
  if (!["diagnostic", "runtime", "miss"].includes(p.kind)) {
    throw new Error(`${ex.id}: no Pleris answer of kind ${p.kind}`);
  }
  const said = {};
  if (p.patch) {
    said.program = plerisCheck(ex, [...base, p.patch]);
    if (p.kind === "diagnostic") {
      said.neighbour = plerisCheck(ex, p.fixed ? [...base, p.patch, p.fixed] : base);
    }
  }
  if (p.test && !CHECKS_ONLY) said.runtime = plerisRuntime(ex);
  return said;
}

// --- one exhibit, then all -----------------------------------------------------------

function verdict(ex, shown, said) {
  const p = ex.pleris;
  const runtimeHolds = !said.runtime || said.runtime.ok;
  let status;
  let holds;
  if (p.kind === "diagnostic") {
    const refused = !said.program.ok && said.program.codes.includes(p.code);
    // The refusal is for this bug: the neighbour checks clean.
    holds = refused && said.neighbour.ok && runtimeHolds;
    status = refused
      ? `refused (diagnostic ${p.code}${said.runtime ? `; runtime, ${p.mechanism}` : ""})`
      : `NOT refused (${said.program.codes.join(", ") || "checks clean"})`;
  } else if (p.kind === "runtime") {
    holds = runtimeHolds && !!said.runtime;
    status = holds ? `refused (runtime, ${p.mechanism})` : "NOT refused (its runtime test failed)";
  } else {
    // A miss: the change Pleris would need to refuse checks clean.
    holds = said.program.ok && runtimeHolds;
    status = said.program.ok ? `miss (${p.why})` : `refused after all (${said.program.codes.join(", ")})`;
  }
  const expected = ex.expect === "miss" ? status.startsWith("miss") : status.startsWith("refused");
  return { status, ok: shown.happens && shown.control && holds && expected };
}

function counts(r) {
  if (r.stage === "not run") return "not run (--checks-only)";
  return r.stage === "spec" ? `${r.passed} passed, ${r.failed} failed` : `stopped at ${r.stage}`;
}

async function main() {
  const prefix = process.argv.includes("--exhibit") ? process.argv[process.argv.indexOf("--exhibit") + 1] : undefined;
  if (!fs.existsSync(PW)) throw new Error(`no ${PW}: build it first (cargo build -p pw-cli)`);
  const rows = [];
  // With --checks-only, an exhibit whose answer is a runtime test alone has
  // nothing to check.
  for (const ex of exhibits(prefix).filter((e) => !CHECKS_ONLY || e.pleris.patch)) {
    console.log(`== ${ex.id}: ${ex.title}`);
    console.log(`   census ${[ex.leaf, ...(ex.also ?? [])].join(", ")}; source ${ex.source.ref ?? ""} ${ex.source.kind}, ${ex.source.date}: ${ex.source.url}`);
    // What changed in the exhibit's answer since the museum first recorded
    // it (H07: a miss, then refused by ADR-0340's checker), said where it is
    // read.
    if (ex.history) console.log(`   history: ${ex.history}`);
    const notRun = { stage: "not run" };
    const shown = CHECKS_ONLY
      ? { skipped: true, happens: true, control: true, bug: notRun, fixed: notRun }
      : await showBug(ex);
    console.log(`   ${ex.repro.stack}, the bug: ${counts(shown.bug)}${shown.happens ? "" : "  <- the bug did not happen"}`);
    for (const f of shown.bug.failures ?? []) console.log(`     ${f}`);
    if (shown.bug.out) console.log(shown.bug.out.replace(/^/gm, "     "));
    // A half not run says only that: no claim about a spec that did not run.
    const controlSaid = shown.skipped ? "" : shown.control ? " (the spec fails: it detects the bug)" : "  <- the repair did not make the spec fail";
    console.log(`   ${ex.repro.stack}, repaired: ${counts(shown.fixed)}${controlSaid}`);
    if (shown.fixed.out) console.log(shown.fixed.out.replace(/^/gm, "     "));
    const said = answer(ex);
    if (said.program) {
      console.log(`   pleris, the same change: ${said.program.ok ? "checks clean" : `refused: ${said.program.first}`}`);
    }
    if (said.neighbour) {
      console.log(`   pleris, its neighbour: ${said.neighbour.ok ? "checks clean" : `refused: ${said.neighbour.first}`}`);
    }
    if (said.runtime) {
      console.log(`   pleris, at runtime: ${said.runtime.ok ? "passes" : "FAILS"}: ${ex.pleris.test.name}`);
      console.log(said.runtime.out.replace(/^/gm, "     "));
    }
    const v = verdict(ex, shown, said);
    console.log(`   => ${v.status}${v.ok ? "" : "  <- NOT AS THE EXHIBIT EXPECTS"}`);
    console.log("");
    rows.push({ ex, shown, ...v });
  }

  // Misses first, then whatever is not as its exhibit expects, then refusals:
  // what Pleris does not catch is read first.
  const order = (r) => (!r.ok ? 1 : r.status.startsWith("miss") ? 0 : 2);
  rows.sort((a, b) => order(a) - order(b) || a.ex.id.localeCompare(b.ex.id));
  console.log("exhibit | census | source | the bug, in | its repair | Pleris");
  for (const r of rows) {
    const bug = `${r.ex.repro.stack}: ${r.shown.skipped ? "not run" : r.shown.happens ? "shown" : "NOT SHOWN"}`;
    const repair = r.shown.skipped ? "not run" : r.shown.control ? "fails the spec" : "DOES NOT FAIL THE SPEC";
    const mark = r.ok ? "" : "  <- NOT AS EXPECTED";
    console.log(`${r.ex.id} | ${r.ex.leaf} | ${r.ex.source.kind}, ${r.ex.source.date} | ${bug} | ${repair} | ${r.status}${mark}`);
  }
  const misses = rows.filter((r) => r.status.startsWith("miss")).length;
  const refused = rows.filter((r) => r.status.startsWith("refused")).length;
  const wrong = rows.filter((r) => !r.ok).length;
  console.log(`museum: ${rows.length} exhibits, ${refused} refused, ${misses} missed, ${wrong} not as expected`);
  process.exit(wrong ? 1 : 0);
}

await main();
