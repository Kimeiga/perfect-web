// A run's changed lines count the files an agent makes beside the ones it
// edits, and nothing git ignores (E14-E's footprint, charter §14 M14). Until
// 2026-10-21 the harness read `git diff --numstat` alone, which lists only
// the files git tracks: T06's Next.js reference, which makes its 41-line form
// in a file of its own, counted 3 (`docs/evidence/E14/harness-T06.txt`).
import { after, test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { changedLines, sandbox } from "../bench.mjs";

// The task directories this test makes, removed when it is done (ADR-0158).
const made = [];
after(() => {
  for (const root of made) fs.rmSync(root, { recursive: true, force: true });
});

function fakeTask() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "pw-bench-changed-lines-"));
  made.push(root);
  fs.mkdirSync(path.join(root, "hidden"));
  fs.writeFileSync(path.join(root, "hidden", "secret.spec.mjs"), "the hidden test\n");
  fs.writeFileSync(path.join(root, "prompt.md"), "do something\n");
  return { id: "TX", root, prompt: path.join(root, "prompt.md"), hidden: "hidden/secret.spec.mjs" };
}

/** The measure the harness had: tracked files' added and removed lines. */
function trackedOnly(app) {
  const out = spawnSync("git", ["diff", "--numstat"], { cwd: app, encoding: "utf8" }).stdout.trim();
  return out
    ? out.split("\n").reduce((n, l) => n + l.split("\t").slice(0, 2).map(Number).reduce((a, b) => a + b, 0), 0)
    : 0;
}

test("a file the agent makes counts with the lines it edits, and an ignored one does not", () => {
  const box = sandbox(fakeTask(), "pleris");
  try {
    // An edit: the program's first line replaced, one line out and one in.
    const program = path.join(box.app, "store", "app.pw");
    const text = fs.readFileSync(program, "utf8");
    fs.writeFileSync(program, text.replace(/^[^\n]*\n/, "// replaced\n"));
    // A file of five lines the agent makes.
    fs.writeFileSync(path.join(box.app, "store", "made.pw"), "a\nb\nc\nd\ne\n");
    // And what a build leaves where the sandbox ignores it.
    fs.mkdirSync(path.join(box.app, "node_modules", "x"), { recursive: true });
    fs.writeFileSync(path.join(box.app, "node_modules", "x", "index.js"), "1\n2\n");

    // The old measure saw the edit alone.
    assert.equal(trackedOnly(box.app), 2);
    assert.equal(changedLines(box.app), 2 + 5);
  } finally {
    fs.rmSync(box.root, { recursive: true, force: true });
  }
});

test("a sandbox nothing changed has no changed lines", () => {
  const box = sandbox(fakeTask(), "pleris");
  try {
    assert.equal(changedLines(box.app), 0);
  } finally {
    fs.rmSync(box.root, { recursive: true, force: true });
  }
});

test("changes git cannot list are refused, not counted as none", () => {
  const notARepository = fs.mkdtempSync(path.join(os.tmpdir(), "pw-bench-no-git-"));
  made.push(notARepository);
  fs.writeFileSync(path.join(notARepository, "made.pw"), "a\n");
  assert.throws(() => changedLines(notARepository), /could not be listed/);
});
