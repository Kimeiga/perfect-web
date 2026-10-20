#!/usr/bin/env python3
"""Mutation controls for the browser host map (ADR-0356).

The host map starts only the hosts the selected tests need. Each mutant
undoes one piece of it, and must be killed by the test named beside it, by
a failed assertion of that test (`ERR_ASSERTION` in Node's TAP report), not
by any test or by an error:
1. the registry (`host-metadata.mjs`): the stride its slots are apart, the
   TCP bound of the last slot, its offsets each its own and inside a slot,
   its providers declared, a test's closure with its providers, a spec's
   host annotations;
2. the map held to the specs: a spec registered with a host it does not
   reach, or without one it does (`host-registry.json`); the derivation of
   what a spec reaches (`hostsReachedBy`); a test's baseURL's host;
3. the builds: a build not whole, of another runtime, of a changed or a
   missing source, keyed's exemption; and the policy as before the host
   map, an optional build's specs left out while it cannot be served, each
   said with the build it lacks (never silently: the integrator's ruling of
   2026-10-19), a stale one's why said, `dist`'s never left out;
4. the plan (`host-plan.mjs`): the collection's arguments, its receipt (whole,
   fresh, its own child's, its configuration's, `forbidOnly`, each test once,
   its roles the registry's), the hosts a selection declares (only its own,
   `dist` refused, an optional host left to its tests' own skip), each
   host's environment (in memory, no deployment's kiokun data, identity,
   blobs, its provider's port), the performance run's one host, an
   inherited receipt kept from an independent run, the reporter's
   publication, and the configuration's `forbidOnly`.

Each runs the Node suite that holds it: `host-plan.test.mjs` (`fast`, under
a second) or `host-plan.native.test.mjs` (`native`, the real Playwright CLI
on private fixtures, some minutes). The unmutated suites must pass first.

Run from the repository root; `just e14-browser-host-map` records the
output. The sources are restored after every mutant, whatever happens, and
on SIGTERM. The native suite's hosts listen on PW_HOST_TEST_PORT (PORT where
unset).
"""

import dataclasses
import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPIKE = ROOT / "spikes/own-renderer"
METADATA = SPIKE / "host-metadata.mjs"
PLAN = SPIKE / "host-plan.mjs"
REPORTER = SPIKE / "host-plan-reporter.mjs"
REGISTRY = SPIKE / "host-registry.json"
CONFIG = SPIKE / "playwright.config.mjs"

SUITES = {
    "fast": SPIKE / "host-plan.test.mjs",
    "native": SPIKE / "host-plan.native.test.mjs",
}

@dataclasses.dataclass(frozen=True)
class Test:
    """The test of its suite that must fail, by the name its file gives it.
    Not a string: a mutant's words are its description and its suite's tag
    (`scripts/evidence_counts.py`)."""

    name: str

    def __str__(self) -> str:
        return f"<{self.name}>"


# Each suite's test names, as its file declares them: what a mutant names.
INVENTORY = Test("registry covers the exact current spec inventory")
DISJOINT = Test("all host and provider listeners are disjoint across adjacent and multiple slots")
TCP = Test("TCP bound includes the final slot and largest concrete listener")
OFFSETS = Test("duplicate, fractional, negative and escaping concrete offsets refuse")
PROVIDERS = Test("provider dependencies are declared and engine compatible")
DECLARATIONS = Test("unknown selected spec, project, role and conflicting declarations refuse")
HELD = Test("every spec's registered hosts are exactly the hosts its source reaches")
READ = Test("a spec's hosts are read from its baseURL overrides, and an unnamed loopback port is refused")
URL_ROLE = Test("a test's baseURL names the one registered host of its engine")
BUILDS = Test("build checks preserve whole-build runtime and source freshness with concrete prerequisites")
KEYED = Test("only keyed external generated copies may be absent and existing copies remain hash checked")
POLICY = Test("specs of an optional build that cannot be served are left out, each said with the build it lacks, and never dist's")
DECLARED = Test("a selection declares only its own hosts, refuses an unservable dist and drops an optional host only for tests that skip themselves")
DESCRIPTORS = Test("selected descriptors preserve memory identity blobs provider ports target and reuse policy")
PERFORMANCE = Test("PW_PERFORMANCE admits only the Chromium performance host")
FRAMING = Test("native collection framing preserves every opaque argument before and after separator")
RECEIPTS = Test("collection receipts require exact issuer selection config completion and role identities")
PUBLISHED = Test("public collection reporter publishes only one successful complete callback sequence")
INHERITED = Test("native independent identical invocation cannot reuse an inherited valid receipt")
OWN_CONFIG = Test("the suite's own configuration lists a selection through its plan, and starts no host")

