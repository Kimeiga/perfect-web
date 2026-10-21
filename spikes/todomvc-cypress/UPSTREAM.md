# What is vendored here, and from where

TodoMVC's official behavioural test sources are **vendored unchanged**.
Execution uses the explicitly identified **todomvc-cypress-compat-v1** copy
described below, holding TodoMVC in Pleris (`examples/todomvc`) to all 29
test bodies and their original storage assertions.

- **Source**: <https://github.com/tastejs/todomvc>, `master` at
  `ff43b02e59dfa604386bb382034b2cd07c2bcd8a` ("Revise README for 2.0.0",
  2026-05-03), read through the GitHub API on 2026-10-14.
- **Licence**: MIT, "Copyright (c) Addy Osmani, Sindre Sorhus, Pascal Hartig,
  Stephen Sawchuk" (`LICENSE-todomvc.md`, the repository's `license.md`: "Everything
  in this repo is MIT License unless otherwise specified"). The notice stays with
  the files.
- **Approved by the owner on 2026-10-08**: Cypress, `todomvc-app-css` and
  `todomvc-common`, "to run TodoMVC's official tests unchanged".

| file here | upstream path | git blob | bytes |
|---|---|---|---|
| `cypress/e2e/spec.cy.js` | `cypress/e2e/spec.cy.js` | `478bcfb17f407d56b4db19c4bffadcd0aa1e0972` | 35412 |
| `cypress/support/commands.js` | `cypress/support/commands.js` | `029abdbaf932cd1dded89e79c7943aa2fcab9c36` | 2985 |
| `cypress/support/e2e.js` | `cypress/support/e2e.js` | `8edffddd850ceaa49a30c8b4542df5f02d16676a` | 83 |
| `cypress/fixtures/example.json` | `cypress/fixtures/example.json` | `da87594a2465392b86ba23faaa7ac1d971c32091` | 26 |
| `tests/knownIssues.js` | `tests/knownIssues.js` | `808649f49a9cde966e2fb8185eac394a0f975217` | 8486 |
| `cypress.config.js` | `cypress.config.js` | `9557466f8850fd01fe05406acbc69d2e713fccd1` | 917 |
| `LICENSE-todomvc.md` | `license.md` | `9275d4371020294b6a107e454a0d1819f8b71c31` | 1166 |

`git hash-object <file>` of each gives its blob: `scripts/tests` and `run.sh`
hold them to it, so the suite cannot drift from what upstream ships without a
test failing. What upstream names `tests/cypress/e2e/spec.cy.js` in the plan
(docs/PARALLEL.md) is `cypress/e2e/spec.cy.js` in the repository today.

Not vendored, and not run: the suite's server (`tests/server.js`, which serves
the repository's `examples/`), its other frameworks' folders, and
`todomvc-common`'s `base.js`.

# How it is run

`run.sh` serves TodoMVC in Pleris (`spikes/own-renderer/dist-todomvc`) on a
host of its own and runs the compatibility copy against it:

```
cypress run --env framework=page/todomvc.app.Todos~ui --config baseUrl=http://127.0.0.1:<port> --project <verified-v1-copy>
```

The suite visits `baseUrl + '/' + framework`; the name is the page's own path
on the host (`/page/todomvc.app.Todos~ui`), so nothing here proxies or rewrites an
address. (The root, `.`, was tried first: Cypress asks for `/.`, which no host
here serves as `/`.) The name is in none of the suite's tables
(`noLocalStorageCheck` and the rest), so every check the suite makes of an app
it knows nothing about is made of this one: including `localStorage`. TodoMVC now declares device storage under
`todos-pleris`; the combined train-12 verification must demonstrate that all
29 tests in the identified compatibility copy pass. That is **the acceptance**.

`DIAGNOSTIC=1 run.sh` runs the same files a second time, **not as the
acceptance**, under a name the suite's own tables take as keeping nothing in
`localStorage` (`--env framework=puremvc`, reaching the page through an address
fragment the page ignores). It shows what the rest of each test says of this
app.

## Compatibility execution copy v1

The original seven files above are never patched. Before launch,
`scripts/todomvc_cypress_compat.py` checks their upstream Git blob hashes,
the source package pin, and the installed Cypress package **and binary**
versions, both exactly 15.21.1. It generates a unique disposable project
with precisely two changes:

