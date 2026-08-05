/* Deterministic delays for the streaming spike.
 * Charter §15.5 specifies exactly these two: recommendations 1200 ms,
 * estimate 400 ms. Kept in a module rather than inline in the template because
 * Marko's attribute parser treats a top-level `>` as the end of a tag.
 */
export const delay = (ms, value) =>
  new Promise((resolve) => setTimeout(() => resolve(value), ms));
