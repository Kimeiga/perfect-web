import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { REGISTRY, STRIDE, ENGINES, MUTABLE_PORTS, STORE_PAYMENTS_PORTS, SCHEDULED_ORDERS_PORTS, TODOMVC_PORTS,
  validateRegistry, listenersForSlots, portsForRole, rolesForTest, buildProblem } from "./host-metadata.mjs";

const { caseFor } = await import(process.env.PW_HOST_TEST_WRAPPER ?? "./host-test-producer.mjs");

const clone = () => structuredClone(REGISTRY);

test(...caseFor("registry covers the exact current spec inventory", () => {
  const actual = readdirSync(new URL("./e2e", import.meta.url)).filter((name) => name.endsWith(".spec.mjs")).sort();
  assert.deepEqual(Object.keys(REGISTRY.specs).sort(), actual);
  assert.equal(STRIDE, 256);
  assert.deepEqual(ENGINES, ["chromium", "firefox", "webkit"]);
}));

test(...caseFor("all host and provider listeners are disjoint across adjacent and multiple slots", () => {
  for (const slots of [2, 8, 31]) {
    const listeners = listenersForSlots(REGISTRY, 13201, slots);
    assert.equal(new Set(listeners.map(({ port }) => port)).size, listeners.length);
    assert.equal(listeners.filter(({ role }) => role.endsWith("provider")).length, 6 * slots);
    assert.equal(listeners.filter(({ role }) => role === "todomvc").length, 3 * slots);
  }
  assert.throws(() => { const bad = clone(); bad.stride = 200; validateRegistry(bad); }, /escapes stride/);
}));

test(...caseFor("TCP bound includes the final slot and largest concrete listener", () => {
  const max = Math.max(...listenersForSlots(REGISTRY, 1).map(({ port }) => port - 1));
  assert.equal(max, 212);
  assert.equal(Math.max(...listenersForSlots(REGISTRY, 65535 - max - REGISTRY.stride, 2).map(({ port }) => port)), 65535);
  assert.throws(() => listenersForSlots(REGISTRY, 65536 - max - REGISTRY.stride, 2), /TCP port/);
  for (const base of [0, -1, 1.5, NaN, Infinity, "13201", 65536]) assert.throws(() => listenersForSlots(REGISTRY, base), /PORT/);
  for (const count of [0, -1, 1.5, NaN, Infinity, "2", 65536]) assert.throws(() => listenersForSlots(REGISTRY, 13201, count), /slot count/);
}));

test(...caseFor("duplicate, fractional, negative and escaping concrete offsets refuse", () => {
  for (const offset of [0, -1, 1.5, NaN, Infinity, "200", 256, 65536]) {
    const bad = clone(); bad.roles.reviews.offsets.chromium = offset;
    assert.throws(() => validateRegistry(bad), /offset/);
  }
}));

test(...caseFor("provider dependencies are declared and engine compatible", () => {
  const missing = clone(); delete missing.roles["payment-provider"];
  assert.throws(() => validateRegistry(missing), /unknown auxiliary dependency/);
  const omitted = clone(); delete omitted.roles.payments.depends;
  assert.throws(() => validateRegistry(omitted), /provider is not a declared dependency/);
  const engine = clone(); delete engine.roles["payment-provider"].offsets.webkit;
  assert.throws(() => validateRegistry(engine), /incompatible engines|every engine/);
  assert.deepEqual(rolesForTest("payments.spec.mjs", "webkit"), ["payment-provider", "payments"]);
  assert.deepEqual(rolesForTest("scheduled-orders.spec.mjs", "chromium"), ["scheduled-orders", "scheduled-provider"]);
}));

test(...caseFor("pure legacy port exports preserve every accepted within-slot offset", () => {
  assert.equal(MUTABLE_PORTS["keyed-list"].chromium - portsForRole("base").shared, 1);
  assert.equal(MUTABLE_PORTS.materialized.webkit - portsForRole("base").shared, 36);
  assert.equal(STORE_PAYMENTS_PORTS.chromium - portsForRole("base").shared, 180);
  assert.equal(SCHEDULED_ORDERS_PORTS.webkit - portsForRole("base").shared, 185);
  assert.equal(TODOMVC_PORTS.chromium - portsForRole("base").shared, 140);
  assert.equal(portsForRole("payment-provider").chromium - portsForRole("base").shared, 190);
  assert.equal(portsForRole("scheduled-provider").webkit - portsForRole("base").shared, 195);
  assert.equal(portsForRole("reviews").webkit - portsForRole("base").shared, 202);
  assert.equal(portsForRole("guestbook").webkit - portsForRole("base").shared, 212);
  assert.throws(() => { REGISTRY.roles.base.offsets.shared = 9; }, TypeError);
}));