# (what, suite, the test that must fail, file, anchor, replacement)
MUTANTS = [
    # 1. The registry.
    ("the stride export is the old 200 again", "fast", INVENTORY, METADATA,
     "export const STRIDE = REGISTRY.stride;", "export const STRIDE = 200;"),
    ("a slot's ports are 200 apart again", "fast", DISJOINT, METADATA,
     "base + slot * registry.stride + offset", "base + slot * 200 + offset"),
    ("the last slot is out of the TCP bound", "fast", TCP, METADATA,
     "base + (count - 1) * registry.stride + max > 65535", "base + max > 65535"),
    ("a fractional or infinite port passes", "fast", TCP, METADATA,
     "!Number.isSafeInteger(value) || value < minimum", 'typeof value !== "number" || value < minimum'),
    ("an offset may leave its slot", "fast", DISJOINT, METADATA,
     "if (offset >= value.stride) fail(", "if (false) fail("),
    ("two hosts may share an offset", "fast", OFFSETS, METADATA,
     "if (seen.has(offset)) fail(", "if (false) fail("),
    ("a provider need not be a dependency", "fast", PROVIDERS, METADATA,
     'if (role.provider !== undefined && (typeof role.provider !== "string" || !role.depends?.includes(role.provider))) fail(',
     "if (false) fail("),
    ("a test's hosts leave out their providers", "fast", PROVIDERS, METADATA,
     "for (const dependency of registry.roles[role].depends ?? []) closure.add(dependency);",
     "for (const dependency of []) closure.add(dependency);"),
    ("a spec of one host may annotate another", "fast", DECLARATIONS, METADATA,
     "if (spec.annotations !== true && written.length) fail(", "if (false) fail("),
    ("an annotated test may name no host or two", "fast", DECLARATIONS, METADATA,
     "if (spec.annotations === true && roles.length !== 1) fail(", "if (false) fail("),
    ("a test may name a reserved host", "fast", DECLARATIONS, METADATA,
     "if (registry.roles[role].reserved === true) fail(", "if (false) fail("),
    # 2. The map held to the specs.
    ("the feed's spec is registered on base, not the feed's host", "fast", HELD, REGISTRY,
     '    "feed.spec.mjs": {\n      "roles": [\n        "feed"\n      ]\n    },',
     '    "feed.spec.mjs": {\n      "roles": [\n        "base"\n      ]\n    },'),
    ("the signals spec is registered with a host it does not reach", "fast", HELD, REGISTRY,
     '    "signals.spec.mjs": {\n      "roles": [\n        "base"\n      ]\n    },',
     '    "signals.spec.mjs": {\n      "roles": [\n        "base",\n        "feed"\n      ]\n    },'),
    ("a describe's override is read as the file's", "fast", HELD, METADATA,
     "[...source.matchAll(/^test\\.use\\(/gm)]", "[...source.matchAll(/^\\s*test\\.use\\(/gm)]"),
    ("a spec on the default host reaches nothing", "fast", HELD, METADATA,
     'if (!topLevel) roles.add("base");', 'if (false) roles.add("base");'),
    ("a mutating suite's host is read as base", "fast", READ, METADATA,
     'if (name === "MUTABLE_PORTS" && member && own(registry.mutable, member)) return registry.mutable[member];',
     'if (name === "MUTABLE_PORTS") return "base";'),
    ("a loopback port written as a number passes", "fast", READ, METADATA,
     'if (m[1] !== "${") fail(`a loopback address without a registered port', 'if (m[1] !== "${") continue; (`a loopback address without a registered port'),
    ("a baseURL's host is any engine's", "fast", URL_ROLE, METADATA,
     '(key === "shared" || key === engine) && base + offset === port', "base + offset === port"),
    # 3. The builds, and the policy as before the host map.
    ("a build with no sources.json is served", "fast", BUILDS, METADATA,
     'if (!existsSync(resolve(directory, "sources.json"))) return problem(`${build.dist} is no whole build (sources.json is missing)`);',
     'if (!existsSync(resolve(directory, "sources.json"))) return null;'),
    ("a build of another runtime is served", "fast", BUILDS, METADATA,
     'return problem(`${build.dist} was built with another pw-runtime.mjs`);', "return null;"),
    ("a recorded source may be gone", "fast", BUILDS, METADATA,
     "return problem(`missing required source ${path}`);", "continue;"),
    ("a changed source passes", "fast", BUILDS, METADATA,
     'if (createHash("sha256").update(readFileSync(source)).digest("hex") !== sha256) return problem(',
     "if (false) return problem("),
    ("keyed's exemption covers the repository", "fast", KEYED, METADATA,
     'if (outside === ".." || outside.startsWith("../") || outside.startsWith("..\\\\") || isAbsolute(outside)) continue;',
     "if (true) continue;"),
    ("an optional build's specs run though it cannot be served", "fast", POLICY, METADATA,
     "const missing = builds.filter((build) => build !== REQUIRED_BUILD && state(build).problem !== null);",
     "const missing = [];"),
    # The integrator's ruling of 2026-10-19 (train-11): never a silent skip.
    ("a spec left out is left out silently", "fast", POLICY, METADATA,
     'out.push({ file, warning: `e2e/${file} is not run: ${why.join("; ")}` });',
     "out.push({ file, warning: null });"),
    ("a stale optional build says nothing of why", "fast", POLICY, METADATA,
     "return built ? problem : `${BUILDS[build].dist} is not built: run ${BUILDS[build].prerequisite}`;",
     "return built ? \"\" : `${BUILDS[build].dist} is not built: run ${BUILDS[build].prerequisite}`;"),
    ("dist's specs are left out quietly too", "fast", POLICY, METADATA,
     "builds.filter((build) => build !== REQUIRED_BUILD && state(build).problem !== null)",
     "builds.filter((build) => state(build).problem !== null)"),
    # 4. The plan.
    ("the collection's --list goes after the separator", "fast", FRAMING, PLAN,
     'return ["test", "--list", ...argv.slice(1)];', 'return [...argv, "--list"];'),
    ("a partial receipt is taken as complete", "fast", RECEIPTS, PLAN,
     "value.complete !== true || value.nonce", "false || value.nonce"),
    ("a stale receipt is taken", "fast", RECEIPTS, PLAN,
     "value.nonce !== request.nonce ||", "false ||"),
    ("another child's receipt is taken", "fast", RECEIPTS, PLAN,
     "value.collectorPid !== collectorPid ||", "false ||"),
    ("another configuration's receipt is taken", "fast", RECEIPTS, PLAN,
     "value.configFile !== request.configFile ||", "false ||"),
    ("a receipt need not prove forbidOnly", "fast", RECEIPTS, PLAN,
     "value.forbidOnly !== true ||", "false ||"),
    ("a test may be planned twice", "fast", RECEIPTS, PLAN,
     "ids.has(test.id) ||", "false ||"),
    ("a receipt's roles may differ from the registry's", "fast", RECEIPTS, PLAN,
     "if (!same(rolesForTest(test.file, test.project, annotations), test.roles)) fail(", "if (false) fail("),
    ("a selected role's host is declared in every engine", "fast", DECLARED, PLAN,
     "      chosen.set(`${role}/${engine}`, { role, engine });",
     '      for (const each of engine === "shared" ? [engine] : REGISTRY.engines) chosen.set(`${role}/${each}`, { role, engine: each });'),
    ("an unservable dist starts its hosts", "fast", DECLARED, PLAN,
     "      if (problem) fail(problem);", "      if (false) fail(problem);"),
    ("an optional host starts though its build cannot be served", "fast", DECLARED, PLAN,
     "} else if (!servable(build, root)) {", "} else if (false) {"),
    ("a test that skips nothing loses its host quietly", "fast", DECLARED, PLAN,
     "if (unskipped) fail(", "if (false) fail("),
    ("a host reads the caller's database", "fast", DESCRIPTORS, PLAN,
     'PORT: String(hostPort), PW_FEED_DATABASE_URL: "", PW_STORE_DATABASE_URL: "",', "PORT: String(hostPort),"),
    ("a host reads a deployment's kiokun data", "fast", DESCRIPTORS, PLAN,
     "KIOKUN_DATA: undefined, KIOKUN_APP: undefined,",
     "KIOKUN_DATA: process.env.KIOKUN_DATA, KIOKUN_APP: process.env.KIOKUN_APP,"),
    ("a signed-in host signs no one in", "fast", DESCRIPTORS, PLAN,
     'if (definition.identity) env.PW_IDENTITY = "dev-accounts";', 'if (false) env.PW_IDENTITY = "dev-accounts";'),
    ("the uploads' hosts share one blob storage", "fast", DESCRIPTORS, PLAN,
     "env.PW_BLOB_DIR = `${REGISTRY.builds[definition.build].dist}/blobs-${hostPort}`;",
     "env.PW_BLOB_DIR = `${REGISTRY.builds[definition.build].dist}/blobs`;"),
    ("a host's provider listens on the host's own port", "fast", DESCRIPTORS, PLAN,
     "env.PW_PAYMENTS_PORT = String(portsForRole(definition.provider, port)[engine]);",
     "env.PW_PAYMENTS_PORT = String(hostPort);"),
    ("the performance run admits other hosts", "fast", PERFORMANCE, PLAN,
     "if (process.env.PW_PERFORMANCE && selection.receipt.tests.some(",
     "if (false && selection.receipt.tests.some("),
    ("an independent run reuses an inherited receipt", "native", INHERITED, PLAN,
     "if (!nativeWorker) return null;", "if (false) return null;"),
    ("the reporter publishes before Playwright's end", "fast", PUBLISHED, REPORTER,
     "if (this.problem || this.begins !== 1 || this.ends !== 1 || !this.selection) return;",
     "if (!this.selection) return;"),
    ("the configuration allows a focused test", "native", OWN_CONFIG, CONFIG,
     "  forbidOnly: true,\n", "  forbidOnly: false,\n"),
]

