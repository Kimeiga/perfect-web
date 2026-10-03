// The harness refuses a sandbox holding a file the agent must not see
// (ADR-0124), and the control: the same task without the leak is accepted.
import { after, test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { sandbox } from "../bench.mjs";

// The task directories this test makes, removed when it is done (ADR-0158).
const made = [];
after(() => {
  for (const root of made) fs.rmSync(root, { recursive: true, force: true });
});

function fakeTask(leak) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "pw-bench-isolation-"));
  made.push(root);
  fs.mkdirSync(path.join(root, "hidden"));
  fs.mkdirSync(path.join(root, "setup"));
  fs.writeFileSync(path.join(root, "hidden", "secret.spec.mjs"), "the hidden test\n");
  fs.writeFileSync(path.join(root, "prompt.md"), "do something\n");
  if (leak) {
    // A setup that writes the hidden test's bytes into the program.
    fs.writeFileSync(
      path.join(root, "setup", "pleris.patch"),
      [
        "diff --git a/leak.txt b/leak.txt",
        "new file mode 100644",
        "--- /dev/null",
        "+++ b/leak.txt",
        "@@ -0,0 +1 @@",
        "+the hidden test",
        "",
      ].join("\n"),
    );
  }
  return { id: "TX", root, prompt: path.join(root, "prompt.md"), hidden: "hidden/secret.spec.mjs" };
}

test("a sandbox holding a hidden file's bytes is refused", () => {
  assert.throws(() => sandbox(fakeTask(true), "pleris"), /holds hidden files/);
});

test("the same task without the leak makes a sandbox", () => {
  const box = sandbox(fakeTask(false), "pleris");
  assert.ok(fs.existsSync(path.join(box.app, "store", "app.pw")));
  assert.ok(fs.existsSync(path.join(box.root, "PROMPT.md")));
  fs.rmSync(box.root, { recursive: true, force: true });
});
