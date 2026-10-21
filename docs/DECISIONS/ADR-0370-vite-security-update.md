# ADR-0370: Vite 8.3.3 closes the bundled editor-launch advisories

Status: accepted under the owner's delegation, 2026-10-20. The owner
resumed the project and authorized the necessary dependency downloads;
the integrator assigned this repair to train-12. Milestone: E14.

## Context and primary sources

The Node audit failed on three advisories published to GitHub's reviewed
database on 2026-10-20. Each covers launch-editor <=2.14.1, including the
copy bundled in Vite 8.2.0 and 8.3.2 in this workspace:

- [GHSA-698v-4r3x-3vcv](https://github.com/advisories/GHSA-698v-4r3x-3vcv):
  Windows command arguments did not escape percent expansion.
- [GHSA-r825-w8mm-5pxw](https://github.com/advisories/GHSA-r825-w8mm-5pxw):
  some editors interpret attacker-controlled paths as command options,
  including Vite's WSL path handling.
- [GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm):
  a Windows path can bypass the file-existence check before launch.

The advisories name Vite 8.3.3 and launch-editor 2.14.2 as patched.
[Vite's signed release](https://github.com/vitejs/vite/releases/tag/v8.3.3)
explicitly updates its bundled launch-editor. Its
[package manifest](https://raw.githubusercontent.com/vitejs/vite/v8.3.3/packages/vite/package.json)
and `pnpm view vite@8.3.3 version license engines dist.integrity
 dist.unpackedSize --json` were read on 2026-10-20: MIT, Node
`^20.19.0 || >=22.12.0`, 2,374,102 unpacked bytes. The workspace's Node
22.21.1 satisfies that range; pnpm records the registry's SHA-512 integrity.
A launch-editor override alone cannot replace the copy bundled in Vite.

## Decision

Pin all three direct consumers to Vite 8.3.3: the SvelteKit benchmark and
the two historical Marko spikes. The latter already reached Vite 8.3.2
through @marko/run alongside their direct 8.2.0. One patched version now
serves every path. Keep other framework and tool versions unchanged.
Update `tools/versions.lock` and regenerate the workspace lock from its
previous contents with `pnpm install --lockfile-only --ignore-scripts`.
This retains the existing rolldown 1.2.12 and other transitive versions;
it removes the obsolete Vite/PostCSS entry and adjusts peer references.
No advisory allowance, severity threshold or audit gate changes here.

## Acceptance and limits

The frozen install, SvelteKit typecheck (0 errors, 0 warnings) and build,
and both Marko builds passed. The generated Marko spike built static,
counter and streamed routes; its historical store route cannot be emitted
after the layout change, so no store compatibility is claimed. Raw
`pnpm audit --json` fell from seven advisory instances to one, with no
instance of the three IDs above, and `scripts/audit-node.sh` passed.
[Commands and output](../evidence/tooling/vite-security-update-2026-10-20.txt).
The existing documented braces advisory remains a separate limitation.
The train's GitHub CI and verification must pass before landing. This
patch does not establish production readiness or remove the historical
Marko dependency; its retirement remains a separate obligation.


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.
