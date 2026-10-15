# What is vendored here, and from where

TodoMVC's official behavioural tests, **unchanged**, so that TodoMVC in Pleris
(`examples/todomvc`) is held to the tests every TodoMVC is held to.

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
host of its own and runs the suite against it:

```
cypress run --env framework=page/todomvc.app.Todos --config baseUrl=http://127.0.0.1:<port>
```

The suite visits `baseUrl + '/' + framework`; the name is the page's own path
on the host (`/page/todomvc.app.Todos`), so nothing here proxies or rewrites an
address. (The root, `.`, was tried first: Cypress asks for `/.`, which no host
here serves as `/`.) The name is in none of the suite's tables
(`noLocalStorageCheck` and the rest), so every check the suite makes of an app
it knows nothing about is made of this one: including `localStorage`, which this
app does not use (see the ADR's section on the suite's result). That is **the
acceptance**.

`DIAGNOSTIC=1 run.sh` runs the same files a second time, **not as the
acceptance**, under a name the suite's own tables take as keeping nothing in
`localStorage` (`--env framework=puremvc`, reaching the page through an address
fragment the page ignores). It shows what the rest of each test says of this
app.

`run.sh` does not skip. It exits 1 where Cypress cannot run (no Xvfb, no
binary, no build of the app, no host) and where a run printed no results, and
its status is the verdict of `scripts/todomvc_cypress_result.py`, which reads
what Cypress printed: the table must count the spec's 29 tests, and each test
must come out as the app's design gives it (an app that never writes
`localStorage` fails every test that asks it, and a test that asks nothing
passes). `scripts/tests/test_todomvc_cypress.py` holds all of this to real
outputs and runs `run.sh` whole with a stand-in Cypress;
`scripts/todomvc_cypress_mutations.py` breaks each rule. `just e14-todomvc`
records both runs; the suite is not part of `just ci`.

# The binary

Cypress 15.14.2 (the version that repository's own lockfile pins) is two
downloads, approved by the owner on 2026-10-08 for this track:

- the npm package `cypress@15.14.2` from the npm registry (7.4 MB installed;
  162 packages with its dependency tree, held by `pnpm-lock.yaml`), and
- its binary, which pnpm 10 does not fetch (it runs no install scripts):
  `pnpm exec cypress install` fetches it from `download.cypress.io` into
  Cypress's own cache (`~/.cache/Cypress/15.14.2`, 805 MB unzipped; the size
  of the zip was not recorded). `just e14-todomvc` runs that command, which
  does nothing where the binary is already there.

Cypress needs a display and starts Xvfb itself (`apt: xvfb`); `run.sh` fails
where it is missing.
