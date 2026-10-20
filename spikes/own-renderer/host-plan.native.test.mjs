// The host plan against the real, pinned Playwright CLI (1.58.0): private
// fixture suites, each in a temporary directory with a configuration of its
// own and a private HTTP host per planned role, never the renderer's builds.
// Each test runs `playwright test` as a caller would and compares the tests
// Playwright planned with the tests it ran, and the hosts it declared with
// the hosts that started. Slow (each run is two Playwright processes):
// `just e14-browser-host-map` runs it, and the host map's mutation controls.
// Its hosts listen on PW_HOST_TEST_PORT (PORT where unset, 3141 where both
// are) plus the registry's offsets.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, existsSync, mkdtempSync, mkdirSync, openSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import net from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const P = Number(process.env.PW_HOST_TEST_PORT ?? process.env.PORT ?? 3141);
const numeric = (a, b) => a - b;
const METADATA_MODULE = new URL("./host-metadata.mjs", import.meta.url).href;

const require = createRequire(import.meta.url);
const CLI = require.resolve("@playwright/test/cli");
const PLAN_MODULE = new URL("./host-plan.mjs", import.meta.url).href;
const SPIKE = fileURLToPath(new URL(".", import.meta.url));
const records = (file) => {
  try { return readFileSync(file, "utf8").trim().split("\n").filter(Boolean).map((line) => JSON.parse(line)); }
  catch (error) { if (error.code === "ENOENT") return []; throw error; }
};

// `owned`: the fixture's host is run as the real configuration runs it, by the
// launcher that ends it with its runner (`ownedCommand`), and its one test
// hangs, so that a runner can be killed in the middle of a run.
function nativeFixture(owned = false) {
  const root = mkdtempSync(join(tmpdir(), "pw-host-native-"));
  mkdirSync(join(root, "e2e"));
  symlinkSync(join(SPIKE, "node_modules"), join(root, "node_modules"), "dir");
  writeFileSync(join(root, "package.json"), JSON.stringify({ private: true, type: "module", packageManager: "pnpm@10.15.1" }));
  mkdirSync(join(root, "public"));
  writeFileSync(join(root, "public/pw-runtime.mjs"), "private test runtime");
  mkdirSync(join(root, "dist/build"), { recursive: true });
  writeFileSync(join(root, "dist/pw-runtime.mjs"), "private test runtime");
  writeFileSync(join(root, "dist/sources.json"), '{"files":[]}');
  const plans = join(root, "plans.jsonl");
  const hosts = join(root, "hosts.jsonl");
  const bodies = join(root, "bodies.jsonl");
  writeFileSync(join(root, "host.mjs"), `import http from "node:http";import {appendFileSync} from "node:fs";
const log = value => appendFileSync(process.env.PW_PROBE_LOG,JSON.stringify(value)+"\\n");
log({phase:"launch",pid:process.pid,port:Number(process.env.PORT),data:Object.hasOwn(process.env,"KIOKUN_DATA"),app:Object.hasOwn(process.env,"KIOKUN_APP")});
const server=http.createServer((q,r)=>r.end("private host")).listen(Number(process.env.PORT),"127.0.0.1");
for(const signal of ["SIGTERM","SIGINT"])process.on(signal,()=>server.close(()=>process.exit(0)));
process.on("exit",()=>log({phase:"exit",pid:process.pid}));
`);
  writeFileSync(join(root, "custom.config.mjs"), `import {defineConfig} from "@playwright/test";import {appendFileSync} from "node:fs";
import {selectedTests,serversForSelection,ownedCommand} from ${JSON.stringify(PLAN_MODULE)};
const selection=await selectedTests({root:${JSON.stringify(root)}});
appendFileSync(${JSON.stringify(plans)},JSON.stringify({pid:process.pid,ppid:process.ppid,reused:selection.reused,list:selection.collectionOnly,ids:selection.receipt?.tests.map(t=>t.id),nonce:selection.receipt?.nonce,roleBindings:selection.receipt?.tests.map(t=>[t.id,t.roles]),context:process.env.PW_HOST_INVOCATION})+"\\n");
export default defineConfig({testDir:"./e2e",forbidOnly:true,outputDir:"./custom-results",projects:[{name:"chromium"},{name:"firefox"},{name:"webkit"}],use:{baseURL:"http://127.0.0.1:${P}"},webServer:serversForSelection(selection,{root:${JSON.stringify(root)},port:${P}}).map(server=>({...server,command:${owned ? 'ownedCommand(["node","host.mjs"])' : '"node host.mjs"'},env:{...server.env,PW_PROBE_LOG:${JSON.stringify(hosts)}}${owned ? "" : ',gracefulShutdown:{signal:"SIGTERM",timeout:1000}'}}))});
`);
  const marker = join(root, "running");
  writeFileSync(join(root, "e2e/signals.spec.mjs"), owned
    ? `import {test} from "@playwright/test";import {writeFileSync} from "node:fs";
test.setTimeout(0);
test("hangs while its host is up",async()=>{writeFileSync(${JSON.stringify(marker)},"running");await new Promise(()=>{});});
`
    : `import {test,expect} from "@playwright/test";import {appendFileSync} from "node:fs";
test("plain alpha",async({request},info)=>{expect((await request.get("/")).status()).toBe(200);appendFileSync(${JSON.stringify(bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});
test("value --ui stays opaque",async({request},info)=>{expect((await request.get("/")).status()).toBe(200);appendFileSync(${JSON.stringify(bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});
`);
  return { root, plans, hosts, bodies, marker, runs: 0 };
}

