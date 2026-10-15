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
cypress run --env framework=. --config baseUrl=http://127.0.0.1:<port>
```

The suite visits `baseUrl + '/' + framework`; the name `.` is this host's root,
where the app is, so nothing here proxies or rewrites an address. The name is in
none of the suite's tables (`noLocalStorageCheck` and the rest), so every check
the suite makes of an app it knows nothing about is made of this one: including
`localStorage`, which this app does not use (see the ADR's section on the
suite's result).

# The binary

Cypress 15.14.2's binary is not in the repository: pnpm 10 does not run its
install script, and `pnpm exec cypress install` fetches it from
`download.cypress.io` into Cypress's own cache (`~/.cache/Cypress/15.14.2`).
`run.sh` says so where it is missing, and does nothing else.
