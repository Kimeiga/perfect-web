// What an oracle gives kiokun.com's server code it copies (track `kiokun`):
// each import resolved to a file the harness copied, and SvelteKit's `json`
// and `error` as a stated stand-in (`json` answers a JSON `Response`, `error`
// throws its status and message, all the handlers ask of them). Any other
// import stops the run: nothing else is stubbed. Shared by search.mjs and
// homophones.mjs.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Rewrite each of `files` in LIB so its imports name the harness's copies:
// `resolved` maps a specifier to a file in LIB; `@sveltejs/kit` is the
// stand-in, written as LIB/kit.ts.
export function prepared(lib, files, resolved) {
  const all = { "@sveltejs/kit": "./kit.ts", ...resolved };
  for (const file of files) {
    let source = readFileSync(join(lib, file), "utf8");
    source = source.replace(/^(import\s+(?:type\s+)?[^'"]*from\s+)['"]([^'"]+)['"];?$/gm, (line, head, spec) => {
      const to = all[spec];
      if (!to) throw new Error(`${file} imports ${spec}, which the harness does not copy`);
      return to.endsWith(".json") ? `${head}'${to}' with { type: 'json' };` : `${head}'${to}';`;
    });
    writeFileSync(join(lib, file), source);
  }
  writeFileSync(
    join(lib, "kit.ts"),
    `export function json(data: unknown): Response {
  return new Response(JSON.stringify(data), { headers: { "content-type": "application/json" } });
}
export class HttpError {
  status: number;
  body: { message: string };
  constructor(status: number, body: { message: string }) { this.status = status; this.body = body; }
}
export function error(status: number, message: string): never { throw new HttpError(status, { message }); }
export type RequestEvent = unknown;
`,
  );
}