test(...caseFor("unknown selected spec, project, role and conflicting declarations refuse", () => {
  assert.throws(() => rolesForTest("unknown.spec.mjs", "chromium"), /unregistered selected spec/);
  assert.throws(() => rolesForTest("signals.spec.mjs", "chrome"), /unknown project/);
  assert.throws(() => rolesForTest("signals.spec.mjs", "chromium", [{ type: "pw-host", description: "base" }]), /conflicting/);
  for (const annotations of [[], [{ type: "pw-host", description: "unknown" }], [{ type: "pw-host", description: "base" }, { type: "pw-host", description: "feed" }]]) {
    assert.throws(() => rolesForTest("page-policy.spec.mjs", "chromium", annotations), /host annotation|host role/);
  }
  assert.deepEqual(rolesForTest("signals.spec.mjs", "chromium"), ["base"]);
  assert.deepEqual(rolesForTest("page-policy.spec.mjs", "webkit", [{ type: "pw-host", description: "payments" }]), ["payment-provider", "payments"]);
}));

test(...caseFor("schema, engines, role builds and spec mappings fail closed", () => {
  for (const mutate of [
    (bad) => bad.schema = 2,
    (bad) => bad.engines.push("chromium"),
    (bad) => bad.roles.base.build = "absent",
    (bad) => bad.mutable.cart = "absent",
    (bad) => bad.portExports.FEED_PORTS = "absent",
    (bad) => bad.specs["signals.spec.mjs"].roles = ["absent"],
    (bad) => bad.specs["signals.spec.mjs"].annotations = true,
    (bad) => bad.roles["payment-provider"].depends = ["scheduled-provider"],
  ]) {
    const bad = clone(); mutate(bad); assert.throws(() => validateRegistry(bad), /browser host registry/);
  }
}));

