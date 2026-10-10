// A stand-in for Cloudflare D1's binding over Node's `node:sqlite`, for the
// search oracle (search.mjs): only the calls kiokun.com's `api/search`
// handler makes, each in D1's documented shape
// (https://developers.cloudflare.com/d1/worker-api/prepared-statements/):
// `prepare(sql)` gives a statement, `bind(...values)` binds its `?`
// placeholders in order and gives the statement, and `all()` resolves to
// `{ results, success, meta }`, `results` an array of row objects keyed by
// column name. A failed statement rejects, as D1's does. Its own test is
// d1.test.mjs (the integrator's ruling of 2026-10-10, B4).

export function d1Of(db) {
  return {
    prepare(sql) {
      let values = [];
      const statement = {
        bind(...bound) {
          values = bound;
          return statement;
        },
        async all() {
          const started = performance.now();
          const results = db.prepare(sql).all(...values).map((row) => ({ ...row }));
          return {
            results,
            success: true,
            meta: { duration: performance.now() - started, rows_read: results.length },
          };
        },
      };
      return statement;
    },
  };
}