async function native(fixture, args, extraEnv = {}, onSpawn = () => {}) {
  const output = join(fixture.root, `native-${fixture.runs++}.log`);
  const descriptor = openSync(output, "wx", 0o600);
  const env = { ...process.env, PORT: String(P), KIOKUN_DATA: join(fixture.root, "private-data"), KIOKUN_APP: join(fixture.root, "private-app"), ...extraEnv };
  if (!Object.hasOwn(extraEnv, "PWTEST_WATCH")) delete env.PWTEST_WATCH;
  const child = spawn(process.execPath, [CLI, "test", "--config", "custom.config.mjs", ...args], { cwd: fixture.root, env, detached: false, stdio: ["ignore", descriptor, descriptor] });
  onSpawn(child);
  let timer;
  let escalation;
  let timedOut = false;
  try {
    const result = await new Promise((accept, reject) => {
      child.once("error", reject);
      child.once("exit", (status, signal) => accept({ status, signal }));
      timer = setTimeout(() => {
        timedOut = true;
        child.kill("SIGTERM");
        escalation = setTimeout(() => child.kill("SIGKILL"), 3000);
      }, 45000);
    });
    assert.equal(timedOut, false, `native fixture exceeded its bound: ${readFileSync(output, "utf8")}`);
    return { ...result, text: readFileSync(output, "utf8") };
  } finally {
    clearTimeout(timer);
    clearTimeout(escalation);
    closeSync(descriptor);
  }
}