test(...caseFor("build checks preserve whole-build runtime and source freshness with concrete prerequisites", () => {
  const root = mkdtempSync(join(tmpdir(), "pw-host-build-"));
  try {
    assert.match(buildProblem("feed", root), /dist-feed is no whole build.*run feed.sh/);
    mkdirSync(join(root, "dist-feed/build"), { recursive: true });
    assert.match(buildProblem("feed", root), /whole build/);
    writeFileSync(join(root, "dist-feed/sources.json"), '{"files":[]}');
    assert.match(buildProblem("feed", root), /another pw-runtime/);
    mkdirSync(join(root, "public"));
    writeFileSync(join(root, "public/pw-runtime.mjs"), "runtime");
    writeFileSync(join(root, "dist-feed/pw-runtime.mjs"), "runtime");
    assert.equal(buildProblem("feed", root), null);
    writeFileSync(join(root, "source.pw"), "before");
    const sha256 = createHash("sha256").update("before").digest("hex");
    writeFileSync(join(root, "dist-feed/sources.json"), JSON.stringify({ files: [{ path: "source.pw", sha256 }] }));
    assert.equal(buildProblem("feed", root), null);
    writeFileSync(join(root, "source.pw"), "after");
    assert.match(buildProblem("feed", root), /source.pw changed/);
    writeFileSync(join(root, "dist-feed/sources.json"), '{"files":false}');
    assert.match(buildProblem("feed", root), /invalid source records/);
    writeFileSync(join(root, "dist-feed/sources.json"), '{');
    assert.match(buildProblem("feed", root), /cannot be read/);
    assert.match(buildProblem("todomvc", root), /dist-todomvc is no whole build/);
    assert.throws(() => buildProblem("absent", root), /unknown build/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}));

test(...caseFor("spec metadata imports cannot recurse through the Playwright config", () => {
  for (const file of readdirSync(new URL("./e2e", import.meta.url)).filter((name) => name.endsWith(".spec.mjs"))) {
    assert.doesNotMatch(readFileSync(new URL(`./e2e/${file}`, import.meta.url), "utf8"), /from ["']\.\.\/playwright\.config\.mjs["']/);
  }
}));
import { spawn } from "node:child_process";
import { closeSync, openSync, symlinkSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { collectionArguments, validatePlan } from "./host-plan.mjs";

const require = createRequire(import.meta.url);
const CLI = require.resolve("@playwright/test/cli");
const PLAN_MODULE = new URL("./host-plan.mjs", import.meta.url).href;
const SPIKE = fileURLToPath(new URL(".", import.meta.url));
const records = (file) => {
  try { return readFileSync(file, "utf8").trim().split("\n").filter(Boolean).map((line) => JSON.parse(line)); }
  catch (error) { if (error.code === "ENOENT") return []; throw error; }
};

function nativeFixture() {
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
import {selectedTests,serversForSelection} from ${JSON.stringify(PLAN_MODULE)};
const selection=await selectedTests({root:${JSON.stringify(root)}});
appendFileSync(${JSON.stringify(plans)},JSON.stringify({pid:process.pid,ppid:process.ppid,reused:selection.reused,list:selection.collectionOnly,ids:selection.receipt?.tests.map(t=>t.id),nonce:selection.receipt?.nonce,roleBindings:selection.receipt?.tests.map(t=>[t.id,t.roles]),context:process.env.PW_HOST_INVOCATION})+"\\n");
export default defineConfig({testDir:"./e2e",forbidOnly:true,outputDir:"./custom-results",projects:[{name:"chromium"},{name:"firefox"},{name:"webkit"}],use:{baseURL:"http://127.0.0.1:13201"},webServer:serversForSelection(selection,{root:${JSON.stringify(root)},port:13201}).map(server=>({...server,command:"node host.mjs",env:{...server.env,PW_PROBE_LOG:${JSON.stringify(hosts)}},gracefulShutdown:{signal:"SIGTERM",timeout:1000}}))});
`);
  writeFileSync(join(root, "e2e/signals.spec.mjs"), `import {test,expect} from "@playwright/test";import {appendFileSync} from "node:fs";
test("plain alpha",async({request},info)=>{expect((await request.get("/")).status()).toBe(200);appendFileSync(${JSON.stringify(bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});
test("value --ui stays opaque",async({request},info)=>{expect((await request.get("/")).status()).toBe(200);appendFileSync(${JSON.stringify(bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});
`);
  return { root, plans, hosts, bodies, runs: 0 };
}

async function native(fixture, args, extraEnv = {}, onSpawn = () => {}) {
  const output = join(fixture.root, `native-${fixture.runs++}.log`);
  const descriptor = openSync(output, "wx", 0o600);
  const env = { ...process.env, PORT: "13201", KIOKUN_DATA: join(fixture.root, "private-data"), KIOKUN_APP: join(fixture.root, "private-app"), ...extraEnv };
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

test(...caseFor("native collection framing preserves every opaque argument before and after separator", () => {
  const argv = ["test", "--config", "custom.config.mjs", "--grep", "--ui", "--project=fire*", "--project=chromium", "--reporter=line", "--", "signals.spec.mjs"];
  assert.deepEqual(collectionArguments(argv), ["test", "--list", ...argv.slice(1)]);
  assert.deepEqual(collectionArguments(["test", "--grep", "--", "--", "signals.spec.mjs"]), ["test", "--list", "--grep", "--", "--", "signals.spec.mjs"]);
  assert.deepEqual(collectionArguments(["test", "signals.spec.mjs"]), ["test", "--list", "signals.spec.mjs"]);
  assert.throws(() => collectionArguments(["show-report"]), /native playwright test/);
}));

test(...caseFor("collection receipts require exact issuer selection config completion and role identities", () => {
  const request = { nonce: "fresh", issuerPid: 12, cwd: "/private", root: "/private", argv: ["test"], collectionArgv: ["test", "--list"], configFile: "/private/custom.config.mjs" };
  const value = { schema: 1, complete: true, ...request, collectorPid: 13, forbidOnly: true, tests: [{ id: "selected", file: "signals.spec.mjs", project: "chromium", roles: ["base"] }] };
  assert.equal(validatePlan(value, request, 13), value);
  for (const mutate of [
    (bad) => bad.schema = 2,
    (bad) => bad.complete = false,
    (bad) => bad.nonce = "stale",
    (bad) => bad.issuerPid++,
    (bad) => bad.collectorPid++,
    (bad) => bad.cwd = "/elsewhere",
    (bad) => bad.root = "/elsewhere",
    (bad) => bad.argv.push("other.spec.mjs"),
    (bad) => bad.collectionArgv.push("other.spec.mjs"),
    (bad) => bad.configFile = "/elsewhere/config.mjs",
    (bad) => bad.forbidOnly = false,
    (bad) => bad.tests.push(bad.tests[0]),
    (bad) => bad.tests[0].roles = ["feed"],
    (bad) => bad.tests[0].project = "unknown",
    (bad) => bad.tests[0].file = "unknown.spec.mjs",
  ]) {
    const bad = structuredClone(value); mutate(bad);
    assert.throws(() => validatePlan(bad, request, 13), /browser host/);
  }
}));

test(...caseFor("native same-config CLI executes its planned ID and real IPC reload with one sanitized host", async () => {
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
    assert.equal(launched[0].port, 13201);
    assert.equal(launched[0].data, false);
    assert.equal(launched[0].app, false);
    assert.deepEqual(hosts.filter((host) => host.phase === "exit").map((host) => host.pid), [launched[0].pid]);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
}));
test(...caseFor("native required separator values and repeated reporters preserve the actual caller reporter", async () => {
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
}));

test(...caseFor("deleted required recorded inputs refuse before any selected native host starts", async () => {
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
}));

test(...caseFor("only keyed external generated copies may be absent and existing copies remain hash checked", () => {
  const privateRoot = mkdtempSync(join(tmpdir(), "pw-host-generated-"));
  const repository = join(privateRoot, "repository");
  const root = join(repository, "spikes/own-renderer");
  try {
    mkdirSync(join(root, "public"), { recursive: true });
    writeFileSync(join(root, "public/pw-runtime.mjs"), "runtime");
    const required = join(repository, "permanent.pw");
    const external = join(privateRoot, "removed-generated.pw");
    const sha256 = createHash("sha256").update("source").digest("hex");
    for (const dist of ["dist-feed", "dist-keyed"]) {
      mkdirSync(join(root, dist, "build"), { recursive: true });
      writeFileSync(join(root, dist, "pw-runtime.mjs"), "runtime");
      writeFileSync(join(root, dist, "sources.json"), JSON.stringify({ files: [{ path: external, sha256 }] }));
    }
    assert.match(buildProblem("feed", root), /missing required source/);
    assert.equal(buildProblem("keyed", root), null);
    writeFileSync(external, "different");
    assert.match(buildProblem("keyed", root), /changed since/);
    rmSync(external);
    writeFileSync(join(root, "dist-keyed/sources.json"), JSON.stringify({ files: [{ path: required, sha256 }] }));
    assert.match(buildProblem("keyed", root), /missing required source/);
    symlinkSync(repository, join(privateRoot, "looks-external"), "dir");
    writeFileSync(join(root, "dist-keyed/sources.json"), JSON.stringify({ files: [{ path: join(privateRoot, "looks-external/permanent.pw"), sha256 }] }));
    assert.match(buildProblem("keyed", root), /missing required source/);
    assert.deepEqual(Object.entries(REGISTRY.builds).filter(([, build]) => build.externalGeneratedSources).map(([name]) => name), ["keyed"]);
    const invalid = clone(); invalid.builds.keyed.externalGeneratedSources = "yes";
    assert.throws(() => validateRegistry(invalid), /externalGeneratedSources/);
  } finally {
    rmSync(privateRoot, { recursive: true, force: true });
  }
}));
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

test(...caseFor("native file line project wildcard repeated grep invert and shards retain exact IDs and roles", async () => {
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
}));

test(...caseFor("native test-list and custom outputDir last-failed preserve selections and list never rewrites last-run", async () => {
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
}));

test(...caseFor("native project dependencies and teardown retain the full planned ID role closure", async () => {
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
}));

test(...caseFor("native empty focused and UI watch selections refuse or remain empty before any host", async () => {
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
}));
// The outer campaign loads an immutable wrapper/reporter. These subjects always
// address the LIVE producer, so its own controls cannot weaken outer evidence.
const LIVE_PRODUCER = new URL("./host-test-producer.mjs", import.meta.url).href;
const CONSUMER = fileURLToPath(new URL("../../scripts", import.meta.url));
async function privateChild(root, args, env, label, executable = process.execPath) {
  const output = join(root, `${label}.log`);
  const descriptor = openSync(output, "wx", 0o600);
  const child = spawn(executable, args, { cwd: root, env, detached: false, stdio: ["ignore", descriptor, descriptor] });
  let timer, escalation;
  let timedOut = false;
  try {
    const result = await new Promise((accept, reject) => {
      child.once("error", reject);
      child.once("exit", (status, signal) => accept({ status, signal }));
      timer = setTimeout(() => { timedOut = true; child.kill("SIGTERM"); escalation = setTimeout(() => child.kill("SIGKILL"), 1000); }, 20000);
    });
    if (timedOut || result.signal) throw new Error(`private producer failed its lifetime contract: ${JSON.stringify(result)}`);
    return { ...result, text: readFileSync(output, "utf8") };
  } finally {
    clearTimeout(timer); clearTimeout(escalation); closeSync(descriptor);
  }
}
async function producerSubject(callback, prefix = "", transform = (value) => value) {
  const root = mkdtempSync(join(tmpdir(), "pw-host-producer-"));
  try {
    const file = join(root, "fixture.test.mjs");
    const source = `import test from "node:test";import{caseFor}from ${JSON.stringify(LIVE_PRODUCER)};import assert from"node:assert/strict";import{before,after,beforeEach,afterEach}from"node:test";\n${prefix}\ntest(...caseFor("intended",${callback}));\n`;
    writeFileSync(file, source);
    const request = { schema: 1, nonce: `${process.pid}-${Date.now()}-${Math.random()}`, file,
      sourceSHA: createHash("sha256").update(source).digest("hex"), mode: "measure", expected: ["intended"], journal: join(root, "journal.jsonl"), report: join(root, "report.json") };
    writeFileSync(request.journal, "");
    writeFileSync(join(root, "request.json"), JSON.stringify(request));
    const env = { ...process.env, PW_HOST_TEST_REQUEST: join(root, "request.json") };
    delete env.NODE_OPTIONS;
    delete env.NODE_TEST_CONTEXT;
    const child = await privateChild(root, ["--import", LIVE_PRODUCER, "--test", "--test-reporter", LIVE_PRODUCER, file], env, "producer");
    let report = "";
    try { report = readFileSync(request.report, "utf8"); } catch (error) { if (error.code !== "ENOENT") throw error; }
    if (!report) throw new Error(`producer omitted its report (status ${child.status}): ${child.text}`);
    const payload = transform({ status: child.status, text: report, request });
    writeFileSync(join(root, "payload.json"), JSON.stringify(payload));
    const program = `import json,sys;sys.path.insert(0,${JSON.stringify(CONSUMER)});from browser_host_map_mutations import node_result;p=json.load(open(sys.argv[1]));r=node_result(p["status"],p["text"],p["request"]);print(json.dumps(dict(valid=r.valid,reason=r.reason,passed=r.passed,assertions=r.assertions)))`;
    const consumed = await privateChild(root, ["-c", program, join(root, "payload.json")], env, "consumer", "python3");
    if (consumed.status !== 0) throw new Error(`consumer did not execute successfully: ${consumed.text}`);
    const result = JSON.parse(consumed.text);
    if (typeof result.valid !== "boolean" || !Array.isArray(result.passed) || !Array.isArray(result.assertions)) throw new Error("invalid consumer response");
    return { result, report: report && JSON.parse(report), status: child.status };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test(...caseFor("owned Node producer binds the original native callback assertion and exact callsite", async () => {
  for (const callback of ["()=>{}", "async()=>{}", "()=>Promise.resolve(undefined)"]) {
    const { result, status } = await producerSubject(callback);
    assert.equal(status, 0);
    assert.equal(result.valid, true, result.reason);
    assert.deepEqual(result.passed, ["intended"]);
  }
  const { result, report, status } = await producerSubject('()=>{const original=new assert.AssertionError({message:"original",actual:1,expected:2,operator:"strictEqual"});throw original}');
  assert.equal(status, 1);
  assert.equal(result.valid, true, result.reason);
  assert.deepEqual(result.assertions, ["intended"]);
  const completion = report.journal.find((event) => event.phase === "completed");
  assert.equal(report.terminals[0].occurrence, completion.occurrence);
  assert.equal(report.terminals[0].causeCode, "ERR_ASSERTION");
}));

test(...caseFor("owned Node producer refuses caught global hooks including mixed body and afterEach failure", async () => {
  for (const hook of ["before", "after", "beforeEach", "afterEach"]) {
    const { result } = await producerSubject("()=>assert.equal(1,2)", `try{${hook}(()=>assert.equal(3,4))}catch{}`);
    assert.equal(result.valid, false, hook);
    assert.match(result.reason, /producer refused.*global/);
  }
  for (const prefix of [
    'import{createRequire}from"node:module";const c=createRequire(import.meta.url)("node:test");try{c.before(()=>{})}catch{}',
    'import*as n from"node:test";try{n.after(()=>{})}catch{}',
    'try{test.beforeEach(()=>{})}catch{}',
    'test.afterEach=()=>{};',
  ]) {
    const { result } = await producerSubject("()=>assert.equal(1,2)", prefix);
    assert.equal(result.valid, false, prefix);
    assert.match(result.reason, /producer refused/);
  }
  for (const key of ["before", "after", "beforeEach", "afterEach", "test", "runOnly"]) {
    const { result } = await producerSubject(`t=>{try{t.${key}("inner",()=>{})}catch{};assert.equal(1,2)}`);
    assert.equal(result.valid, false, key);
  }
}));

test(...caseFor("owned Node producer refuses deferred generator and nonvoid results before body success", async () => {
  for (const callback of ['function*(){assert.equal(1,2)}', 'async function*(){assert.equal(1,2)}',
    '()=> (function*(){assert.equal(1,2)})()', '()=> (async function*(){assert.equal(1,2)})()',
    'async()=> (function*(){assert.equal(1,2)})()', 'async()=> (async function*(){assert.equal(1,2)})()',
    '()=>42', 'async()=>42', '()=>({then(resolve){resolve()}})']) {
    const { result } = await producerSubject(callback);
    assert.equal(result.valid, false, callback);
    assert.match(result.reason, /producer refused/);
  }
}));

test(...caseFor("owned Node consumer refuses body errors missing mismatched skipped and malformed evidence", async () => {
  for (const prefix of ['throw new Error("import fixture");', 'test.skip("unwrapped",()=>{});', 'test("unwrapped",()=>{});']) {
    const { result } = await producerSubject("()=>assert.equal(1,2)", prefix);
    assert.equal(result.valid, false, prefix);
  }
  const bodyError = await producerSubject('()=>{throw new Error("body error")}');
  assert.equal(bodyError.result.valid, false);
  for (const transform of [
    p => ({ ...p, status: 7 }), p => ({ ...p, status: -15 }), p => ({ ...p, text: "" }),
    p => ({ ...p, text: "{" }), p => ({ ...p, text: "[".repeat(1200) + "0" + "]".repeat(1200) }),
    p => { const r=JSON.parse(p.text);r.nonce="stale";return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.journal=r.journal.filter(e=>e.phase!=="entered");return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.terminals[0].skip=true;return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.terminals[0].occurrence="other";return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.summaries=[false,false];return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.journal.reverse();return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.summaries[0].counts.tests++;return {...p,text:JSON.stringify(r)}; },
    p => { const r=JSON.parse(p.text);r.schema=true;return {...p,text:JSON.stringify(r)}; },
  ]) {
    const { result } = await producerSubject("()=>assert.equal(1,2)", "", transform);
    assert.equal(result.valid, false, result.reason);
  }
}));
const editConfig = (fixture, update) => {
  const file = join(fixture.root, "custom.config.mjs");
  writeFileSync(file, update(readFileSync(file, "utf8")));
};
const alive = (pid) => {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === "ESRCH") return false; throw error; }
};

test(...caseFor("native independent identical invocation cannot reuse an inherited valid receipt", async () => {
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
}));

test(...caseFor("native missing malformed partial stale receipts and unknown selected declarations refuse before hosts", async () => {
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
}));

test(...caseFor("native selected role groups deduplicate hosts and reject only selected missing or stale builds", async () => {
  const fixture = nativeFixture();
  try {
    const file = join(fixture.root, "e2e/page-policy.spec.mjs");
    const make = (roles) => 'import{test,expect}from"@playwright/test";import{appendFileSync}from"node:fs";\n' + roles.map(role => `test(${JSON.stringify(role)},{annotation:{type:"pw-host",description:${JSON.stringify(role)}}},async({request},info)=>{const port=${JSON.stringify({base:13201,feed:13261,payments:13381})}[${JSON.stringify(role)}];expect((await request.get("http://127.0.0.1:"+port)).status()).toBe(200);appendFileSync(${JSON.stringify(fixture.bodies)},JSON.stringify({id:info.testId,role:${JSON.stringify(role)},pid:process.pid})+"\\n");});`).join("\n");
    writeFileSync(file, make(["base", "feed", "payments"]));
    // Feed is absent but not selected; a base-only selection still executes.
    const base = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=base$", "--workers=1"]);
    assert.equal(base.status, 0, base.text);
    assert.deepEqual(records(fixture.hosts).filter(r=>r.phase==="launch").map(r=>r.port), [13201]);
    const before = records(fixture.hosts).length;
    const missing = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=feed$"]);
    assert.equal(missing.status, 1, missing.text);
    assert.match(missing.text, /dist-feed is no whole build/);
    assert.equal(records(fixture.hosts).length, before);
    mkdirSync(join(fixture.root, "dist-feed/build"), { recursive: true });
    writeFileSync(join(fixture.root, "dist-feed/pw-runtime.mjs"), "private test runtime");
    writeFileSync(join(fixture.root, "dist-feed/sources.json"), '{"files":[]}');
    const startBodies = records(fixture.bodies).length;
    const startPlans = records(fixture.plans).length;
    const mixed = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--workers=1"]);
    assert.equal(mixed.status, 0, mixed.text);
    const bodies = records(fixture.bodies).slice(startBodies);
    const plan = records(fixture.plans).slice(startPlans).find(r=>r.ids&&!r.reused);
    assert.deepEqual(plan.ids.sort(), bodies.map(r=>r.id).sort());
    assert.deepEqual(plan.roleBindings.sort(), bodies.map(r=>[r.id,r.role==="payments"?["payment-provider","payments"]:[r.role]]).sort());
    assert.deepEqual(records(fixture.hosts).slice(before).filter(r=>r.phase==="launch").map(r=>r.port).sort(), [13201,13261,13381]);
    writeFileSync(join(fixture.root, "dist-feed/pw-runtime.mjs"), "stale runtime");
    const hostCount = records(fixture.hosts).length;
    const stale = await native(fixture, ["page-policy.spec.mjs", "--project=chromium", "--grep=feed$"]);
    assert.equal(stale.status, 1, stale.text);
    assert.match(stale.text, /another pw-runtime/);
    assert.equal(records(fixture.hosts).length, hostCount);
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
}));

test(...caseFor("native controlled hanging in-process collection is reaped on timeout and cancellation", async () => {
  for (const cancel of [false, true]) {
    const fixture = nativeFixture();
    let monitor;
    try {
      const entered = join(fixture.root, "collector.jsonl");
      editConfig(fixture, s => s.replace(`root:${JSON.stringify(fixture.root)}})`, `root:${JSON.stringify(fixture.root)},timeoutMs:${cancel?15000:1000}})`));
      writeFileSync(join(fixture.root, "e2e/signals.spec.mjs"), `import{appendFileSync}from"node:fs";appendFileSync(${JSON.stringify(entered)},JSON.stringify({pid:process.pid,ppid:process.ppid,ipc:typeof process.send})+"\\n");process.on("SIGTERM",()=>process.exit(0));setInterval(()=>{},1000);await new Promise(()=>{});`);
      const started = Date.now();
      const outcome = await native(fixture, ["signals.spec.mjs", "--project=chromium"], {}, child => {
        if (cancel) monitor = setInterval(() => { if(records(entered).length){clearInterval(monitor);child.kill("SIGTERM");} }, 10);
      });
      const collector = records(entered);
      assert.equal(collector.length, 1, outcome.text);
      assert.equal(collector[0].ipc, "undefined");
      assert.equal(outcome.status, 1, outcome.text);
      assert.match(outcome.text, cancel ? /collection interrupted by SIGTERM/ : /collection timed out after 1000ms/);
      assert.equal(alive(collector[0].pid), false);
      assert.ok(Date.now()-started < 12000);
      assert.deepEqual(records(fixture.hosts), []);
    } finally { clearInterval(monitor); rmSync(fixture.root, { recursive: true, force: true }); }
  }
}));

test(...caseFor("native execution worker retains IPC identity and exits through the normal stop path", async () => {
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
    assert.ok(rows.every(r=>!r.preload.includes(PLAN_MODULE)&&!r.preload.includes(LIVE_PRODUCER)));
    assert.equal(alive(worker.pid), false);
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
}));
test(...caseFor("native pinned IPC disconnect invokes its controlled runner close then reaps the direct child", async () => {
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
}));
test(...caseFor("selected descriptors preserve memory identity blobs provider ports target and reuse policy", async () => {
  const fixture = nativeFixture();
  try {
    for (const dist of ["dist-feed", "dist-kiokun", "dist-guestbook"]) {
      mkdirSync(join(fixture.root, dist, "build"), { recursive: true });
      writeFileSync(join(fixture.root, dist, "pw-runtime.mjs"), "private test runtime");
      writeFileSync(join(fixture.root, dist, "sources.json"), '{"files":[]}');
    }
    const file = join(fixture.root, "probe.mjs");
    writeFileSync(file, `import{serversForSelection}from${JSON.stringify(PLAN_MODULE)};import{rolesForTest}from${JSON.stringify(new URL("./host-metadata.mjs",import.meta.url).href)};const tests=["signals.spec.mjs","identity.spec.mjs","uploads.spec.mjs","payments.spec.mjs","scheduled-orders.spec.mjs","reviews.spec.mjs","guestbook.spec.mjs"].map(file=>({file,project:"webkit",roles:rolesForTest(file,"webkit")}));const hosts=serversForSelection({collectionOnly:false,receipt:{tests}},{root:${JSON.stringify(fixture.root)},port:13201});console.log(JSON.stringify(hosts.map(h=>({...h,deploymentUndefined:["KIOKUN_DATA","KIOKUN_APP"].every(k=>Object.hasOwn(h.env,k)&&h.env[k]===undefined)}))));`);
    const env = { ...process.env, CARGO_TARGET_DIR: join(fixture.root, "private-target"), PW_REUSE: "1" };
    delete env.PW_PERFORMANCE;
    const outcome = await privateChild(fixture.root, [file], env, "descriptors");
    if (outcome.status !== 0) throw new Error(outcome.text);
    const hosts = JSON.parse(outcome.text);
    assert.equal(hosts.length, 7);
    assert.ok(hosts.every(h=>h.deploymentUndefined&&h.env.PW_FEED_DATABASE_URL===""&&h.env.PW_STORE_DATABASE_URL===""&&h.reuseExistingServer));
    assert.ok(hosts.every(h=>h.command.startsWith(`'${fixture.root}/private-target/debug/pw-dev-server'`)));
    for (const port of [13273,13403,13413]) assert.equal(hosts.find(h=>h.port===port).env.PW_IDENTITY, "dev-accounts");
    assert.equal(hosts.find(h=>h.port===13283).env.PW_BLOB_DIR, "dist-feed/blobs-13283");
    assert.equal(hosts.find(h=>h.port===13383).env.PW_PAYMENTS_PORT, "13393");
    assert.equal(hosts.find(h=>h.port===13386).env.PW_PAYMENTS_PORT, "13396");
    assert.equal(hosts.find(h=>h.port===13383).env.PW_PAYMENTS, "dev");
    assert.ok(hosts.filter(h=>![13383,13386].includes(h.port)).every(h=>!Object.hasOwn(h.env,"PW_PAYMENTS_PORT")));
  } finally { rmSync(fixture.root,{recursive:true,force:true}); }
}));

test(...caseFor("native selected host startup and body failures complete upstream teardown", async () => {
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
}));
test(...caseFor("native mutable engine isolation and Chromium performance select only their declared listeners", async () => {
  const fixture=nativeFixture();
  try {
    const spec=(title,ports)=>`import{test,expect}from"@playwright/test";import{appendFileSync}from"node:fs";test(${JSON.stringify(title)},async({request},info)=>{const port=${JSON.stringify(ports)}[info.project.name];expect((await request.get("http://127.0.0.1:"+port)).status()).toBe(200);appendFileSync(${JSON.stringify(fixture.bodies)},JSON.stringify({id:info.testId,pid:process.pid})+"\\n");});`;
    writeFileSync(join(fixture.root,"e2e/keyed-list.spec.mjs"),spec("mutable",{chromium:13202,firefox:13203,webkit:13204}));
    const ran=await native(fixture,["keyed-list.spec.mjs","--project=chromium","--project=firefox","--workers=2"]);
    assert.equal(ran.status,0,ran.text);
    const bodies=records(fixture.bodies);
    const plan=records(fixture.plans).find(r=>r.ids&&!r.reused);
    assert.deepEqual(plan.ids.sort(),bodies.map(b=>b.id).sort());
    assert.deepEqual(plan.roleBindings.sort(),bodies.map(b=>[b.id,["mutable:keyed-list"]]).sort());
    assert.deepEqual(records(fixture.hosts).filter(h=>h.phase==="launch").map(h=>h.port).sort(),[13202,13203]);
    writeFileSync(join(fixture.root,"e2e/runtime-size.spec.mjs"),spec("performance",{chromium:13208,firefox:13209,webkit:13210}));
    const start=records(fixture.hosts).length;
    const performance=await native(fixture,["runtime-size.spec.mjs","--project=chromium","--workers=1"],{PW_PERFORMANCE:"1"});
    assert.equal(performance.status,0,performance.text);
    assert.deepEqual(records(fixture.hosts).slice(start).filter(h=>h.phase==="launch").map(h=>h.port),[13208]);
    const count=records(fixture.hosts).length;
    for(const args of [["runtime-size.spec.mjs","--project=firefox"],["signals.spec.mjs","--project=chromium"]]) {
      const refused=await native(fixture,args,{PW_PERFORMANCE:"1"});
      assert.equal(refused.status,1,refused.text);
      assert.match(refused.text,/PW_PERFORMANCE requires only the Chromium/);
      assert.equal(records(fixture.hosts).length,count);
    }
  } finally {rmSync(fixture.root,{recursive:true,force:true});}
}));
test(...caseFor("public collection reporter publishes only one successful complete callback sequence", async () => {
  const fixture=nativeFixture();
  try {
    const output=join(fixture.root,"callback-outcomes.json");
    const program=join(fixture.root,"reporter-callbacks.mjs");
    writeFileSync(program,`import Reporter from${JSON.stringify(new URL("./host-plan-reporter.mjs",import.meta.url).href)};import{writeFileSync,existsSync,readFileSync}from"node:fs";const root=${JSON.stringify(fixture.root)};const rows=[];for(const mode of["complete","no-end","two-begins","two-ends","error","test-begin","failed"]){const report=root+"/"+mode+"-report.json";const request={schema:1,nonce:mode,issuerPid:process.ppid,cwd:process.cwd(),root,report,argv:[],collectionArgv:process.argv.slice(2)};const path=root+"/"+mode+"-request.json";writeFileSync(path,JSON.stringify(request));process.env.PW_HOST_COLLECTION_REQUEST=path;const reporter=new Reporter();const config={forbidOnly:true,configFile:root+"/custom.config.mjs"};const suite={allTests:()=>[{id:"known",location:{file:root+"/e2e/signals.spec.mjs"},parent:{project:()=>({name:"chromium"})},annotations:[]}]};reporter.onBegin(config,suite);if(mode==="two-begins")reporter.onBegin(config,suite);if(mode==="error")reporter.onError({message:"controlled callback error"});if(mode==="test-begin")reporter.onTestBegin();if(mode!=="no-end")reporter.onEnd({status:mode==="failed"?"failed":"passed"});if(mode==="two-ends")reporter.onEnd({status:"passed"});reporter.onExit();rows.push({mode,published:existsSync(report),receipt:existsSync(report)?JSON.parse(readFileSync(report,"utf8")):null});}writeFileSync(${JSON.stringify(output)},JSON.stringify(rows),{flag:"wx"});`);
    const child=await privateChild(fixture.root,[program],{...process.env},"callback-sequence");
    if(child.status!==0)throw new Error(child.text);
    const rows=JSON.parse(readFileSync(output,"utf8"));
    assert.deepEqual(rows.map(r=>[r.mode,r.published]),[["complete",true],["no-end",false],["two-begins",false],["two-ends",false],["error",false],["test-begin",false],["failed",false]]);
    assert.equal(rows[0].receipt.complete,true);
    assert.deepEqual(rows[0].receipt.tests,[{id:"known",file:"signals.spec.mjs",project:"chromium",roles:["base"]}]);
  } finally {rmSync(fixture.root,{recursive:true,force:true});}
}));