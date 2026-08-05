// spike: koka-js-interop — a narrow reader for Koka's `.kki` interface files.
//
// Charter §14 Milestone 1 task 8 asks: "Determine whether machine-readable
// inferred types/effects can be obtained without a fork." Task 9 permits "a
// narrow parser for Koka compiler output or explicit generated metadata only as
// a temporary bridge".
//
// This is that parser, built one milestone early because Milestone 0 needs to
// know whether the Milestone 1 plan is viable at all.
//
// ANSWER: yes. `koka --target=js` writes `<module>.kki` next to `<module>.mjs`.
// It is plain text and contains, for every public declaration, the fully
// elaborated signature INCLUDING the inferred effect row, plus source spans and
// ADT constructor tag numbers.
//
// LIMITATIONS (recorded honestly, charter §3.3):
//   - `.kki` is an internal format with no stability guarantee. It changes when
//     Koka changes. This parser is pinned to Koka 3.2.3 and MUST be re-verified
//     against any upgrade. `assertKokaVersion` enforces that at runtime.
//   - It is not JSON. This reader is regex-based and deliberately shallow: it
//     extracts the effect row and arity, not a full type AST.
//   - Effect names appear in their fully-qualified elaborated form, e.g.
//     `(std/core/types/handled :: ((E, V) -> V) -> X)<(database-read :: (E, V) -> V)>`.
//     `effectNames()` reduces that to the readable `database-read`.

import { readFileSync } from "node:fs";

export const VERIFIED_AGAINST_KOKA = "3.2.3";

export function assertKokaVersion(actual) {
  if (!actual.includes(VERIFIED_AGAINST_KOKA)) {
    throw new Error(
      `kki.mjs was verified against Koka ${VERIFIED_AGAINST_KOKA}, but found "${actual}". ` +
        `The .kki format is internal and unstable — re-verify this parser before trusting it.`,
    );
  }
}

/**
 * Parse the `#kki: declarations` section.
 * @returns {Map<string, {name: string, span: number[], signature: string}>}
 */
export function parseInterface(kkiPath) {
  const text = readFileSync(kkiPath, "utf8");

  // Only the declarations section; the inline-definitions section repeats names
  // with bodies and would otherwise shadow the real signatures.
  const start = text.indexOf("//#kki: declarations");
  const end = text.indexOf("//#kki: external declarations");
  const section = text.slice(start >= 0 ? start : 0, end >= 0 ? end : text.length);

  const out = new Map();
  // e.g.  pub fun load-store[103,9,103,18] : (id : ...) -> <...> ...;
  const re = /^pub\s+(?:fip\s+|fbip\s+)?(fun|val)\s+([A-Za-z0-9_@$-]+)\[([0-9,]+)\]\s*:\s*([\s\S]*?);\s*$/gm;
  let m;
  while ((m = re.exec(section)) !== null) {
    const [, , name, spanStr, signature] = m;
    out.set(name, {
      name,
      span: spanStr.split(",").map(Number),
      signature: signature.replace(/\s+/g, " ").trim(),
    });
  }
  return out;
}

/**
 * Extract the top-level effect row from an elaborated signature.
 *
 * Koka writes the result type as `... -> <e1,e2> result` when effects are
 * present and `... -> result` when the function is total. So the presence of a
 * `<...>` immediately after the LAST top-level `->` is the signal.
 *
 * @returns {string[]} readable effect names; `[]` means total/pure.
 */
export function effectNames(signature) {
  // NOTE: `->` must be consumed as ONE token. Treating its `>` as a closing
  // bracket corrupts depth tracking, because elaborated signatures are full of
  // nested arrows such as `(std/core/types/handled :: ((E, V) -> V) -> X)`.
  // That bug silently reported every effectful function as pure.
  const findLastTopLevelArrow = (s) => {
    let depth = 0;
    let last = -1;
    for (let i = 0; i < s.length; i++) {
      if (s[i] === "-" && s[i + 1] === ">") {
        if (depth === 0) last = i;
        i++;
        continue;
      }
      const c = s[i];
      if (c === "(" || c === "<" || c === "[") depth++;
      else if (c === ")" || c === ">" || c === "]") depth--;
    }
    return last;
  };

  const lastArrow = findLastTopLevelArrow(signature);
  if (lastArrow < 0) return [];

  const tail = signature.slice(lastArrow + 2).trim();
  if (!tail.startsWith("<")) return []; // total — no effect row

  // Take the balanced <...> group, again skipping `->`.
  let d = 0;
  let close = -1;
  for (let i = 0; i < tail.length; i++) {
    if (tail[i] === "-" && tail[i + 1] === ">") {
      i++;
      continue;
    }
    if (tail[i] === "<") d++;
    else if (tail[i] === ">") {
      d--;
      if (d === 0) {
        close = i;
        break;
      }
    }
  }
  if (close < 0) return [];
  const row = tail.slice(1, close);

  // Reduce `(std/core/types/handled :: ...)<(database-read :: (E, V) -> V)>`
  // to `database-read`, and `(std/core/console/console :: X)` to `console`.
  const names = new Set();
  const nameRe = /\(([A-Za-z0-9_/@$-]+)\s*::/g;
  let n;
  while ((n = nameRe.exec(row)) !== null) {
    const qualified = n[1];
    if (qualified.endsWith("/handled") || qualified === "handled") continue;
    names.add(qualified.split("/").pop());
  }
  return [...names].sort();
}

/** Convenience: name -> effect names, for a whole module. */
export function moduleEffects(kkiPath) {
  const decls = parseInterface(kkiPath);
  const out = new Map();
  for (const [name, d] of decls) out.set(name, effectNames(d.signature));
  return out;
}