# How long one suite may run before it is a hang, killed with everything it
# started, and counted as failing (not as a kill).
BOUND = 1800

TOP = re.compile(r"^(not ok|ok) \d+ - (.*)$")


def run_suite(suite):
    """(exit status, {test name: None where it passed, else its failure's
    code}) for one run of a suite, or (None, {}) past the bound."""
    env = {**os.environ}
    env.setdefault("PW_HOST_TEST_PORT", env.get("PORT", "3141"))
    env.pop("PW_PERFORMANCE", None)
    p = subprocess.Popen(
        ["node", "--test", "--test-reporter=tap", str(SUITES[suite])],
        cwd=SPIKE, env=env, start_new_session=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return None, {}
    results, current = {}, None
    for line in out.splitlines():
        m = TOP.match(line)
        if m:
            current = m.group(2).replace("\\#", "#")
            results[current] = None if m.group(1) == "ok" else "failed"
            continue
        code = re.match(r"^  code: '([A-Z_]+)'$", line)
        if code and current is not None and results.get(current) == "failed":
            results[current] = code.group(1)
    return p.returncode, results


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    baselines = {}
    for suite in sorted({m[1] for m in MUTANTS}):
        status, results = run_suite(suite)
        passed = sum(1 for r in results.values() if r is None)
        print(f"baseline {suite}: {passed} passed, {len(results) - passed} failed", flush=True)
        if status != 0 or not results or any(r is not None for r in results.values()):
            print(f"FAIL: the unmutated {suite} suite is not green; no control can mean anything")
            mutation_baseline.explain()
            return 1
        baselines[suite] = results
    missing = sorted({m[2].name for m in MUTANTS if m[2].name not in baselines[m[1]]})
    if missing:
        print("FAIL: a mutant names a test its suite does not have: " + "; ".join(missing))
        return 1

    survivors = 0
    for what, suite, witness, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            status, results = run_suite(suite)
        finally:
            path.write_text(original)
        failed = sorted(name for name, r in results.items() if r is not None)
        if status is not None and results.get(witness.name) == "ERR_ASSERTION":
            verdict = f"KILLED (by an assertion of its test; {len(failed)} of {len(results)} tests fail)"
        else:
            # Not its test's assertion: a run past its bound, a test that
            # failed by an error, or another test, kills nothing.
            why = ("it ran past its bound" if status is None
                   else f"its test {witness.name!r}: {results.get(witness.name) or 'passed'}; failing: {'; '.join(failed) or 'none'}")
            print(f"  ({why})", flush=True)
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.path.insert(0, str(ROOT / "scripts"))
    sys.exit(main())