1. The spec's one `.invoke('reset')` becomes `.invoke('resetHistory')`.
   This clears the spy's recorded calls while retaining the real storage
   operation. The obsolete spy method does not exist in the pinned tool.
2. `createTodo` keeps its logging `.then(...)` and appends
   `return cy.get(todoListSelector, { log: false }).contains('li', todo.trim(), { log: false })`.
   Its alias can re-query the current DOM after reload; no stale DOM element
   from before reload is treated as the persisted todo.

All test bodies, storage predicates, test titles, skip tables, fixtures and
configuration otherwise remain byte-identical. There is no application
detection or private Cypress API. This is an adapted execution, **not an
unchanged-suite pass**. The previous failed original executions remain failed.

The generator requires unique anchors, a fixed patchset SHA-256 and fixed
patched-output SHA-256 values. It emits `compatibility-manifest.json` with
the upstream revision and blobs, package/binary versions, patch identity and
every staged file's SHA-256. `run.sh` prints that manifest and its hash,
revalidates source, manifest and complete staged inventory immediately before
launch, then removes the owned temporary project after the host exits. A
changed source, version, patch, file or manifest refuses the run. New upstream
sources or a different adaptation require a new reviewed compatibility identity.

The public [Cypress version CLI](https://docs.cypress.io/app/references/command-line),
[query alias behavior](https://docs.cypress.io/app/core-concepts/variables-and-aliases)
and [Sinon spy resetHistory](https://sinonjs.org/concepts/spies/api/reset-history) explain
the tool contracts; native tests against the pinned Cypress establish the
specific behavior. `scripts/tests/test_todomvc_cypress_compat.py` and
`scripts/todomvc_compat_mutations.py` hold the generation/refusal contract.

`run.sh` does not skip. It exits 1 where Cypress cannot run (no Xvfb, no
binary, no build of the app, no host) and where a run printed no results, and
its status is the verdict of `scripts/todomvc_cypress_result.py`, which reads
what Cypress printed. Acceptance requires all 29 official tests to pass,
with none failed, pending or skipped and Cypress exit status zero
([ADR-0368](../../docs/DECISIONS/ADR-0368-todomvc-acceptance-means-passing.md)).
Understanding a failure never turns it into acceptance. `scripts/tests/test_todomvc_cypress.py` holds all of this to real
outputs and runs `run.sh` whole with a stand-in Cypress;
`scripts/todomvc_cypress_mutations.py` breaks each rule. `just e14-todomvc`
records both runs; the suite is not part of `just ci`.

# The binary

Cypress 15.21.1 is two downloads, approved by the owner on 2026-10-08 for
this track:

- the npm package `cypress@15.21.1` from the npm registry (7.6 MB installed,
  with its dependency tree held by `pnpm-lock.yaml`), and
- its binary, which pnpm 10 does not fetch (it runs no install scripts):
  `pnpm exec cypress install` fetches it from `download.cypress.io` into
  Cypress's own cache (`~/.cache/Cypress/15.21.1`): a zip of 248,957,235 bytes
  (the length its CDN reports), 813 MB unzipped. `just e14-todomvc` runs that
  command, which does nothing where the binary is already there.

**Why 15.21.1 and not 15.14.2**, which that repository's own lockfile pins
(first installed here, 2026-10-14): 15.14.2's dependency tree carries six
advisories that `scripts/audit-node.sh` refuses (`uuid` GHSA-w5hq-g745-h8pq;
`qs` GHSA-q8mj-m7cp-5q26, GHSA-x5fp-wj9c-mxmx and GHSA-4mjr-xmp4-gh2g;
`extract-zip` GHSA-jmr9-qjv8-65gv and GHSA-7pqw-9j4j-h8q3), and 15.21.1's has
none (its direct dependencies no longer include `extract-zip`, and
`@cypress/request` is 4.x, as the registry's metadata shows). It is the
last 15.x: the spec reads `Cypress.env()`, which Cypress 15 warns a future
major will drop, so 16.x (16.2.0 is current) is not used. The suite printed the
same outcomes on both (`scripts/tests/fixtures/todomvc-cypress/`).

Cypress needs a display and starts Xvfb itself (`apt: xvfb`); `run.sh` fails
where it is missing.