test("native same-config CLI executes its planned ID and real IPC reload with one sanitized host", async () => {
  const fixture = nativeFixture();
  try {
    const listed = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--list"]);
    assert.equal(listed.status, 0, listed.text);
    assert.deepEqual(records(fixture.hosts), []);
    const ran = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--grep", "--ui", "--workers=1", "--reporter=line"]);
    assert.equal(ran.status, 0, ran.text);
    const body = records(fixture.bodies);
    const plans = records(fixture.plans);
    const hosts = records(fixture.hosts);
    assert.equal(body.length, 1);
    const root = plans.find((plan) => !plan.reused && plan.list === false && plan.ids);
    const worker = plans.find((plan) => plan.reused);
    assert.deepEqual(root.ids, [body[0].id]);
    assert.deepEqual(worker.ids, root.ids);
    assert.equal(worker.pid, body[0].pid);
    assert.equal(worker.ppid, root.pid);
    assert.equal(worker.nonce, root.nonce);
    assert.notEqual(plans.find((plan) => plan.list && plan.ids).nonce, root.nonce);
    const launched = hosts.filter((host) => host.phase === "launch");
    assert.equal(launched.length, 1);
    assert.equal(launched[0].port, P);
    assert.equal(launched[0].data, false);
    assert.equal(launched[0].app, false);
    assert.deepEqual(hosts.filter((host) => host.phase === "exit").map((host) => host.pid), [launched[0].pid]);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});
test("native required separator values and repeated reporters preserve the actual caller reporter", async () => {
  const fixture = nativeFixture();
  try {
    const ran = await native(fixture, ["--grep", "--", "--reporter=list", "--reporter=line", "--project=chromium", "--workers=1", "--", "signals.spec.mjs"]);
    assert.equal(ran.status, 0, ran.text);
    assert.match(ran.text, /1 passed/);
    assert.match(ran.text, /\[1\/1\]/);
    const body = records(fixture.bodies);
    const plan = records(fixture.plans).find((value) => !value.list && value.ids && !value.reused);
    assert.equal(body.length, 1);
    assert.deepEqual(plan.ids, [body[0].id]);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("deleted required recorded inputs refuse before any selected native host starts", async () => {
  const fixture = nativeFixture();
  try {
    const source = join(fixture.root, "required.pw");
    writeFileSync(source, "required source");
    const sha256 = createHash("sha256").update("required source").digest("hex");
    writeFileSync(join(fixture.root, "dist/sources.json"), JSON.stringify({ files: [{ path: source, sha256 }] }));
    rmSync(source);
    const ran = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--workers=1", "--reporter=line"]);
    assert.equal(ran.status, 1, ran.text);
    assert.match(ran.text, /missing required source/);
    assert.deepEqual(records(fixture.hosts), []);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

async function measuredNative(fixture, args, extraEnv = {}, status = 0) {
  const startBody = records(fixture.bodies).length;
  const startPlans = records(fixture.plans).length;
  const outcome = await native(fixture, args, extraEnv);
  assert.equal(outcome.status, status, outcome.text);
  const body = records(fixture.bodies).slice(startBody);
  const root = records(fixture.plans).slice(startPlans).find((plan) => plan.ids && !plan.reused);
  assert.ok(root, outcome.text);
  if (!root.list) {
    assert.deepEqual([...root.ids].sort(), body.map((value) => value.id).sort());
    assert.deepEqual([...root.roleBindings].sort(), body.map((value) => [value.id, ["base"]]).sort());
  }
  return { outcome, root, body };
}

test("native file line project wildcard repeated grep invert and shards retain exact IDs and roles", async () => {
  const fixture = nativeFixture();
  try {
    const multiple = await measuredNative(fixture, ["signals.*spec.mjs", "--project=fire*", "--project=chromium", "--grep", "plain|value", "--grep-invert", "plain", "--workers=1"]);
    assert.equal(multiple.body.length, 2);
    const line = await measuredNative(fixture, ["signals.spec.mjs:3", "--project=firefox", "--workers=1"]);
    assert.equal(line.body.length, 1);
    const config = join(fixture.root, "custom.config.mjs");
    writeFileSync(config, readFileSync(config, "utf8").replace('testDir:"./e2e",', 'testDir:"./e2e",fullyParallel:true,'));
    const first = await measuredNative(fixture, ["signals.spec.mjs", "--project=chromium", "--shard=1/2", "--workers=1"]);
    const second = await measuredNative(fixture, ["signals.spec.mjs", "--project=chromium", "--shard=2/2", "--workers=1"]);
    assert.equal(first.body.length, 1);
    assert.equal(second.body.length, 1);
    assert.notEqual(first.body[0].id, second.body[0].id);
    assert.equal(new Set([...first.root.ids, ...second.root.ids]).size, 2);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native test-list and custom outputDir last-failed preserve selections and list never rewrites last-run", async () => {
  const fixture = nativeFixture();
  try {
    const listed = await measuredNative(fixture, ["signals.spec.mjs", "--project=chromium", "--grep", "value", "--list"]);
    assert.equal(listed.root.ids.length, 1);
    writeFileSync(join(fixture.root, "selection.txt"), listed.outcome.text);
    const selected = await measuredNative(fixture, ["--test-list=selection.txt", "--project=chromium", "--workers=1"]);
    assert.deepEqual(selected.root.ids, listed.root.ids);
    const file = join(fixture.root, "e2e/signals.spec.mjs");
    const source = readFileSync(file, "utf8");
    writeFileSync(file, source.replace('test("plain alpha",async({request},info)=>{', 'test("plain alpha",async({request},info)=>{if(process.env.PW_FAIL_ALPHA)expect(1).toBe(2);'));
    const failed = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--workers=1"], { PW_FAIL_ALPHA: "1" });
    assert.equal(failed.status, 1, failed.text);
    const lastFile = join(fixture.root, "custom-results/.last-run.json");
    const before = readFileSync(lastFile);
    const failedIds = JSON.parse(before).failedTests;
    assert.equal(failedIds.length, 1);
    const lastList = await measuredNative(fixture, ["signals.spec.mjs", "--project=chromium", "--last-failed", "--list"]);
    assert.deepEqual(lastList.root.ids, failedIds);
    assert.deepEqual(readFileSync(lastFile), before);
    const lastRun = await measuredNative(fixture, ["signals.spec.mjs", "--project=chromium", "--last-failed", "--workers=1"]);
    assert.deepEqual(lastRun.root.ids, failedIds);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native project dependencies and teardown retain the full planned ID role closure", async () => {
  const fixture = nativeFixture();
  try {
    const config = join(fixture.root, "custom.config.mjs");
    writeFileSync(config, readFileSync(config, "utf8").replace('projects:[{name:"chromium"},{name:"firefox"},{name:"webkit"}]', 'projects:[{name:"firefox",testMatch:"**/pages.spec.mjs",teardown:"webkit"},{name:"chromium",testMatch:"**/signals.spec.mjs",dependencies:["firefox"]},{name:"webkit",testMatch:"**/views.spec.mjs"}]'));
    const single = (title) => `import{test,expect}from"@playwright/test";import{appendFileSync}from"node:fs";test(${JSON.stringify(title)},async({request},info)=>{expect((await request.get("/")).status()).toBe(200);appendFileSync(${JSON.stringify(fixture.bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});`;
    writeFileSync(join(fixture.root, "e2e/pages.spec.mjs"), single("setup body"));
    writeFileSync(join(fixture.root, "e2e/views.spec.mjs"), single("teardown body"));
    const result = await measuredNative(fixture, ["--project=chromium", "--workers=1"]);
    assert.equal(result.body.length, 4);
    assert.equal(records(fixture.hosts).filter((host) => host.phase === "launch").length, 1);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("native empty focused and UI watch selections refuse or remain empty before any host", async () => {
  const fixture = nativeFixture();
  try {
    const empty = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--grep", "no-test-has-this-name"]);
    assert.equal(empty.status, 1, empty.text);
    const allowed = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--grep", "no-test-has-this-name", "--pass-with-no-tests"]);
    assert.equal(allowed.status, 0, allowed.text);
    for (const [args, env] of [[["--ui"], {}], [[], { PWTEST_WATCH: "1" }]]) {
      const refused = await native(fixture, args, env);
      assert.equal(refused.status, 1, refused.text);
      assert.match(refused.text, /UI\/watch mode is unsupported/);
    }
    const file = join(fixture.root, "e2e/signals.spec.mjs");
    writeFileSync(file, readFileSync(file, "utf8").replace('test("plain alpha"', 'test.only("plain alpha"'));
    const focused = await native(fixture, ["signals.spec.mjs", "--project=chromium"]);
    assert.equal(focused.status, 1, focused.text);
    assert.match(focused.text, /forbid-only|forbidOnly|test.only/i);
    assert.deepEqual(records(fixture.hosts), []);
    assert.deepEqual(records(fixture.bodies), []);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});
const editConfig = (fixture, update) => {
  const file = join(fixture.root, "custom.config.mjs");
  writeFileSync(file, update(readFileSync(file, "utf8")));
};
const alive = (pid) => {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === "ESRCH") return false; throw error; }
};

test("native independent identical invocation cannot reuse an inherited valid receipt", async () => {
  const fixture = nativeFixture();
  try {
    editConfig(fixture, s => 'import {spawnSync} from"node:child_process";\n' + s.replace('export default defineConfig', `if(selection.receipt&&!selection.reused&&!process.env.PW_PRIVATE_CLONE){const child=spawnSync(process.execPath,process.argv.slice(1),{cwd:process.cwd(),env:{...process.env,PW_PRIVATE_CLONE:"1"},encoding:"utf8",timeout:20000});appendFileSync(${JSON.stringify(fixture.plans)},JSON.stringify({phase:"clone",status:child.status,args:process.argv.slice(2),inherited:process.env.PW_HOST_INVOCATION,text:child.stdout+child.stderr})+"\\n");if(child.error||child.status!==0)throw child.error??new Error(child.stderr);}\nexport default defineConfig`));
    const result = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--list"]);
    assert.equal(result.status, 0, result.text);
    const rows = records(fixture.plans);
    const clone = rows.find(r => r.phase === "clone");
    assert.equal(clone.status, 0, clone.text);
    const invocations = rows.filter(r => r.ids);
    assert.equal(invocations.length, 2);
    assert.ok(invocations.every(r => r.reused === false && r.list === true));
    assert.notEqual(invocations[0].nonce, invocations[1].nonce);
    assert.deepEqual(invocations[0].ids, invocations[1].ids);
    assert.equal(JSON.parse(clone.inherited).issuerPid, invocations[0].pid);
    assert.deepEqual(clone.args, JSON.parse(clone.inherited).argv);
    assert.deepEqual(records(fixture.hosts), []);
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});

test("native missing malformed partial stale receipts and unknown selected declarations refuse before hosts", async () => {
  for (const mode of ["missing", "malformed", "partial", "stale"]) {
    const fixture = nativeFixture();
    try {
      editConfig(fixture, s => 'import{readFileSync,writeFileSync}from"node:fs";\n' + s.replace('export default defineConfig', `if(process.env.PW_HOST_COLLECTION_REQUEST){const r=JSON.parse(readFileSync(process.env.PW_HOST_COLLECTION_REQUEST,"utf8"));writeFileSync(${JSON.stringify(mode)}==="missing"?r.report+".part":r.report,${JSON.stringify(mode === "malformed" ? "{" : mode === "partial" ? '{"schema":1,"complete":false}' : '{"schema":1,"complete":true,"nonce":"stale"}')});}\nexport default defineConfig`));
      const result = await native(fixture, ["signals.spec.mjs", "--project=chromium"]);
      assert.equal(result.status, 1, result.text);
      assert.match(result.text, /missing.*collection receipt|malformed collection receipt|invalid or incomplete collection receipt/);
      assert.deepEqual(records(fixture.hosts), []);
      assert.deepEqual(records(fixture.bodies), []);
    } finally { rmSync(fixture.root, { recursive: true, force: true }); }
  }
  for (const mode of ["unknown-file", "unknown-project", "unknown-role", "reserved-todomvc"]) {
    const fixture = nativeFixture();
    try {
      let args = ["--project=chromium"];
      if (mode === "unknown-file") {
        writeFileSync(join(fixture.root, "e2e/unknown.spec.mjs"), 'import{test}from"@playwright/test";test("unknown",()=>{});');
        args.push("unknown.spec.mjs");
      } else if (mode === "unknown-project") args = ["--project=unregistered"];
      else {
        writeFileSync(join(fixture.root, "e2e/page-policy.spec.mjs"), 'import{test}from"@playwright/test";test("unknown",{annotation:{type:"pw-host",description:"' + (mode === 'reserved-todomvc' ? 'todomvc' : 'unregistered') + '"}},()=>{});');
        args.push("page-policy.spec.mjs");
      }
      const result = await native(fixture, args);
      assert.equal(result.status, 1, result.text);
      assert.deepEqual(records(fixture.hosts), []);
      assert.deepEqual(records(fixture.bodies), []);
    } finally { rmSync(fixture.root, { recursive: true, force: true }); }
  }
});

// An optional build that cannot be served starts no host; a spec whose tests
// differ skips its own tests of it (`page-policy.spec.mjs`), as before.
test("native selected role groups deduplicate hosts and leave an unservable optional build's host unstarted", async () => {
  const fixture = nativeFixture();
  try {
    const file = join(fixture.root, "e2e/page-policy.spec.mjs");
    const ports = { base: P, feed: P + 60, payments: P + 180 };
    const body = (role) => `async({request},info)=>{expect((await request.get("http://127.0.0.1:${ports[role]}")).status()).toBe(200);appendFileSync(${JSON.stringify(fixture.bodies)},JSON.stringify({id:info.testId,role:${JSON.stringify(role)},pid:process.pid})+"\\n");}`;
    const annotation = (role) => `{annotation:{type:"pw-host",description:${JSON.stringify(role)}}}`;
    writeFileSync(file, `import{test,expect}from"@playwright/test";import{appendFileSync}from"node:fs";import{servable}from${JSON.stringify(METADATA_MODULE)};
test("base",${annotation("base")},${body("base")});
test.describe("the feed",${annotation("feed")},()=>{test.skip(!servable("feed",${JSON.stringify(fixture.root)}),"dist-feed is not built");test("feed",${body("feed")});});
test("payments",${annotation("payments")},${body("payments")});
`);
    const launches = () => records(fixture.hosts).filter((r) => r.phase === "launch").map((r) => r.port).sort(numeric);
    // The feed's build is absent: a base-only selection runs on base alone.
    const base = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=base$", "--workers=1"]);
    assert.equal(base.status, 0, base.text);
    assert.deepEqual(launches(), [P]);
    // Its feed test is selected, skips itself, and its host is not started.
    const missing = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=feed$", "--workers=1"]);
    assert.equal(missing.status, 0, missing.text);
    assert.match(missing.text, /1 skipped/);
    assert.deepEqual(launches(), [P]);
    assert.deepEqual(records(fixture.bodies).filter((r) => r.role === "feed"), []);
    mkdirSync(join(fixture.root, "dist-feed/build"), { recursive: true });
    writeFileSync(join(fixture.root, "dist-feed/pw-runtime.mjs"), "private test runtime");
    writeFileSync(join(fixture.root, "dist-feed/sources.json"), '{"files":[]}');
    const before = records(fixture.hosts).length;
    const startBodies = records(fixture.bodies).length;
    const startPlans = records(fixture.plans).length;
    const mixed = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--workers=1"]);
    assert.equal(mixed.status, 0, mixed.text);
    const bodies = records(fixture.bodies).slice(startBodies);
    const plan = records(fixture.plans).slice(startPlans).find((r) => r.ids && !r.reused);
    assert.deepEqual(plan.ids.sort(), bodies.map((r) => r.id).sort());
    assert.deepEqual(plan.roleBindings.sort(), bodies.map((r) => [r.id, r.role === "payments" ? ["payment-provider", "payments"] : [r.role]]).sort());
    assert.deepEqual(records(fixture.hosts).slice(before).filter((r) => r.phase === "launch").map((r) => r.port).sort(numeric), [P, P + 60, P + 180]);
    // A stale feed build is not served either: skipped again, no host.
    writeFileSync(join(fixture.root, "dist-feed/pw-runtime.mjs"), "stale runtime");
    const hostCount = records(fixture.hosts).length;
    const stale = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=feed$", "--workers=1"]);
    assert.equal(stale.status, 0, stale.text);
    assert.match(stale.text, /1 skipped/);
    assert.equal(records(fixture.hosts).length, hostCount);
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});

test("native controlled hanging in-process collection is reaped on timeout and cancellation", async () => {
  for (const cancel of [false, true]) {
    const fixture = nativeFixture();
    let monitor;
    try {
      const entered = join(fixture.root, "collector.jsonl");
      editConfig(fixture, s => s.replace(`root:${JSON.stringify(fixture.root)}})`, `root:${JSON.stringify(fixture.root)},timeoutMs:${cancel?15000:8000}})`));
      writeFileSync(join(fixture.root, "e2e/signals.spec.mjs"), `import{appendFileSync}from"node:fs";appendFileSync(${JSON.stringify(entered)},JSON.stringify({pid:process.pid,ppid:process.ppid,ipc:typeof process.send})+"\\n");process.on("SIGTERM",()=>process.exit(0));setInterval(()=>{},1000);await new Promise(()=>{});`);
      const started = Date.now();
      const outcome = await native(fixture, ["signals.spec.mjs", "--project=chromium"], {}, child => {
        if (cancel) monitor = setInterval(() => { if(records(entered).length){clearInterval(monitor);child.kill("SIGTERM");} }, 10);
      });
      const collector = records(entered);
      assert.equal(collector.length, 1, outcome.text);
      assert.equal(collector[0].ipc, "undefined");
      assert.equal(outcome.status, 1, outcome.text);
      assert.match(outcome.text, cancel ? /collection interrupted by SIGTERM/ : /collection timed out after 8000ms/);
      assert.equal(alive(collector[0].pid), false);
      assert.ok(Date.now()-started < 25000);
      assert.deepEqual(records(fixture.hosts), []);
    } finally { clearInterval(monitor); rmSync(fixture.root, { recursive: true, force: true }); }
  }
});

test("native execution worker retains IPC identity and exits through the normal stop path", async () => {
  const fixture = nativeFixture();
  try {
    const events = join(fixture.root, "worker-events.jsonl");
    const file = join(fixture.root, "e2e/signals.spec.mjs");
    writeFileSync(file, readFileSync(file,"utf8")+`\nif(typeof process.send==="function"){const log=phase=>appendFileSync(${JSON.stringify(events)},JSON.stringify({phase,pid:process.pid,entry:process.argv[1],preload:process.execArgv})+"\\n");log("load");process.once("disconnect",()=>log("disconnect"));process.once("exit",()=>log("exit"));}\n`);
    const result = await native(fixture, ["signals.spec.mjs", "--project=chromium", "--grep=plain", "--workers=1"]);
    assert.equal(result.status, 0, result.text);
    const worker = records(fixture.plans).find(r=>r.reused);
    assert.ok(worker);
    const rows = records(events).filter(r=>r.pid===worker.pid);
    assert.deepEqual(rows.map(r=>r.phase), ["load", "exit"]);
    assert.ok(rows.every(r=>r.entry.endsWith("/lib/common/process.js")));
    assert.ok(rows.every(r=>!r.preload.includes(PLAN_MODULE)));
    assert.equal(alive(worker.pid), false);
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});
test("native pinned IPC disconnect invokes its controlled runner close then reaps the direct child", async () => {
  const root = mkdtempSync(join(tmpdir(), "pw-host-ipc-"));
  const output = join(root, "worker.log");
  const events = join(root, "events.jsonl");
  const descriptor = openSync(output, "wx", 0o600);
  let child, timer, escalation, monitor;
  try {
    const testRequire = createRequire(require.resolve("@playwright/test/package.json"));
    const entry = fileURLToPath(new URL("./lib/common/process.js", new URL(testRequire.resolve("playwright/package.json"), "file:")));
    const runner = join(root, "controlled-runner.cjs");
    writeFileSync(runner, `const{appendFileSync}=require("node:fs");const log=phase=>appendFileSync(${JSON.stringify(events)},JSON.stringify({phase,pid:process.pid,ipc:process.connected})+"\\n");process.once("exit",()=>log("exit"));exports.create=()=>{log("created");return{async gracefullyClose(){log("close")}}};`);
    const { fork } = await import("node:child_process");
    const env = { ...process.env, PWTEST_FORCE_EXIT_TIMEOUT: "2000" };
    delete env.NODE_OPTIONS; delete env.NODE_TEST_CONTEXT;
    child = fork(entry, [], { cwd: root, env, detached: false, execArgv: [], stdio: ["ignore", descriptor, descriptor, "ipc"] });
    let timedOut = false;
    const outcome = await new Promise((accept, reject) => {
      child.once("error", reject);
      child.once("exit", (status, signal) => accept({ status, signal }));
      child.on("message", message => {
        if (message.method === "ready") child.send({method:"__init__",params:{processParams:{timeOrigin:Date.now(),processName:""},runnerParams:{},runnerScript:runner}});
      });
      monitor = setInterval(() => { if(records(events).some(e=>e.phase==="created")){clearInterval(monitor);child.disconnect();} }, 10);
      timer = setTimeout(() => { timedOut=true;child.kill("SIGTERM");escalation=setTimeout(()=>child.kill("SIGKILL"),1000); }, 6000);
    });
    if (timedOut) throw new Error("controlled pinned IPC worker exceeded bounded lifetime");
    assert.deepEqual(outcome, { status: 0, signal: null }, readFileSync(output, "utf8"));
    assert.deepEqual(records(events).map(r=>r.phase), ["created","close","exit"]);
    assert.deepEqual(records(events).map(r=>r.ipc), [true,false,false]);
    assert.equal(alive(child.pid), false);
  } finally {
    clearTimeout(timer); clearTimeout(escalation); clearInterval(monitor); closeSync(descriptor);
    rmSync(root, {recursive:true,force:true});
  }
});

test("native selected host startup and body failures complete upstream teardown", async () => {
  for (const startup of [false,true]) {
    const fixture = nativeFixture();
    try {
      if (startup) writeFileSync(join(fixture.root,"host.mjs"),readFileSync(join(fixture.root,"host.mjs"),"utf8")+'\nthrow new Error("controlled startup failure");');
      else {
        const file=join(fixture.root,"e2e/signals.spec.mjs");
        writeFileSync(file,readFileSync(file,"utf8").replace('test("plain alpha",async({request},info)=>{','test("plain alpha",async({request},info)=>{expect(1).toBe(2);'));
      }
      const result=await native(fixture,["signals.spec.mjs","--project=chromium","--grep=plain","--workers=1"]);
      assert.equal(result.status,1,result.text);
      const hosts=records(fixture.hosts);
      const launched=hosts.filter(h=>h.phase==="launch");
      assert.equal(launched.length,1,result.text);
      assert.deepEqual(hosts.filter(h=>h.phase==="exit").map(h=>h.pid),launched.map(h=>h.pid));
      assert.equal(alive(launched[0].pid),false);
    } finally { rmSync(fixture.root,{recursive:true,force:true}); }
  }
});
test("native mutable engine isolation and Chromium performance select only their declared listeners", async () => {
  const fixture=nativeFixture();
  try {
    const spec=(title,ports)=>`import{test,expect}from"@playwright/test";import{appendFileSync}from"node:fs";test(${JSON.stringify(title)},async({request},info)=>{const port=${JSON.stringify(ports)}[info.project.name];expect((await request.get("http://127.0.0.1:"+port)).status()).toBe(200);appendFileSync(${JSON.stringify(fixture.bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});`;
    writeFileSync(join(fixture.root,"e2e/keyed-list.spec.mjs"),spec("mutable",{chromium:P+1,firefox:P+2,webkit:P+3}));
    const ran=await native(fixture,["keyed-list.spec.mjs","--project=chromium","--project=firefox","--workers=2"]);
    assert.equal(ran.status,0,ran.text);
    const bodies=records(fixture.bodies);
    const plan=records(fixture.plans).find(r=>r.ids&&!r.reused);
    assert.deepEqual(plan.ids.sort(),bodies.map(b=>b.id).sort());
    assert.deepEqual(plan.roleBindings.sort(),bodies.map(b=>[b.id,["mutable:keyed-list"]]).sort());
    assert.deepEqual(records(fixture.hosts).filter(h=>h.phase==="launch").map(h=>h.port).sort(numeric),[P+1,P+2]);
    writeFileSync(join(fixture.root,"e2e/runtime-size.spec.mjs"),spec("performance",{chromium:P+7,firefox:P+8,webkit:P+9}));
    const start=records(fixture.hosts).length;
    const performance=await native(fixture,["runtime-size.spec.mjs","--project=chromium","--workers=1"],{PW_PERFORMANCE:"1"});
    assert.equal(performance.status,0,performance.text);
    assert.deepEqual(records(fixture.hosts).slice(start).filter(h=>h.phase==="launch").map(h=>h.port),[P+7]);
    const count=records(fixture.hosts).length;
    for(const args of [["runtime-size.spec.mjs","--project=firefox"],["signals.spec.mjs","--project=chromium"]]) {
      const refused=await native(fixture,args,{PW_PERFORMANCE:"1"});
      assert.equal(refused.status,1,refused.text);
      assert.match(refused.text,/PW_PERFORMANCE requires only the Chromium/);
      assert.equal(records(fixture.hosts).length,count);
    }
  } finally {rmSync(fixture.root,{recursive:true,force:true});}
});

// The renderer's own configuration, collected through its own plan: a
// `--list` starts no host and needs no build, and what it lists is what
// Playwright selects. Its collection child refuses a configuration without
// `forbidOnly`, so a run would start nothing.
test("the suite's own configuration lists a selection through its plan, and starts no host", () => {
  const env = { ...process.env, PORT: String(P) };
  delete env.PW_PERFORMANCE;
  delete env.PWTEST_WATCH;
  const list = (...args) => spawnSync(process.execPath, [CLI, "test", "--list", ...args], { cwd: SPIKE, env, encoding: "utf8", timeout: 120000 });
  const one = list("e2e/signals.spec.mjs", "--project=chromium");
  assert.equal(one.status, 0, one.stdout + one.stderr);
  const listed = one.stdout.split("\n").filter((line) => /^\s+\[chromium\] › signals\.spec\.mjs:\d+:\d+ › /.test(line));
  assert.ok(listed.length > 0, one.stdout);
  assert.match(one.stdout, new RegExp(`Total: ${listed.length} tests? in 1 file`));
  assert.doesNotMatch(one.stdout + one.stderr, /browser host map:|webServer/);
});

const connects = (port) => new Promise((accept) => {
  const socket = net.connect(port, "127.0.0.1");
  socket.once("connect", () => { socket.destroy(); accept(true); });
  socket.once("error", () => accept(false));
});
// Until `check` holds: the event itself, with a cap on a hang.
async function until(check, what, cap = 30000) {
  const deadline = Date.now() + cap;
  while (!(await check())) {
    if (Date.now() > deadline) assert.fail(`${what}: not within ${cap} ms`);
    await new Promise((accept) => setTimeout(accept, 10));
  }
}

// **A runner killed outright leaves none of its hosts listening.** A recipe's
// bound kills a run's process group (`os.killpg(p.pid, SIGKILL)`), which
// Playwright does not survive to stop its hosts, and each host is the leader
// of a group of its own: 52 of them kept their ports after e14-refusal's
// bound of 1,200 s, and every later run on those ports refused to start. The
// host here is the real configuration's kind (`ownedCommand`) and the test
// waits for its port to close, the event itself.
test("native a runner killed outright leaves none of its hosts listening", async () => {
  const fixture = nativeFixture(true);
  const output = join(fixture.root, "owned.log");
  const descriptor = openSync(output, "wx", 0o600);
  const env = { ...process.env, PORT: String(P), KIOKUN_DATA: join(fixture.root, "private-data"), KIOKUN_APP: join(fixture.root, "private-app") };
  delete env.PWTEST_WATCH;
  // A group of its own, as the bound's kill takes one.
  const runner = spawn(process.execPath, [CLI, "test", "--config", "custom.config.mjs", "signals.spec.mjs", "--project=chromium", "--workers=1", "--reporter=line"], { cwd: fixture.root, env, detached: true, stdio: ["ignore", descriptor, descriptor] });
  let host;
  try {
    try {
      // The test is running: Playwright started the host and waited for it.
      await until(() => existsSync(fixture.marker), "the hanging test running");
    } catch (error) {
      error.message += `\n${readFileSync(output, "utf8")}`;
      throw error;
    }
    host = records(fixture.hosts).find((record) => record.phase === "launch");
    assert.ok(host, "the host launched");
    assert.equal(await connects(host.port), true, "the host listens while its runner runs");
    process.kill(-runner.pid, "SIGKILL");
    await until(async () => !(await connects(host.port)), `the host's port ${host.port} closing after its runner was killed`);
  } finally {
    try { process.kill(-runner.pid, "SIGKILL"); } catch { /* gone */ }
    if (host) { try { process.kill(host.pid, "SIGKILL"); } catch { /* gone */ } }
    closeSync(descriptor);
    rmSync(fixture.root, { recursive: true, force: true });
  }
});
