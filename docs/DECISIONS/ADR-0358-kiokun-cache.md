# ADR-0358: Kiokun search cache publication and reader lifetime

Status: accepted under the integrator's delegated ruling, 2026-10-16.
Track: `kiokun-cache`. Base: `7ec6a5342acb27a4c5b498d365e88fb3a6b9668a`.
This record precedes the production correction. The integrator assigns its
number and authoritative status/history at integration.

## Measured problem

ADR-0314 decision 4 used a deterministic `<digest>.building` path, removed
it before a build, renamed it to `<digest>.db`, then removed every other
`kiokun-search-*` file. A process-local OnceLock does not serialize another
process, Search instance, or concurrent cache version.

Five new tests use private synthetic CSVs above 6 MiB (the disk threshold is
4 MiB), real bundled SQLite, and per-call channel barriers compiled only in
tests. No external dictionary or publisher is inspected or changed. The
unchanged algorithm compiled and failed all five (0/5):

- A second same-digest builder unlinks the first builder's open staging
  database; the first builder returns SQLite `disk I/O error`.
- A same-digest builder delayed after closing its complete SQLite loses its
  staging pathname when another builder wins; its rename returns ENOENT.
- A new digest's pruning unlinks another digest's open staging database;
  that live builder returns SQLite `disk I/O error`.
- A new digest removes an old reader's published pathname. Its existing
  query still succeeds on this Unix run; pathname metadata returns ENOENT.
  This proves loss of stable file identity, not inevitable query failure.
- A malformed CSV returns an error but leaks its own staging file while an
  independent builder remains active.

The complete pre-correction output is `docs/evidence/kiokun-cache-before.txt`.
These controlled cases establish causal races; they do not establish the
cause of any earlier errors with unrelated external data.

## Ruling before correction

The integrator explicitly amends ADR-0314's stale-cache deletion policy:
concurrency and reader lifetime outrank deleting versions another reader
may hold. A cache opener must never unlink or rename an open SQLite file,
replace an existing complete digest, or remove another builder's staging
path. No process-wide production hook or environment mutation is added.

## Decision

1. Keep small CSVs in memory. For disk inputs, use a versioned directory
   named by the full CSV/schema SHA-256. Ignore older flat cache files
   without deleting them; this avoids changing any old reader's identity.
2. Exclusively create a random per-builder directory in that same cache
   root with `create_dir`, using the existing `getrandom` dependency. Only
   that builder writes its `index.db`; close SQLite after a successful
   fill and identity metadata commit, before publication.
3. Publish the completed nonempty directory with `std::fs::rename` to the
   digest directory. A concurrent nonempty winner must not be replaced.
   A losing builder validates and opens the winner's final `index.db`.
   Every returned disk connection opens only the final pathname, read-only.
4. Validate the final SQLite file's stored full CSV/schema fingerprint and
   exact FTS table schema before use, including on the reuse path. Refuse
   invalid existing candidates and nonempty concurrent winners without
   deleting or replacing them. This checks the cache's identity and
   structure; it is not a complete
   integrity scan or an adversarial tampering guarantee.
5. A guard removes only its own private staging directory on ordinary
   errors and lost races, after its SQLite connection has closed. It is
   disarmed on successful publication. Published versions and other
   stages are retained. An abruptly killed process may leave its isolated
   stage, which later openers ignore. Cleanup of abandoned stages or old
   versions is a separate operation requiring readers/builders to stop;
   this change adds no unsafe sweep or global retention service.

## Platform evidence and limits

Installed Rust: 1.97.1 (8bab26f4f, 2026-07-14). Actual target: Linux
6.18.54.1-microsoft-standard-WSL2 on ext4. A std-only Rust probe under this
worktree's `target/probes` confirmed: rename to an absent directory succeeds;
rename over an existing nonempty directory refuses with OS error 39 and
preserves the winner's inode/content and the loser's stage. The concurrency
regressions exercise real SQLite on this platform. Other platforms are not
certified by this run; a refused publication returns an error unless a valid
winner can be reused. Atomic namespace publication is claimed within this
same-filesystem design. Crash durability, fsync guarantees, and safe manual
deletion while readers run are not claimed.

SQLite's [open-file unlink/rename warning](https://www.sqlite.org/howtocorrupt.html)
and sections 2.5/2.6 on the same page motivate stable final names and no
hardlink publication. [Rust rename](https://doc.rust-lang.org/stable/std/fs/fn.rename.html)
(stable since 1.0) and [Linux rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html)
document the nonempty-directory exclusion. The current online Rust page is
newer than the installed toolchain; the installed-toolchain probe supplies
the target-specific evidence.

Cooperating publishers install only completed nonempty digest directories.
Every existing reuse candidate is validated, and nonempty concurrent winners
are preserved and validated. The protocol does not certify arbitrary
concurrent empty-directory creation by an out-of-protocol writer: rename
may replace such an empty directory, which holds neither a published index
nor a live-reader SQLite inode. The integrator explicitly accepted this
scope clarification on 2026-10-16; no stronger syscall/dependency is added.
The real nonempty partial-winner refusal and complete-cache reader lifetime
regressions remain required.

## Validation

Keep all five red witnesses, add invalid-winner/fingerprint and abandoned
stage controls, and test actual SQLite answers and path identity. New
mutation controls must remove independently reachable guarantees at their
authority and die through built assertions. Run that script whole, the
affected existing Kiokun word script whole, fmt/anchors, and default full CI
before a source commit. Report Linux evidence separately from unclaimed
platform or crash properties.

The whole own recipe passed 10 private cache tests and killed all 10 new
controls through built assertions. The whole affected Kiokun word script
passed its 56-test baseline and killed all 138 controls through built
assertions, with no compile-error kills, missing anchors, survivors or
process stops. All 1,855 frozen source/config hashes matched after its
natural completion. Default full CI passed on those restored source bytes (exit 0, `ci: OK`);
the complete-log failure scan is empty. See
`docs/evidence/E14/kiokun-cache-validation.md` for exact raw hashes and gates.

The earlier unit-only campaigns inherited setup's deployment variables;
code inspection confirms external/oracle measurements are ignored and the
ordinary test factory uses the committed sample. Direct cache witnesses use
only private CSV/cache paths. The final CI driver removes `KIOKUN_DATA` and
`KIOKUN_APP` from its child environment after setup. No external checkout is
inspected, read, changed or stopped. Browser host isolation is a separate
queued assignment; no browser isolation guarantee is made here.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0358** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0353, in train order),
  merged from `3019936e` (`track/kiokun-cache`'s own commit, cherry-picked).
  `ADR-XXXX` reads `ADR-0358` in `just/kiokun-cache.just` (its recipe's
  header line too), ADR-0314's amendment, and its evidence's header and
  validation note, as in this file's name and title.
